/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use crate::{property::{self, Prop, Expr}, common, content};

pub(crate) struct If<T: Expand> {
	else0: Option<syn::Token![else]>,
	  if0: Option<syn::Token![if]>,
	 expr: Option<syn::Expr>,
	props: Vec<Inner<T>>,
}

impl<T: Expand> Parse for If<T> {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let else0 = input.parse()?;
		let if0: Option<_> = input.parse()?;
		let expr = if0.is_some().then(|| syn::Expr::parse_without_eager_brace(input)).transpose()?;
		let props = common::props(input)?;
		
		Ok(If { else0, if0, expr, props })
	}
}

pub(crate) fn parse_ifs<T: Parse>(input: ParseStream) -> syn::Result<Vec<T>> {
	let mut ifs = vec![input.parse()?];
	while input.peek(syn::Token![else]) { ifs.push(input.parse()?); }
	Ok(ifs)
}

pub(crate) struct Match<T: Expand> {
	token: syn::Token![match],
	 expr: syn::Expr,
	 arms: Vec<Arm<T>>,
}

impl<T: Expand> Parse for Match<T> {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		Ok(Match {
			token: input.parse()?,
			 expr: syn::Expr::parse_without_eager_brace(input)?,
			 arms: {
				let content; syn::braced!(content in input);
				let mut arms = vec![];
				while !content.is_empty() { arms.push(content.parse()?) }
				arms
			},
		})
	}
}

pub(crate) struct Arm<T: Expand> {
	attrs: Vec<syn::Attribute>,
	  pat: syn::Pat,
	guard: Option<(syn::Token![if], syn::Expr)>,
	arrow: syn::Token![=>],
	 body: Vec<Inner<T>>,
	comma: Option<syn::Token![,]>,
}

impl<T: Expand> Parse for Arm<T> {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		Ok(Arm {
			attrs: syn::Attribute::parse_outer(input)?,
			  pat: syn::Pat::parse_multi_with_leading_vert(input)?,
			guard: input.peek(syn::Token![if]).then(|| Ok::<_, syn::Error>((input.parse()?, input.parse()?))).transpose()?,
			arrow: input.parse()?,
			 body: common::props(input)?,
			comma: input.parse()?,
		})
	}
}

pub(crate) enum Inner<T: Expand> {
	If { attrs: Vec<syn::Attribute>, if0: Vec<If<T>> },
	Match { attrs: Vec<syn::Attribute>, mat: Match<T> },
	Prop(T),
}

impl<T: Expand> Parse for Inner<T> {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let ahead = input.fork();
		syn::Attribute::parse_outer(&ahead)?;
		
		if ahead.peek(syn::Token![if]) {
			let attrs = syn::Attribute::parse_outer(input)?;
			Ok(Inner::If { attrs, if0: parse_ifs(input)? })
		} else if ahead.peek(syn::Token![match]) {
			let attrs = syn::Attribute::parse_outer(input)?;
			Ok(Inner::Match { attrs, mat: input.parse()? })
		} else {
			Ok(Inner::Prop(input.parse()?))
		}
	}
}

