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

impl Parse for Pass {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		if input.peek(syn::Token![mut]) {
			input.parse::<syn::Token![mut]>()?;
			Ok(Pass(quote![&mut]))
		} else if input.peek(syn::Token![move]) {
			input.parse::<syn::Token![move]>()?;
			Ok(Pass(quote![]))
		} else {
			Ok(Pass(quote![&]))
		}
	}
}

pub(crate) fn chain(input: ParseStream) -> syn::Result<TokenStream> {
	let mut stream = TokenStream::new();
	
	loop {
		let ident = input.parse::<syn::Ident>()?;
		
		let (colons, gens) = if input.peek(syn::Token![::]) {
			let colons = input.parse::<syn::Token![::]>()?;
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

pub(crate) fn content<const B: bool>(input: ParseStream) -> syn::Result<Vec<Content<B>>> {
	let mut props = vec![];
	while !input.is_empty() { props.push(Content::parse(&input)?) }
	Ok(props)
}

pub(crate) struct Back<const B: bool> {
	pub  mut0: Option<syn::Token![mut]>,
	pub  name: syn::Ident,
	pub build: Option<syn::Token![!]>,
	pub props: Vec<Content<B>>,
}

pub(crate) fn back<const B: bool>(input: ParseStream) -> syn::Result<Back<B>> {
	let mut0 = input.parse()?;
	
	let name = input.peek(syn::Ident)
		.then(|| input.parse()).transpose()?
		.unwrap_or_else(|| syn::Ident::new(&count(), input.span()));
	
	let build = input.parse()?;
	
	let content; syn::braced!(content in input);
	let mut props = vec![];
	while !content.is_empty() { props.push(content.parse()?) }
	
	Ok(Back { mut0, name, build, props })
}

pub(crate) fn item_content<const B: bool>(input: ParseStream) -> syn::Result<(Vec<Content<B>>, Option<Back<B>>)> {
	let mut props = vec![];
	let mut  back = None;
	let braces; syn::braced!(braces in input);
	
	while !braces.is_empty() {
		let content = Content::parse(&braces)?;
		
		if let Content::Back(token) = content {
			if back.is_some() {
				Err(syn::Error::new_spanned(token, "cannot use 'back more than once"))?
			}
			
			back = Some(self::back(&braces)?)
		} else { props.push(content); }
	}
	
	Ok((props, back))
}

pub(crate) fn props<T: Parse>(input: ParseStream) -> syn::Result<Vec<T>> {
	if input.peek(syn::token::Brace) {
		let content; syn::braced!(content in input);
		let mut props = vec![];
		while !content.is_empty() { props.push(T::parse(&content)?) }
		Ok(props)
	} else { Ok(vec![input.parse()?]) }
}

pub(crate) struct Clone(syn::Ident, Option<syn::Expr>);

impl Parse for Clone {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let ident = input.parse()?;
		
		let expr = if input.peek(syn::Token![as]) {
			input.parse::<syn::Token![as]>()?;
			Some(input.parse()?)
		} else { None };
		
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

pub(crate) fn clones(input: ParseStream) -> syn::Result<Punctuated<Clone, syn::Token![,]>> {
	if input.peek(syn::token::Brace) {
		let content; syn::braced!(content in input);
		content.parse_terminated(<Clone as Parse>::parse, syn::Token![,])
	} else {
		let mut clones = Punctuated::new();
		clones.push(input.parse()?);
		Ok(clones)
	}
}
