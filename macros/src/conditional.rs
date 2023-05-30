/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::TokenStream;
use quote::quote;
use crate::{property, content, ParseReactive};

pub enum Inner<T> {
	   If (Vec<syn::Attribute>, Vec<If<T>>),
	Match (Vec<syn::Attribute>, Box<Match<T>>),
	 Prop (T),
}

impl<T: Expand> ParseReactive for Inner<T> {
	fn parse(input: syn::parse::ParseStream,
	         attrs: Option<Vec<syn::Attribute>>,
	      reactive: bool,
	) -> syn::Result<Self> {
		// it is possible to put a condition after the arrow of the match arm:
		let map = || input.call(syn::Attribute::parse_outer);
		
		if input.peek(syn::Token![if]) {
			let attrs = attrs.map_or_else(map, Result::Ok)?;
			Ok(Inner::If(attrs, parse_vec(input, reactive)?))
		} else if input.peek(syn::Token![match]) {
			let attrs = attrs.map_or_else(map, Result::Ok)?;
			Ok(Inner::Match(attrs, ParseReactive::parse(input, None, reactive)?))
		} else {
			Ok(Inner::Prop(T::parse(input, attrs, reactive)?))
		}
	}
}

pub(crate) fn expand<T: Expand>(
	   inner: Inner<T>,
	 objects: &mut TokenStream,
	builders: &mut Vec<crate::Builder>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	   block: Option<&mut TokenStream>,
	    name: &[&syn::Ident],
) {
	match inner {
		Inner::If(attrs, if_vec) => if let Some(block) = block {
			block.extend(quote![#(#attrs)*]);
			
			for If { else_, if_, expr, inner } in if_vec {
				let stream = &mut TokenStream::new();
				
				for inner in inner { expand(
					inner, objects, builders, settings, bindings, Some(stream), name
				) }
				
				block.extend(quote![#else_ #if_ #expr { #stream }]);
			}
		} else {
			settings.extend(quote![#(#attrs)*]);
			
			for If { else_, if_, expr, inner } in if_vec {
				let  objects = &mut TokenStream::new();
				let builders = &mut vec![];
				let    setup = &mut TokenStream::new();
				
				for inner in inner { expand(
					inner, objects, builders, setup, bindings, None, name
				) }
				
				let builders = builders.iter().rev();
				settings.extend(quote![#else_ #if_ #expr { #objects #(#builders;)* #setup #block }]);
			}
		}
		Inner::Match(attrs, match_) => {
			let Match { token, expr, arms } = *match_;
			
			if let Some(block) = block {
				block.extend(quote![#(#attrs)*]);
				
				let body = TokenStream::from_iter(arms.into_iter()
					.map(|Arm { attrs, pat, guard, arrow, body }| {
						let stream = &mut TokenStream::new();
						let (if_, expr) = guard.as_deref().map(|(a, b)| (a, b)).unzip();
						
						for inner in body { expand(
							inner, objects, builders, settings, bindings, Some(stream), name
						) }
						
						quote![#(#attrs)* #pat #if_ #expr #arrow { #stream }]
					}));
				
				block.extend(quote![#token #expr { #body }])
			} else {
				settings.extend(quote![#(#attrs)*]);
				
				let body = arms.into_iter().map(|Arm { attrs, pat, guard, arrow, body }| {
					let (if_, expr) = guard.as_deref().map(|(a, b)| (a, b)).unzip();
					let  objects = &mut TokenStream::new();
					let builders = &mut vec![];
					let    setup = &mut TokenStream::new();
					
					for inner in body { expand(
						inner, objects, builders, setup, bindings, None, name
					) }
					
					let builders = builders.iter().rev();
					
					quote![#(#attrs)* #pat #if_ #expr #arrow {
						#objects #(#builders;)* #setup #block
					}]
				});
				
				settings.extend(quote![#token #expr { #(#body)* }])
			}
		}
		Inner::Prop(prop) => {
			let stream = prop.expand(name, objects, builders, settings, bindings);
			if let Some(block) = block { block.extend(stream) }
		}
	}
}

pub(crate) trait Expand: ParseReactive {
	fn expand(self, name: &[&syn::Ident],
		 objects: &mut TokenStream, builders: &mut Vec<crate::Builder>,
		settings: &mut TokenStream, bindings: &mut crate::Bindings,
	) -> Option<TokenStream>;
}

impl Expand for property::Prop {
	fn expand(self, name: &[&syn::Ident],
		 objects: &mut TokenStream, builders: &mut Vec<crate::Builder>,
		settings: &mut TokenStream, bindings: &mut crate::Bindings,
	) -> Option<TokenStream> {
		property::expand(self, objects, builders, settings, bindings, &[], name, None)
	}
}

impl Expand for Box<property::Prop> {
	fn expand(self, name: &[&syn::Ident],
		 objects: &mut TokenStream, builders: &mut Vec<crate::Builder>,
		settings: &mut TokenStream, bindings: &mut crate::Bindings,
	) -> Option<TokenStream> {
		property::expand(*self, objects, builders, settings, bindings, &[], name, None)
	}
}

impl Expand for content::Content {
	fn expand(self, name: &[&syn::Ident],
		 objects: &mut TokenStream, builders: &mut Vec<crate::Builder>,
		settings: &mut TokenStream, bindings: &mut crate::Bindings,
	) -> Option<TokenStream> {
		content::expand(self, objects, builders, settings, bindings, &[], name, None);
		None
	}
}

pub struct If<T> {
	pub else_: Option<syn::Token![else]>,
	pub   if_: Option<syn::Token![if]>,
	pub  expr: Option<Box<syn::Expr>>,
	pub inner: Vec<Inner<T>>,
}

impl<T: Expand> ParseReactive for If<T> {
	fn parse(input: syn::parse::ParseStream,
	        _attrs: Option<Vec<syn::Attribute>>,
	      reactive: bool,
	) -> syn::Result<Self> {
		let else_ = input.parse()?;
		let if_: Option<_> = input.parse()?;
		let expr = if if_.is_some() {
			Some(Box::new(input.call(syn::Expr::parse_without_eager_brace)?))
		} else { None };
		
		let inner = crate::parse_vec(input, reactive)?;
		Ok(If { else_, if_, expr, inner })
	}
}

pub(crate) fn parse_vec<T: ParseReactive>(
	input: syn::parse::ParseStream, reactive: bool
) -> syn::Result<Vec<T>> {
	let mut vec = vec![T::parse(input, None, reactive)?];
	while input.peek(syn::Token![else]) { vec.push(T::parse(input, None, reactive)?); }
	Ok(vec)
}

pub struct Match<T> {
	pub token: syn::Token![match],
	pub  expr: syn::Expr,
	pub  arms: Vec<Arm<T>>,
}

impl<T: Expand> ParseReactive for Box<Match<T>> {
	fn parse(input: syn::parse::ParseStream,
	        _attrs: Option<Vec<syn::Attribute>>,
	     _reactive: bool,
	) -> syn::Result<Self> {
		let token = input.parse()?;
		let expr = input.call(syn::Expr::parse_without_eager_brace)?;
		let braces; syn::braced!(braces in input);
		let mut arms = vec![];
		while !braces.is_empty() { arms.push(braces.parse()?) }
		Ok(Box::new(Match { token, expr, arms }))
	}
}

pub struct Arm<T> {
	pub attrs: Vec<syn::Attribute>,
	pub   pat: syn::Pat,
	pub guard: Option<Box<(syn::Token![if], syn::Expr)>>,
	pub arrow: syn::Token![=>],
	pub  body: Vec<Inner<T>>,
}

impl<T: Expand> syn::parse::Parse for Arm<T> {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		Ok(Arm {
			attrs: input.call(syn::Attribute::parse_outer)?,
			  pat: input.call(syn::Pat::parse_multi_with_leading_vert)?,
			guard: input.parse::<syn::Token![if]>().ok()
				.map(|if_| Ok::<_, syn::Error>(Box::new((if_, input.parse()?))))
				.transpose()?,
			arrow: input.parse()?,
			 body: crate::parse_vec(input, false)?,
		})
	}
}
