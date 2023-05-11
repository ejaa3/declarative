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
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{ToTokens, quote};
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

fn expand(Roots(roots): Roots) -> TokenStream2 {
	let objects  = &mut TokenStream2::new();
	let builders = &mut vec![];
	let settings = &mut TokenStream2::new();
	let bindings = &mut TokenStream2::new();
	
	for root in roots { item::expand(
		root, objects, builders, settings, bindings, &[], None, None
	) }
	
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
	expand(syn::parse_macro_input!(stream)).into()
}

struct Visitor { name: &'static str, stream: TokenStream2 }

impl VisitMut for Visitor {
	fn visit_expr_mut(&mut self, node: &mut syn::Expr) {
		if self.name.is_empty() { return }
		
		if let syn::Expr::Macro(mac) = node {
			if mac.mac.path.is_ident(self.name) {
				self.name = "";
				let stream = &self.stream;
				return *node = syn::Expr::Verbatim(syn::parse_quote![{#stream}]);
			}
		}
		syn::visit_mut::visit_expr_mut(self, node);
	}
	fn visit_stmt_mut(&mut self, node: &mut syn::Stmt) {
		if self.name.is_empty() { return }
		
		if let syn::Stmt::Macro(mac) = node {
			if mac.mac.path.is_ident(self.name) {
				self.name = "";
				let stream = &self.stream;
				return *node = syn::Stmt::Expr(
					syn::Expr::Verbatim(syn::parse_quote![#stream]), None
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
	let syntax_tree = &mut syn::parse2(code.into()).unwrap();
	
	let mut visitor = Visitor { name: "expand_view_here", stream };
	visitor.visit_file_mut(syntax_tree);
	
	if !visitor.name.is_empty() {
		panic!("The view must be consumed with the pseudo-macro `expand_view_here!`")
	}
	
	quote![#syntax_tree].into()
}

thread_local![static COUNT: std::cell::RefCell<usize> = {
	std::cell::RefCell::new(0)
}];

fn count() -> String {
	COUNT.with(move |cell| {
		let count = *cell.borrow();
		*cell.borrow_mut() = count.wrapping_add(1);
		format!("_declarative_{}", count)
	})
}

#[cfg(not(feature = "builder-mode"))]
type Builder = TokenStream2;

#[cfg(feature = "builder-mode")]
struct Builder(TokenStream2, TokenStream2, Option<syn::Token![;]>);

#[cfg(feature = "builder-mode")]
impl ToTokens for Builder {
	fn to_tokens(&self, tokens: &mut TokenStream2) {
		let Builder(left, right, end) = self;
		tokens.extend(quote![#left builder_mode!(#end #right)])
	}
}

enum Object { Expr(Box<syn::Expr>), Type(Box<syn::TypePath>) }

impl ToTokens for Object {
	fn to_tokens(&self, tokens: &mut TokenStream2) {
		match self {
			Object::Expr(expr) => expr.to_tokens(tokens),
			Object::Type(ty) => ty.to_tokens(tokens),
		}
	}
}

impl Parse for Object {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let ahead = input.fork();
		
		if ahead.parse::<syn::TypePath>().is_ok() && (
			   ahead.peek(syn::Token![mut])
			|| ahead.peek(syn::Ident)
			|| ahead.peek(syn::Token![#])
			|| ahead.peek(syn::Token![!])
			|| ahead.peek(syn::token::Brace)
		) {
			Ok(Object::Type(input.parse()?))
		} else {
			Ok(Object::Expr(input.parse()?))
		}
	}
}

fn expand_object(
	  object: &Object,
	 objects: &mut TokenStream2,
	builders: &mut Vec<Builder>,
	   attrs: &[syn::Attribute],
	    mut0: Option<syn::Token![mut]>,
	    name: &syn::Ident,
	 builder: bool,
) -> Option<usize> {
	if builder {
		builders.push(match object {
			#[cfg(feature = "builder-mode")]
			Object::Type(ty) => Builder(
				quote![#(#attrs)* let #mut0 #name = ], quote![#ty => ], None
			),
			#[cfg(not(feature = "builder-mode"))]
			Object::Type(ty) => quote![#(#attrs)* let #mut0 #name = #ty::default()],
			
			#[cfg(feature = "builder-mode")]
			Object::Expr(expr) => Builder(
				quote![#(#attrs)* let #mut0 #name = ], quote![#expr ], None
			),
			#[cfg(not(feature = "builder-mode"))]
			Object::Expr(expr) => quote![#(#attrs)* let #mut0 #name = #expr],
		});
		Some(builders.len() - 1)
	} else {
		objects.extend(match object {
			Object::Type(ty) => quote![#(#attrs)* let #mut0 #name = #ty::default();],
			Object::Expr(call) => quote![#(#attrs)* let #mut0 #name = #call;],
		});
		None
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
