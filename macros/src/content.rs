/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::{Delimiter, Group, Punct, Spacing, TokenStream};
use quote::{TokenStreamExt, ToTokens, quote};
use crate::{conditional as cond, item, property as prop};

pub enum Content {
	 Property (Box<prop::Prop>),
	Extension (Box<Extension>),
	    Built { object: bool, tilde: syn::Token![~], built: Box<Built> },
	     Item (Box<item::Item>),
	    Inner (Box<cond::Inner<Content>>),
	     Bind (Box<Bind>),
	BindColon (Box<BindColon>),
	  Binding (Box<Expr>),
}

pub struct Extension {
	 attrs: Vec<syn::Attribute>,
	   ext: syn::TypePath,
	 paren: syn::token::Paren,
	tokens: syn::buffer::TokenBuffer,
	  back: Option<Box<prop::Back>>,
}

pub struct Built {
	no_auto: Option<syn::Token![~]>,
	 penult: Content,
	content: Vec<Content>,
}

pub struct Bind {
	  attrs: Vec<syn::Attribute>,
	  token: syn::Lifetime,
	   init: Option<syn::Token![@]>,
	binding: Binding,
}

enum Binding {
	   If (Vec<cond::If<prop::Prop>>),
	Match (Box<cond::Match<prop::Prop>>),
	Props (Vec<prop::Prop>),
}

pub struct BindColon {
	attrs: Vec<syn::Attribute>,
	token: syn::Lifetime,
	colon: syn::Token![:],
	  if_: syn::Token![if],
	 cond: syn::Expr,
	props: Vec<cond::Inner<Box<prop::Prop>>>,
}

pub struct Expr {
	attrs: Vec<syn::Attribute>,
	   at: syn::Token![@],
	 mut_: Option<syn::Token![mut]>,
	 name: syn::Ident,
	equal: syn::Token![=],
	 expr: syn::Expr,
}

impl crate::ParseReactive for Content {
	fn parse(input: syn::parse::ParseStream,
	         attrs: Option<Vec<syn::Attribute>>,
	      reactive: bool,
	) -> syn::Result<Self> {
		if let Ok(tilde) = input.parse::<syn::Token![~]>() {
			let no_auto = input.parse()?;
			let object = input.parse::<Option<syn::Token![/]>>()?.is_some();
			let penult = property_or_item(input, None, reactive)?;
			let mut content = vec![]; parse_vec(&mut content, input, reactive)?;
			
			return Ok(Content::Built {
				object, tilde, built: Box::new(Built { no_auto, penult, content })
			})
		}
		
		let attrs = attrs.map_or_else(|| input.call(syn::Attribute::parse_outer), Result::Ok)?;
		
		if let Ok(token) = input.parse::<syn::Lifetime>() {
			if token.ident == "bind" {
				if !reactive {
					Err(syn::Error::new(
						token.span(), format!("cannot use {token} here")
					))
				} else if let Ok(colon) = input.parse::<syn::Token![:]>() {
					Ok(Content::BindColon(Box::new(BindColon {
						attrs, token, colon,
						  if_: input.parse::<syn::Token![if]>()?,
						 cond: input.parse()?,
						props: crate::parse_vec(input, false)?,
					})))
				} else {
					Ok(Content::Bind(Box::new(Bind {
						attrs, token, init: input.parse()?,
						binding: if input.peek(syn::Token![if]) {
							Binding::If(cond::parse_vec(input, false)?)
						} else if input.peek(syn::Token![match]) {
							Binding::Match(crate::ParseReactive::parse(input, None, false)?)
						} else {
							Binding::Props(crate::parse_vec(input, false)?)
						}
					})))
				}
			} else {
				Err(syn::Error::new(
					token.span(), format!("expected 'bind (or maybe 'back), found {token}")
				))
			}
		} else if input.peek(syn::Token![if]) || input.peek(syn::Token![match]) {
			Ok(Content::Inner(
				Box::new(cond::Inner::parse(input, Some(attrs), false)?)
			))
		} else if let Ok(at) = input.parse::<syn::Token![@]>() {
			if input.peek2(syn::Token![=]) {
				if !reactive {
					Err(syn::Error::new(at.span, "cannot consume bindings here"))?
				}
				Ok(Content::Binding(Box::new(Expr {
					attrs, at,
					 mut_: input.parse()?,
					 name: input.parse()?,
					equal: input.parse()?,
					 expr: input.parse()?,
				})))
			} else {
				let ext = input.parse()?;
				let parens;
				let paren = syn::parenthesized!(parens in input);
				let tokens = syn::buffer::TokenBuffer::new2(parens.parse()?);
				let back = prop::parse_back(input, reactive)?;
				Ok(Content::Extension(Box::new(Extension { attrs, ext, paren, tokens, back })))
			}
		} else { Ok(property_or_item(input, Some(attrs), reactive)?) }
	}
}

pub fn parse_vec(
	 content: &mut Vec<Content>,
	   input: syn::parse::ParseStream,
	reactive: bool,
) -> syn::Result<()> {
	while !input.is_empty() {
		content.push(crate::ParseReactive::parse(input, None, reactive)?)
	} Ok(())
}

