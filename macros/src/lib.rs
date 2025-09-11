/*
 * SPDX-FileCopyrightText: 2025 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

#![warn(missing_docs)]

//! Generic DSL macros for easy view code manipulation.

mod content;
mod item;
mod property;
mod view;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::ToTokens;
use std::{iter::Map, slice::Iter};
use syn::{punctuated::Punctuated, visit_mut::VisitMut};

macro_rules! unwrap (($expr:expr) => (match $expr {
	Ok(value) => value,
	Err(error) => return TokenStream::from(error.into_compile_error())
}));

#[proc_macro]
/// The [repository examples](https://github.com/ejaa3/declarative) try to illustrate the use of this macro.
///
/// ### Basic usage
/// ~~~
/// use declarative_macros::block as view;
/// 
/// fn usage() -> String {
///     view! {
///         String::new() mut greeting {
///             push_str: "Hello world!"
///         }
///     }
///     greeting
/// }
/// ~~~
pub fn block(stream: TokenStream) -> TokenStream {
	if stream.is_empty() {
		let error = syn::Error::new(Span::call_site(), "this view block has no content");
		return TokenStream::from(error.into_compile_error())
	}
	
	let mut structs = vec![];
	let view::Streaming::Roots(roots) = syn::parse_macro_input!(stream) else { panic!() };
	let (mut stream, bindings) = unwrap!(view::expand(&mut structs, roots));
	
	bindings_error(&mut stream, bindings.spans);
	for strukt in structs { strukt.to_tokens(&mut stream) }
	TokenStream::from(stream)
}

#[proc_macro_attribute]
/// The [repository examples](https://github.com/ejaa3/declarative) try to illustrate the use of this macro.
///
/// ### Basic usage
/// ~~~
/// use declarative_macros::view;
/// 
/// #[view {
///     String::new() mut greeting {
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
///         String::new() mut greeting {
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
	
	match syn::parse_macro_input!(stream) {
		view::Streaming::Struct { vis, fields } => {
			let structs = vec![syn::ItemStruct {
				vis, fields: syn::Fields::Named(syn::FieldsNamed {
					brace_token: Default::default(), named: fields
				}),
				attrs: vec![],
				struct_token: Default::default(),
				ident: syn::Ident::new("Unknown", Span::call_site()),
				generics: Default::default(),
				semi_token: Default::default(),
			}];
			
			let mut visitor = view::Visitor::Ok { structs, deque: Default::default() };
			visitor.visit_item_mut(item);
			
			match visitor {
				view::Visitor::Ok { mut structs, mut deque } => {
					if deque.is_empty() { return TokenStream::from(
						syn::Error::new(Span::call_site(), NO_VIEW_ERROR).into_compile_error()
					) }
					
					while let Some((spans, stream, bindings)) = deque.pop_front() {
						unwrap!(view::parse(item, &mut output, spans, stream, bindings));
						fill(item, &mut output, &mut structs)
					}
				}
				view::Visitor::Error(error) => return TokenStream::from(error.into_compile_error())
			}
		}
		view::Streaming::Roots(roots) => {
			let (range, mut structs) = (Range(Span::call_site(), Span::call_site()), vec![]);
			let (stream, bindings) = unwrap!(view::expand(&mut structs, roots));
			unwrap!(view::parse(item, &mut output, range, stream, bindings));
			fill(item, &mut output, &mut structs)
		}
	}
	
	item.to_tokens(&mut output); TokenStream::from(output)
}

#[derive(Copy, Clone)]
enum Assignee<'a> {
	Field (Option<&'a Assignee<'a>>, &'a Punctuated<syn::Ident, syn::Token![.]>),
	Ident (Option<&'a Assignee<'a>>, &'a syn::Ident),
}

#[derive(Copy, Clone)]
enum Attributes<T: AsRef<[syn::Attribute]>> { Some(T), None(usize) }

#[derive(Default)]
struct Bindings { spans: Vec<Span>, stream: TokenStream2 }

enum Construction {
	BuilderPattern {
		 left: TokenStream2,
		right: TokenStream2,
		 span: Span,
		tilde: Option<syn::Token![~]>,
	},
	StructLiteral {
		  left: TokenStream2,
		    ty: TokenStream2,
		fields: TokenStream2,
		  span: Span,
		 tilde: Option<syn::Token![~]>,
	},
}

enum Path {
	Type(syn::TypePath), Field {
		access: Punctuated<syn::Ident, syn::Token![.]>,
		  gens: Option<syn::AngleBracketedGenericArguments>,
	}
}

struct Range(Span, Span);

enum Visitor<'a, 'b> {
	#[allow(clippy::type_complexity)]
	Ok {  items: Option<&'a mut Map<Iter<'b, item::Item>, fn(&item::Item) -> Assignee>>,
	   assignee: &'a mut Option<Assignee<'b>>,
	placeholder: &'static str,
	     stream: &'a mut TokenStream2 },
	
	Error(syn::Error)
}

fn bindings_error(stream: &mut TokenStream2, spans: Vec<Span>) {
	for span in spans { stream.extend(syn::Error::new(span, BINDINGS_ERROR).to_compile_error()) }
}

fn extend_attributes(attrs: &mut Vec<syn::Attribute>, pattrs: &[syn::Attribute]) {
	let current = std::mem::take(attrs);
	attrs.reserve(pattrs.len() + current.len());
	attrs.extend_from_slice(pattrs);
	attrs.extend(current);
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

const BINDINGS_ERROR: &str = "bindings must be consumed with the `bindings!` placeholder macro";

const NO_VIEW_ERROR: &str = "if no view code is written as the content of this attribute, at \
	least one view must be created with `view!` in the scope of a `mod`, `impl` or `trait`";

struct ConstrError(&'static str);

impl std::fmt::Display for ConstrError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(self.0)?;
		f.write_str(" in the initial content of an item whose definition should be expanded in a builder pattern or a struct literal")
	}
}
