/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::TokenStream;
use quote::quote;
use std::cell::RefCell;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use crate::content::Content;

thread_local![static COUNT: RefCell<usize> = RefCell::new(0)];

pub(crate) fn count() -> String {
	COUNT.with(move |cell| {
		let count = *cell.borrow();
		*cell.borrow_mut() = count.wrapping_add(1);
		format!("_declarative_{}", count)
	})
}

pub(crate) trait ParseReactive: Sized {
	fn parse(input: ParseStream,
	         attrs: Option<Vec<syn::Attribute>>,
	      reactive: bool,
	) -> syn::Result<Self>;
}

pub(crate) enum Object { Constructor(syn::Expr), Type(syn::TypePath) }

impl Parse for Object {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let ahead = input.fork();
		
		if ahead.parse::<syn::Path>().is_ok() && !(
			ahead.peek(syn::Token![.]) || ahead.peek(syn::token::Paren)
		) {
			Ok(Object::Type(input.parse()?))
		} else {
			Ok(Object::Constructor(input.parse()?))
		}
	}
}

pub(crate) fn expand_object(
	  object: Object,
	 objects: &mut TokenStream,
	builders: &mut Vec<TokenStream>,
	   attrs: &[syn::Attribute],
	    mut0: Option<syn::Token![mut]>,
	    name: &syn::Ident,
	 builder: bool,
) -> Option<usize> {
	if builder {
		builders.push(match object {
			Object::Type(ty) => quote![#(#attrs)* let #mut0 #name = #ty::builder()],
			Object::Constructor(call) => quote![#(#attrs)* let #mut0 #name = #call],
		});
		Some(builders.len() - 1)
	} else {
		objects.extend(match object {
			Object::Type(ty) => quote![#(#attrs)* let #mut0 #name = #ty::default();],
			Object::Constructor(call) => quote![#(#attrs)* let #mut0 #name = #call;],
		});
		None
	}
}

pub(crate) struct Pass(pub TokenStream);

pub(crate) fn parse_pass(input: ParseStream, root: bool) -> syn::Result<Pass> {
	if let Ok(mut0) = input.parse::<syn::Token![mut]>() {
		if root { Err(syn::Error::new_spanned(mut0, "cannot use mut"))? }
		Ok(Pass(quote![&#mut0]))
	} else if let Ok(mov) = input.parse::<syn::Token![move]>() {
		if root { Err(syn::Error::new_spanned(mov, "cannot use move"))? }
		Ok(Pass(quote![]))
	} else {
		Ok(Pass(quote![&]))
	}
}

pub(crate) fn chain(input: ParseStream) -> syn::Result<TokenStream> {
	let mut stream = TokenStream::new();
	loop {
		let ident = input.parse::<syn::Ident>()?;
		
		let (colons, gens) = if let Ok(colons) = input.parse::<syn::Token![::]>() {
			let gens = input.parse::<syn::AngleBracketedGenericArguments>()?;
			(Some(colons), Some(gens))
		} else { (None, None) };
		
		let parens = if input.peek(syn::token::Paren) {
			Some(input.parse::<proc_macro2::TokenTree>()?)
		} else { None };
		
		stream.extend(quote![.#ident #colons #gens #parens]);
		
		if !input.peek(syn::Token![.]) { break }
	}
	Ok(stream)
}

pub(crate) fn content(input: ParseStream, reactive: bool) -> syn::Result<Vec<Content>> {
	let mut props = vec![];
	while !input.is_empty() { props.push(Content::parse(input, None, reactive)?) }
	Ok(props)
}

pub(crate) struct Back {
	pub  token: syn::Lifetime,
	pub battrs: Vec<syn::Attribute>,
	pub   mut0: Option<syn::Token![mut]>,
	pub   back: syn::Ident, // name
	pub  build: Option<syn::Token![!]>,
	pub  props: Vec<Content>,
}

impl Back {
	pub(crate) fn do_not_use(self, stream: &mut TokenStream) {
		let error = syn::Error::new_spanned(self.token, "cannot use 'back");
		stream.extend(error.into_compile_error())
	}
}

pub(crate) fn parse_back(
	   input: ParseStream,
	   token: syn::Lifetime,
	  battrs: Vec<syn::Attribute>,
	reactive: bool,
) -> syn::Result<Back> {
	let mut0 = input.parse()?;
	let back = input.parse().unwrap_or_else(|_| syn::Ident::new(&count(), input.span()));
	let build = input.parse()?;
	
	let braces; syn::braced!(braces in input);
	let mut props = vec![];
	while !braces.is_empty() { props.push(Content::parse(&braces, None, reactive)?) }
	
	Ok(Back { token, battrs, mut0, back, build, props })
}

pub(crate) fn object_content(
	input: ParseStream, reactive: bool, root: bool
) -> syn::Result<(Vec<Content>, Option<Back>)> {
	let mut props = vec![];
	let mut  back = None;
	let braces; syn::braced!(braces in input);
	
	while !braces.is_empty() {
		let content = Content::parse(&braces, None, reactive)?;
		
		if let Content::Back(ba) = content {
			if root { return Err(syn::Error::new_spanned(ba.token, "cannot use 'back")) }
			if back.is_some() {
				return Err(syn::Error::new_spanned(ba.token, "cannot use 'back more than once"))
			}
			back = Some(ba)
		} else { props.push(content); }
	}
	
	Ok((props, back))
}

pub(crate) fn props<T: ParseReactive>(input: ParseStream, reactive: bool) -> syn::Result<Vec<T>> {
	if input.peek(syn::token::Brace) {
		let braces; syn::braced!(braces in input);
		let mut props = vec![];
		
		while !braces.is_empty() {
			props.push(T::parse(&braces, Some(braces.call(syn::Attribute::parse_outer)?), reactive)?)
		}
		
		Ok(props)
	} else { Ok(vec![T::parse(input, None, reactive)?]) }
}

pub(crate) struct Clone(syn::Ident, Option<syn::Expr>);

impl Parse for Clone {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let ident = input.parse()?;
		let expr = input.parse::<syn::Token![as]>()
			.is_ok().then(|| input.parse()).transpose()?;
		Ok(Clone(ident, expr))
	}
}

impl quote::ToTokens for Clone {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		tokens.extend(match &self {
			Clone(ident, Some(expr)) => quote![#ident = #expr],
			Clone(ident, None) => quote![#ident = #ident.clone()],
		})
	}
}

pub(crate) fn extend_attributes(attrs: &mut Vec<syn::Attribute>, pattrs: &[syn::Attribute]) {
	let current = std::mem::take(attrs);
	attrs.reserve(pattrs.len() + current.len());
	attrs.extend_from_slice(pattrs);
	attrs.extend(current.into_iter());
}

pub(crate) fn parse_clones(input: ParseStream) -> syn::Result<Punctuated<Clone, syn::Token![,]>> {
	let Ok(keyword) = input.parse::<syn::Lifetime>() else { return Ok(Punctuated::new()) };
	
	if keyword.ident != "clone" {
		Err(syn::Error::new_spanned(keyword, "expected 'clone"))
	} else if input.peek(syn::token::Brace) {
		let braces; syn::braced!(braces in input);
		braces.parse_terminated(Clone::parse, syn::Token![,])
	} else {
		let mut clones = Punctuated::new();
		clones.push(input.parse()?);
		Ok(clones)
	}
}
