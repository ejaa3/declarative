/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::{Delimiter, Group, TokenStream};
use quote::quote;
use syn::{punctuated::Punctuated, visit_mut::VisitMut};
use crate::{property, Construction};

pub enum Content {
	     Bind (Box<Bind>),
	BindColon (Box<BindColon>),
	Construct (Box<Construct>),
	  Consume (Box<Consume>),
	     Edit (Box<property::Edit>),
	       If (Box<(Vec<syn::Attribute>, Vec<If>)>),
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
	 init: Option<syn::Token![#]>,
	 mode: BindMode,
}

enum BindMode {
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

pub struct Construct {
	  object: bool,
	   tilde: syn::Token![~],
	    last: Option<syn::Token![~]>,
	  penult: Content,
	pub rest: Vec<Content>,
}

pub struct Consume {
	attrs: Vec<syn::Attribute>,
	token: syn::Lifetime,
	 mut_: Option<syn::Token![mut]>,
	 name: syn::Ident,
	equal: syn::Token![=],
	 expr: syn::Expr,
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

struct Arm {
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
		let     last = input.parse()?;
		let   object = input.parse::<Option<syn::Token![/]>>()?.is_some();
		let   penult = Content::Property(property::parse(input, attrs)?);
		let mut rest = vec![]; while !input.is_empty() { rest.push(input.parse()?) }
		
