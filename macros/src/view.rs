/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::{Delimiter, Group, Punct, Spacing, Span, TokenStream};
use quote::TokenStreamExt;
use syn::{punctuated::Punctuated, visit_mut::VisitMut};
use crate::{item, Assignee, Attributes, Bindings};

pub enum Root { Struct(syn::ItemStruct), Item(item::Item) }

pub struct Roots(Vec<Root>);

impl syn::parse::Parse for Roots {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		let mut props = vec![];
		
		while !input.is_empty() {
			let attrs = input.call(syn::Attribute::parse_outer)?;
			
			if input.peek(syn::Token![pub]) || input.peek(syn::Token![struct]) {
				let mut item = input.parse::<syn::ItemStruct>()?;
				
				let syn::Fields::Named(_) = item.fields else { Err(
					syn::Error::new_spanned(item, "must be a struct with braces (named fields)")
				)? };
				
				item.attrs = attrs; props.push(Root::Struct(item))
			} else if input.parse::<syn::Token![ref]>().is_ok() {
				props.push(Root::Item(item::parse(input, attrs, None, true)?))
			} else { props.push(Root::Item(item::parse(input, attrs, Some(input.parse()?), true)?)) }
		}
		
		Ok(Self(props))
	}
}

pub(crate) fn expand(
	items: &mut Vec<syn::ItemStruct>, Roots(roots): Roots
) -> syn::Result<(TokenStream, Bindings)> {
	let (mut objects, mut builders, mut settings, mut bindings) = Default::default();
	let (mut final_struct, mut item_struct) = (false, None);
	
	macro_rules! check_struct {
		($($item:ident)?) => {
			if final_struct { return Err(
				syn::Error::new_spanned(item_struct, "structs must be followed by items")
			) }
			if let Some(item) = item_struct.take() { items.push(item) }
			$(item_struct = Some($item); final_struct = true)?
		}
	}
	
	for root in roots { match root {
		Root::Struct (item) => { check_struct!(item); }
		Root::Item   (item) => {
			final_struct = false;
			
			let fields = &mut if let Some(item) = item_struct.as_mut() {
				let syn::Fields::Named(named) = &mut item.fields else { panic!() };
				Some(&mut named.named)
			} else { None };
			
			item::expand(
				item, &mut objects, &mut builders, &mut settings, &mut bindings,
				fields, Attributes::Some(&[]), Assignee::None, None
			)?;
		}
	} }
	
	check_struct! { }
	for builder in builders.into_iter().rev() { builder.extend_into(&mut objects) }
	objects.extend(settings); Ok((objects, bindings))
}

pub(crate) enum Visitor {
	Error(syn::Error), Ok {
		items: Vec<syn::ItemStruct>,
		deque: std::collections::VecDeque<(crate::Spans, TokenStream, Bindings)>,
	}
}

