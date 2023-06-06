/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::{Delimiter, Group, Punct, Spacing, TokenStream};
use quote::{TokenStreamExt, ToTokens, quote};
use crate::{conditional, item, property, Builder};

pub enum Content {
	     Edit (Box<property::Edit>),
	 Property (Box<property::Prop>),
	Extension (Box<Extension>),
	    Built (Box<Built>),
	     Item (Box<item::Item>),
	    Inner (Box<conditional::Inner<Content>>),
	     Bind (Box<Bind>),
	BindColon (Box<BindColon>),
	  Binding (Box<Binding>),
}

pub struct Extension {
	 attrs: Vec<syn::Attribute>,
	   ext: syn::TypePath,
	 paren: syn::token::Paren,
	tokens: syn::buffer::TokenBuffer,
	  back: Option<Box<property::Back>>,
}

pub struct Built {
	 object: bool,
	  tilde: syn::Token![~],
	   last: Option<syn::Token![~]>,
	 penult: Content,
	content: Vec<Content>,
}

pub struct Bind {
	token: syn::Lifetime,
	 init: Option<syn::Token![@]>,
	 mode: BindMode,
}

pub enum BindMode {
	Braced {
		  attrs: Vec<syn::Attribute>,
		  brace: syn::token::Brace,
		content: Vec<conditional::Inner<Box<property::Prop>>>
	},
	Unbraced (conditional::Inner<Box<property::Prop>>)
}

pub struct BindColon {
	attrs: Vec<syn::Attribute>,
	token: syn::Lifetime,
	  if_: syn::Token![if],
	 cond: syn::Expr,
	brace: syn::token::Brace,
	props: Vec<conditional::Inner<Box<property::Prop>>>,
}

pub struct Binding {
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
			let last = input.parse()?;
			let object = input.parse::<Option<syn::Token![/]>>()?.is_some();
			let penult = next(input, None, reactive)?;
			let mut content = vec![]; parse_vec(&mut content, input, reactive)?;
			
