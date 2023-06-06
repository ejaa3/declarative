/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use {proc_macro2::{Delimiter, Group, TokenStream}, quote::quote};
use crate::{property, content, Assignee, Builder, Bindings, ParseReactive};

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
		// it is possible to put something after the match arrow:
		let map = || input.call(syn::Attribute::parse_outer);
		
		if input.peek(syn::Token![if]) {
			let attrs = attrs.map_or_else(map, Result::Ok)?;
			let mut vec = vec![If::parse(input, None, reactive)?];
			while input.peek(syn::Token![else]) { vec.push(If::parse(input, None, reactive)?) }
			Ok(Inner::If(attrs, vec))
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
	settings: &mut TokenStream,
	bindings: &mut Bindings,
	  pattrs: &[syn::Attribute],
	assignee: Assignee,
	    bind: Option<bool>,
) {
	macro_rules! stream {
		($expr:expr) => {
			let stream = $expr;
			
			if let Some(set) = bind {
				if set { settings.extend(stream.clone()) }
				bindings.stream.extend(stream)
			} else { settings.extend(stream) }
		};
	}
	
	match inner {
		Inner::If(attrs, if_vec) => {
			stream!(quote![#(#pattrs)* #(#attrs)*]);
			
			for If { else_, if_, expr, brace, inner } in if_vec {
				let mut setup = TokenStream::new();
				
				for inner in inner { expand(
					inner, &mut setup, bindings, &[], assignee, None
				) }
				
				let mut body = Group::new(Delimiter::Brace, setup);
				body.set_span(brace.span.join());
				stream!(quote![#else_ #if_ #expr #body]);
			}
		}
		Inner::Match(attrs, match_) => {
			let Match { token, expr, arms } = *match_;
			stream!(quote![#(#pattrs)* #(#attrs)*]);
			
			let body = arms.into_iter().map(|Arm { attrs, pat, guard, arrow, brace, body }| {
				let (if_, expr) = guard.as_deref().map(|(a, b)| (a, b)).unzip();
				let mut setup = TokenStream::new();
				
				for inner in body { expand(
					inner, &mut setup, bindings, &[], assignee, None
				) }
				
				let mut body = Group::new(Delimiter::Brace, setup);
				if let Some(brace) = brace { body.set_span(brace.span.join()); } // WARNING not always hygienic
				quote![#(#attrs)* #pat #if_ #expr #arrow #body]
			});
			
			stream!(quote![#token #expr { #(#body)* }]);
		}
		Inner::Prop(prop) => {
			let  objects = &mut TokenStream::new();
			let builders = &mut vec![];
			let    setup = &mut TokenStream::new();
			
			prop.expand(assignee, objects, builders, setup, bindings);
			
			let builders = builders.iter().rev();
			stream!(quote![#objects #(#builders;)* #setup]);
		}
	}
}

pub(crate) trait Expand: ParseReactive {
	fn expand(self, assignee: Assignee,
		 objects: &mut TokenStream, builders: &mut Vec<Builder>,
		settings: &mut TokenStream, bindings: &mut Bindings,
	);
}

impl Expand for Box<property::Prop> {
	fn expand(self, assignee: Assignee,
		 objects: &mut TokenStream, builders: &mut Vec<Builder>,
		settings: &mut TokenStream, bindings: &mut Bindings,
	) { property::expand(*self, objects, builders, settings, bindings, &[], assignee, None) }
}

impl Expand for content::Content {
	fn expand(self, assignee: Assignee,
		 objects: &mut TokenStream, builders: &mut Vec<Builder>,
		settings: &mut TokenStream, bindings: &mut Bindings,
	) { content::expand(self, objects, builders, settings, bindings, &[], assignee, None) }
}

pub struct If<T> {
	else_: Option<syn::Token![else]>,
	  if_: Option<syn::Token![if]>,
	 expr: Option<Box<syn::Expr>>,
	brace: syn::token::Brace,
	inner: Vec<Inner<T>>,
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
		
		let (brace, inner) = crate::parse_vec(input, reactive)?;
		Ok(If { else_, if_, expr, brace, inner })
	}
}

pub struct Match<T> { token: syn::Token![match], expr: syn::Expr, arms: Vec<Arm<T>> }

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

struct Arm<T> {
	attrs: Vec<syn::Attribute>,
	  pat: syn::Pat,
	guard: Option<Box<(syn::Token![if], syn::Expr)>>,
	arrow: syn::Token![=>],
	brace: Option<syn::token::Brace>,
	 body: Vec<Inner<T>>,
}

impl<T: Expand> syn::parse::Parse for Arm<T> {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		let attrs = input.call(syn::Attribute::parse_outer)?;
		let pat = input.call(syn::Pat::parse_multi_with_leading_vert)?;
		
		let guard = if let Ok(if_) = input.parse::<syn::Token![if]>() {
			Some(Box::new((if_, input.parse()?)))
		} else { None };
		
		let arrow = input.parse()?;
		
		let (brace, body) = if let Ok((brace, body)) = crate::parse_vec(input, false) {
			(Some(brace), body)
		} else { (None, vec![Inner::parse(input, None, false)?]) };
		
		Ok(Arm { attrs, pat, guard, arrow, brace, body })
	}
}