macro_rules! item {
	($visit:ident: $item:ident) => {
		fn $visit(&mut self, node: &mut syn::$item) {
			let Self::Ok { items, deque } = self else { return };
			
			if let syn::$item::Macro(mac) = node {
				if mac.mac.path.is_ident("view") {
					let spans = crate::Spans::Range(
						mac.mac.path.segments[0].ident.span(),
						mac.mac.bang_token.span,
					);
					
					if mac.mac.tokens.is_empty() {
						return *self = Self::Error(spans.error("this view has no content"))
					}
					
					return match mac.mac.parse_body().map(|root| expand(items, root)) {
						Ok(expansion) => match expansion {
							Ok((stream, bindings)) => {
								deque.push_back((spans, stream, bindings));
								*node = syn::$item::Verbatim(TokenStream::new())
							}
							Err(error) => *self = Self::Error(error)
						}
						Err(error) => *self = Self::Error(error)
					}
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

pub(crate) fn parse(
	    item: &mut syn::Item,
	  output: &mut TokenStream,
	   range: crate::Spans,
	  stream: TokenStream,
	bindings: Bindings
) -> syn::Result<()> {
	let (placeholder, stream) = ("expand_view_here", Some(stream));
	let mut visitor = crate::Visitor::Ok { placeholder, stream };
	visitor.visit_item_mut(item);
	
	if visitor.has_stream()? { Err(range.error(ERROR))? }
	
	if !bindings.spans.is_empty() {
		let stream = Some(bindings.stream);
		let mut visitor = crate::Visitor::Ok { placeholder: "bindings", stream };
		visitor.visit_item_mut(item);
		
		if visitor.has_stream()? { crate::bindings_error(output, bindings.spans) }
	} Ok(())
}

pub fn display_ty(ty: &syn::TypePath, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
	if ty.qself.is_some() { write!(f, "qualified_")? }
	
	for segment in &ty.path.segments {
		let mut ident = compact_str::format_compact!("{}", segment.ident);
		ident.make_ascii_lowercase();
		write!(f, "{ident}_")?
	} Ok(())
}

impl<'a> crate::Assignee<'a> {
	pub fn spanned_to(&'a self, span: Span) -> impl Iterator<Item = syn::Ident> + 'a {
		self.iter().cloned().map(move |mut ident| { ident.set_span(span); ident })
	}
	
	fn iter(&'a self) -> Box<dyn Iterator<Item = &'a syn::Ident> + 'a> {
		match self {
			Self::Field(assignee, field) => Box::new(assignee.iter()
				.flat_map(|assignee| assignee.iter()).chain(field.iter())),
			
			Self::Ident(assignee, ident) => Box::new(assignee.iter()
				.flat_map(|assignee| assignee.iter()).chain(std::iter::once(*ident))),
			
			Self::None => unreachable!(),
		}
	}
}

impl quote::ToTokens for crate::Assignee<'_> {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let (assignee, chain): (_, &dyn quote::ToTokens) = match self {
			Self::Field(assignee, field) => (assignee, field),
			Self::Ident(assignee, ident) => (assignee, ident),
			Self::None => unreachable!(),
		};
		if let Some(assignee) = assignee {
			assignee.to_tokens(tokens);
			tokens.append(Punct::new('.', Spacing::Alone))
		}
		chain.to_tokens(tokens)
	}
}

impl<T: AsRef<[syn::Attribute]>> Attributes<T> {
	pub fn get<'a>(&'a self, fields: &'a mut Option<&mut Punctuated<syn::Field, syn::Token![,]>>) -> &[syn::Attribute] {
		match self {
			Self::Some(value) => value.as_ref(),
			Self::None(index) => &fields.as_deref().unwrap().iter().nth(*index).unwrap().attrs,
		}
	}
	pub fn as_slice(&self) -> Attributes<&[syn::Attribute]> {
		match self {
			Self::Some(value) => Attributes::Some(value.as_ref()),
			Self::None(index) => Attributes::None(*index),
		}
	}
}

impl crate::Builder {
	pub fn extend_into(self, objects: &mut TokenStream) {
		match self {
			#[cfg(not(feature = "builder-mode"))]
			Self::Builder(stream, _) => objects.extend(stream),
			
			#[cfg(feature = "builder-mode")]
			Self::Builder { left, right, span, tilde } => {
				objects.extend(left);
				objects.extend(quote::quote_spanned! {
					span => builder_mode!(#tilde #right)
				})
			}
			#[cfg(not(feature = "builder-mode"))]
			Self::Struct { ty, fields, call, span } => {
				objects.extend(ty);
				let mut fields = Group::new(Delimiter::Brace, fields);
				fields.set_span(span);
				objects.append(fields);
				if let Some(call) = call { objects.extend(call) }
			}
			#[cfg(feature = "builder-mode")]
			Self::Struct { left, mut ty, fields, span, tilde } => {
				objects.extend(left);
				
				let mut fields = Group::new(Delimiter::Brace, fields);
				fields.set_span(span);
				ty.append(fields);
				
				let mut value = Group::new(Delimiter::Parenthesis, ty);
				value.set_span(span);
				
				objects.extend(quote::quote_spanned! {
					span => builder_mode!(#tilde #value)
				})
			}
		}
		objects.append(Punct::new(';', Spacing::Alone))
	}
}

impl crate::Path {
	pub fn is_long(&self) -> bool {
		let Self::Type(path) = self else { return false };
		path.path.segments.len() > 1 || path.qself.is_some()
	}
	
	pub fn span(&self) -> Span {
		match self {
			Self::Type(ty) => ty.path.segments.last().map(|seg| seg.ident.span()),
			Self::Field { access, .. } => access.last().map(syn::Ident::span),
		}.unwrap_or(Span::call_site())
	}
}

impl std::fmt::Display for crate::Path {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Type(ty) => display_ty(ty, f)?,
			Self::Field { access, .. } => for ident in access { write!(f, "{ident}_")? }
		} Ok(())
	}
}

impl syn::parse::Parse for crate::Path {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		if input.peek(syn::Ident) && input.peek2(syn::Token![.]) {
			let access = crate::parse_unterminated(input)?;
			let gens = syn::AngleBracketedGenericArguments::parse_turbofish(input).ok();
			Ok(Self::Field { access, gens })
		} else { Ok(Self::Type(input.parse()?)) }
	}
}

impl quote::ToTokens for crate::Path {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		match self {
			Self::Type(path) => path.to_tokens(tokens),
			Self::Field { access, gens } => {
				access.to_tokens(tokens);
				gens.to_tokens(tokens);
			}
		}
	}
}

