/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

#![warn(missing_docs)]

//! A proc-macro library for creating complex reactive views declaratively and quickly.

mod conditional;
mod content;
mod item;
mod property;

use proc_macro::TokenStream;
use proc_macro2::{Delimiter, Group, Span, TokenStream as TokenStream2, TokenTree};
use quote::{TokenStreamExt, quote, ToTokens};
use syn::{parse::{Parse, ParseStream}, visit_mut::VisitMut};

const B_ERROR: &str = "bindings must be consumed with the `bindings!` placeholder macro";
const V_ERROR: &str = "views must be consumed with the `expand_view_here!` placeholder macro";

struct Roots(Vec<item::Item>);

impl Parse for Roots {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut props = vec![];
		while !input.is_empty() {
			let attrs = input.call(syn::Attribute::parse_outer)?;
			props.push(item::parse(input, attrs, true, true)?)
		}
		Ok(Roots(props))
	}
}

#[derive(Default)]
struct Bindings { tokens: Vec<TokenStream2>, stream: TokenStream2 }

fn bindings_error(stream: &mut TokenStream2, bindings: Vec<TokenStream2>) {
	for tokens in bindings {
		stream.extend(syn::Error::new_spanned(tokens, B_ERROR).to_compile_error());
	}
}

fn expand(Roots(roots): Roots) -> (TokenStream2, Bindings) {
	let objects  = &mut TokenStream2::new();
	let builders = &mut vec![];
	let settings = &mut TokenStream2::new();
	let mut bindings = Bindings::default();
	
	for root in roots { item::expand(
		root, objects, builders, settings, &mut bindings, &[], None, None
	) }
	
	let builders = builders.iter().rev();
	(quote![#objects #(#builders;)* #settings], bindings)
}

#[proc_macro]
/// To fully understand this macro, please read the examples
/// in the [repository](https://github.com/ejaa3/declarative).
///
/// ### Basic usage
/// ~~~
/// use declarative_macros::block as view;
/// 
/// fn usage() -> String {
///     view! {
///         String mut greeting {
///             push_str: "Hello world!"
///         }
///     }
///     greeting
/// }
/// ~~~
pub fn block(stream: TokenStream) -> TokenStream {
	let (mut stream, bindings) = expand(syn::parse_macro_input!(stream));
	bindings_error(&mut stream, bindings.tokens);
	TokenStream::from(stream)
}

struct Visitor { placeholder: &'static str, stream: Option<TokenStream2> }

impl VisitMut for Visitor {
	fn visit_expr_mut(&mut self, node: &mut syn::Expr) {
		if self.stream.is_none() { return }
		
		if let syn::Expr::Macro(mac) = node {
			if mac.mac.path.is_ident(self.placeholder) {
				let group = Group::new(Delimiter::Brace, self.stream.take().unwrap());
				let mut stream = TokenStream2::new(); stream.append(group);
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

struct ViewVisitor { deque: std::collections::VecDeque<(TokenStream2, Bindings)> }

macro_rules! item {
	($visit:ident: $item:ident) => {
		fn $visit(&mut self, node: &mut syn::$item) {
			if let syn::$item::Macro(mac) = node {
				if mac.mac.path.is_ident("view") {
					self.deque.push_back(mac.mac.parse_body().map(expand)
						.unwrap_or_else(|error| (
							syn::Error::into_compile_error(error), Bindings::default()
						)));
					return *node = syn::$item::Verbatim(TokenStream2::new())
				}
			}
			syn::visit_mut::$visit(self, node)
		}
	}
}

impl VisitMut for ViewVisitor {
	item!(visit_foreign_item_mut: ForeignItem);
	item!(visit_impl_item_mut: ImplItem);
	item!(visit_item_mut: Item);
	item!(visit_trait_item_mut: TraitItem);
}

#[proc_macro_attribute]
/// To fully understand this macro, please read the examples
/// in the [repository](https://github.com/ejaa3/declarative).
///
/// ### Basic usage
/// ~~~
/// use declarative_macros::view;
/// 
/// #[view {
///     String mut greeting {
///         push_str: "Hello world!"
///     }
/// }]
/// fn usage() -> String {
///     expand_view_here! { }
///     greeting
/// }
/// ~~~
///
/// ### Alternate usage
/// ~~~
/// use declarative_macros::view;
/// 
/// #[view]
/// mod example {
///     view! {
///         String mut greeting {
///             push_str: "Hello world!"
///         }
///     }
///     fn usage() -> String {
///         expand_view_here! { }
///         greeting
///     }
/// }
/// ~~~
pub fn view(stream: TokenStream, code: TokenStream) -> TokenStream {
	let file = &mut syn::parse_macro_input!(code);
	let mut output = TokenStream2::new();
	
	if stream.is_empty() {
		let mut visitor = ViewVisitor { deque: Default::default() };
		visitor.visit_file_mut(file);
		
		if visitor.deque.is_empty() { panic!("there must be at least one `view!`") }
		
		while let Some(view) = visitor.deque.pop_front() { parse(file, &mut output, view) }
	} else { parse(file, &mut output, expand(syn::parse_macro_input!(stream))) }
	
	file.to_tokens(&mut output);
	TokenStream::from(output)
}

fn parse(file: &mut syn::File, output: &mut TokenStream2, view: (TokenStream2, Bindings)) {
	let (stream, bindings) = view;
	
	let mut visitor = Visitor { placeholder: "expand_view_here", stream: Some(stream) };
	visitor.visit_file_mut(file);
	
	if let Some(stream) = visitor.stream {
		output.extend(syn::Error::new_spanned(stream, V_ERROR).into_compile_error())
	}
	
	if !bindings.tokens.is_empty() {
		let stream = Some(bindings.stream);
		let mut visitor = Visitor { placeholder: "bindings", stream };
		visitor.visit_file_mut(file);
		
		if visitor.stream.is_some() { bindings_error(output, bindings.tokens) }
	}
}

fn count() -> compact_str::CompactString {
	use std::cell::RefCell;
	thread_local![static COUNT: RefCell<usize> = RefCell::new(0)];
	
	COUNT.with(|cell| {
		let count = *cell.borrow();
		*cell.borrow_mut() = count.wrapping_add(1);
		compact_str::format_compact!("_declarative_{}", count)
	})
}

#[cfg(not(feature = "builder-mode"))]
type Builder = TokenStream2;

#[cfg(feature = "builder-mode")]
struct Builder {
	 left: TokenStream2,
	right: TokenStream2,
	 span: Span,
	tilde: Option<syn::Token![~]>,
}

#[cfg(feature = "builder-mode")]
impl quote::ToTokens for Builder {
	fn to_tokens(&self, tokens: &mut TokenStream2) {
		let Builder { left, right, span, tilde } = self;
		left.to_tokens(tokens);
		
		quote::quote_spanned! {
			*span => builder_mode!(#tilde #right)
		}.to_tokens(tokens)
	}
}

trait ParseReactive: Sized {
	fn parse(input: ParseStream,
	         attrs: Option<Vec<syn::Attribute>>,
	      reactive: bool,
	) -> syn::Result<Self>;
}

fn parse_vec<T: ParseReactive>(input: ParseStream, reactive: bool) -> syn::Result<Vec<T>> {
	if input.peek(syn::token::Brace) {
		let braces; syn::braced!(braces in input);
		let mut props = vec![];
		
		while !braces.is_empty() {
			props.push(T::parse(&braces, Some(braces.call(syn::Attribute::parse_outer)?), reactive)?)
		}
		
		Ok(props)
	} else { Ok(vec![T::parse(input, None, reactive)?]) }
}

fn extend_attributes(attrs: &mut Vec<syn::Attribute>, pattrs: &[syn::Attribute]) {
	let current = std::mem::take(attrs);
	attrs.reserve(pattrs.len() + current.len());
	attrs.extend_from_slice(pattrs);
	attrs.extend(current.into_iter());
}

fn find_pound(rest: &mut syn::buffer::Cursor, outer: &mut TokenStream2, name: &[&syn::Ident]) -> bool {
	while let Some((token_tree, next)) = rest.token_tree() {
		match token_tree {
			TokenTree::Group(group) => {
				let delimiter = group.delimiter();
				let (mut into, _, next) = rest.group(delimiter).unwrap();
				let mut inner = TokenStream2::new();
				let found = find_pound(&mut into, &mut inner, name);
				
				let mut copy = proc_macro2::Group::new(delimiter, inner);
				copy.set_span(group.span());
				outer.append(copy);
				
				*rest = next;
				if found { outer.extend(next.token_stream()); return true }
			}
			
			TokenTree::Punct(punct) => if punct.as_char() == '#' {
				if let Some((punct, next)) = next.punct() {
					if punct.as_char() == '#' {
						outer.append(punct);
						*rest = next;
						continue;
					}
				}
				let name = span_to(name, punct.span());
				outer.extend(quote![#(#name).*]);
				outer.extend(next.token_stream());
				return true
			} else { outer.append(punct); *rest = next; }
			
			token_tree => { outer.append(token_tree); *rest = next; }
		}
	}
	false
}

fn try_bind(at: syn::Token![@],
       objects: &mut TokenStream2,
      bindings: &mut Bindings,
          expr: &mut syn::Expr,
) {
	if std::mem::take(&mut bindings.tokens).is_empty() {
		return objects.extend(syn::Error::new(
			at.span, "there are no bindings to consume"
		).to_compile_error())
	}
	
	let stream = Some(std::mem::take(&mut bindings.stream));
	let mut visitor = Visitor { placeholder: "bindings", stream };
	visitor.visit_expr_mut(expr);
	
	if visitor.stream.is_some() {
		objects.extend(syn::Error::new(at.span, B_ERROR).to_compile_error())
	}
}

fn span_to<'a>(assignee: &'a [&'a syn::Ident], span: Span) -> std::iter::Map <
	std::slice::Iter<'a, &'a syn::Ident>,
	impl FnMut(&'a &'a syn::Ident) -> syn::Ident
> {
	assignee.iter().map(move |name| {
		let mut name = (*name).clone();
		name.set_span(span);
		name
	})
}
