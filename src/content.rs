/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use common::ParseReactive;
use proc_macro2::TokenStream;
use property::{Expr, Prop, Value, expand_expr, expand_value};
use quote::quote;
use syn::{punctuated::Punctuated, visit_mut::VisitMut};
use crate::{conditional, common, property, component, Visit};

pub(crate) enum Binding {
	If(Vec<conditional::If<Prop<Expr>>>),
	Match(conditional::Match<Prop<Expr>>),
	Props(Vec<Prop<Expr>>),
}

pub(crate) enum Content {
	Back(syn::Lifetime),
	None(Prop<Value>),
	Built(Vec<Content>),
	Component(component::Component),
	Inner(Box<conditional::Inner<Content>>),
	Bind {
		attrs: Vec<syn::Attribute>,
		 cond: Option<syn::Expr>,
		props: Vec<Prop<Expr>>,
	},
	BindOnly (Vec<syn::Attribute>, Binding),
	BindNow  (Vec<syn::Attribute>, Binding),
	Binding {
		  attrs: Vec<syn::Attribute>,
		   mut0: Option<syn::Token![mut]>,
		   name: syn::Ident,
		closure: syn::ExprClosure,
		 clones: Punctuated<common::Clone, syn::Token![,]>,
	},
}

impl ParseReactive for Content {
	fn parse(input: syn::parse::ParseStream, reactive: bool) -> syn::Result<Self> {
		if input.peek(syn::Token![..]) {
			input.parse::<syn::Token![..]>()?;
			return Ok(Content::Built(common::content(input, reactive)?))
		}
		
		let ahead = input.fork();
		ahead.call(syn::Attribute::parse_outer)?;
		
		if let Ok(keyword) = ahead.parse::<syn::Lifetime>() {
			match keyword.ident.to_string().as_str() {
				"use" => return Ok(Content::Component(component::parse(input, reactive)?)),
				"back" => return Ok(Content::Back(input.parse()?)),
				"bind" | "bind_only" | "bind_now" | "binding" => if !reactive {
					return Err(input.error(format!("cannot use {keyword} here")))
				}
				_ => return Err(input.error(format!("unknown keyword {keyword}")))
			}
		} else if ahead.peek(syn::Token![if]) || ahead.peek(syn::Token![match]) {
			return Ok(Content::Inner(conditional::Inner::parse(input, false)?.into()))
		} else {
			let is_root = ahead.peek(syn::Token![mut])
				|| ahead.peek(syn::Token![move])
				|| ahead.parse::<common::Object>().is_ok()
				&& ahead.peek(syn::Ident)
				|| ahead.peek(syn::Lifetime)
				|| ahead.peek(syn::Token![!])
				&& ahead.peek2(syn::token::Brace)
				|| ahead.peek(syn::token::Brace);
			
			return Ok(if is_root
			     { Content::Component(component::parse(input, reactive)?) }
			else { Content::None(Prop::parse(input, reactive)?) })
		};
		
		let attrs = input.call(syn::Attribute::parse_outer)?;
		let keyword = input.parse::<syn::Lifetime>()?;
		
		match keyword.ident.to_string().as_str() {
			"bind" => Ok(Content::Bind {
				attrs,
				
				cond: if input.peek(syn::Token![if]) {
					input.parse::<syn::Token![if]>()?;
					Some(input.parse()?)
				} else { None },
				
				props: common::props(input, false)?,
			}),
			
			"bind_only" => Ok(Content::BindOnly(attrs, {
				if input.peek(syn::Token![if]) {
					Binding::If(conditional::parse_ifs(input, false)?)
				} else if input.peek(syn::Token![match]) {
					Binding::Match(conditional::Match::parse(input, false)?)
				} else {
					Binding::Props(common::props(input, false)?)
				}
			})),
			
			"bind_now" => Ok(Content::BindNow(attrs, {
				if input.peek(syn::Token![if]) {
					Binding::If(conditional::parse_ifs(input, false)?)
				} else if input.peek(syn::Token![match]) {
					Binding::Match(conditional::Match::parse(input, false)?)
				} else {
					Binding::Props(common::props(input, false)?)
				}
			})),
			
			"binding" => {
				let mut0 = input.parse()?;
				let name = input.parse()?;
				input.parse::<syn::Token![:]>()?;
				
				let clones = common::parse_clones(input)?;
				
				Ok(Content::Binding { attrs, mut0, name, clones, closure: input.parse()?})
			}
			
			_ => Err(input.error("expected 'bind, 'bind_only, 'bind_now or 'binding")),
		}
	}
}

