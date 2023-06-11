/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

#![warn(missing_docs)]

//! A proc-macro library for creating complex reactive views declaratively and quickly.

mod content;
mod item;
mod property;
mod view;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2, TokenTree};
use quote::{TokenStreamExt, ToTokens, quote_spanned};
use syn::{punctuated::Punctuated, visit_mut::VisitMut};

macro_rules! unwrap (($expr:expr) => (match $expr {
	Ok(value) => value,
	Err(error) => return TokenStream::from(error.into_compile_error())
}));

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
	let mut items = vec![];
	let (mut stream, bindings) = unwrap! {
		view::expand(&mut items, syn::parse_macro_input!(stream))
	};
	
	bindings_error(&mut stream, bindings.spans);
	for item in items { item.to_tokens(&mut stream) }
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
	let item = &mut syn::parse_macro_input!(code);
	let mut output = TokenStream2::new();
	
	let fill = |item: &mut _, output: &mut _, structs: &mut Vec<_>| {
		if let syn::Item::Mod(mod_) = item {
			if let Some((_, items)) = &mut mod_.content {
				items.reserve(structs.len());
				while let Some(item) = structs.pop() {
					items.push(syn::Item::Struct(item))
				} return
			}
		}
		while let Some(item) = structs.pop() { item.to_tokens(output) }
	};
	
	if stream.is_empty() {
		let mut visitor = view::Visitor::Ok { items: vec![], deque: Default::default() };
		visitor.visit_item_mut(item);
		
		match visitor {
			view::Visitor::Ok { mut items, mut deque } => {
				if deque.is_empty() { panic!("there must be at least one `view!`") }
				
				while let Some((stream, bindings)) = deque.pop_front() {
					unwrap!(view::parse(item, &mut output, stream, bindings));
					fill(item, &mut output, &mut items)
				}
			}
			view::Visitor::Error(error) => return TokenStream::from(error.into_compile_error())
		}
	} else {
		let mut structs = vec![];
		let (stream, bindings) = unwrap!(view::expand(&mut structs, syn::parse_macro_input!(stream)));
		unwrap!(view::parse(item, &mut output, stream, bindings));
		fill(item, &mut output, &mut structs)
	}
	
	item.to_tokens(&mut output);
	TokenStream::from(output)
}

#[derive(Copy, Clone)]
enum Assignee<'a> {
	Field (Option<&'a Assignee<'a>>, &'a Punctuated<syn::Ident, syn::Token![.]>),
	Ident (Option<&'a Assignee<'a>>, &'a syn::Ident),
	 None,
}

#[derive(Copy, Clone)]
enum Attributes<T: AsRef<[syn::Attribute]>> { Some(T), None(usize) }

#[derive(Default)]
struct Bindings { spans: Vec<Span>, stream: TokenStream2 }

enum Builder {
	#[cfg(not(feature = "builder-mode"))]
	Builder(TokenStream2, Span),
	
	#[cfg(feature = "builder-mode")]
	Builder {
		 left: TokenStream2,
		right: TokenStream2,
		 span: Span,
		tilde: Option<syn::Token![~]>,
	},
	#[cfg(not(feature = "builder-mode"))]
	Struct {
		    ty: TokenStream2,
		fields: TokenStream2,
		  call: Option<TokenStream2>,
		  span: Span,
	},
	#[cfg(feature = "builder-mode")]
	Struct {
		  left: TokenStream2,
		    ty: TokenStream2,
		fields: TokenStream2,
		  span: Span,
		 tilde: Option<syn::Token![~]>,
	},
}

struct Field {
	  vis: syn::Visibility,
	 mut_: Option<syn::Token![mut]>,
	 name: syn::Ident,
	colon: Option<syn::Token![:]>,
	   ty: Option<Box<syn::TypePath>>,
}

enum Mode { Field(Span), Method(Span), FnField(Span) }

enum Path {
	Type(syn::TypePath), Field {
		access: Punctuated<syn::Ident, syn::Token![.]>,
		  gens: Option<syn::AngleBracketedGenericArguments>,
	}
}

struct Visitor { placeholder: &'static str, stream: Option<TokenStream2> }

fn bindings_error(stream: &mut TokenStream2, spans: Vec<Span>) {
	for span in spans { stream.extend(syn::Error::new(span, ERROR).to_compile_error()) }
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
				let (mut into, _, next) = rest.group(group.delimiter()).unwrap();
				let mut inner = TokenStream2::new();
				let found = find_pound(&mut into, &mut inner, assignee);
				
				let mut copy = proc_macro2::Group::new(group.delimiter(), inner);
				copy.set_span(group.span());
				outer.append(copy);
				
				*rest = next;
				if found { outer.extend(next.token_stream()); return true }
			}
			
			TokenTree::Punct(punct) => if punct.as_char() == '#' {
				if let Some((mut inner, next)) = next.punct() {
					if inner.as_char() == '#' {
						inner.set_span(punct.span());
						outer.append(punct);
						*rest = next;
						continue
					}
				}
				
				let assignee = assignee.spanned_to(punct.span());
				outer.extend(quote_spanned![punct.span() => #(#assignee).*]);
				outer.extend(next.token_stream());
				return true
			} else { outer.append(punct); *rest = next }
			
			token_tree => { outer.append(token_tree); *rest = next }
		}
	}
	false
}

fn try_bind(at: syn::Token![@], bindings: &mut Bindings, expr: &mut syn::Expr) -> syn::Result<()> {
	if std::mem::take(&mut bindings.spans).is_empty() {
		Err(syn::Error::new(at.span, "there are no bindings to consume or \
			you are trying from an inner binding or conditional scope"))?
	}
	
	let stream = Some(std::mem::take(&mut bindings.stream));
	let mut visitor = Visitor { placeholder: "bindings", stream };
	visitor.visit_expr_mut(expr);
	
	if visitor.stream.is_some() { Err(syn::Error::new_spanned(expr, ERROR)) } else { Ok(()) }
}

fn parse_unterminated<T, P>(input: syn::parse::ParseStream) -> syn::Result<Punctuated<T, P>>
where T: syn::parse::Parse, P: syn::parse::Parse {
	let mut punctuated = Punctuated::new();
	punctuated.push_value(input.parse()?);
	
	while let Ok(punct) = input.parse() {
		punctuated.push_punct(punct);
		punctuated.push_value(input.parse()?)
	}
	Ok(punctuated)
}

const    ERROR: &str = "bindings must be consumed with the `bindings!` placeholder macro";
const  NO_TYPE: &str = "a type must be specified after the colon";
const NO_FIELD: &str = "a colon cannot be used if a struct has not been \
	declared before the root item or within a binding or conditional scope";