		Ok(Content::Construct(Box::new(Construct { object, tilde, last, penult, rest })))
	} else if let Ok(token) = input.parse::<syn::Lifetime>() {
		if token.ident == "bind" {
			if input.parse::<syn::Token![:]>().is_ok() {
				let (if_, cond) = (input.parse::<syn::Token![if]>()?, input.parse()?);
				let (brace, body) = parse_vec(input)?;
				
				Ok(Content::BindColon(Box::new(BindColon { attrs, token, if_, cond, brace, body })))
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
		} else if token.ident == "consume" {
			let  mut_ = input.parse()?;
			let  name = input.parse()?;
			let equal = input.parse()?;
			let  expr = input.parse()?;
			let     _ = input.parse::<syn::Token![;]>();
			Ok(Content::Consume(Box::new(Consume { attrs, token, mut_, name, equal, expr })))
		} else { Err(syn::Error::new(
			token.span(), format!("expected 'bind, 'consume or maybe 'back, found {token}")
		)) }
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
	} else if input.parse::<syn::Token![ref]>().is_ok() {
		Ok(Content::Edit(property::parse_edit(input, attrs)?))
	} else { Ok(Content::Property(property::parse(input, attrs)?)) }
}

pub fn parse_vec(input: syn::parse::ParseStream) -> syn::Result<(syn::token::Brace, Vec<Content>)> {
	let braces;
	let (brace, mut content) = (syn::braced!(braces in input), vec![]);
	while !braces.is_empty() { content.push(braces.parse()?) }
	Ok((brace, content))
}

fn scope(
	content: impl IntoIterator<Item = Content>, attrs: &[syn::Attribute], assignee: crate::Assignee
) -> syn::Result<TokenStream> {
	let (mut objects, mut constrs, mut settings, mut bindings) = Default::default();
	
	for content in content { expand(
		content, &mut objects, &mut constrs, &mut settings, &mut bindings,
		&mut None, crate::Attributes::Some(attrs), assignee, None
	)? }
	
	crate::bindings_error(&mut settings, bindings.spans);
	
	for constr in constrs.into_iter().rev() { constr.extend_into(&mut objects) }
	objects.extend(settings);
	Ok(objects)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expand(
	 content: Content,
	 objects: &mut TokenStream,
	 constrs: &mut Vec<Construction>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	  fields: &mut Option<&mut Punctuated<syn::Field, syn::Token![,]>>,
	  pattrs: crate::Attributes<&[syn::Attribute]>,
	assignee: crate::Assignee,
	  constr: Option<usize>,
) -> syn::Result<()> {
	match content {
		Content::Bind(bind) => {
			let Bind { token, init, mode } = *bind;
			bindings.spans.push(token.span());
			
			match mode {
				BindMode::Braced { attrs, brace, body } => {
					let mut body = Group::new(Delimiter::Brace, scope(body, &[], assignee)?);
					body.set_span(brace.span.join());
					
					let pattrs = pattrs.get(fields);
					let body = quote![#(#pattrs)* #(#attrs)* #body];
					if init.is_some() { settings.extend(body.clone()) }
					bindings.stream.extend(body)
				}
				BindMode::Unbraced(content) => {
					let scope = scope([content], pattrs.get(fields), assignee)?;
					if init.is_some() { settings.extend(scope.clone()) }
					bindings.stream.extend(scope)
				}
			} Ok(())
		}
		Content::BindColon(bind_colon) => {
			let BindColon { attrs, token, if_, cond, brace, body } = *bind_colon;
			let mut body = Group::new(Delimiter::Brace, scope(body, &[], assignee)?);
			body.set_span(brace.span.join());
			
			let pattrs = pattrs.get(fields);
			bindings.spans.push(token.span());
			bindings.stream.extend(quote![#(#pattrs)* #(#attrs)* #if_ #cond #body]);
			Ok(settings.extend(quote![#(#pattrs)* #(#attrs)* #body]))
		}
		Content::Construct(construct) => {
			let Construct { object, tilde, last, penult, rest } = *construct;
			
			let Some(index) = constr
			else { Err(syn::Error::new(tilde.span, crate::ConstrError("only allowed once")))? };
			
			match &mut constrs[index] {
				Construction::BuilderPattern { span, tilde: t, .. } |
				Construction::StructLiteral  { span, tilde: t, .. } => (*span, *t) = (tilde.span, last)
			}
			
			expand(penult, objects, constrs, settings, bindings, fields, pattrs, assignee, constr)?;
			
			if object { constrs.remove(index).extend_into(objects) }
			
			for content in rest { expand(
				content, objects, constrs, settings, bindings, fields, pattrs, assignee, None
			)? } Ok(())
		}
		Content::Consume(consume) => {
			let Consume { attrs, token, mut_, name, equal, mut expr } = *consume;
			
			if bindings.spans.is_empty() {
				Err(syn::Error::new(token.span(), "there are no bindings to consume \
					or you are trying from an inner binding or conditional scope"))?
			}
			
			let mut visitor = crate::Visitor::Ok {
				items: None, assignee: &mut None, placeholder: "bindings", stream: &mut bindings.stream
			};
			visitor.visit_expr_mut(&mut expr);
			
			if visitor.stream_is_empty()? { bindings.spans.clear(); }
			else { return Err(syn::Error::new_spanned(expr, crate::BINDINGS_ERROR)) }
			
			let pattrs = pattrs.get(fields);
			let let_ = syn::Ident::new("let", token.span());
			
			Ok(settings.extend(quote![#(#pattrs)* #(#attrs)* #let_ #mut_ #name #equal #expr;]))
		}
		Content::Edit(edit) => property::expand_edit(
			*edit, objects, constrs, settings, bindings, fields, pattrs, assignee
		),
		Content::If(if_) => {
			let (pattrs, (attrs, if_vec)) = (pattrs.get(fields), *if_);
			settings.extend(quote![#(#pattrs)* #(#attrs)*]);
			
			for If { else_, if_, expr, brace, body } in if_vec {
				let mut body = Group::new(Delimiter::Brace, scope(body, &[], assignee)?);
				body.set_span(brace.span.join());
				settings.extend(quote![#else_ #if_ #expr #body])
			} Ok(())
		}
		Content::Match(match_) => {
			let Match { attrs, token, expr, brace, arms } = *match_;
			let pattrs = pattrs.get(fields);
			
			let body: syn::Result<_> = arms.into_iter()
				.map(|Arm { attrs, pat, guard, arrow, brace, body }| {
					let (if_, expr) = guard.as_deref().map(|(a, b)| (a, b)).unzip();
					let mut body = Group::new(Delimiter::Brace, scope(body, &[], assignee)?);
					if let Some(brace) = brace { body.set_span(brace.span.join()); } // WARNING not always hygienic
					Ok(quote![#(#attrs)* #pat #if_ #expr #arrow #body])
				}).collect();
			
			let mut body = Group::new(Delimiter::Brace, body?); body.set_span(brace.span.join());
			Ok(settings.extend(quote![#(#pattrs)* #(#attrs)* #token #expr #body]))
		}
		Content::Property(prop) => property::expand(
			*prop, objects, constrs, settings, bindings, fields, pattrs, assignee, constr
		)
	}
}
