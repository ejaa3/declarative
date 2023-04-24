/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::TokenStream;
use syn::punctuated::Punctuated;
use crate::{content::{self, Content}, common};

pub(crate) struct Component<const B: bool> {
	 attrs: Vec<syn::Attribute>,
	  pass: common::Pass,
	object: Option<common::Object>,
	  name: syn::Ident,
	  with: Option<syn::Expr>,
	 chain: Option<TokenStream>,
	  mut0: Option<syn::Token![mut]>,
	 build: Option<syn::Token![!]>,
	 props: Vec<Content<B>>,
	  back: Option<common::Back<B>>,
}

impl<const B: bool> syn::parse::Parse for Component<B> {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		let attrs = input.call(syn::Attribute::parse_outer)?;
		let pass = input.parse()?;
		let error = input.fork();
		let use0 = input.parse::<syn::Lifetime>()
			.map(|kw| if kw.ident == "use" { Ok(true) } else { Err(error.error("expected 'use")) })
			.unwrap_or(Ok(false))?;
		
		let object = (!use0).then(|| input.parse()).transpose()?;
		let mut0 = if !use0 { input.parse()? } else { None };
		let name = if use0 { Some(input.parse()?) } else { input.parse()? }
			.unwrap_or_else(|| syn::Ident::new(&common::count(), input.span()));
		
		let mut with = None;
		let mut chain = None;
		
		let (build, (props, back)) = 'back: {
			for _ in 0..3 {
				if !input.peek(syn::Lifetime) { break }
				let error = input.fork();
				
				match input.parse::<syn::Lifetime>()?.ident.to_string().as_str() {
					"with" => {
						if with.is_some() { Err(input.error("expected a single 'with"))? }
						with = Some(input.parse()?);
					}
					"chain" => chain = {
						if chain.is_some() { Err(input.error("expected a single 'chain"))? }
						Some(common::chain(input)?)
					},
					"back" => break 'back (None, (vec![], Some(common::back(input)?))),
					_ => Err(error.error("expected 'with, 'back or 'chain"))?
				}
			}
			
			(input.parse()?, common::item_content(input)?)
		};
		
		Ok(Component { attrs, pass, object, name, with, chain, mut0, build, props, back })
	}
}

pub(crate) fn expand<const B: bool>(
	Component { mut attrs, mut0, object, name, with, chain, pass, build, props, back }: Component<B>,
	   objects: &mut TokenStream,
	  builders: &mut Vec<TokenStream>,
	  settings: &mut TokenStream,
	  bindings: &mut TokenStream,
	    pattrs: &[syn::Attribute],
	composable: Option<&[&syn::Ident]>,
) {
	common::extend_attributes(&mut attrs, pattrs);
	
	let builder = object.map(|object| common::expand_object(
		object, objects, builders, &attrs, mut0, &name, build.is_some()
	)).unwrap_or(None);
	
	props.into_iter().for_each(|keyword| content::expand(
		keyword, objects, builders, settings, bindings, &attrs, &[&name], builder
	));
	
	if let Some(composable) = composable {
		let with = if let Some(with) = with { with } else {
			syn::Expr::Tuple(syn::ExprTuple {
				attrs: vec![], paren_token: syn::token::Paren::default(), elems: Punctuated::new()
			})
		};
		
		let common::Pass(pass) = pass;
		
		if let Some(common::Back { mut0, name: back, build, props }) = back {
			let (semi, index) = if build.is_some() {
				builders.push(TokenStream::new());
				(None, Some(builders.len() - 1))
			} else {
				(Some(<syn::Token![;]>::default()), None)
			};
			
			settings.extend(quote::quote! {
				#(#attrs)*
				let #mut0 #back = #(#composable.)* as_composable_add_component(
					#pass #name #chain, #with
				) #semi
			});
			
			props.into_iter().for_each(|keyword| content::expand(
				keyword, objects, builders, settings, bindings, &attrs, &[&back], index
			));
			
			if let Some(index) = index {
				let builder = builders.remove(index);
				settings.extend(quote::quote![#builder;])
			}
		} else {
			settings.extend(quote::quote! {
				#(#attrs)*
				#(#composable.)* as_composable_add_component(#pass #name #chain, #with);
			});
		}
	}
}
