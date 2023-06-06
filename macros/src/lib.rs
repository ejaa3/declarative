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
mod view;

use std::cell::RefCell;
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2, TokenTree};
use quote::{TokenStreamExt, ToTokens, quote};
use syn::{punctuated::Punctuated, visit_mut::VisitMut};

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
	let (mut stream, bindings) = view::expand(syn::parse_macro_input!(stream));
	view::bindings_error(&mut stream, bindings.spans);
	TokenStream::from(stream)
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
		let mut visitor = view::Visitor { deque: Default::default() };
		visitor.visit_file_mut(file);
		
		if visitor.deque.is_empty() { panic!("there must be at least one `view!`") }
		
		while let Some(view) = visitor.deque.pop_front() { view::parse(file, &mut output, view) }
	} else { view::parse(file, &mut output, view::expand(syn::parse_macro_input!(stream))) }
	
	file.to_tokens(&mut output);
	TokenStream::from(output)
}

#[derive(Copy, Clone)]
enum Assignee<'a> {
	Field (&'a Punctuated<syn::Ident, syn::Token![.]>),
	Ident (&'a syn::Ident),
	None,
}

#[derive(Default)]
struct Bindings { spans: Vec<Span>, stream: TokenStream2 }

enum Builder {
	#[cfg(not(feature = "builder-mode"))]
	Builder(TokenStream2),
	
	#[cfg(feature = "builder-mode")]
	Builder {
		 left: TokenStream2,
		right: TokenStream2,
		 span: Span,
		tilde: Option<syn::Token![~]>,
	},
	
	#[cfg(not(feature = "builder-mode"))]
	Struct { ty: TokenStream2, fields: RefCell<TokenStream2>, call: Option<TokenStream2> },
	
	#[cfg(feature = "builder-mode")]
	Struct {
		  left: TokenStream2,
		    ty: TokenStream2,
		fields: RefCell<TokenStream2>,
		  span: Span,
		 tilde: Option<syn::Token![~]>,
	},
}

enum Mode { Field(Span), Method(Span), FnField(Span) }

trait ParseReactive: Sized {
	fn parse(input: syn::parse::ParseStream,
	         attrs: Option<Vec<syn::Attribute>>,
	      reactive: bool,
	) -> syn::Result<Self>;
}

enum Path {
	Type(syn::TypePath), Field {
		access: Punctuated<syn::Ident, syn::Token![.]>,
		  gens: Option<syn::AngleBracketedGenericArguments>,
	}
}

struct Visitor { placeholder: &'static str, stream: Option<TokenStream2> }

fn count() -> compact_str::CompactString {
	thread_local![static COUNT: RefCell<usize> = RefCell::new(0)];
	
	COUNT.with(|cell| {
		let count = *cell.borrow();
		*cell.borrow_mut() = count.wrapping_add(1);
		compact_str::format_compact!("_declarative_{}", count)
	})
}

fn extend_attributes(attrs: &mut Vec<syn::Attribute>, pattrs: &[syn::Attribute]) {
	let current = std::mem::take(attrs);
	attrs.reserve(pattrs.len() + current.len());
	attrs.extend_from_slice(pattrs);
	attrs.extend(current.into_iter());
}

fn find_pound(rest: &mut syn::buffer::Cursor, outer: &mut TokenStream2, assignee: Assignee) -> bool {
	while let Some((token_tree, next)) = rest.token_tree() {
		match token_tree {
			TokenTree::Group(group) => {
				let delimiter = group.delimiter();
				let (mut into, _, next) = rest.group(delimiter).unwrap();
				let mut inner = TokenStream2::new();
				let found = find_pound(&mut into, &mut inner, assignee);
				
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
						continue
					}
				}
				
				let assignee = assignee.spanned_to(punct.span());
				outer.extend(quote![#(#assignee).*]);
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
	if std::mem::take(&mut bindings.spans).is_empty() {
		return objects.extend(syn::Error::new(
			at.span, "there are no bindings to consume"
		).to_compile_error())
	}
	
	let stream = Some(std::mem::take(&mut bindings.stream));
	let mut visitor = Visitor { placeholder: "bindings", stream };
	visitor.visit_expr_mut(expr);
	
	if visitor.stream.is_some() {
		objects.extend(syn::Error::new_spanned(expr, ERROR).to_compile_error())
	}
}

fn parse_unterminated<T, P>(input: syn::parse::ParseStream) -> syn::Result<Punctuated<T, P>>
where T: syn::parse::Parse, P: syn::parse::Parse {
	let mut punctuated = Punctuated::new();
	punctuated.push_value(input.parse()?);
	
	while let Ok(punct) = input.parse() {
		punctuated.push_punct(punct);
		punctuated.push_value(input.parse()?);
	}
	Ok(punctuated)
}

fn parse_vec<T: ParseReactive>(
	input: syn::parse::ParseStream, reactive: bool
) -> syn::Result<(syn::token::Brace, Vec<T>)> {
	let braces;
	let (brace, mut props) = (syn::braced!(braces in input), vec![]);
	
	while !braces.is_empty() {
		props.push(T::parse(&braces, Some(braces.call(syn::Attribute::parse_outer)?), reactive)?)
	}
	Ok((brace, props))
}

const ERROR: &str = "bindings must be consumed with the `bindings!` placeholder macro";
