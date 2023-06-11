/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::{Delimiter, Group, TokenStream};
use quote::quote;
use syn::punctuated::Punctuated;
use crate::{item, property, Builder};

pub enum Content {
	     Bind (Box<Bind>),
	BindColon (Box<BindColon>),
	  Binding (Box<Binding>),
	    Built (Box<Built>),
	     Edit (Box<property::Edit>),
	Extension (Box<Extension>),
	       If (Box<(Vec<syn::Attribute>, Vec<If>)>),
	     Item (Box<item::Item>),
	    Match (Box<Match>),
	 Property (Box<property::Property>),
}

impl syn::parse::Parse for Content {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		with_attrs(input, input.call(syn::Attribute::parse_outer)?)
	}
}

pub struct Bind {
	token: syn::Lifetime,
	 init: Option<syn::Token![@]>,
	 mode: BindMode,
}

pub enum BindMode {
	Unbraced(Content), Braced {
		attrs: Vec<syn::Attribute>,
		brace: syn::token::Brace,
		 body: Vec<Content>,
	}
}

pub struct BindColon {
	attrs: Vec<syn::Attribute>,
	token: syn::Lifetime,
	  if_: syn::Token![if],
	 cond: syn::Expr,
	brace: syn::token::Brace,
	 body: Vec<Content>,
}

pub struct Binding {
	attrs: Vec<syn::Attribute>,
	   at: syn::Token![@],
	 mut_: Option<syn::Token![mut]>,
	 name: syn::Ident,
	equal: syn::Token![=],
	 expr: syn::Expr,
}

pub struct Built {
	object: bool,
	 tilde: syn::Token![~],
	  last: Option<syn::Token![~]>,
	penult: Content,
	  rest: Vec<Content>,
}

pub struct Extension {
	 attrs: Vec<syn::Attribute>,
	   ext: syn::TypePath,
	 paren: syn::token::Paren,
	tokens: syn::buffer::TokenBuffer,
	  back: Option<Box<property::Back>>,
}

pub struct If {
	else_: Option<syn::Token![else]>,
	  if_: Option<syn::Token![if]>,
	 expr: Option<Box<syn::Expr>>,
	brace: syn::token::Brace,
	 body: Vec<Content>,
}

impl syn::parse::Parse for If {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		let else_ = input.parse()?;
		let if_: Option<_> = input.parse()?;
		let expr = if if_.is_some() {
			Some(Box::new(input.call(syn::Expr::parse_without_eager_brace)?))
		} else { None };
		
		let (brace, body) = parse_vec(input)?;
		Ok(If { else_, if_, expr, brace, body })
	}
}

pub struct Match {
	attrs: Vec<syn::Attribute>,
	token: syn::Token![match],
	 expr: syn::Expr,
	brace: syn::token::Brace,
	 arms: Vec<Arm>,
}

pub struct Arm {
	attrs: Vec<syn::Attribute>,
	  pat: syn::Pat,
	guard: Option<Box<(syn::Token![if], syn::Expr)>>,
	arrow: syn::Token![=>],
	brace: Option<syn::token::Brace>,
	 body: Vec<Content>,
}

impl syn::parse::Parse for Arm {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		let attrs = input.call(syn::Attribute::parse_outer)?;
		let pat = input.call(syn::Pat::parse_multi_with_leading_vert)?;
		
		let guard = if let Ok(if_) = input.parse::<syn::Token![if]>() {
			Some(Box::new((if_, input.parse()?)))
		} else { None };
		
		let arrow = input.parse()?;
		
		let (brace, body) = if let Ok((brace, body)) = parse_vec(input) {
			(Some(brace), body)
		} else { (None, vec![input.parse()?]) };
		
		Ok(Arm { attrs, pat, guard, arrow, brace, body })
	}
}

