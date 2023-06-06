/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::{Delimiter, Group, Punct, Spacing, Span, TokenStream};
use quote::{TokenStreamExt, quote};
use syn::{punctuated::Punctuated, spanned::Spanned};
use crate::{content, property, Assignee, Builder, Mode};

pub struct Item {
	  attrs: Vec<syn::Attribute>,
	 object: Object,
	  spans: [Span; 3], // start, end, pound
	  inter: Option<Box<Inter>>,
	   mode: ItemMode,
	content: Vec<content::Content>,
}

pub enum ItemMode { Builder(syn::Token![!]), Struct(syn::Token![~]), Normal }

pub fn parse(
	   input: syn::parse::ParseStream,
	   attrs: Vec<syn::Attribute>,
	reactive: bool,
	    root: bool,
) -> syn::Result<Item> {
	let mut spans = [input.span(), Span::call_site(), Span::call_site()];
	let object = parse_object(input)?;
	
	let (tildable, assignee) = match &object {
		Object::Path(path, _) => (match path.path {
			crate::Path::Type(_) => path.group.is_none(), _ => false
		}, Assignee::Ident(&path.name)),
		
		Object::Ref(ref_) => (false, Assignee::Field(ref_)),
	};
	
	let mut inter = if root { None } else {
		spans[1] = input.span();
		parse_inter(input, assignee, reactive, &mut spans[2])?
	};
	
	let mut content = vec![];
	let body = inter.as_ref().map(|inter| inter.back.is_none()).unwrap_or(true);
	let mode = if body {
		if let (true, Ok(tilde)) = (tildable, input.parse::<syn::Token![~]>()) {
			ItemMode::Struct(tilde)
		} else if let Ok(pound) = input.parse::<syn::Token![!]>() {
			ItemMode::Builder(pound)
		} else { ItemMode::Normal }
	} else { ItemMode::Normal };
	
	if root {
		let braces; syn::braced!(braces in input);
		content::parse_vec(&mut content, &braces, reactive)?
	} else if body {
		let braces; syn::braced!(braces in input);
		
		while !braces.is_empty() {
			if inter.is_none() {
				if braces.peek(syn::Token![#]) && (
					braces.peek2(syn::Ident) || braces.peek2(syn::Token![<])
				) {
					inter = parse_inter(&braces, assignee, reactive, &mut spans[2])?;
					continue
				}
				content.push(crate::ParseReactive::parse(&braces, None, reactive)?)
			} else { content::parse_vec(&mut content, &braces, reactive)? }
		}
	}
	Ok(Item { attrs, object, spans, inter, mode, content })
}

enum Object {
	Path (Box<Path>, Option<syn::Token![mut]>),
	 Ref (Punctuated<syn::Ident, syn::Token![.]>),
}

struct Path { path: crate::Path, group: Option<Group>, name: syn::Ident }

fn parse_object(input: syn::parse::ParseStream) -> syn::Result<Object> {
	if input.parse::<syn::Token![ref]>().is_ok() {
		return Ok(Object::Ref(crate::parse_unterminated(input)?))
	}
	
	let path = input.parse()?;
	let group = input.peek(syn::token::Paren).then(|| input.parse()).transpose()?;
	let mut_ = input.parse()?;
	
	let name = input.parse::<Option<_>>()?
		.unwrap_or_else(|| syn::Ident::new(&crate::count(), Span::call_site()));
	
	Ok(Object::Path(Box::new(Path { path, group, name }), mut_))
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
	   input: syn::parse::ParseStream,
	assignee: Assignee,
	reactive: bool,
	    span: &mut Span,
) -> syn::Result<Option<Box<Inter>>> {
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
	
	let back = if let Mode::FnField(_) | Mode::Method(_) = &mode {
		property::parse_back(input, reactive)?
	} else { None };
	
	Ok(Some(Box::new(Inter { inter, by_ref, mut_, mode, tokens, back })))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expand(
	Item { mut attrs, object, spans, inter, mode, content }: Item,
	 objects: &mut TokenStream,
	builders: &mut Vec<Builder>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	  pattrs: &[syn::Attribute],
	assignee: Assignee,
	 builder: Option<usize>,
) {
	let let_ = syn::Ident::new("let", spans[2]);
	crate::extend_attributes(&mut attrs, pattrs);
	
	let (new_assignee, new_builder) = match &object {
		Object::Path(path, mut_) => (Assignee::Ident(&path.name), {
			let Path { path, group, name } = path.as_ref();
			
			let stream = || {
				let group = group.as_ref().map(|group| quote![#group]).unwrap_or_else(|| quote![::default()]);
				quote![#(#attrs)* #let_ #mut_ #name = #path #group]
			};
			
			match mode {
				ItemMode::Builder(_builder) => {
					#[cfg(not(feature = "builder-mode"))]
					builders.push(Builder::Builder(stream()));
					
					#[cfg(feature = "builder-mode")]
					builders.push(Builder::Builder {
						 left: quote![#(#attrs)* #let_ #mut_ #name =],
						right: group.as_ref().map(|group| quote![#path #group]).unwrap_or_else(|| quote![#path =>]),
						 span: _builder.span,
						tilde: None,
					});
					
					Some(builders.len() - 1)
				}
				
				ItemMode::Struct(_tilde) => {
					#[cfg(not(feature = "builder-mode"))]
					builders.push(Builder::Struct {
						    ty: quote![#(#attrs)* #let_ #mut_ #name = #path],
						fields: Default::default(),
						  call: None,
					});
					
					#[cfg(feature = "builder-mode")]
					builders.push(Builder::Struct {
						  left: quote![#(#attrs)* #let_ #mut_ #name =],
						    ty: quote![#path],
						fields: Default::default(),
						  span: _tilde.span,
						 tilde: None,
					});
					Some(builders.len() - 1)
				},
				
				ItemMode::Normal => {
					objects.extend(stream());
					objects.append(Punct::new(';', Spacing::Alone));
					None
				}
			}
		}),
		
		Object::Ref(idents) => {
			settings.extend(quote![#(#attrs)* #let_ _ = #idents;]);
			(Assignee::Field(idents), None)
		}
	};
	
	for content in content { content::expand(
		content, objects, builders, settings, bindings, &attrs, new_assignee, new_builder
	) }
	
	if let Assignee::None = assignee { return }
	
	let Some(inter) = inter else {
		let mut start = Punct::new('<', Spacing::Alone); start.set_span(spans[0]);
		let mut   end = Punct::new('>', Spacing::Alone);   end.set_span(spans[1]);
		let error = syn::Error::new_spanned(quote![#start #end], "missing #interpolation").into_compile_error();
		return objects.extend(error)
	};
	
	let Inter { inter, by_ref, mut_, mode, tokens, back } = *inter;
	use property::check;
	
	match builder.map(|index| &mut builders[index]) {
		#[cfg(not(feature = "builder-mode"))]
		Some(Builder::Builder(stream)) =>
			return if check(objects, Some(&attrs), &inter, mode, true, back).is_ok() {
				let group = Group::new(Delimiter::Parenthesis, tokens);
				stream.extend(quote![.#inter #group]);
			},
		
		#[cfg(feature = "builder-mode")]
		Some(Builder::Builder { right, .. }) =>
			return if check(objects, Some(&attrs), &inter, mode, true, back).is_ok() {
				let group = Group::new(Delimiter::Parenthesis, tokens);
				right.extend(quote![.#inter #group]);
			},
		
		#[cfg(not(feature = "builder-mode"))]
		Some(Builder::Struct { ty: _, fields, call }) => return if check(
			objects, call.is_some().then_some(&attrs), &inter, mode, true, back
		).is_ok() {
			let Some(call) = call else { return fields.get_mut().extend(quote![#inter: #tokens,]) };
			call.extend(quote![.#inter(#tokens)])
		},
		
		#[cfg(feature = "builder-mode")]
		Some(Builder::Struct { fields, .. }) =>
			return if check(objects, None, &inter, mode, true, back).is_ok() {
				fields.get_mut().extend(quote![#inter: #tokens,])
			},
		
		None => ()
	}
	
	let right = match mode {
		Mode::Field(span) => {
			let assignee = assignee.spanned_to(span);
			if back.is_some() { quote![#(#assignee.)* #inter = #tokens] }
			else { quote![#(#attrs)* #(#assignee.)* #inter = #tokens;] }
		}
		Mode::FnField(span) => {
			let assignee = assignee.spanned_to(span);
			
			let mut field = Group::new(Delimiter::Parenthesis, quote![#(#assignee.)* #inter]);
			field.set_span(inter.span());
			
			let mut group = Group::new(Delimiter::Parenthesis, tokens);
			group.set_span(span);
			
			if back.is_some() { quote![#field #group] }
			else { quote![#(#attrs)* #field #group;] }
		}
		Mode::Method(span) => {
			let assignee = assignee.spanned_to(span);
			
			if inter.is_long() {
				let group = quote![#by_ref #mut_ #(#assignee).*, #tokens];
				
				let mut group = Group::new(Delimiter::Parenthesis, group);
				group.set_span(span);
				
				if back.is_some() { quote![#inter #group] }
				else { quote![#(#attrs)* #inter #group;] }
			} else {
				let mut group = Group::new(Delimiter::Parenthesis, tokens);
				group.set_span(span);
				
				if back.is_some() { quote![#(#assignee.)* #inter #group] }
				else { quote![#(#attrs)* #(#assignee.)* #inter #group;] }
			}
		}
	};
	
	if let Some(back) = back {
		property::expand_back(*back, objects, builders, settings, bindings, attrs, right)
	} else { settings.extend(right); }
}