pub(crate) fn expand_if<T: Expand>(
	If { else0, if0, expr, props }: If<T>,
	 objects: &mut TokenStream,
	builders: &mut Vec<TokenStream>,
	settings: &mut TokenStream,
	bindings: &mut TokenStream,
	    name: &[&syn::Ident],
	     now: bool,
) {
	let block = &mut TokenStream::new();
	
	props.into_iter().for_each(|inner| expand_inner(
		inner, objects, builders, settings, bindings, Some(block), name)
	);
	
	if now { settings.extend(quote![#else0 #if0 #expr { #block }]) };
	bindings.extend(quote![#else0 #if0 #expr { #block }])
}

pub(crate) fn expand_match<T: Expand>(
	Match { token, expr, arms }: Match<T>,
	 objects: &mut TokenStream,
	builders: &mut Vec<TokenStream>,
	settings: &mut TokenStream,
	bindings: &mut TokenStream,
	    name: &[&syn::Ident],
	     now: bool,
) {
	let body: Vec<_> = arms.into_iter()
		.map(|Arm { attrs, pat, guard, arrow, body, comma }| {
			let block = &mut TokenStream::new();
			let (if0, expr) = guard.unzip();
			
			body.into_iter().for_each(|inner| expand_inner(
				inner, objects, builders, settings, bindings, Some(block), name)
			);
			
			quote![#(#attrs)* #pat #if0 #expr #arrow { #block } #comma]
		})
		.collect();
	
	if now { settings.extend(quote![#token #expr { #(#body)* }]) };
	bindings.extend(quote![#token #expr { #(#body)* }])
}

pub(crate) fn expand_inner<T: Expand>(
	   inner: Inner<T>,
	 objects: &mut TokenStream,
	builders: &mut Vec<TokenStream>,
	settings: &mut TokenStream,
	bindings: &mut TokenStream,
	   block: Option<&mut TokenStream>,
	    name: &[&syn::Ident],
) {
	match inner {
		Inner::If { attrs, if0 } => if let Some(block) = block {
			block.extend(quote![#(#attrs)*]);
			
			if0.into_iter().for_each(|If { else0, if0, expr, props }| {
				let stream = &mut TokenStream::new();
				props.into_iter().for_each(|inner| expand_inner(
					inner, objects, builders, settings, bindings, Some(stream), name
				));
				block.extend(quote![#else0 #if0 #expr { #stream }]);
			})
		} else {
			settings.extend(quote![#(#attrs)*]);
			
			if0.into_iter().for_each(|If { else0, if0, expr, props }| {
				let  objects = &mut TokenStream::new();
				let builders = &mut vec![];
				let    setup = &mut TokenStream::new();
				
				props.into_iter().for_each(|inner| expand_inner(
					inner, objects, builders, setup, bindings, None, name
				));
				settings.extend(quote![#else0 #if0 #expr { #objects #(#builders;)* #setup #block }]);
			})
		}
		Inner::Match { attrs, mat: Match { token, expr, arms } } => if let Some(block) = block {
			block.extend(quote![#(#attrs)*]);
			
			let body: Vec<_> = arms.into_iter()
				.map(|Arm { attrs, pat, guard, arrow, body, comma }| {
					let stream = &mut TokenStream::new();
					let (if0, expr) = guard.unzip();
					
					body.into_iter().for_each(|inner| expand_inner(
						inner, objects, builders, settings, bindings, Some(stream), name)
					);
					
					quote![#(#attrs)* #pat #if0 #expr #arrow { #stream } #comma]
				})
				.collect();
			
			settings.extend(quote![#token #expr { #(#body)* }])
		} else {
			settings.extend(quote![#(#attrs)*]);
			
			let body = arms.into_iter()
				.map(|Arm { attrs, pat, guard, arrow, body, comma }| {
					let (if0, expr) = guard.unzip();
					let  objects = &mut TokenStream::new();
					let builders = &mut vec![];
					let    setup = &mut TokenStream::new();
					
					body.into_iter().for_each(|inner| expand_inner(
						inner, objects, builders, setup, bindings, None, name)
					);
					
					quote![#(#attrs)* #pat #if0 #expr #arrow { #objects #(#builders;)* #setup #block } #comma]
				});
			
			settings.extend(quote![#token #expr { #(#body)* }])
		}
		Inner::Prop(prop) => {
			let stream = prop.expand(objects, builders, settings, bindings, name);
			if let Some(block) = block { block.extend(stream) }
		}
	}
}

pub(crate) trait Expand: Parse {
	fn expand(self,
		 objects: &mut TokenStream,
		builders: &mut Vec<TokenStream>,
		settings: &mut TokenStream,
		bindings: &mut TokenStream,
		    name: &[&syn::Ident],
	) -> Option<TokenStream>;
}

impl Expand for Prop<Expr<false>> {
	fn expand(self,
		 objects: &mut TokenStream,
		builders: &mut Vec<TokenStream>,
		settings: &mut TokenStream,
		bindings: &mut TokenStream,
		    name: &[&syn::Ident],
	) -> Option<TokenStream> {
		property::expand_expr(
			self, objects, builders, settings, bindings, &[], name, false
		)
	}
}

impl Expand for content::Content<false> {
	fn expand(self,
		 objects: &mut TokenStream,
		builders: &mut Vec<TokenStream>,
		settings: &mut TokenStream,
		bindings: &mut TokenStream,
		    name: &[&syn::Ident],
	) -> Option<TokenStream> {
		content::expand(self, objects, builders, settings, bindings, &[], name, None);
		None
	}
}
