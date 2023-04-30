/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use common::ParseReactive;
use proc_macro2::TokenStream;
use quote::quote;
use crate::{property::{self, Prop, Expr}, common, content};

pub(crate) struct If<T> {
	else0: Option<syn::Token![else]>,
	  if0: Option<syn::Token![if]>,
	 expr: Option<syn::Expr>,
	props: Vec<Inner<T>>,
}

impl<T: Expand> ParseReactive for If<T> {
	fn parse(input: syn::parse::ParseStream,
	        _attrs: Option<Vec<syn::Attribute>>,
	      reactive: bool,
	) -> syn::Result<Self> {
		let else0 = input.parse()?;
		let if0: Option<_> = input.parse()?;
		let expr = if0.is_some()
			.then(|| input.call(syn::Expr::parse_without_eager_brace))
			.transpose()?;
		let props = common::props(input, reactive)?;
		
		Ok(If { else0, if0, expr, props })
	}
}

pub(crate) fn parse_ifs<T: ParseReactive>(
	input: syn::parse::ParseStream, reactive: bool
) -> syn::Result<Vec<T>> {
	let mut ifs = vec![T::parse(input, None, reactive)?];
	while input.peek(syn::Token![else]) { ifs.push(T::parse(input, None, reactive)?); }
	Ok(ifs)
}

pub(crate) struct Match<T> {
	token: syn::Token![match],
	 expr: syn::Expr,
	 arms: Vec<Arm<T>>,
}

impl<T: Expand> ParseReactive for Match<T> {
	fn parse(input: syn::parse::ParseStream,
	        _attrs: Option<Vec<syn::Attribute>>,
	     _reactive: bool,
	) -> syn::Result<Self> {
		let token = input.parse()?;
		let expr = input.call(syn::Expr::parse_without_eager_brace)?;
		let braces; syn::braced!(braces in input);
		let mut arms = vec![];
		while !braces.is_empty() { arms.push(braces.parse()?) }
		Ok(Match { token, expr, arms })
	}
}

pub(crate) struct Arm<T> {
	attrs: Vec<syn::Attribute>,
	  pat: syn::Pat,
	guard: Option<(syn::Token![if], syn::Expr)>,
	arrow: syn::Token![=>],
	 body: Vec<Inner<T>>,
}

impl<T: Expand> syn::parse::Parse for Arm<T> {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		Ok(Arm {
			attrs: input.call(syn::Attribute::parse_outer)?,
			  pat: input.call(syn::Pat::parse_multi_with_leading_vert)?,
			guard: input.parse::<syn::Token![if]>().ok()
				.map(|if0| Ok::<_, syn::Error>((if0, input.parse()?)))
				.transpose()?,
			arrow: input.parse()?,
			 body: common::props(input, false)?,
		})
	}
}

pub(crate) enum Inner<T> {
	If { attrs: Vec<syn::Attribute>, ifs: Vec<If<T>> },
	Match { attrs: Vec<syn::Attribute>, mat: Match<T> },
	Prop(T),
}

impl<T: Expand> ParseReactive for Inner<T> {
	fn parse(input: syn::parse::ParseStream,
	         attrs: Option<Vec<syn::Attribute>>,
	      reactive: bool,
	) -> syn::Result<Self> {
		if input.peek(syn::Token![if]) {
			let attrs = attrs.ok_or_else(|| input.error("BUG: missing [if] attribute"))?;
			Ok(Inner::If { attrs, ifs: parse_ifs(input, reactive)? })
		} else if input.peek(syn::Token![match]) {
			let attrs = attrs.ok_or_else(|| input.error("BUG: missing [match] attribute"))?;
			Ok(Inner::Match { attrs, mat: Match::parse(input, None, reactive)? })
		} else {
			Ok(Inner::Prop(T::parse(input, attrs, reactive)?))
		}
	}
}

