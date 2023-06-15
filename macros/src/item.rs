/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::{Delimiter, Group, Punct, Spacing, Span, TokenStream};
use quote::{TokenStreamExt, quote, quote_spanned};
use syn::punctuated::Punctuated;
use crate::{content, property, Assignee, Builder, Mode};

enum Object { Path (Box<Path>), Ref (Punctuated<syn::Ident, syn::Token![.]>) }

struct Path { path: crate::Path, group: Option<Group>, field: crate::Field }

fn as_assignee(object: &Object) -> Assignee {
	match object {
		Object::Path(path) => Assignee::Ident(None, &path.field.name),
		Object::Ref(ref_) => Assignee::Field(None, ref_),
	}
}

struct Inter {
	 inter: crate::Path,
	by_ref: Option<syn::Token![&]>,
	  mut_: Option<syn::Token![mut]>,
	  mode: Mode,
	tokens: TokenStream,
	  back: Option<Box<property::Back>>,
}

fn parse_inter(
	input: syn::parse::ParseStream, assignee: Assignee, span: &mut Span
) -> syn::Result<Option<Inter>> {
	let Ok(pound) = input.parse::<syn::Token![#]>() else { return Ok(None) };
	*span = pound.span;
	
	let inter: crate::Path = input.parse()?;
	let buffer;
	
	let (by_ref, mut_) = if inter.is_long() {
		(input.parse()?, input.parse()?)
	} else { (None, None) };
	
	let mode = if input.peek(syn::token::Paren) {
		Mode::Method(syn::parenthesized!(buffer in input).span.join())
	} else if input.peek(syn::token::Brace) {
		Mode::Field(syn::braced!(buffer in input).span.join())
	} else {
		Mode::FnField(syn::bracketed!(buffer in input).span.join())
	};
	
	let tokens = buffer.step(|cursor| {
		let mut rest = *cursor;
		let mut stream = TokenStream::new();
		
		crate::find_pound(&mut rest, &mut stream, assignee)
			.then(|| (stream, syn::buffer::Cursor::empty()))
			.ok_or_else(|| cursor.error("no single `#` found around here"))
	})?;
	
	let back = if let Mode::Field(_) = &mode { None } else { property::parse_back(input)? };
	Ok(Some(Inter { inter, by_ref, mut_, mode, tokens, back }))
}

pub struct Item {
	 attrs: Vec<syn::Attribute>,
	object: Object,
	 spans: [Span; 4], // start, end, pound, braces
	 inter: Option<Inter>,
	  mode: ItemMode,
	  body: Vec<content::Content>,
}

pub enum ItemMode { Builder(syn::Token![!]), Struct(syn::Token![~]), Normal }

pub(crate) fn parse(
	input: syn::parse::ParseStream,
	attrs: Vec<syn::Attribute>,
	 path: Option<crate::Path>,
	 root: bool,
) -> syn::Result<Item> {
	let mut spans = [Span::call_site(); 4]; spans[0] = input.span();
	
	let (tildable, mut object) = if let Some(path) = path {
		let group = input.peek(syn::token::Paren).then(|| input.parse()).transpose()?;
		let field = crate::parse_field(Some(&path), input)?;
		(group.is_none(), Object::Path(Box::new(Path { path, group, field })))
	} else { (false, Object::Ref(crate::parse_unterminated(input)?)) };
	
	let mut inter = if root { None } else {
		spans[1] = input.span();
		parse_inter(input, as_assignee(&object), &mut spans[2])?
	};
	
	let mut body = vec![];
	let has_body = inter.as_ref().map(|inter| inter.back.is_none()).unwrap_or(true);
	let mode = if has_body {
		if let (true, Ok(tilde)) = (tildable, input.parse::<syn::Token![~]>()) {
			ItemMode::Struct(tilde)
		} else if let Ok(pound) = input.parse::<syn::Token![!]>() {
			ItemMode::Builder(pound)
		} else { ItemMode::Normal }
	} else { ItemMode::Normal };
	
	if root | has_body {
		let braces;
		let brace = syn::braced!(braces in input);
		spans[3] = brace.span.join();
		
		if let Object::Path(path) = &mut object {
			if path.field.auto { path.field.name.set_span(spans[3]) }
		}
		
		if root {
			while !braces.is_empty() { body.push(braces.parse()?) }
		} else {
			while !braces.is_empty() {
				if inter.is_some() {
					while !braces.is_empty() { body.push(braces.parse()?) }
				} else if braces.peek(syn::Token![#]) && (
					braces.peek2(syn::Ident) || braces.peek2(syn::Token![<])
				) {
					inter = parse_inter(&braces, as_assignee(&object), &mut spans[2])?;
				} else { body.push(braces.parse()?) }
			}
		}
	}
	Ok(Item { attrs, object, spans, inter, mode, body })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expand(
	Item { mut attrs, object, spans, inter, mode, body }: Item,
	 objects: &mut TokenStream,
	builders: &mut Vec<Builder>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	  fields: &mut Option<&mut Punctuated<syn::Field, syn::Token![,]>>,
	  pattrs: crate::Attributes<&[syn::Attribute]>,
	assignee: Assignee,
	 builder: Option<usize>,
) -> syn::Result<()> {
	let let_ = syn::Ident::new("let", spans[2]);
	crate::extend_attributes(&mut attrs, pattrs.get(fields));
	let (attributes, assignee_field, assignee_ident);
	
	let (new_assignee, new_builder) = match object {
		Object::Path(path) => {
			let Path { path, group, field } = *path;
			let crate::Field { vis, mut_, name, colon, ty, auto: _ } = field;
			
			let stream = || {
				let group = group.as_ref().map(|group| quote![#group])
					.unwrap_or_else(|| quote_spanned![spans[3] => ::default()]);
				
				quote![#(#attrs)* #let_ #mut_ #name = #path #group]
			};
			
			let builder = match mode {
				ItemMode::Builder(not) => {
					#[cfg(not(feature = "builder-mode"))]
					builders.push(Builder::Builder(stream(), not.span));
					
					#[cfg(feature = "builder-mode")]
					builders.push(Builder::Builder {
						 left: quote![#(#attrs)* #let_ #mut_ #name =],
						right: group.as_ref().map(|group| quote![#path #group])
							.unwrap_or_else(|| quote_spanned![not.span => #path =>]),
						 span: not.span,
						tilde: None,
					});
					
					Some(builders.len() - 1)
				}
				ItemMode::Struct(tilde) => {
					#[cfg(not(feature = "builder-mode"))]
					builders.push(Builder::Struct {
						    ty: quote![#(#attrs)* #let_ #mut_ #name = #path],
						fields: Default::default(),
						  call: None,
						  span: tilde.span,
					});
					
					#[cfg(feature = "builder-mode")]
					builders.push(Builder::Struct {
						  left: quote![#(#attrs)* #let_ #mut_ #name =],
						    ty: quote![#path],
						fields: Default::default(),
						  span: tilde.span,
						 tilde: None,
					});
					
					Some(builders.len() - 1)
				}
				ItemMode::Normal => {
					objects.extend(stream());
					objects.append(Punct::new(';', Spacing::Alone));
					None
				}
			};
			
			if let Some(colon) = colon {
				let fields = fields.as_deref_mut().ok_or_else(
					|| syn::Error::new_spanned(quote![#vis #colon #ty], crate::NO_FIELD)
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
					
					Err(syn::Error::new_spanned(quote![#path #group], crate::NO_TYPE))?
				};
				
				attributes = crate::Attributes::None(fields.len());
				
				fields.push(syn::Field {
					attrs, vis, ty: syn::Type::Path(ty),
					    mutability: syn::FieldMutability::None,
					         ident: Some(name.clone()),
					   colon_token: Some(colon),
				});
			} else { attributes = crate::Attributes::Some(attrs) }
			
			assignee_ident = name;
			(Assignee::Ident(None, &assignee_ident), builder)
		}
		Object::Ref(idents) => {
			settings.extend(quote![#(#attrs)* #let_ _ = #idents;]);
			assignee_field = idents;
			attributes = crate::Attributes::Some(attrs);
			(Assignee::Field(None, &assignee_field), None)
		}
	};
	
	for content in body { content::expand(
		content, objects, builders, settings, bindings, fields,
		attributes.as_slice(), new_assignee, new_builder
	)? }
	
	if let Assignee::None = assignee { return Ok(()) }
	
	let Some(Inter { inter, by_ref, mut_, mode, tokens, back }) = inter else {
		Err(crate::Spans::Range(spans[0], spans[1]).error("missing #interpolation"))?
	};
	
	let attrs = attributes.get(fields);
	use property::check;
	
	match builder.map(|index| &mut builders[index]) {
		#[cfg(not(feature = "builder-mode"))]
		Some(Builder::Builder(stream, span)) => return {
			let paren = check(Some(attrs), &inter, mode, true, back)?;
			let mut group = Group::new(Delimiter::Parenthesis, tokens); group.set_span(paren);
			Ok(stream.extend(quote_spanned![*span => .#inter #group]))
		},
		#[cfg(feature = "builder-mode")]
		Some(Builder::Builder { right, span, .. }) => return {
			let paren = check(Some(attrs), &inter, mode, true, back)?;
			let mut group = Group::new(Delimiter::Parenthesis, tokens); group.set_span(paren);
			Ok(right.extend(quote_spanned![*span => .#inter #group]))
		},
		#[cfg(not(feature = "builder-mode"))]
		Some(Builder::Struct { ty: _, fields, call, span }) => return {
			let paren = check(call.is_some().then_some(attrs), &inter, mode, true, back)?;
			let Some(call) = call else { return Ok(fields.extend(quote![#inter: #tokens,])) };
			let mut group = Group::new(Delimiter::Parenthesis, tokens); group.set_span(paren);
			Ok(call.extend(quote_spanned![*span => .#inter #group]))
		},
		#[cfg(feature = "builder-mode")]
		Some(Builder::Struct { fields, span, .. }) => return {
			check(None, &inter, mode, true, back)?;
			Ok(fields.extend(quote_spanned![*span => #inter: #tokens,]))
		},
		None => ()
	}
	
	let right = match mode {
		Mode::Field(span) => {
			let assignee = assignee.spanned_to(span);
			if back.is_some() { quote_spanned![span => #(#assignee.)* #inter = #tokens] }
			else { quote_spanned![span => #(#attrs)* #(#assignee.)* #inter = #tokens;] }
		}
		Mode::FnField(span) => {
			let assignee = assignee.spanned_to(span);
			let field = quote_spanned![span => #(#assignee.)* #inter];
			
			let mut field = Group::new(Delimiter::Parenthesis, field);
			field.set_span(inter.span());
			
			let mut group = Group::new(Delimiter::Parenthesis, tokens);
			group.set_span(span);
			
			if back.is_some() { quote![#field #group] }
			else { quote![#(#attrs)* #field #group;] }
		}
		Mode::Method(span) => {
			let assignee = assignee.spanned_to(span);
			
			if inter.is_long() {
				let group = quote_spanned![span => #by_ref #mut_ #(#assignee).*, #tokens];
				
				let mut group = Group::new(Delimiter::Parenthesis, group);
				group.set_span(span);
				
				if back.is_some() { quote![#inter #group] }
				else { quote![#(#attrs)* #inter #group;] }
			} else {
				let mut group = Group::new(Delimiter::Parenthesis, tokens);
				group.set_span(span);
				
				if back.is_some() { quote_spanned![span => #(#assignee.)* #inter #group] }
				else { quote_spanned![span => #(#attrs)* #(#assignee.)* #inter #group;] }
			}
		}
	};
	
	let Some(back) = back else { settings.extend(right); return Ok(()) };
	property::expand_back(*back, objects, builders, settings, bindings, fields, attributes, right)
}
