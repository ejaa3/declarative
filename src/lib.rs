/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

#![warn(missing_docs)]

//! Currently a procedural macro for creating complex reactive views declaratively and quickly.

mod common;
mod component;
mod conditional;
mod content;
mod item;
mod property;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse::{Parse, ParseStream}, visit_mut::VisitMut};

struct Block(Vec<Content>);

impl Parse for Block {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut props = vec![];
		while !input.is_empty() { props.push(input.parse()?) }
		Ok(Block(props))
	}
}

enum Content { Root(component::Component<true>), Code(TokenStream2) }

impl Parse for Content {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		if input.peek(syn::Token![..]) {
			input.parse::<syn::Token![..]>()?;
			Ok(Content::Code(input.parse()?))
		} else { Ok(Content::Root(input.parse()?)) }
	}
}

struct Visit { name: &'static str, stream: TokenStream2 }

impl VisitMut for Visit {
	fn visit_stmt_mut(&mut self, node: &mut syn::Stmt) {
		if let syn::Stmt::Macro(mac) = node {
			if mac.mac.path.is_ident(self.name) {
				let stream = &self.stream;
				return *node = syn::Stmt::Expr(
					syn::Expr::Verbatim(syn::parse_quote!(#stream)), None
				);
			}
		}
		syn::visit_mut::visit_stmt_mut(self, node);
	}
}

#[proc_macro]
/// To learn how to use this macro, please visit the
/// [repository](https://github.com/ejaa3/declarative).
pub fn view(stream: TokenStream) -> TokenStream {
	let Block(content) = syn::parse_macro_input!(stream);
	
	let objects  = &mut TokenStream2::new();
	let builders = &mut vec![];
	let settings = &mut TokenStream2::new();
	let bindings = &mut TokenStream2::new();
	
	let mut code = None;
	
	content.into_iter().for_each(|content| match content {
		Content::Root(root) => component::expand(
			root, objects, builders, settings, bindings, &[], None
		),
		Content::Code(stream) => code = Some(stream),
	});
	
	let builders = builders.into_iter().rev();
	let stream = quote![#objects #(#builders;)* #settings];
	
	if let Some(code) = code {
		let syntax_tree = &mut syn::parse2(code).unwrap();
		
		Visit { name: "expand_view_here", stream }.visit_file_mut(syntax_tree);
		quote![#syntax_tree].into()
	} else { stream.into() }
}
