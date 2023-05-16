/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use crate::{content, property, Builder};

pub(crate) struct Item {
	  attrs: Vec<syn::Attribute>,
	 object: Object,
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
	let object = input.parse()?;
	let pound = input.parse::<syn::Token![#]>();
	let name = Name::from(&object);
	
	if let (Ok(pound), true) = (&pound, root) {
		Err(syn::Error::new(pound.span, "cannot #interpolate here"))?
	}
	
	let mut annex = pound.is_ok().then(|| parse_annex(input, &name, reactive)).transpose()?;
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
	
	Ok(Item { attrs, object, annex, build, content })
}

enum Object {
	Expr (Box<syn::Expr>    , Option<syn::Token![mut]>, syn::Ident),
	Type (Box<syn::TypePath>, Option<syn::Token![mut]>, syn::Ident),
	 Ref (Vec<syn::Ident>),
}

impl ToTokens for Object {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		match self {
			Object::Expr(expr, mut0, name) => {
				expr.to_tokens(tokens);
				mut0.to_tokens(tokens);
				name.to_tokens(tokens);
			},
			Object::Type(ty, mut0, name) => {
				  ty.to_tokens(tokens);
				mut0.to_tokens(tokens);
				name.to_tokens(tokens);
			},
			Object::Ref(idents) => for ident in idents { ident.to_tokens(tokens) },
		}
	}
}

impl syn::parse::Parse for Object {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		if input.parse::<syn::Token![ref]>().is_ok() {
			let mut idents = vec![input.parse()?];
			while input.parse::<syn::Token![.]>().is_ok() {
				idents.push(input.parse()?);
			}
			return Ok(Object::Ref(idents))
		}
		
		let ahead = input.fork();
		
