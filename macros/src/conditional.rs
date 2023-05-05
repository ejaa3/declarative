/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::TokenStream;
use quote::quote;
use crate::{property, content};

pub(crate) enum Inner<T> {
	If    (Vec<syn::Attribute>, Vec<If<T>>),
	Match (Vec<syn::Attribute>, Box<Match<T>>),
	Prop  (T),
}

impl<T: Expand> crate::ParseReactive for Inner<T> {
	fn parse(input: syn::parse::ParseStream,
	         attrs: Option<Vec<syn::Attribute>>,
	      reactive: bool,
	) -> syn::Result<Self> {
		// it is possible to put a condition after the arrow of the match arm:
		let map = || input.call(syn::Attribute::parse_outer);
		
		if input.peek(syn::Token![if]) {
			let attrs = attrs.map_or_else(map, Result::Ok)?;
			Ok(Inner::If(attrs, parse_ifs(input, reactive)?))
		} else if input.peek(syn::Token![match]) {
			let attrs = attrs.map_or_else(map, Result::Ok)?;
			Ok(Inner::Match(attrs, Match::parse(input, None, reactive)?.into()))
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
	bindings: &mut TokenStream,
	   block: Option<&mut TokenStream>,
	    name: &[&syn::Ident],
) {
	match inner {
		Inner::If(attrs, ifs) => if let Some(block) = block {
			block.extend(quote![#(#attrs)*]);
			
			for If { else0, if0, expr, inner } in ifs {
				let stream = &mut TokenStream::new();
				
				for inner in inner { expand(
					inner, objects, builders, settings, bindings, Some(stream), name
				) }
				
				block.extend(quote![#else0 #if0 #expr { #stream }]);
			}
		} else {
			settings.extend(quote![#(#attrs)*]);
			
			for If { else0, if0, expr, inner } in ifs {
				let  objects = &mut TokenStream::new();
				let builders = &mut vec![];
				let    setup = &mut TokenStream::new();
				
				for inner in inner { expand(
					inner, objects, builders, setup, bindings, None, name
				) }
				
				let builders = builders.iter().rev();
				settings.extend(quote![#else0 #if0 #expr { #objects #(#builders;)* #setup #block }]);
			}
		}
		Inner::Match(attrs, match0) => {
			let Match { token, expr, arms } = *match0;
			
			if let Some(block) = block {
				block.extend(quote![#(#attrs)*]);
				
				let body = TokenStream::from_iter(arms.into_iter()
					.map(|Arm { attrs, pat, guard, arrow, body }| {
						let stream = &mut TokenStream::new();
						let (if0, expr) = guard.map(|boxed| *boxed).unzip();
						
						for inner in body { expand(
							inner, objects, builders, settings, bindings, Some(stream), name
						) }
						
						quote![#(#attrs)* #pat #if0 #expr #arrow { #stream }]
					}));
				
				block.extend(quote![#token #expr { #body }])
			} else {
				settings.extend(quote![#(#attrs)*]);
				
				let body = arms.into_iter().map(|Arm { attrs, pat, guard, arrow, body }| {
					let (if0, expr) = guard.map(|boxed| *boxed).unzip();
					let  objects = &mut TokenStream::new();
					let builders = &mut vec![];
					let    setup = &mut TokenStream::new();
					
					for inner in body { expand(
						inner, objects, builders, setup, bindings, None, name
					) }
					
					let builders = builders.iter().rev();
					
					quote![#(#attrs)* #pat #if0 #expr #arrow {
						#objects #(#builders;)* #setup #block
					}]
				});
				
				settings.extend(quote![#token #expr { #(#body)* }])
			}
		}
		Inner::Prop(prop) => {
			let stream = prop.expand(objects, builders, settings, bindings, name);
			if let Some(block) = block { block.extend(stream) }
		}
	}
}

pub(crate) trait Expand: crate::ParseReactive {
	fn expand(self,
		 objects: &mut TokenStream,
		builders: &mut Vec<crate::Builder>,
		settings: &mut TokenStream,
		bindings: &mut TokenStream,
		    name: &[&syn::Ident],
	) -> Option<TokenStream>;
}

impl Expand for property::Prop {
	fn expand(self,
		 objects: &mut TokenStream,
		builders: &mut Vec<crate::Builder>,
		settings: &mut TokenStream,
		bindings: &mut TokenStream,
		    name: &[&syn::Ident],
	) -> Option<TokenStream> {
		property::expand(
			self, objects, builders, settings, bindings, &[], name, None
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

pub(crate) struct If<T> {
	pub else0: Option<syn::Token![else]>,
	pub   if0: Option<syn::Token![if]>,
	pub  expr: Option<Box<syn::Expr>>,
	pub inner: Vec<Inner<T>>,
}

impl<T: Expand> crate::ParseReactive for If<T> {
	fn parse(input: syn::parse::ParseStream,
	        _attrs: Option<Vec<syn::Attribute>>,
	      reactive: bool,
	) -> syn::Result<Self> {
		let else0 = input.parse()?;
		let if0: Option<_> = input.parse()?;
		let expr = if if0.is_some() {
			Some(input.call(syn::Expr::parse_without_eager_brace)?.into())
		} else { None };
		
		let inner = crate::parse_vec(input, reactive)?;
		
		Ok(If { else0, if0, expr, inner })
	}
}

pub(crate) fn parse_ifs<T: crate::ParseReactive>(
	input: syn::parse::ParseStream, reactive: bool
) -> syn::Result<Vec<T>> {
	let mut ifs = vec![T::parse(input, None, reactive)?];
	while input.peek(syn::Token![else]) { ifs.push(T::parse(input, None, reactive)?); }
	Ok(ifs)
}

pub(crate) struct Match<T> {
	pub token: syn::Token![match],
	pub  expr: syn::Expr,
	pub  arms: Vec<Arm<T>>,
}

impl<T: Expand> crate::ParseReactive for Match<T> {
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
				.map(|if0| Ok::<_, syn::Error>((if0, input.parse()?).into()))
				.transpose()?,
			arrow: input.parse()?,
			 body: crate::parse_vec(input, false)?,
		})
	}
}
