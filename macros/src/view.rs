/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use std::{collections::VecDeque, mem::take};
use proc_macro2::{Delimiter, Group, Span, TokenStream};
use quote::{TokenStreamExt, quote};
use syn::{parse::{Parse, ParseStream}, visit_mut::VisitMut};
use crate::{item, Assignee, Bindings, Builder, Path};

pub struct Roots(Vec<item::Item>);

impl Parse for Roots {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut props = vec![];
		while !input.is_empty() {
			let attrs = input.call(syn::Attribute::parse_outer)?;
			props.push(item::parse(input, attrs, true, true)?)
		}
		Ok(Self(props))
	}
}

pub(crate) fn expand(Roots(roots): Roots) -> (TokenStream, Bindings) {
	let objects  = &mut TokenStream::new();
	let builders = &mut vec![];
	let settings = &mut TokenStream::new();
	let mut bindings = Bindings::default();
	
	for root in roots { item::expand(
		root, objects, builders, settings, &mut bindings, &[], Assignee::None, None
	) }
	
	let builders = builders.iter().rev();
	(quote![#objects #(#builders;)* #settings], bindings)
}

pub struct Visitor { pub(crate) deque: VecDeque<(TokenStream, Bindings)> }

macro_rules! item {
	($visit:ident: $item:ident) => {
		fn $visit(&mut self, node: &mut syn::$item) {
			if let syn::$item::Macro(mac) = node {
				if mac.mac.path.is_ident("view") {
					self.deque.push_back(mac.mac.parse_body().map(expand)
						.unwrap_or_else(|error| (
							error.into_compile_error(), Bindings::default()
						)));
					return *node = syn::$item::Verbatim(TokenStream::new())
				}
			}
			syn::visit_mut::$visit(self, node)
		}
	}
}

impl VisitMut for Visitor {
	item!(visit_foreign_item_mut: ForeignItem);
	item!(visit_impl_item_mut: ImplItem);
	item!(visit_item_mut: Item);
	item!(visit_trait_item_mut: TraitItem);
}

const ERROR: &str = "views must be consumed with the `expand_view_here!` placeholder macro";

pub fn bindings_error(stream: &mut TokenStream, spans: Vec<Span>) {
	for span in spans {
		stream.extend(syn::Error::new(span, crate::ERROR).to_compile_error());
	}
}

pub(crate) fn parse(file: &mut syn::File, output: &mut TokenStream, view: (TokenStream, Bindings)) {
	let (stream, bindings) = view;
	
	let mut visitor = crate::Visitor { placeholder: "expand_view_here", stream: Some(stream) };
	visitor.visit_file_mut(file);
	
	if let Some(stream) = visitor.stream {
		output.extend(syn::Error::new_spanned(stream, ERROR).into_compile_error())
	}
	
	if !bindings.spans.is_empty() {
		let stream = Some(bindings.stream);
		let mut visitor = crate::Visitor { placeholder: "bindings", stream };
		visitor.visit_file_mut(file);
		
		if visitor.stream.is_some() { bindings_error(output, bindings.spans) }
	}
}

impl<'a> crate::Assignee<'a> {
	pub fn spanned_to(&'a self, span: Span) -> Box<dyn Iterator<Item = syn::Ident> + 'a> {
		match self {
			Self::Field(field) => Box::new(field.iter().map(move |ident| {
				let mut ident = ident.clone();
				ident.set_span(span);
				ident
			})),
			
			Self::Ident(ident) => Box::new(std::iter::once({
				let mut ident = (*ident).clone();
				ident.set_span(span);
				ident
			})),
			
			Self::None => unreachable!(),
		}
	}
}

impl quote::ToTokens for crate::Assignee<'_> {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		match self {
			Self::Field(field) => field.to_tokens(tokens),
			Self::Ident(ident) => ident.to_tokens(tokens),
			Self::None => unreachable!(),
		}
	}
}

impl quote::ToTokens for Builder {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		match self {
			#[cfg(not(feature = "builder-mode"))]
			Builder::Builder(stream) => stream.to_tokens(tokens),
			
			#[cfg(feature = "builder-mode")]
			Builder::Builder { left, right, span, tilde } => {
				left.to_tokens(tokens);
				tokens.extend(quote::quote_spanned! {
					*span => builder_mode!(#tilde #right)
				})
			}
			
			#[cfg(not(feature = "builder-mode"))]
			Builder::Struct { ty, fields, call } => {
				ty.to_tokens(tokens);
				tokens.append(Group::new(Delimiter::Brace, take(&mut fields.borrow_mut())));
				if let Some(call) = call { call.to_tokens(tokens); }
			}
			
			#[cfg(feature = "builder-mode")]
			Builder::Struct { left, ty, fields, span, tilde } => {
				left.to_tokens(tokens);
				let fields = Group::new(Delimiter::Brace, take(&mut fields.borrow_mut()));
				let  value = Group::new(Delimiter::Parenthesis, quote![#ty #fields]);
				
				tokens.extend(quote::quote_spanned! {
					*span => builder_mode!(#tilde #value)
				})
			}
		}
	}
}

impl crate::Path {
	pub fn is_long(&self) -> bool {
		let Self::Type(path) = self else { return false };
		path.path.segments.len() > 1 || path.qself.is_some()
	}
}

impl syn::parse::Parse for Path {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		if input.peek(syn::Ident) && input.peek2(syn::Token![.]) {
			let access = crate::parse_unterminated(input)?;
			let gens = syn::AngleBracketedGenericArguments::parse_turbofish(input).ok();
			Ok(Path::Field { access, gens })
		} else { Ok(Path::Type(input.parse()?)) }
	}
}

impl quote::ToTokens for Path {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		match self {
			Path::Type(path) => path.to_tokens(tokens),
			Path::Field { access, gens } => {
				access.to_tokens(tokens);
				gens.to_tokens(tokens);
			}
		}
	}
}

impl VisitMut for crate::Visitor {
	fn visit_expr_mut(&mut self, node: &mut syn::Expr) {
		if self.stream.is_none() { return }
		
		if let syn::Expr::Macro(mac) = node {
			if mac.mac.path.is_ident(self.placeholder) {
				let group = Group::new(Delimiter::Brace, self.stream.take().unwrap());
				let mut stream = TokenStream::new(); stream.append(group);
				return *node = syn::Expr::Verbatim(stream);
			}
		}
		syn::visit_mut::visit_expr_mut(self, node)
	}
	
	fn visit_stmt_mut(&mut self, node: &mut syn::Stmt) {
		if self.stream.is_none() { return }
		
		if let syn::Stmt::Macro(mac) = node {
			if mac.mac.path.is_ident(self.placeholder) {
				return *node = syn::Stmt::Expr(
					syn::Expr::Verbatim(self.stream.take().unwrap()), None
				);
			}
		}
		syn::visit_mut::visit_stmt_mut(self, node)
	}
}