			return Ok(Content::Built(Box::new(
				Built { object, tilde, last, penult, content }
			)))
		}
		
		let attrs = attrs.map_or_else(|| input.call(syn::Attribute::parse_outer), Result::Ok)?;
		
		if let Ok(token) = input.parse::<syn::Lifetime>() {
			if token.ident == "bind" {
				if !reactive { Err(syn::Error::new(token.span(), "cannot use 'bind here")) }
				else if input.parse::<syn::Token![:]>().is_ok() {
					let if_ = input.parse::<syn::Token![if]>()?;
					let cond = input.parse()?;
					let (brace, props) = crate::parse_vec(input, false)?;
					
					Ok(Content::BindColon(Box::new(BindColon {
						attrs, token, if_, cond, brace, props
					})))
				} else {
					let init = input.parse()?;
					
					Ok(if input.peek(syn::token::Brace) {
						let (brace, content) = crate::parse_vec(input, false)?;
						Content::Bind(Box::new(Bind {
							token, init, mode: BindMode::Braced { attrs, brace, content }
						}))
					} else {
						Content::Bind(Box::new(Bind {
							token, init, mode: BindMode::Unbraced(
								crate::ParseReactive::parse(input, Some(attrs), false)?
							)
						}))
					})
				}
			} else {
				Err(syn::Error::new(
					token.span(), format!("expected 'bind (or maybe 'back), found {token}")
				))
			}
		} else if input.peek(syn::Token![if]) || input.peek(syn::Token![match]) {
			Ok(Content::Inner(Box::new(
				conditional::Inner::parse(input, Some(attrs), false)?
			)))
		} else if let Ok(at) = input.parse::<syn::Token![@]>() {
			if input.peek2(syn::Token![=]) {
				if !reactive {
					Err(syn::Error::new(at.span, "cannot consume bindings here"))?
				}
				Ok(Content::Binding(Box::new(Binding {
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
				let back = property::parse_back(input, reactive)?;
				Ok(Content::Extension(Box::new(Extension { attrs, ext, paren, tokens, back })))
			}
		} else { Ok(next(input, Some(attrs), reactive)?) }
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

fn next(input: &syn::parse::ParseBuffer,
        attrs: Option<Vec<syn::Attribute>>,
     reactive: bool,
) -> syn::Result<Content> {
	if input.peek(syn::Token![ref]) {
		return Ok(Content::Item(Box::new(
			item::parse(input, attrs.unwrap_or_default(), reactive, false)?
		)))
	}
	
	let ahead = input.fork();
	let path = ahead.parse::<crate::Path>()?;
	
	let edit = match path {
		crate::Path::Type(path) => path.path.get_ident().is_some(),
		crate::Path::Field { gens, .. } => gens.is_none(),
	};
	
	Ok(if edit && ahead.peek(syn::Token![=>]) {
		Content::Edit(property::parse_edit(input, attrs.unwrap_or_default(), reactive)?)
	} else if ahead.peek(syn::Token![:])
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
	builders: &mut Vec<Builder>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	  pattrs: &[syn::Attribute],
	assignee: crate::Assignee,
	 builder: Option<usize>,
) {
	match content {
		Content::Edit(edit) => property::expand_edit(
			*edit, objects, builders, settings, bindings, pattrs, assignee
		),
		Content::Property(prop) => property::expand(
			*prop, objects, builders, settings, bindings, pattrs, assignee, builder
		),
		Content::Extension(extension) => {
			let Extension { mut attrs, ext, paren, tokens, back } = *extension;
			let mut stream = TokenStream::new();
			
			if crate::find_pound(&mut tokens.begin(), &mut stream, assignee) {
				let mut group = Group::new(Delimiter::Parenthesis, stream);
				group.set_span(paren.span.join());
				
				if let Some(back) = back {
					crate::extend_attributes(&mut attrs, pattrs);
					property::expand_back(
						*back, objects, builders, settings, bindings, attrs, quote![#ext #group]
					)
				} else { settings.extend(quote![#(#pattrs)* #(#attrs)* #ext #group;]) }
			} else {
				objects.extend(syn::Error::new(
					tokens.begin().span(), "no single `#` found around here"
				).into_compile_error())
			}
		}
		Content::Built(built) => {
			let Built { object, tilde, last, penult, content } = *built;
			
			let Some(index) = builder else {
				let error = syn::Error::new(tilde.span, "only allowed in builder or struct mode");
				return objects.extend(error.into_compile_error())
			};
			
			match &mut builders[index] {
				#[cfg(not(feature = "builder-mode"))]
				Builder::Builder(_) => { }
				
				#[cfg(feature = "builder-mode")]
				Builder::Builder { span, tilde: t, .. } |
				Builder::Struct { span, tilde: t, .. } => (*span, *t) = (tilde.span, last),
				
				#[cfg(not(feature = "builder-mode"))]
				Builder::Struct { call, .. } => *call = last.is_none().then(TokenStream::new)
			}
			
			expand(penult, objects, builders, settings, bindings, pattrs, assignee, builder);
			
			if object {
				builders.remove(index).to_tokens(objects);
				objects.append(Punct::new(';', Spacing::Alone));
			}
			
			for content in content { expand(
				content, objects, builders, settings, bindings, pattrs, assignee, None
			) }
		}
		Content::Item(item) => item::expand(
			*item, objects, builders, settings, bindings, pattrs, assignee, builder
		),
		Content::Inner(inner) => conditional::expand(
			*inner, settings, bindings, pattrs, assignee, None
		),
		Content::BindColon(bind_colon) => {
			let BindColon { attrs, token, if_, cond, brace, props } = *bind_colon;
			let mut block = TokenStream::new();
			
			for inner in props { conditional::expand(
				inner, &mut block, bindings, &[], assignee, None
			) }
			
			let mut body = Group::new(Delimiter::Brace, block);
			body.set_span(brace.span.join());
			
			bindings.spans.push(token.span());
			bindings.stream.extend(quote![#(#pattrs)* #(#attrs)* #if_ #cond #body]);
			settings.extend(quote![#(#pattrs)* #(#attrs)* #body]);
		}
		Content::Bind(bind) => {
			let Bind { token, init, mode } = *bind;
			bindings.spans.push(token.span());
			
			match mode {
				BindMode::Braced { mut attrs, brace: _, content } => { // WARNING unused brace
					crate::extend_attributes(&mut attrs, pattrs);
					
					for inner in content { conditional::expand(
						inner, settings, bindings, &attrs, assignee, Some(init.is_some())
					) }
				}
				BindMode::Unbraced(inner) => conditional::expand(
					inner, settings, bindings, pattrs, assignee, Some(init.is_some())
				),
			}
		}
		Content::Binding(binding) => {
			let Binding { attrs, at, mut_, name, equal, mut expr } = *binding;
			crate::try_bind(at, objects, bindings, &mut expr);
			let let_ = syn::Ident::new("let", at.span);
			
			settings.extend(quote![#(#pattrs)* #(#attrs)* #let_ #mut_ #name #equal #expr;])
		}
	}
}