fn with_attrs(input: syn::parse::ParseStream, attrs: Vec<syn::Attribute>) -> syn::Result<Content> {
	if let Ok(tilde) = input.parse::<syn::Token![~]>() {
		let   last = input.parse()?;
		let object = input.parse::<Option<syn::Token![/]>>()?.is_some();
		let penult = next(input, attrs)?;
		let mut rest = vec![]; while !input.is_empty() { rest.push(input.parse()?) }
		
		Ok(Content::Built(Box::new(
			Built { object, tilde, last, penult, rest }
		)))
	} else if let Ok(token) = input.parse::<syn::Lifetime>() {
		if token.ident == "bind" {
			if input.parse::<syn::Token![:]>().is_ok() {
				let (if_, cond) = (input.parse::<syn::Token![if]>()?, input.parse()?);
				let (brace, body) = parse_vec(input)?;
				
				Ok(Content::BindColon(Box::new(
					BindColon { attrs, token, if_, cond, brace, body }
				)))
			} else {
				let init = input.parse()?;
				
				if input.peek(syn::token::Brace) {
					let (brace, body) = parse_vec(input)?;
					
					Ok(Content::Bind(Box::new(Bind {
						token, init, mode: BindMode::Braced { attrs, brace, body }
					})))
				} else {
					Ok(Content::Bind(Box::new(Bind {
						token, init, mode: BindMode::Unbraced(with_attrs(input, attrs)?)
					})))
				}
			}
		} else { Err(syn::Error::new(
			token.span(), format!("expected 'bind (or maybe 'back), found {token}")
		)) }
	} else if let Ok(at) = input.parse::<syn::Token![@]>() {
		if input.peek2(syn::Token![=]) {
			let  mut_ = input.parse()?;
			let  name = input.parse()?;
			let equal = input.parse()?;
			let  expr = input.parse()?;
			let     _ = input.parse::<syn::Token![;]>();
			Ok(Content::Binding(Box::new(Binding { attrs, at, mut_, name, equal, expr })))
		} else {
			let ext = input.parse()?;
			let parens;
			let paren = syn::parenthesized!(parens in input);
			let tokens = syn::buffer::TokenBuffer::new2(parens.parse()?);
			let back = property::parse_back(input)?;
			Ok(Content::Extension(Box::new(Extension { attrs, ext, paren, tokens, back })))
		}
	} else if input.peek(syn::Token![if]) {
		let mut vec = vec![input.parse()?];
		while input.peek(syn::Token![else]) { vec.push(input.parse()?) }
		Ok(Content::If(Box::new((attrs, vec))))
	} else if input.peek(syn::Token![match]) {
		let token = input.parse()?;
		let expr = input.call(syn::Expr::parse_without_eager_brace)?;
		let braces;
		let brace = syn::braced!(braces in input);
		let mut arms = vec![]; while !braces.is_empty() { arms.push(braces.parse()?) }
		Ok(Content::Match(Box::new(Match { attrs, token, expr, brace, arms })))
	} else { Ok(next(input, attrs)?) }
}

fn next(input: &syn::parse::ParseBuffer, attrs: Vec<syn::Attribute>) -> syn::Result<Content> {
	if input.parse::<syn::Token![ref]>().is_ok() { return Ok(
		Content::Item(Box::new(item::parse(input, attrs, None, false)?))
	) }
	
	let path = match input.parse()? {
		crate::Path::Type(mut path) if path.path.get_ident().is_some() && input.peek(syn::Token![=>]) => {
			let punctuated = Punctuated::from_iter([path.path.segments.pop().unwrap().into_value().ident]);
			return Ok(Content::Edit(property::parse_edit(input, attrs, punctuated)?))
		}
		crate::Path::Field { access, gens } if gens.is_none() && input.peek(syn::Token![=>]) => {
			return Ok(Content::Edit(property::parse_edit(input, attrs, access)?))
		}
		path => path
	};
	
	Ok(if input.peek(syn::Token![:])
	   || input.peek(syn::Token![;])
	   || input.peek(syn::Token![=])
	   || input.peek(syn::Token![&]) { Content::Property(property::parse(input, attrs, path)?) }
	else { Content::Item(Box::new(item::parse(input, attrs, Some(path), false)?)) })
}