pub(crate) fn expand(
	 content: Content,
	 objects: &mut TokenStream,
	builders: &mut Vec<TokenStream>,
	settings: &mut TokenStream,
	bindings: &mut TokenStream,
	  pattrs: &[syn::Attribute],
	    name: &[&syn::Ident],
	 builder: Option<usize>,
) {
	match content {
		Content::Back(token) => objects.extend(
			syn::Error::new_spanned(token, "cannot use 'back").into_compile_error()
		),
		Content::None(prop) => expand_value(
			prop, objects, builders, settings, bindings, &pattrs, name, builder
		),
		Content::Built(keywords) => for keyword in keywords {
			expand(keyword, objects, builders, settings, bindings, pattrs, name, None);
		}
		Content::Component(component) => component::expand(
			component, objects, builders, settings, bindings, pattrs, Some(name)
		),
		Content::Inner(inner) => {
			conditional::expand_inner(*inner, objects, builders, settings, bindings, None, name);
		}
		Content::Bind { attrs, cond, props } => {
			let stream: TokenStream = props.into_iter()
				.filter_map(|prop| expand_expr(
					prop, objects, builders, settings, bindings, &[], name, false
				))
				.collect();
			
			if let Some(expr) = cond {
				bindings.extend(quote![#(#pattrs)* #(#attrs)* if #expr { #stream }]);
			} else { bindings.extend(quote![#(#pattrs)* #(#attrs)* { #stream }]) }
			
			settings.extend(quote![#(#pattrs)* #(#attrs)* { #stream }]);
		}
		Content::BindOnly(attrs, bind) => match bind {
			Binding::If(ifs) => {
				bindings.extend(quote![#(#pattrs)* #(#attrs)*]);
				ifs.into_iter().for_each(|if0| conditional::expand_if(
					if0, objects, builders, settings, bindings, name, false
				))
			}
			Binding::Match(mat) => {
				bindings.extend(quote![#(#pattrs)* #(#attrs)*]);
				conditional::expand_match(mat, objects, builders, settings, bindings, name, false);
			}
			Binding::Props(props) => {
				let stream: TokenStream = props.into_iter()
					.filter_map(|prop| expand_expr(
						prop, objects, builders, settings, bindings, &[], name, false
					))
					.collect();
				
				bindings.extend(quote![#(#pattrs)* #(#attrs)* { #stream }])
			}
		}
		Content::BindNow(attrs, bind) => match bind {
			Binding::If(ifs) => {
				settings.extend(quote![#(#pattrs)* #(#attrs)*]);
				bindings.extend(quote![#(#pattrs)* #(#attrs)*]);
				ifs.into_iter().for_each(|if0| conditional::expand_if(
					if0, objects, builders, settings, bindings, name, true
				))
			}
			Binding::Match(mat) => {
				bindings.extend(quote![#(#pattrs)* #(#attrs)*]);
				conditional::expand_match(mat, objects, builders, settings, bindings, name, true);
			}
			Binding::Props(props) => {
				let stream: TokenStream = props.into_iter()
					.filter_map(|prop| expand_expr(
						prop, objects, builders, settings, bindings, &[], name, false
					))
					.collect();
				
				bindings.extend(quote![#(#pattrs)* #(#attrs)* { #stream }])
			}
		}
		Content::Binding { attrs, mut0, name, mut closure, clones } => {
			let clones = clones.into_iter();
			let stream = std::mem::take(bindings);
			
			Visit { name: "bindings", stream }.visit_expr_closure_mut(&mut closure);
			settings.extend(quote![#(#pattrs)* #(#attrs)* let #mut0 #name = { #(let #clones;)* #closure };])
		}
	}
}