impl crate::Spans {
	pub fn error(self, message: impl std::fmt::Display) -> syn::Error {
		match self {
			Self::Single(span) => syn::Error::new(span, message),
			Self::Range(start, end) => {
				let mut a = Punct::new('<', Spacing::Alone); a.set_span(start);
				let mut b = Punct::new('>', Spacing::Alone); b.set_span(end);
				syn::Error::new_spanned(quote::quote![#a #b], message)
			}
		}
	}
}

impl crate::Visitor {
	pub fn has_stream(self) -> syn::Result<bool> {
		match self {
			crate::Visitor::Ok { stream, .. } => Ok(stream.is_some()),
			crate::Visitor::Error(error) => Err(error),
		}
	}
	fn make_error(&mut self, mac: &syn::Macro) {
		*self = Self::Error(crate::Spans::Range(
			mac.path.get_ident().unwrap().span(), mac.bang_token.span,
		).error("this placeholder must have no content"))
	}
}

impl VisitMut for crate::Visitor {
	fn visit_expr_mut(&mut self, node: &mut syn::Expr) {
		let crate::Visitor::Ok { placeholder, stream } = self else { return };
		if stream.is_none() { return }
		
		if let syn::Expr::Macro(mac) = node {
			if mac.mac.path.is_ident(placeholder) {
				if !mac.mac.tokens.is_empty() { return self.make_error(&mac.mac) }
				let group = Group::new(Delimiter::Brace, stream.take().unwrap());
				let mut stream = TokenStream::new(); stream.append(group);
				return *node = syn::Expr::Verbatim(stream)
			}
		}
		syn::visit_mut::visit_expr_mut(self, node)
	}
	
	fn visit_stmt_mut(&mut self, node: &mut syn::Stmt) {
		let crate::Visitor::Ok { placeholder, stream } = self else { return };
		if stream.is_none() { return }
		
		if let syn::Stmt::Macro(mac) = node {
			if mac.mac.path.is_ident(placeholder) {
				if !mac.mac.tokens.is_empty() { return self.make_error(&mac.mac) }
				return *node = syn::Stmt::Expr(
					syn::Expr::Verbatim(stream.take().unwrap()), None
				)
			}
		}
		syn::visit_mut::visit_stmt_mut(self, node)
	}
}
