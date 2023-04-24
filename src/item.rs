/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use crate::{content::{self, Content}, common};

pub(crate) struct Item<const B: bool> {
	  mut0: Option<syn::Token![mut]>,
	object: Option<common::Object>,
	  name: syn::Ident,
	  wrap: Punctuated<syn::Path, syn::Token![,]>,
	 chain: Option<TokenStream>,
	  pass: common::Pass,
	 build: Option<syn::Token![!]>,
	 props: Vec<Content<B>>,
	  back: Option<common::Back<B>>,
}

impl<const B: bool> syn::parse::Parse for Item<B> {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		let pass = input.parse()?;
		let error = input.fork();
		let use0 = input.parse::<syn::Lifetime>()
			.map(|kw| if kw.ident == "use" { Ok(true) } else { Err(error.error("expected 'use")) })
			.unwrap_or(Ok(false))?;
		
		let object = (!use0).then(|| input.parse()).transpose()?;
		let mut0 = if !use0 { input.parse()? } else { None };
		let name = if use0 { Some(input.parse()?) } else { input.parse()? }
			.unwrap_or_else(|| syn::Ident::new(&common::count(), input.span()));
		let mut wrap = None;
		let mut chain = None;
		
		let (build, (props, back)) = 'back: {
			for _ in 0..3 {
				if !input.peek(syn::Lifetime) { break }
				let error = input.fork();
				
				match input.parse::<syn::Lifetime>()?.ident.to_string().as_str() {
					"chain" => chain = {
						if chain.is_some() { Err(input.error("expected a single 'chain"))? }
						Some(common::chain(input)?)
					},
					"wrap" => wrap = {
						if wrap.is_some() { Err(input.error("expected a single 'wrap"))? }
						
						if input.peek(syn::token::Paren) {
							let parens; syn::parenthesized!(parens in input);
							Some(parens.parse_terminated(syn::Path::parse, syn::Token![,])?)
						} else {
							let mut wrap = Punctuated::new();
							wrap.push(input.parse()?);
							Some(wrap)
						}
					},
					"back" => break 'back (None, (vec![], Some(common::back(input)?))),
					_ => Err(error.error("expected 'back, 'chain or 'wrap"))?
				}
			}
			
			(input.parse()?, common::item_content(input)?)
		};
		
		let wrap = wrap.unwrap_or_default();
		
		Ok(Item { mut0, object, name, wrap, chain, pass, build, props, back })
	}
}

pub(crate) fn expand<const B: bool>(
	Item { mut0, object, name, wrap, chain, pass, build, props, back }: Item<B>,
	 objects: &mut TokenStream,
	builders: &mut Vec<TokenStream>,
	settings: &mut TokenStream,
	bindings: &mut TokenStream,
	    root: &[&syn::Ident],
	   attrs: Vec<syn::Attribute>,
	   ident: syn::Ident,
	    gens: Option<syn::AngleBracketedGenericArguments>,
	    args: Punctuated<syn::Expr, syn::Token![,]>,
	    rest: Option<TokenStream>,
	 builder: Option<usize>,
	   field: bool,
) {
	let new_builder = object.map(|object| common::expand_object(
		object, objects, builders, &attrs, mut0, &name, build.is_some()
	)).unwrap_or(None);
	
	props.into_iter().for_each(|keyword| content::expand(
		keyword, objects, builders, settings, bindings, &attrs, &[&name], new_builder
	));
	
	let gens = gens.into_iter();
	let common::Pass(pass) = pass;
	
	let mut set = quote![#pass #name];
	wrap.into_iter().for_each(|wrap| { set = quote![#wrap(#set)] });
	
	if field {
		settings.extend(quote![#(#attrs)* #(#root.)* #ident = #set #chain;])
	} else if let Some(index) = builder {
		builders[index].extend(quote![.#ident #(::#gens)* (#args #set #chain, #rest)])
	} else if let Some(common::Back { mut0, name, build, props }) = back {
		let (semi, index) = if build.is_some() {
			builders.push(TokenStream::new());
			(None, Some(builders.len() - 1))
		} else {
			(Some(<syn::Token![;]>::default()), None)
		};
		
		settings.extend(quote! {
			#(#attrs)* let #mut0 #name = #(#root.)* #ident #(::#gens)* (#args #set #chain, #rest) #semi
		});
		
		props.into_iter().for_each(|keyword| content::expand(
			keyword, objects, builders, settings, bindings, &attrs, &[&name], index
		));
		
		if let Some(index) = index {
			let builder = builders.remove(index);
			settings.extend(quote![#builder;])
		}
	} else {
		settings.extend(quote![#(#attrs)* #(#root.)* #ident #(::#gens)* (#args #set #chain, #rest);])
	}
}