		Ok(if ahead.parse::<syn::TypePath>().is_ok() && (
			   ahead.peek(syn::Token![mut])
			|| ahead.peek(syn::Ident)
			|| ahead.peek(syn::Token![#])
			|| ahead.peek(syn::Token![!])
			|| ahead.peek(syn::token::Brace)
		) {
			Object::Type(input.parse()?, input.parse()?, input.parse::<Option<_>>()?
				.unwrap_or_else(|| syn::Ident::new(&crate::count(), input.span())))
		} else {
			Object::Expr(input.parse()?, input.parse()?, input.parse::<Option<_>>()?
				.unwrap_or_else(|| syn::Ident::new(&crate::count(), input.span())))
		})
	}
}

fn expand_object(
	  object: &Object,
	 objects: &mut TokenStream,
	builders: &mut Vec<Builder>,
	   attrs: &[syn::Attribute],
	 builder: bool,
) -> Option<usize> {
	if !builder {
		objects.extend(match object {
			Object::Type(ty, mut0, name) => quote![#(#attrs)* let #mut0 #name = #ty::default();],
			Object::Expr(call, mut0, name) => quote![#(#attrs)* let #mut0 #name = #call;],
			Object::Ref(_) => return None,
		});
		return None
	}
	
	builders.push(match object {
		#[cfg(feature = "builder-mode")]
		Object::Type(ty, mut0, name) => Builder(
			quote![#(#attrs)* let #mut0 #name =], quote![#ty =>], None
		),
		#[cfg(not(feature = "builder-mode"))]
		Object::Type(ty, mut0, name) => quote![#(#attrs)* let #mut0 #name = #ty::default()],
		
		#[cfg(feature = "builder-mode")]
		Object::Expr(expr, mut0, name) => Builder(
			quote![#(#attrs)* let #mut0 #name =], quote![#expr], None
		),
		#[cfg(not(feature = "builder-mode"))]
		Object::Expr(expr, mut0, name) => quote![#(#attrs)* let #mut0 #name = #expr],
		
		Object::Ref(_) => return None,
	});
	
	Some(builders.len() - 1)
}

struct Annex {
	 annex: syn::Path,
	by_ref: Option<syn::Token![&]>,
	  mut0: Option<syn::Token![mut]>,
	  mode: AnnexMode,
	tokens: TokenStream,
	  back: Option<Box<property::Back>>,
}

enum AnnexMode { Field (Span), FnField (Span), Method (Span) }

fn parse_annex(
	   input: syn::parse::ParseStream,
	    name: &[&syn::Ident],
	reactive: bool,
) -> syn::Result<Box<Annex>> {
	let annex: syn::Path = input.parse()?;
	
	let (by_ref, mut0) = if annex.segments.len() > 1 {
		(input.parse()?, input.parse()?)
	} else { (None, None) };
	
	let (mode, buffer) = if input.peek(syn::token::Paren) {
		let parens; syn::parenthesized!(parens in input);
		(AnnexMode::Method(parens.span()), parens)
	} else if input.peek(syn::token::Brace) {
		let braces; syn::braced!(braces in input);
		(AnnexMode::Field(braces.span()), braces)
	} else {
		let brackets; syn::bracketed!(brackets in input);
		(AnnexMode::FnField(brackets.span()), brackets)
	};
	
	let tokens = buffer.step(|cursor| {
		let mut rest = *cursor;
		let mut stream = TokenStream::new();
		
		crate::find_pound(&mut rest, &mut stream, name)
			.then(|| (stream, syn::buffer::Cursor::empty()))
			.ok_or_else(|| cursor.error("no single `#` found around here"))
	})?;
	
	let back = if let AnnexMode::FnField(_) | AnnexMode::Method(_) = &mode {
		property::parse_back(input, reactive)?
	} else { None };
	
	Ok(Box::new(Annex { annex, by_ref, mut0, mode, tokens, back }))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expand(
	Item { mut attrs, object, annex, build, content }: Item,
	 objects: &mut TokenStream,
	builders: &mut Vec<Builder>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	  pattrs: &[syn::Attribute],
	assignee: Option<&[&syn::Ident]>,
	 builder: Option<usize>,
) {
	crate::extend_attributes(&mut attrs, pattrs);
	let new_builder = expand_object(&object, objects, builders, &attrs, build.is_some());
	
	for content in content { content::expand(
		content, objects, builders, settings, bindings, &attrs, &Name::from(&object), new_builder
	) }
	
	let Some(assignee) = assignee else { return };
	
	let Some(annex) = annex else { return objects.extend(syn::Error::new_spanned(
		quote![#object #build], "missing #interpolation"
	).into_compile_error()) };
	
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
			
			AnnexMode::Method(span) => if annex.segments.len() == 1 {
				quote![#(#assignee.)* #annex(#tokens)]
			} else {
				let assignee = crate::span_to(assignee, span);
				quote![#annex(#by_ref #mut0 #(#assignee).*, #tokens)]
			}
		};
		property::expand_back(*back, objects, builders, settings, bindings, attrs, right)
	} else {
		settings.extend(match mode {
			// WARNING #annex must be a field name:
			AnnexMode::Field   (_) => quote![#(#attrs)* #(#assignee.)* #annex = {#tokens};],
			AnnexMode::FnField (_) => quote![#(#attrs)* (#(#assignee.)* #annex) (#tokens);],
			
			AnnexMode::Method(span) => if annex.segments.len() == 1 {
				quote![#(#attrs)* #(#assignee.)* #annex(#tokens);]
			} else {
				let assignee = crate::span_to(assignee, span);
				quote![#(#attrs)* #annex(#by_ref #mut0 #(#assignee).*, #tokens);]
			}
		});
	}
}

enum Name<'a> { Slice([&'a syn::Ident; 1]), Vec(Vec<&'a syn::Ident>) }

impl<'a> From <&'a Object> for Name<'a> {
	fn from(object: &'a Object) -> Self {
		match &object {
			Object::Expr(.., name) => Name::Slice([name]),
			Object::Type(.., name) => Name::Slice([name]),
			Object::Ref(ref0) => Name::Vec(ref0.iter().collect()),
		}
	}
}

impl<'a> std::ops::Deref for Name<'a> {
	type Target = [&'a syn::Ident];
	
	fn deref(&self) -> &Self::Target {
		match self {
			Name::Slice(slice) => slice,
			Name::Vec(vec) => vec.as_slice(),
		}
	}
}