fn property_or_item(
	   input: &syn::parse::ParseBuffer,
	   attrs: Option<Vec<syn::Attribute>>,
	reactive: bool,
) -> syn::Result<Content> {
	let ahead = input.fork();
	
	Ok(if ahead.parse::<syn::TypePath>().is_ok()
	&& ahead.peek(syn::Token![:])
	|| ahead.peek(syn::Token![;])
	|| ahead.peek(syn::Token![=])
	|| ahead.peek(syn::Token![&]) {
		Content::Property(crate::ParseReactive::parse(input, attrs, reactive)?)
	} else {
		Content::Item(Box::new(item::parse(input, attrs.unwrap_or_default(), reactive, false)?))
	})
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expand(
	 content: Content,
	 objects: &mut TokenStream,
	builders: &mut Vec<crate::Builder>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	  pattrs: &[syn::Attribute],
	    name: &[&syn::Ident],
	 builder: Option<usize>,
) {
	match content {
		Content::Property(prop) => {
			let Some(expr) = prop::expand(
				*prop, objects, builders, settings, bindings, pattrs, name, builder
			) else { return };
			
			let Some(index) = builder else { return settings.extend(expr) };
			
			#[cfg(feature = "builder-mode")]
			builders[index].right.extend(expr);
			
			#[cfg(not(feature = "builder-mode"))]
			builders[index].extend(expr);
		}
		Content::Extension(extension) => {
			let Extension { mut attrs, ext, paren, tokens, back } = *extension;
			let mut stream = TokenStream::new();
			
			if crate::find_pound(&mut tokens.begin(), &mut stream, name) {
				let mut group = Group::new(Delimiter::Parenthesis, stream);
				group.set_span(paren.span.join());
				
				if let Some(back) = back {
					crate::extend_attributes(&mut attrs, pattrs);
					prop::expand_back(
						*back, objects, builders, settings, bindings, attrs, quote![#ext #group]
					)
				} else { settings.extend(quote![#(#pattrs)* #(#attrs)* #ext #group;]) }
			} else {
				objects.extend(syn::Error::new(
					tokens.begin().span(), "no single `#` found around here"
				).into_compile_error())
			}
		}
		Content::Built { object, tilde, built } => {
			let Built { no_auto: _no_auto, penult, content } = *built;
			expand(penult, objects, builders, settings, bindings, pattrs, name, builder);
			
			let Some(index) = builder else {
				let error = syn::Error::new(tilde.span, "only allowed in builder mode");
				return objects.extend(error.into_compile_error())
			};
			
			#[cfg(feature = "builder-mode")] {
				let builder = &mut builders[index];
				builder.span = tilde.span;
				builder.tilde = _no_auto;
			}
			
			if object {
				builders.remove(index).to_tokens(objects);
				objects.append(Punct::new(';', Spacing::Alone));
			}
			
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
		Content::BindColon(bind_colon) => {
			let BindColon { attrs, token, colon, if_, cond, props } = *bind_colon;
			let block = &mut TokenStream::new();
			
			for inner in props { cond::expand(
				inner, objects, builders, settings, bindings, Some(block), name
			) }
			
			bindings.tokens.push(quote![#token #colon]);
			bindings.stream.extend(quote![#(#pattrs)* #(#attrs)* #if_ #cond { #block }]);
			settings.extend(quote![#(#pattrs)* #(#attrs)* { #block }]);
		}
		Content::Bind(bind) => {
			let Bind { attrs, token, init, binding } = *bind;
			bindings.tokens.push(quote![#token #init]);
			let init = init.is_some();
			
			match binding {
				Binding::If(if_vec) => {
					if init { settings.extend(quote![#(#pattrs)* #(#attrs)*]); }
					bindings.stream.extend(quote![#(#pattrs)* #(#attrs)*]);
					
					for cond::If { else_, if_, expr, inner } in if_vec {
						let block = &mut TokenStream::new();
						
						for inner in inner { cond::expand(
							inner, objects, builders, settings, bindings, Some(block), name
						) }
						
						if init { settings.extend(quote![#else_ #if_ #expr { #block }]) };
						bindings.stream.extend(quote![#else_ #if_ #expr { #block }])
					}
				}
				Binding::Match(match_) => {
					if init { settings.extend(quote![#(#pattrs)* #(#attrs)*]); }
					bindings.stream.extend(quote![#(#pattrs)* #(#attrs)*]);
					
					let cond::Match { token, expr, arms } = *match_;
					
					let body = TokenStream::from_iter(arms.into_iter()
						.map(|cond::Arm { attrs, pat, guard, arrow, body }| {
							let block = &mut TokenStream::new();
							let (if_, expr) = guard.as_deref().map(|(a, b)| (a, b)).unzip();
							
							for inner in body { cond::expand(
								inner, objects, builders, settings, bindings, Some(block), name
							) }
							
							quote![#(#attrs)* #pat #if_ #expr #arrow { #block }]
						}));
					
					if init { settings.extend(quote![#token #expr { #body }]) };
					bindings.stream.extend(quote![#token #expr { #body }])
				}
				Binding::Props(props) => {
					let brace = props.len() > 1 && !(pattrs.is_empty() && attrs.is_empty());
					let stream: TokenStream = props.into_iter()
						.filter_map(|prop| prop::expand(
							prop, objects, builders, settings, bindings, &[], name, None
						))
						.collect();
					
					let delim = if brace { Delimiter::Brace } else { Delimiter::None };
					let group = Group::new(delim, stream);
					
					if init { settings.extend(quote![#(#pattrs)* #(#attrs)* #group]); }
					bindings.stream.extend(quote![#(#pattrs)* #(#attrs)* #group])
				}
			}
		}
		Content::Binding(binding) => {
			let Expr { attrs, at, mut_, name, equal, mut expr } = *binding;
			crate::try_bind(at, objects, bindings, &mut expr);
			let let_ = syn::Ident::new("let", at.span);
			
			settings.extend(quote![#(#pattrs)* #(#attrs)* #let_ #mut_ #name #equal #expr;])
		}
	}
}
