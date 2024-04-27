/*
 * SPDX-FileCopyrightText: 2024 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::{Group, Punct, Spacing, Span, TokenStream};
use quote::{TokenStreamExt, quote, quote_spanned};
use syn::punctuated::Punctuated;
use crate::{content, Assignee, Attributes, Construction};

struct Field {
	 vis: Option<syn::Visibility>,
	mut_: Option<syn::Token![mut]>,
	name: syn::Ident,
	  ty: Option<Box<syn::TypePath>>,
	auto: bool,
}

fn parse_field(display: Option<&dyn std::fmt::Display>, input: syn::parse::ParseStream) -> syn::Result<Field> {
	let vis = input.parse()?;
	let vis = if let syn::Visibility::Inherited = vis {
		input.parse::<syn::Token![ref]>().map(|_| vis).ok()
	} else { Some(vis) };
	
	let (mut_, name) = (input.parse()?, input.parse());
	let ty = (vis.is_some() && input.parse::<syn::Token![as]>().is_ok()).then(|| input.parse()).transpose()?;
	
	struct Name<'a>(Option<&'a syn::TypePath>);
	
	impl std::fmt::Display for Name<'_> {
		fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
			if let Some(ty) = self.0 { crate::view::display_ty(ty, f) } else { write!(f, "back_") }
		}
	}
	
	thread_local![static COUNT: std::cell::RefCell<usize> = const { std::cell::RefCell::new(0) }];
	
	let (name, auto) = if vis.is_some() { (name?, false) } else {
		name.map(|name| (name, false)).unwrap_or_else(|_| (syn::Ident::new(&COUNT.with(|cell| {
			let count = *cell.borrow();
			*cell.borrow_mut() = count.wrapping_add(1);
			compact_str::format_compact!("{}{count}", display.unwrap_or(&Name(ty.as_deref())))
		}), Span::call_site()), true))
	};
	
	Ok(Field { vis, mut_, name, ty, auto })
}

struct Path { path: crate::Path, group: Option<Group>, pub field: Field }

enum Object { Path(Box<Path>), Ref(Punctuated<syn::Ident, syn::Token![.]>) }

pub enum Mode { Builder(Span), Normal(Span), StructLiteral(syn::Token![?]) }

pub(crate) struct Item {
	  attrs: Option<Vec<syn::Attribute>>,
	at_span: Span,
	 object: Object,
	   mode: Mode,
	   body: Vec<content::Content>,
}

impl Item {
	pub fn set_attrs(&mut self, attrs: Vec<syn::Attribute>) { self.attrs = Some(attrs) }
	pub fn as_assignee(&self) -> Assignee {
		match &self.object {
			Object::Ref (ref_) => Assignee::Field(None, ref_),
			Object::Path(path) => Assignee::Ident(None, &path.field.name),
		}
	}
}

pub(crate) fn parse(input: syn::parse::ParseStream, attrs: Option<Vec<syn::Attribute>>) -> syn::Result<Item> {
	let at_span = if attrs.is_none() { input.parse::<syn::Token![@]>()?.span } else { Span::call_site() };
	let path = input.parse::<syn::Token![ref]>().is_err().then(|| input.parse()).transpose()?;
	
	let (literable, mut object) = if let Some(path) = path {
		let group = input.peek(syn::token::Paren).then(|| input.parse()).transpose()?;
		let field = parse_field(Some(&path), input)?;
		(group.is_none(), Object::Path(Box::new(Path { path, group, field })))
	} else { (false, Object::Ref(crate::parse_unterminated(input)?)) };
	
	let braces;
	let brace = syn::braced!(braces in input);
	let span = brace.span.join();
	
	if let Object::Path(path) = &mut object {
		if path.field.auto { path.field.name.set_span(span) }
	}
	
	let mut body = vec![];
	while !braces.is_empty() { body.push(braces.parse()?) }
	
	let mode = if let Some(Ok(tilde)) = literable.then(|| input.parse::<syn::Token![?]>()) {
		Mode::StructLiteral(tilde)
	} else if let Ok(pound) = input.parse::<syn::Token![!]>() {
		if let Object::Path(ref path) = object {
			if path.group.is_some() { Mode::Builder(pound.span) }
			else { Mode::Normal(pound.span) }
		} else { Mode::Normal(pound.span) }
	} else if let Object::Path(ref path) = object {
		if path.group.is_some() { Mode::Normal(span) }
		else { Mode::Builder(span) }
	} else { Mode::Builder(span) };
	
	Ok(Item { attrs, at_span, object, mode, body })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expand(
	Item { attrs, at_span, object, mode, body }: Item,
	 objects: &mut TokenStream,
	 constrs: &mut Vec<Construction>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	  fields: &mut Option<&mut Punctuated<syn::Field, syn::Token![,]>>,
	  pattrs: crate::Attributes<&[syn::Attribute]>,
) -> syn::Result<()> {
	let mut attrs = attrs.unwrap();
	crate::extend_attributes(&mut attrs, pattrs.get(fields));
	
	let let_ = syn::Ident::new("let", at_span);
	let (attributes, assignee_field, assignee_ident);
	
	let (new_assignee, new_constr) = match object {
		Object::Path(path) => {
			let Path { path, group, field } = *path;
			let Field { vis, mut_, name, ty, auto: _ } = field;
			
			let constr = match mode {
				Mode::Builder(span) => {
					constrs.push(Construction::BuilderPattern {
						 left: quote![#(#attrs)* #let_ #mut_ #name =],
						right: group.as_ref().map(|group| quote![#path #group])
							.unwrap_or_else(|| quote_spanned![span => #path =>]),
						 span,
						tilde: None,
					});
					
					Some(constrs.len() - 1)
				}
				Mode::StructLiteral(question) => {
					constrs.push(Construction::StructLiteral {
						  left: quote![#(#attrs)* #let_ #mut_ #name =],
						    ty: quote![#path],
						fields: Default::default(),
						  span: question.span,
						 tilde: None,
					});
					
					Some(constrs.len() - 1)
				}
				Mode::Normal(span) => {
					objects.extend(match &group {
						None => quote_spanned![span => #(#attrs)* #let_ #mut_ #name = construct!(? #path)],
						Some(group) => quote![#(#attrs)* #let_ #mut_ #name = #path #group]
					});
					objects.append(Punct::new(';', Spacing::Alone));
					None
				}
			};
			
			if let Some(vis) = vis {
				let fields = fields.as_deref_mut().ok_or_else(
					|| syn::Error::new_spanned(quote![#vis #ty], NO_FIELD_ERROR)
				)?;
				
				let ty = 'ty: {
					if let Some(ty) = ty { break 'ty *ty }
					
					let path = match path {
						crate::Path::Type(mut ty) => if group.is_none() {
							break 'ty ty
						} else if ty.path.segments.len() > 1 {
							ty.path.segments.pop();
							break 'ty ty
						} else { crate::Path::Type(ty) }
						
						path => path
					};
					
					Err(syn::Error::new_spanned(quote![#path #group], NO_TYPE_ERROR))?
				};
				
				attributes = crate::Attributes::None(fields.len());
				
				fields.push(syn::Field {
					attrs, vis, ty: syn::Type::Path(ty),
					    mutability: syn::FieldMutability::None,
					         ident: Some(name.clone()),
					   colon_token: None,
				});
			} else { attributes = crate::Attributes::Some(attrs) }
			
			assignee_ident = name;
			(Assignee::Ident(None, &assignee_ident), constr)
		}
		Object::Ref(idents) => {
			settings.extend(quote![#(#attrs)* #let_ _ = #idents;]);
			assignee_field = idents;
			attributes = crate::Attributes::Some(attrs);
			(Assignee::Field(None, &assignee_field), None)
		}
	};
	
	for content in body { content::expand(
		content, objects, constrs, settings, bindings, fields,
		attributes.as_slice(), new_assignee, new_constr
	)? }
	
	Ok(())
}

pub struct Back {
	pub token: syn::Lifetime,
	    field: Field,
	     body: Vec<content::Content>,
	    build: Option<Span>,
}

pub fn parse_back(input: syn::parse::ParseStream) -> syn::Result<Option<Box<Back>>> {
	let token = if input.fork().parse::<syn::Lifetime>()
		.map(|keyword| keyword.ident == "back").unwrap_or(false) {
			input.parse::<syn::Lifetime>()?
		} else { return Ok(None) };
	
	let mut field = parse_field(None, input)?;
	let braces;
	let brace = syn::braced!(braces in input);
	let build = input.parse::<syn::Token![!]>().err().map(|_| brace.span.join());
	
	if field.auto { field.name.set_span(brace.span.join()) }
	
	let mut body = vec![];
	while !braces.is_empty() { body.push(braces.parse()?) }
	
	Ok(Some(Box::new(Back { token, field, body, build })))
}

fn builds(content: Option<&content::Content>) -> bool {
	content.map(|content| match content {
		| content::Content::Bind(_)
		| content::Content::BindColon(_)
		| content::Content::Edit(_)
		| content::Content::If(_)
		| content::Content::Match(_) => false,
		
		| content::Content::Consume(_)
		| content::Content::Property(_) => true,
		
		| content::Content::Construct(built) => built.rest.is_empty()
	}).unwrap_or(true)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expand_back(
	Back { token, field, body, build }: Back,
	 objects: &mut TokenStream,
	 constrs: &mut Vec<Construction>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	  fields: &mut Option<&mut Punctuated<syn::Field, syn::Token![,]>>,
	   attrs: Attributes<Vec<syn::Attribute>>,
	   right: TokenStream,
) -> syn::Result<()> {
	let pattrs = attrs.get(fields);
	let let_ = syn::Ident::new("let", token.span());
	let Field { vis, mut_, name, ty, auto } = field;
	
	let left = if auto && build.is_some() && builds(body.first()) && builds(body.last())
		{ quote![#(#pattrs)*] } else { quote![#(#pattrs)* #let_ #mut_ #name =] };
	
	let index = if let Some(span) = build {
		constrs.push(Construction::BuilderPattern { left, right, span, tilde: None });
		Some(constrs.len() - 1)
	} else { settings.extend(quote![#left #right;]); None };
	
	let mut setup = TokenStream::new();
	
	for content in body { content::expand(
		content, objects, constrs, &mut setup, bindings,
		fields, attrs.as_slice(), Assignee::Ident(None, &name), index
	)? }
	
	if let Some(vis) = vis {
		let fields = fields.as_deref_mut().ok_or_else(
			|| syn::Error::new_spanned(quote![#vis #ty], NO_FIELD_ERROR)
		)?;
		
		let ty = ty.ok_or_else(|| syn::Error::new_spanned(quote![#name], NO_TYPE_ERROR))?;
		
		let attrs = match attrs {
			Attributes::Some(attrs) => attrs,
			Attributes::None(index) => fields.iter().nth(index).unwrap().attrs.clone()
		};
		
		fields.push(syn::Field {
			attrs, vis, ty: syn::Type::Path(*ty),
			    mutability: syn::FieldMutability::None,
			         ident: Some(name.clone()),
			   colon_token: None,
		});
	}
	
	if let Some(index) = index {
		if constrs.get(index).is_some() {
			constrs.remove(index).extend_into(settings)
		}
	}
	settings.extend(setup); Ok(())
}

const NO_FIELD_ERROR: &str = "a visibility cannot be specified if a struct has not \
	been declared before the root item or within a binding or conditional scope";

const NO_TYPE_ERROR: &str = "a type must be specified after the name (e.g. `some_name as SomeType`)";