pub fn parse_vec(input: syn::parse::ParseStream) -> syn::Result<(syn::token::Brace, Vec<Content>)> {
	let braces;
	let (brace, mut content) = (syn::braced!(braces in input), vec![]);
	while !braces.is_empty() { content.push(braces.parse()?) }
	Ok((brace, content))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expand(
	 content: Content,
	 objects: &mut TokenStream,
	builders: &mut Vec<Builder>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	  fields: &mut Option<&mut Punctuated<syn::Field, syn::Token![,]>>,
	  pattrs: crate::Attributes<&[syn::Attribute]>,
	assignee: crate::Assignee,
	 builder: Option<usize>,
) -> syn::Result<()> {
	match content {
		Content::Bind(bind) => {
			let Bind { token, init, mode } = *bind;
			bindings.spans.push(token.span());
			
			match mode {
				BindMode::Braced { attrs, brace, body } => {
					let mut stream = TokenStream::new();
					
					for content in body { expand(
						content, objects, builders, &mut stream, bindings,
						&mut None, crate::Attributes::Some(&[]), assignee, None
					)? }
					
					let mut body = Group::new(Delimiter::Brace, stream);
					body.set_span(brace.span.join());
					
					let pattrs = pattrs.get(fields);
					let body = quote![#(#pattrs)* #(#attrs)* #body];
					if init.is_some() { settings.extend(body.clone()) }
					bindings.stream.extend(body)
				}
				BindMode::Unbraced(content) => {
					let mut body = TokenStream::new();
					expand(
						content, objects, builders, &mut body, bindings,
						&mut None, crate::Attributes::Some(&[]), assignee, None
					)?;
					if init.is_some() { settings.extend(body.clone()) }
					bindings.stream.extend(body)
				}
			} Ok(())
		}
		Content::BindColon(bind_colon) => {
			let BindColon { attrs, token, if_, cond, brace, body } = *bind_colon;
			let mut stream = TokenStream::new();
			
			for content in body { expand(
				content, objects, builders, &mut stream, bindings,
				&mut None, crate::Attributes::Some(&[]), assignee, None
			)? }
			
			let mut body = Group::new(Delimiter::Brace, stream);
			body.set_span(brace.span.join());
			
			let pattrs = pattrs.get(fields);
			bindings.spans.push(token.span());
			bindings.stream.extend(quote![#(#pattrs)* #(#attrs)* #if_ #cond #body]);
			Ok(settings.extend(quote![#(#pattrs)* #(#attrs)* #body]))
		}
		Content::Binding(binding) => {
			let Binding { attrs, at, mut_, name, equal, mut expr } = *binding;
			crate::try_bind(at, bindings, &mut expr)?;
			
			let pattrs = pattrs.get(fields);
			let let_ = syn::Ident::new("let", at.span);
			
			Ok(settings.extend(quote![#(#pattrs)* #(#attrs)* #let_ #mut_ #name #equal #expr;]))
		}
		Content::Built(built) => {
			let Built { object, tilde, last, penult, rest } = *built;
			
			let Some(index) = builder else {
				Err(syn::Error::new(tilde.span, "only allowed in builder mode"))?
			};
			
			match &mut builders[index] {
				#[cfg(not(feature = "builder-mode"))]
				Builder::Builder(_, span) => *span = tilde.span,
				
				#[cfg(feature = "builder-mode")]
				Builder::Builder { span, tilde: t, .. } |
				Builder::Struct { span, tilde: t, .. } => (*span, *t) = (tilde.span, last),
				
				#[cfg(not(feature = "builder-mode"))]
				Builder::Struct { call, .. } => *call = last.is_none().then(TokenStream::new)
			}
			
			expand(penult, objects, builders, settings, bindings, fields, pattrs, assignee, builder)?;
			
			if object { builders.remove(index).extend_into(objects) }
			
			for content in rest { expand(
				content, objects, builders, settings, bindings, fields, pattrs, assignee, None
			)? } Ok(())
		}
		Content::Edit(edit) => property::expand_edit(
			*edit, objects, builders, settings, bindings, fields, pattrs, assignee
		),
		Content::Extension(extension) => {
			let Extension { mut attrs, ext, paren, tokens, back } = *extension;
			let mut stream = TokenStream::new();
			
			if crate::find_pound(&mut tokens.begin(), &mut stream, assignee) {
				let pattrs = pattrs.get(fields);
				let mut group = Group::new(Delimiter::Parenthesis, stream);
				group.set_span(paren.span.join());
				
				if let Some(back) = back {
					crate::extend_attributes(&mut attrs, pattrs);
					property::expand_back(
						*back, objects, builders, settings, bindings, fields,
						crate::Attributes::Some(attrs), quote![#ext #group]
					)
				} else { Ok(settings.extend(quote![#(#pattrs)* #(#attrs)* #ext #group;])) }
			} else { Err(syn::Error::new(tokens.begin().span(), "no single `#` found around here")) }
		}
		Content::If(if_) => {
			let (pattrs, (attrs, if_vec)) = (pattrs.get(fields), *if_);
			settings.extend(quote![#(#pattrs)* #(#attrs)*]);
			
			for If { else_, if_, expr, brace, body } in if_vec {
				let (mut objects, mut builders, mut setup, mut bindings) = Default::default();
				
				for content in body { expand(
					content, &mut objects, &mut builders, &mut setup, &mut bindings,
					&mut None, crate::Attributes::Some(&[]), assignee, None
				)? }
				
				crate::bindings_error(&mut setup, bindings.spans);
				
				for builder in builders.into_iter().rev() { builder.extend_into(&mut objects) }
				objects.extend(setup);
				
				let mut body = Group::new(Delimiter::Brace, objects);
				body.set_span(brace.span.join());
				
				settings.extend(quote![#else_ #if_ #expr #body])
			} Ok(())
		}
		Content::Item(item) => item::expand(
			*item, objects, builders, settings, bindings, fields, pattrs, assignee, builder
		),
		Content::Match(match_) => {
			let Match { attrs, token, expr, brace, arms } = *match_;
			let pattrs = pattrs.get(fields);
			
			let body: Result<_, syn::Error> = arms.into_iter()
				.map(|Arm { attrs, pat, guard, arrow, brace, body }| {
					let (if_, expr) = guard.as_deref().map(|(a, b)| (a, b)).unzip();
					let (mut objects, mut builders, mut setup, mut bindings) = Default::default();
					
					for content in body { expand(
						content, &mut objects, &mut builders, &mut setup, &mut bindings,
						&mut None, crate::Attributes::Some(&[]), assignee, None
					)? }
					
					crate::bindings_error(&mut setup, bindings.spans);
					
					for builder in builders.into_iter().rev() { builder.extend_into(&mut objects) }
					objects.extend(setup);
					
					let mut body = Group::new(Delimiter::Brace, objects);
					if let Some(brace) = brace { body.set_span(brace.span.join()); } // WARNING not always hygienic
					Ok(quote![#(#attrs)* #pat #if_ #expr #arrow #body])
				}).collect();
			
			let mut body = Group::new(Delimiter::Brace, body?); body.set_span(brace.span.join());
			Ok(settings.extend(quote![#(#pattrs)* #(#attrs)* #token #expr #body]))
		}
		Content::Property(prop) => property::expand(
			*prop, objects, builders, settings, bindings, fields, pattrs, assignee, builder
		)
	}
}