pub(crate) fn expand_if<T: Expand>(
	If { else0, if0, expr, props }: If<T>,
	 objects: &mut TokenStream,
	builders: &mut Vec<crate::Builder>,
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
	builders: &mut Vec<crate::Builder>,
	settings: &mut TokenStream,
	bindings: &mut TokenStream,
	    name: &[&syn::Ident],
	     now: bool,
) {
	let body: Vec<_> = arms.into_iter()
		.map(|Arm { attrs, pat, guard, arrow, body }| {
			let block = &mut TokenStream::new();
			let (if0, expr) = guard.unzip();
			
			body.into_iter().for_each(|inner| expand_inner(
				inner, objects, builders, settings, bindings, Some(block), name)
			);
			
			quote![#(#attrs)* #pat #if0 #expr #arrow { #block }]
		})
		.collect();
	
	if now { settings.extend(quote![#token #expr { #(#body)* }]) };
	bindings.extend(quote![#token #expr { #(#body)* }])
}

pub(crate) fn expand_inner<T: Expand>(
	   inner: Inner<T>,
	 objects: &mut TokenStream,
	builders: &mut Vec<crate::Builder>,
	settings: &mut TokenStream,
	bindings: &mut TokenStream,
	   block: Option<&mut TokenStream>,
	    name: &[&syn::Ident],
) {
	match inner {
		Inner::If { attrs, ifs } => if let Some(block) = block {
			block.extend(quote![#(#attrs)*]);
			
			ifs.into_iter().for_each(|If { else0, if0, expr, props }| {
				let stream = &mut TokenStream::new();
				
				props.into_iter().for_each(|inner| expand_inner(
					inner, objects, builders, settings, bindings, Some(stream), name
				));
				
				block.extend(quote![#else0 #if0 #expr { #stream }]);
			})
		} else {
			settings.extend(quote![#(#attrs)*]);
			
			ifs.into_iter().for_each(|If { else0, if0, expr, props }| {
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
				.map(|Arm { attrs, pat, guard, arrow, body }| {
					let stream = &mut TokenStream::new();
					let (if0, expr) = guard.unzip();
					
					body.into_iter().for_each(|inner| expand_inner(
						inner, objects, builders, settings, bindings, Some(stream), name)
					);
					
					quote![#(#attrs)* #pat #if0 #expr #arrow { #stream }]
				})
				.collect();
			
			settings.extend(quote![#token #expr { #(#body)* }])
		} else {
			settings.extend(quote![#(#attrs)*]);
			
			let body = arms.into_iter()
				.map(|Arm { attrs, pat, guard, arrow, body }| {
					let (if0, expr) = guard.unzip();
					let  objects = &mut TokenStream::new();
					let builders = &mut vec![];
					let    setup = &mut TokenStream::new();
					
					body.into_iter().for_each(|inner| expand_inner(
						inner, objects, builders, setup, bindings, None, name)
					);
					
					quote![#(#attrs)* #pat #if0 #expr #arrow { #objects #(#builders;)* #setup #block }]
				});
			
			settings.extend(quote![#token #expr { #(#body)* }])
		}
		Inner::Prop(prop) => {
			let stream = prop.expand(objects, builders, settings, bindings, name);
			if let Some(block) = block { block.extend(stream) }
		}
	}
}

pub(crate) trait Expand: ParseReactive {
	fn expand(self,
		 objects: &mut TokenStream,
		builders: &mut Vec<crate::Builder>,
		settings: &mut TokenStream,
		bindings: &mut TokenStream,
		    name: &[&syn::Ident],
	) -> Option<TokenStream>;
}

impl Expand for Prop<Expr> {
	fn expand(self,
		 objects: &mut TokenStream,
		builders: &mut Vec<crate::Builder>,
		settings: &mut TokenStream,
		bindings: &mut TokenStream,
		    name: &[&syn::Ident],
	) -> Option<TokenStream> {
		property::expand_expr(
			self, objects, builders, settings, bindings, &[], name, false
		)
	}
}

impl Expand for content::Content {
	fn expand(self,
		 objects: &mut TokenStream,
		builders: &mut Vec<crate::Builder>,
		settings: &mut TokenStream,
		bindings: &mut TokenStream,
		    name: &[&syn::Ident],
	) -> Option<TokenStream> {
		content::expand(self, objects, builders, settings, bindings, &[], name, None);
		None
	}
}
