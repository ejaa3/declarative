/*
 * SPDX-FileCopyrightText: 2025 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::{Delimiter, Group, Punct, Spacing, Span, TokenStream};
use quote::{TokenStreamExt, quote_spanned};
use syn::{punctuated::Punctuated, visit_mut::VisitMut};
use crate::{item, Attributes, Bindings, Range};

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
			} else { props.push(Root::Item(item::parse(input, Some(attrs))?)) }
		}
		
		Ok(Self(props))
	}
}

pub enum Streaming {
	Roots(Roots), Struct {
		     vis: syn::Visibility,
		   ident: Option<syn::Ident>,
		generics: syn::Generics,
		  fields: Punctuated<syn::Field, syn::Token![,]>
	}
}

impl syn::parse::Parse for Streaming {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		if input.is_empty() { return Ok(Self::Struct {
			     vis: syn::Visibility::Inherited,
			   ident: None,
			generics: syn::Generics::default(),
			  fields: Default::default(),
		}) }
		
		if input.peek(syn::Token![pub]) && (input.peek2(syn::Token![struct]) || (
			input.peek2(syn::token::Paren) && input.peek3(syn::Token![struct])
		)) { return Ok(Self::Roots(input.parse()?)) }
		
		let mut vis = input.parse()?;
		let ident = if input.peek(syn::Ident) && (
			   input.peek2(syn::parse::End)
			|| input.peek2(syn::Token![<])
			|| input.peek2(syn::Token![,])
		) { Some(input.parse()?) } else { None };
		
		let generics: syn::Generics = input.parse()?;
		let mut fields;
		
		if let syn::Visibility::Inherited = vis {
			if ident.is_some() || generics.lt_token.is_some() {
				let comma = input.parse::<syn::Token![,]>();
				fields = input.parse_terminated(syn::Field::parse_named, syn::Token![,])?;
				if !fields.is_empty() { comma?; }
			} else if input.peek(syn::Ident) && input.peek2(syn::Token![:]) && (
				input.peek3(syn::Token![::]) || !input.peek3(syn::Token![:])
			) {
				fields = input.parse_terminated(syn::Field::parse_named, syn::Token![,])?
			} else if generics.lt_token.is_some() {
				return Err(syn::Error::new_spanned(generics, "unexpected generics"))
			} else { return Ok(Self::Roots(input.parse()?)) }
		} else {
			let comma = input.parse::<syn::Token![,]>();
			if input.is_empty() { fields = Default::default() } else {
				fields = input.parse_terminated(syn::Field::parse_named, syn::Token![,])?;
				if comma.is_err() { fields[0].vis = vis; vis = syn::Visibility::Inherited }
			} // this condition prevents panicking with `attempt to subtract with overflow`
		}     // while expanding #[view(pub)] as it should
		Ok(Self::Struct { vis, ident, generics, fields })
	}
}

pub fn expand(structs: &mut Vec<syn::ItemStruct>, roots: Roots, errable: bool) -> (TokenStream, Bindings) {
	let mut objects = TokenStream::new();
	let (mut constrs, mut settings, mut bindings) = Default::default();
	let (mut n_fields, mut strukt, mut followed) = ([0; 3], None, true);
	
	if let Some(syn::Fields::Named(fields)) = structs.first()
		.map(|strukt| &strukt.fields) { n_fields[0] = fields.named.len() }
	
	macro_rules! check_struct(() => (
		if !followed {
			objects.extend(syn::Error::new_spanned(
				strukt.as_ref(), "structs must be followed by items"
			).into_compile_error())
		}
		if let Some(strukt) = strukt.take() {
			if n_fields[1] == n_fields[2] {
				objects.extend(syn::Error::new_spanned(
					&strukt, "this struct does not refer to any item"
				).into_compile_error())
			}
			structs.push(strukt)
		}
	));
	
	for root in roots.0 { match root {
		Root::Struct(item) => {
			check_struct!();
			let syn::Fields::Named(fields) = &item.fields else { panic!() };
			n_fields[1] = fields.named.len();
			strukt = Some(item);
			followed = false;
		}
		Root::Item(item) => {
			followed = true;
			
			let fields = &mut if let Some(strukt) = strukt.as_mut().or(structs.first_mut()) {
				let syn::Fields::Named(fields) = &mut strukt.fields else { panic!() };
				Some(&mut fields.named)
			} else { None };
			
			item::expand(
				item, &mut objects, &mut constrs, &mut settings,
				&mut bindings, fields, Attributes::Some(&[])
			);
			if let Some(fields) = fields { n_fields[2] = fields.len() }
		}
	} }
	
	if let Some(strukt) = structs.first() {
		let syn::Fields::Named(fields) = &strukt.fields else { panic!() };
		if n_fields[0] == fields.named.len() {
			structs.swap_remove(0);
			if errable {
				objects.extend(syn::Error::new(
					Span::call_site(), "these arguments would declare a \
					struct that does not reference any item in the view"
				).into_compile_error())
			}
		}
	}
	
	check_struct! { }
	for constr in constrs.into_iter().rev() { constr.extend_into(&mut objects) }
	objects.extend(settings); (objects, bindings)
}

pub enum Visitor {
	Error(syn::Error), Ok {
		structs: Vec<syn::ItemStruct>,
		errable: bool,
		  deque: std::collections::VecDeque<(Range, TokenStream, Bindings)>
	}
}

macro_rules! item {
	($visit:ident, $item:ident) => {
		fn $visit(&mut self, node: &mut syn::$item) {
			let Self::Ok { structs, errable, deque } = self else { return };
			
			if let syn::$item::Macro(mac) = node {
				if mac.mac.path.is_ident("view") {
					let range = Range(
						mac.mac.path.segments[0].ident.span(),
						mac.mac.bang_token.span,
					);
					if mac.mac.tokens.is_empty() {
						return *self = Self::Error(range.error("this view has no content"))
					}
					return match mac.mac.parse_body().map(|root| expand(structs, root, *errable)) {
						Ok((stream, bindings)) => {
							deque.push_back((range, stream, bindings));
							*node = syn::$item::Verbatim(TokenStream::new())
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
	item!(visit_foreign_item_mut, ForeignItem);
	item!(visit_impl_item_mut, ImplItem);
	item!(visit_item_mut, Item);
	item!(visit_trait_item_mut, TraitItem);
	
	fn visit_item_impl_mut(&mut self, node: &mut syn::ItemImpl) {
		if let Self::Ok { structs, .. } = self {
			if let syn::Type::Path(path) = node.self_ty.as_ref() {
				path.path.get_ident().map(|ident| structs.first_mut().map(|strukt| {
					strukt.attrs = node.attrs.clone();
					if strukt.generics.lt_token.is_none() {
						strukt.generics = node.generics.clone()
					}
					if strukt.ident == "_" { strukt.ident = ident.clone() }
				}));
			}
		}
		syn::visit_mut::visit_item_impl_mut(self, node)
	}
}

pub fn parse(item: &mut syn::Item,
           output: &mut TokenStream,
            range: Range,
     mut   stream: TokenStream,
     mut bindings: Bindings,
) {
	let mut visitor = crate::Visitor::Ok {
		items: None, assignee: &mut None, placeholder: "expand_view_here", stream: &mut stream
	};
	visitor.visit_item_mut(item);
	
	const ERROR: &str = "views must be consumed with the `expand_view_here!` placeholder macro";
	match visitor.stream_is_empty() {
		Ok(empty) => if !empty { stream.extend(range.error(ERROR).into_compile_error()) },
		Err(error) => stream.extend(error.into_compile_error()),
	}
	
	if !bindings.spans.is_empty() {
		let mut visitor = crate::Visitor::Ok {
			items: None, assignee: &mut None, placeholder: "bindings", stream: &mut bindings.stream
		};
		visitor.visit_item_mut(item);
		
		match visitor.stream_is_empty() {
			Ok(empty) => if !empty { crate::bindings_error(output, bindings.spans) },
			Err(error) => stream.extend(error.into_compile_error()),
		}
	}
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
		}
	}
}

impl quote::ToTokens for crate::Assignee<'_> {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let (assignee, chain): (_, &dyn quote::ToTokens) = match self {
			Self::Field(assignee, field) => (assignee, field),
			Self::Ident(assignee, ident) => (assignee, ident),
		};
		if let Some(assignee) = assignee {
			assignee.to_tokens(tokens);
			tokens.append(Punct::new('.', Spacing::Alone))
		}
		chain.to_tokens(tokens)
	}
}

impl<T: AsRef<[syn::Attribute]>> Attributes<T> {
	pub fn get<'a>(&'a self, fields: &'a mut Option<&mut Punctuated<syn::Field, syn::Token![,]>>) -> &'a[syn::Attribute] {
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

impl crate::Construction {
	pub fn extend_into(self, objects: &mut TokenStream) {
		match self {
			Self::BuilderPattern { left, right, span, tilde } => {
				objects.extend(left);
				objects.extend(quote::quote_spanned! {
					span => construct!(#tilde #right)
				})
			}
			Self::StructLiteral { left, ty, fields, span, tilde } => {
				objects.extend(left);
				
				let mut fields = Group::new(Delimiter::Brace, fields);
				fields.set_span(span);
				
				objects.extend(quote::quote_spanned! {
					span => construct!(? #tilde #ty #fields)
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

impl Range {
	pub fn error(self, message: &str) -> syn::Error {
		let a = syn::Ident::new("a", self.0);
		let b = syn::Ident::new("b", self.1);
		syn::Error::new_spanned(quote::quote![#a #b], message)
	}
}

impl crate::Visitor<'_, '_> {
	pub fn stream_is_empty(self) -> syn::Result<bool> {
		match self {
			Self::Ok { stream, .. } => Ok(stream.is_empty()),
			Self::Error(error) => Err(error),
		}
	}
	fn make_error(&mut self, mac: &syn::Macro) {
		let range = Range(mac.path.get_ident().unwrap().span(), mac.bang_token.span);
		*self = Self::Error(range.error("this placeholder must have no content"));
	}
}

impl VisitMut for crate::Visitor<'_, '_> {
	fn visit_expr_mut(&mut self, node: &mut syn::Expr) {
		let Self::Ok { items, assignee, placeholder, stream } = self else { return };
		
		if stream.is_empty() && items.is_none() { return }
		
		if let (Some(items), syn::Expr::Infer(syn::ExprInfer { underscore_token, .. })) = (items, &node) {
			let Some(item) = items.next().or_else(|| assignee.take()) else {
				return *self = Self::Error(syn::Error::new(
					underscore_token.span, "not enough items for as many placeholders as this one"
				))
			};
			let assignee = item.spanned_to(underscore_token.span);
			return *node = syn::Expr::Verbatim(quote_spanned!(underscore_token.span => #(#assignee).*))
		}
		
		if let syn::Expr::Macro(mac) = node {
			if !stream.is_empty() && mac.mac.path.is_ident(placeholder) {
				if !mac.mac.tokens.is_empty() { return self.make_error(&mac.mac) }
				let group = Group::new(Delimiter::Brace, std::mem::take(stream));
				let mut stream = TokenStream::new(); stream.append(group);
				return *node = syn::Expr::Verbatim(stream)
			}
		}
		syn::visit_mut::visit_expr_mut(self, node)
	}
	
	fn visit_stmt_mut(&mut self, node: &mut syn::Stmt) {
		let Self::Ok { items: _, assignee: _, placeholder, stream } = self else { return };
		if stream.is_empty() { return }
		
		if let syn::Stmt::Macro(mac) = node {
			if mac.mac.path.is_ident(placeholder) {
				if !mac.mac.tokens.is_empty() { return self.make_error(&mac.mac) }
				return *node = syn::Stmt::Expr(
					syn::Expr::Verbatim(std::mem::take(stream)), None
				)
			}
		}
		syn::visit_mut::visit_stmt_mut(self, node)
	}
}
