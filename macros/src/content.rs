/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::TokenStream;
use quote::quote;
use syn::visit_mut::VisitMut;
use crate::{conditional as cond, item, property as prop, Visit};

pub(crate) enum Content {
	Prop  (Box<prop::Prop>),
	Built {
		  token: syn::Token![#],
		    end: Option<syn::Token![.]>,
		content: Vec<Content>,
	},
	
	Item  (Box<item::Item>),
	Inner (Box<cond::Inner<Content>>),
	Bind  (bool, Binding, Vec<syn::Attribute>),
	
	BindColon {
		attrs: Vec<syn::Attribute>,
		  if0: syn::Token![if],
		 cond: Box<syn::Expr>,
		props: Vec<cond::Inner<prop::Prop>>,
	},
	
	Binding (Box<Expr>),
}

pub(crate) enum Binding {
	If    (Vec<cond::If<prop::Prop>>),
	Match (Box<cond::Match<prop::Prop>>),
	Props (Vec<prop::Prop>),
}

pub(crate) struct Expr {
	attrs: Vec<syn::Attribute>,
	 mut0: Option<syn::Token![mut]>,
	 name: syn::Ident,
	equal: syn::Token![=],
	 expr: syn::Expr,
}

impl crate::ParseReactive for Content {
	fn parse(input: syn::parse::ParseStream,
	         attrs: Option<Vec<syn::Attribute>>,
	      reactive: bool,
	) -> syn::Result<Self> {
		if input.peek(syn::Token![#]) && !input.peek2(syn::token::Bracket) {
			let token = input.parse()?;
			let mut end = Some(input.parse::<syn::Token![.]>()?);
			if input.parse::<syn::Token![.]>().is_ok() { end = None; };
			let content = parse_vec(input, reactive)?;
			return Ok(Content::Built { token, end, content })
		}
		
		let attrs = attrs.map_or_else(|| input.call(syn::Attribute::parse_outer), Result::Ok)?;
		
		let keyword = if let Ok(keyword) = input.parse::<syn::Lifetime>() {
			match keyword.ident.to_string().as_str() {
				"bind" | "binding" => if reactive { keyword } else {
					Err(syn::Error::new(
						keyword.span(), format!("cannot use {keyword} here")
					))?
				}
				_ => return Err(syn::Error::new(
					keyword.span(), format!("unknown keyword {keyword}")
				))
			}
		} else if input.peek(syn::Token![if]) || input.peek(syn::Token![match]) {
			return Ok(Content::Inner(
				cond::Inner::parse(input, Some(attrs), false)?.into()
			))
		} else {
			return Ok({
				let ahead = input.fork();
				
				if ahead.parse::<syn::Path>().is_ok()
				&& ahead.peek(syn::Token![:])
				|| ahead.peek(syn::Token![;])
				|| ahead.peek(syn::Token![=])
				|| ahead.peek(syn::Token![&]) {
					Content::Prop(prop::Prop::parse(input, Some(attrs), reactive)?.into())
				} else {
					Content::Item(item::parse(input, attrs, reactive, false)?.into())
				}
			})
		};
		
		match keyword.ident.to_string().as_str() {
			"bind" => if input.parse::<syn::Token![:]>().is_ok() {
				Ok(Content::BindColon {
					attrs,
					if0: input.parse::<syn::Token![if]>()?,
					cond: input.parse()?,
					props: crate::parse_vec(input, false)?,
				})
			} else {
				Ok(Content::Bind(input.parse::<syn::Token![!]>().is_ok(), {
					if input.peek(syn::Token![if]) {
						Binding::If(cond::parse_ifs(input, false)?)
					} else if input.peek(syn::Token![match]) {
						Binding::Match(cond::Match::parse(input, None, false)?.into())
					} else {
						Binding::Props(crate::parse_vec(input, false)?)
					}
				}, attrs))
			}
			
			"binding" => Ok(Content::Binding(Expr {
				attrs,
				 mut0: input.parse()?,
				 name: input.parse()?,
				equal: input.parse()?,
				 expr: input.parse()?
			}.into())),
			
			_ => Err(input.error("expected 'bind, 'bind_only, 'bind_now or 'binding")),
		}
	}
}

pub(crate) fn parse_vec(
	   input: syn::parse::ParseStream,
	reactive: bool
) -> syn::Result<Vec<Content>> {
	let mut props = vec![];
	while !input.is_empty() {
		props.push(crate::ParseReactive::parse(input, None, reactive)?)
	}
	Ok(props)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expand(
	 content: Content,
	 objects: &mut TokenStream,
	builders: &mut Vec<crate::Builder>,
	settings: &mut TokenStream,
	bindings: &mut TokenStream,
	  pattrs: &[syn::Attribute],
	    name: &[&syn::Ident],
	 builder: Option<usize>,
) {
	match content {
		Content::Prop(prop) => {
			let Some(expr) = prop::expand(
				*prop, objects, builders, settings, bindings, pattrs, name, builder
			) else { return };
			
			let Some(index) = builder else { return settings.extend(expr) };
			
			#[cfg(feature = "builder-mode")]
			builders[index].1.extend(expr);
			
			#[cfg(not(feature = "builder-mode"))]
			builders[index].extend(expr);
		}
		Content::Built { token, end: _end, content } => {
			let Some(_index) = builder else {
				let error = syn::Error::new(token.span, "only allowed in builder mode");
				return objects.extend(error.into_compile_error())
			};
			
			#[cfg(feature = "builder-mode")]
			{ builders[_index].2 = _end; }
			
			for content in content { expand(
				content, objects, builders, settings, bindings, pattrs, name, None
			) }
		}
		Content::Item(item) => item::expand(
			*item, objects, builders, settings, bindings, pattrs, Some(name), builder
		),
		Content::Inner(inner) => cond::expand(
			*inner, objects, builders, settings, bindings, None, name
		),
		Content::BindColon { attrs, if0, cond, props } => {
			let block = &mut TokenStream::new();
			
			for inner in props { cond::expand(
				inner, objects, builders, settings, bindings, Some(block), name
			) }
			
			bindings.extend(quote![#(#pattrs)* #(#attrs)* #if0 #cond { #block }]);
			settings.extend(quote![#(#pattrs)* #(#attrs)* { #block }]);
		}
		Content::Bind(now, bind, attrs) => match bind {
			Binding::If(ifs) => {
				if now { settings.extend(quote![#(#pattrs)* #(#attrs)*]); }
				bindings.extend(quote![#(#pattrs)* #(#attrs)*]);
				
				for cond::If { else0, if0, expr, inner } in ifs {
					let block = &mut TokenStream::new();
					
					for inner in inner { cond::expand(
						inner, objects, builders, settings, bindings, Some(block), name
					) }
					
					if now { settings.extend(quote![#else0 #if0 #expr { #block }]) };
					bindings.extend(quote![#else0 #if0 #expr { #block }])
				}
			}
			Binding::Match(match0) => {
				if now { settings.extend(quote![#(#pattrs)* #(#attrs)*]); }
				bindings.extend(quote![#(#pattrs)* #(#attrs)*]);
				
				let cond::Match { token, expr, arms } = *match0;
				
				let body = TokenStream::from_iter(arms.into_iter()
					.map(|cond::Arm { attrs, pat, guard, arrow, body }| {
						let block = &mut TokenStream::new();
						let (if0, expr) = guard.map(|boxed| *boxed).unzip();
						
						for inner in body { cond::expand(
							inner, objects, builders, settings, bindings, Some(block), name
						) }
						
						quote![#(#attrs)* #pat #if0 #expr #arrow { #block }]
					}));
				
				if now { settings.extend(quote![#token #expr { #body }]) };
				bindings.extend(quote![#token #expr { #body }])
			}
			Binding::Props(props) => {
				let stream: TokenStream = props.into_iter()
					.filter_map(|prop| prop::expand(
						prop, objects, builders, settings, bindings, &[], name, None
					))
					.collect();
				
				if now { settings.extend(quote![#(#pattrs)* #(#attrs)* { #stream }]); }
				bindings.extend(quote![#(#pattrs)* #(#attrs)* { #stream }])
			}
		}
		Content::Binding(expr) => {
			let Expr { attrs, mut0, name, equal, mut expr } = *expr;
			let stream = std::mem::take(bindings);
			
			Visit { name: "bindings", stream }.visit_expr_mut(&mut expr);
			
			settings.extend(quote![#(#pattrs)* #(#attrs)* let #mut0 #name #equal #expr;])
		}
	}
}
