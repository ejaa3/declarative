/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::quote;
use syn::spanned::Spanned;
use crate::{content, property};

pub(crate) struct Item {
	  attrs: Vec<syn::Attribute>,
	 object: Option<crate::Object>,
	   mut0: Option<syn::Token![mut]>,
	   name: syn::Ident,
	  annex: Option<Box<Annex>>,
	  build: Option<syn::Token![!]>,
	content: Vec<content::Content>,
}

pub(crate) fn parse(
	   input: syn::parse::ParseStream,
	   attrs: Vec<syn::Attribute>,
	reactive: bool,
	    root: bool,
) -> syn::Result<Item> {
	let ref0 = input.parse::<syn::Token![ref]>().is_ok();
	let object = (!ref0).then(|| input.parse()).transpose()?;
	let mut0 = if !ref0 { input.parse()? } else { None };
	let name = if ref0 { Some(input.parse()?) } else { input.parse()? }
		.unwrap_or_else(|| syn::Ident::new(&crate::count(), input.span()));
	
	let pound = input.parse::<syn::Token![#]>();
	
	if let (Ok(pound), true) = (&pound, root) {
		Err(syn::Error::new(pound.span, "cannot #interpolate here"))?
	}
	
	let mut annex = pound.is_ok()
		.then(|| parse_annex(input, &name, reactive)).transpose()?;
	
	let body = annex.as_ref().map(|annex| annex.back.is_none()).unwrap_or(true);
	
	let build = if body { input.parse()? } else { None };
	let mut content = vec![];
	
	if body {
		let braces; syn::braced!(braces in input);
		
		while !braces.is_empty() {
			if braces.peek(syn::Token![#]) && braces.peek2(syn::Ident) {
				let pound = braces.parse::<syn::Token![#]>()?;
				
				if root { Err(syn::Error::new(pound.span, "cannot #interpolate here"))? }
				
				if annex.is_some() {
					Err(syn::Error::new(pound.span, "expected a single #interpolation"))?
				}
				
				annex = Some(parse_annex(&braces, &name, reactive)?);
				continue
			}
			
			content.push(crate::ParseReactive::parse(&braces, None, reactive)?)
		}
	}
	
	Ok(Item { attrs, object, mut0, name, annex, build, content })
}

struct Annex {
	 annex: syn::Path,
	by_ref: Option<syn::Token![&]>,
	  mut0: Option<syn::Token![mut]>,
	  mode: AnnexMode,
	tokens: TokenStream,
	  back: Option<Box<property::Back>>,
}

enum AnnexMode {
	  Field(syn::token::Brace),
	FnField(syn::token::Bracket),
	 Method(syn::token::Paren),
}

fn parse_annex(
	   input: syn::parse::ParseStream,
	    name: &syn::Ident,
	reactive: bool,
) -> syn::Result<Box<Annex>> {
	let annex: syn::Path = input.parse()?;
	
	let (by_ref, mut0) = if annex.segments.len() > 1 {
		(input.parse()?, input.parse()?)
	} else { (None, None) };
	
	let (mode, buffer) = if input.peek(syn::token::Paren) {
		let parens;
		(AnnexMode::Method(syn::parenthesized!(parens in input)), parens)
	} else if input.peek(syn::token::Brace) {
		let braces;
		(AnnexMode::Field(syn::braced!(braces in input)), braces)
	} else {
		let brackets;
		(AnnexMode::FnField(syn::bracketed!(brackets in input)), brackets)
	};
	
	let tokens = buffer.step(|cursor| {
		let mut rest = *cursor;
		let mut stream = TokenStream::new();
		
		find_pound(&mut rest, &mut stream, &[&name])
			.then(|| (stream, syn::buffer::Cursor::empty()))
			.ok_or_else(|| cursor.error("no `#` was found after this point"))
	})?;
	
	let back = if let AnnexMode::FnField(_) | AnnexMode::Method(_) = &mode {
		property::parse_back(input, reactive)?
	} else { None };
	
	Ok(Annex { annex, by_ref, mut0, mode, tokens, back }.into())
}

pub(crate) fn find_pound(
	 rest: &mut syn::buffer::Cursor,
	outer: &mut TokenStream,
	 name: &[&syn::Ident],
) -> bool {
	while let Some((tt, next)) = rest.token_tree() {
		match tt {
			TokenTree::Group(group) => {
				let delimiter = group.delimiter();
				let (mut into, _, next) = rest.group(delimiter).unwrap();
				let mut inner = TokenStream::new();
				let found = find_pound(&mut into, &mut inner, name);
				
				outer.extend(match delimiter {
					Delimiter::Parenthesis => quote![(#inner)],
					Delimiter::Brace => quote![{#inner}],
					Delimiter::Bracket => quote![[#inner]],
					Delimiter::None => quote![#inner],
				});
				
				*rest = next;
				if found { outer.extend(next.token_stream()); return true }
			}
			
			TokenTree::Punct(punct) => if punct.as_char() == '#' {
				if let Some((punct, next)) = next.punct() {
					if punct.as_char() == '#' {
						outer.extend(quote![#punct]);
						*rest = next;
						continue;
					}
				}
				let name = crate::span_to(name, punct.span());
				outer.extend(quote![#(#name)*]);
				outer.extend(next.token_stream());
				return true
			} else { outer.extend(quote![#punct]); *rest = next; }
			
			tt => { outer.extend(quote![#tt]); *rest = next; }
		}
	}
	false
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expand(
	Item { mut attrs, object, mut0, name, annex, build, content }: Item,
	 objects: &mut TokenStream,
	builders: &mut Vec<crate::Builder>,
	settings: &mut TokenStream,
	bindings: &mut TokenStream,
	  pattrs: &[syn::Attribute],
	assignee: Option<&[&syn::Ident]>,
	 builder: Option<usize>,
) {
	crate::extend_attributes(&mut attrs, pattrs);
	
	let new_builder = object.map(|object| crate::expand_object(
		object, objects, builders, &attrs, mut0, &name, build.is_some()
	)).unwrap_or(None);
	
	for content in content { content::expand(
		content, objects, builders, settings, bindings, &attrs, &[&name], new_builder
	) }
	
	let Some(assignee) = assignee else { return };
	
	let Some(annex) = annex else { return objects.extend(
		syn::Error::new(name.span(), "missing #interpolation").into_compile_error()
	) };
	
	let Annex { annex, by_ref, mut0, mode, tokens, back } = *annex;
	
	if let Some(index) = builder {
		if let Some(back) = back { return back.do_not_use(objects) }
		
		if annex.segments.len() == 1 {
			#[cfg(feature = "builder-mode")]
			builders[index].1.extend(quote![.#annex(#tokens)]);
			
			#[cfg(not(feature = "builder-mode"))]
			builders[index].extend(quote![.#annex(#tokens)]);
		} else {
			objects.extend(syn::Error::new_spanned(
				annex, "cannot use long path in builder mode"
			).into_compile_error());
		}
	} else if let Some(back) = back {
		let right = match mode {
			// WARNING #annex must be a field name:
			AnnexMode::Field   (_) => quote![ #(#assignee.)* #annex = {#tokens}],
			AnnexMode::FnField (_) => quote![(#(#assignee.)* #annex)  (#tokens)],
			
			AnnexMode::Method(paren) => if annex.segments.len() == 1 {
				quote![#(#assignee.)* #annex(#tokens)]
			} else { // BUG span_to does not seem to work here
				let assignee = crate::span_to(assignee, paren.span.span());
				quote![#annex(#by_ref #mut0 #(#assignee).*, #tokens)]
			}
		};
		property::expand_back(*back, objects, builders, settings, bindings, attrs, right)
	} else {
		settings.extend(match mode {
			// WARNING #annex must be a field name:
			AnnexMode::Field   (_) => quote![#(#attrs)* #(#assignee.)* #annex = {#tokens};],
			AnnexMode::FnField (_) => quote![#(#attrs)* (#(#assignee.)* #annex) (#tokens);],
			
			AnnexMode::Method(paren) => if annex.segments.len() == 1 {
				quote![#(#attrs)* #(#assignee.)* #annex(#tokens);]
			} else { // BUG span_to does not seem to work here
				let assignee = crate::span_to(assignee, paren.span.span());
				quote![#(#attrs)* #annex(#by_ref #mut0 #(#assignee).*, #tokens);]
			}
		});
	}
}
