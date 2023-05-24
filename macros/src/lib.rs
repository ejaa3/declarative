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
use proc_macro2::{Span, TokenStream as TokenStream2, TokenTree};
use quote::quote;
use syn::{parse::{Parse, ParseStream}, visit_mut::VisitMut};

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
struct Bindings { tokens: Vec<TokenStream2>, stream: TokenStream2, }

fn expand(Roots(roots): Roots) -> TokenStream2 {
	let objects  = &mut TokenStream2::new();
	let builders = &mut vec![];
	let settings = &mut TokenStream2::new();
	let bindings = &mut Bindings::default();
	
	for root in roots { item::expand(
		root, objects, builders, settings, bindings, &[], None, None
	) }
	
	if !bindings.tokens.is_empty() {
		for tokens in &bindings.tokens {
			objects.extend(syn::Error::new_spanned(
				tokens, "bindings must be consumed with the macro placeholder `bindings!`"
			).to_compile_error());
		}
	}
	
	let builders = builders.iter().rev();
	quote![#objects #(#builders;)* #settings]
}

#[proc_macro]
/// To fully understand this macro, please read the examples
/// in the [repository](https://github.com/ejaa3/declarative).
///
/// ### Basic usage
/// ~~~
/// fn usage() -> String {
///     declarative::block! {
///         String mut greeting {
///             push_str: "Hello world!"
///         }
///     }
///     greeting
/// }
/// ~~~
pub fn block(stream: TokenStream) -> TokenStream {
	TokenStream::from(expand(syn::parse_macro_input!(stream)))
}

struct Visitor { placeholder: &'static str, stream: TokenStream2 }

impl VisitMut for Visitor {
	fn visit_expr_mut(&mut self, node: &mut syn::Expr) {
		if self.placeholder.is_empty() { return }
		
		if let syn::Expr::Macro(mac) = node {
			if mac.mac.path.is_ident(self.placeholder) {
				self.placeholder = "";
				let stream = &self.stream;
				return *node = syn::Expr::Verbatim(quote![{#stream}]);
			}
		}
		syn::visit_mut::visit_expr_mut(self, node);
	}
	
	fn visit_stmt_mut(&mut self, node: &mut syn::Stmt) {
		if self.placeholder.is_empty() { return }
		
		if let syn::Stmt::Macro(mac) = node {
			if mac.mac.path.is_ident(self.placeholder) {
				self.placeholder = "";
				let stream = &self.stream;
				return *node = syn::Stmt::Expr(
					syn::Expr::Verbatim(quote![#stream]), None
				);
			}
		}
		syn::visit_mut::visit_stmt_mut(self, node);
	}
}

#[proc_macro_attribute]
/// To fully understand this macro, please read the examples
/// in the [repository](https://github.com/ejaa3/declarative).
///
/// ### Basic usage
/// ~~~
/// #[declarative::view {
///     String mut greeting {
///         push_str: "Hello world!"
///     }
/// }]
/// fn usage() -> String {
///     expand_view_here! { }
///     greeting
/// }
/// ~~~
pub fn view(stream: TokenStream, code: TokenStream) -> TokenStream {
	let stream = expand(syn::parse_macro_input!(stream));
	let syntax_tree = &mut syn::parse2(TokenStream2::from(code)).unwrap();
	
	let mut visitor = Visitor { placeholder: "expand_view_here", stream };
	visitor.visit_file_mut(syntax_tree);
	
	if !visitor.placeholder.is_empty() {
		panic!("the view must be consumed with the macro placeholder `expand_view_here!`")
	}
	TokenStream::from(quote![#syntax_tree])
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
struct Builder(TokenStream2, TokenStream2, Option<syn::Token![;]>);

#[cfg(feature = "builder-mode")]
impl quote::ToTokens for Builder {
	fn to_tokens(&self, tokens: &mut TokenStream2) {
		let Builder(left, right, end) = self;
		tokens.extend(quote![#left builder_mode!(#end #right)])
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
				outer.extend(quote![#copy]);
				
				*rest = next;
				if found { outer.extend(next.token_stream()); return true }
			}
			
			TokenTree::Punct(punct) => if punct.as_char() == '#' {
				if let Some((punct, next)) = next.punct() {
					if punct.as_char() == '#' {
						outer.extend(quote![#punct]);
						*rest = next;
						continue;
					}
				}
				let name = span_to(name, punct.span());
				outer.extend(quote![#(#name).*]);
				outer.extend(next.token_stream());
				return true
			} else { outer.extend(quote![#punct]); *rest = next; }
			
			token_tree => { outer.extend(quote![#token_tree]); *rest = next; }
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
	
	let stream = std::mem::take(&mut bindings.stream);
	let mut visitor = Visitor { placeholder: "bindings", stream };
	visitor.visit_expr_mut(expr);
	
	if !visitor.placeholder.is_empty() {
		objects.extend(syn::Error::new(
			at.span, "bindings must be consumed with the macro placeholder `bindings!`"
		).to_compile_error())
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
