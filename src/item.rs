/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use crate::{content::{self, Content}, common};

pub(crate) struct Item {
	  mut0: Option<syn::Token![mut]>,
	object: Option<common::Object>,
	  name: syn::Ident,
	  wrap: Punctuated<syn::Path, syn::Token![,]>,
	 chain: Option<TokenStream>,
	  pass: common::Pass,
	 build: Option<syn::Token![!]>,
	 props: Vec<Content>,
	  back: Option<common::Back>,
}

pub(crate) fn parse(input: syn::parse::ParseStream, reactive: bool) -> syn::Result<Item> {
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
			let Ok(keyword) = input.parse::<syn::Lifetime>() else { break };
			
			match keyword.ident.to_string().as_str() {
				"back" => break 'back (None, (vec![], Some(
					common::parse_back(input, keyword, vec![], reactive)?
				))),
				
				"chain" => if chain.is_some() {
					Err(syn::Error::new_spanned(keyword, "expected a single 'chain"))?
				} else { chain = Some(common::chain(input)?) }
				
				"wrap" => if wrap.is_none() {
					wrap = if input.peek(syn::token::Paren) {
						let parens; syn::parenthesized!(parens in input);
						Some(parens.parse_terminated(syn::Path::parse_mod_style, syn::Token![,])?) // TODO non-mod style
					} else {
						let mut wrap = Punctuated::new();
						wrap.push(input.parse()?);
						Some(wrap)
					}
				} else { Err(input.error("expected a single 'wrap"))? }
				
				_ => Err(error.error("expected 'back, 'chain or 'wrap"))?
			}
		}
		
		(input.parse()?, common::object_content(input, reactive, false)?)
	};
	
	let wrap = wrap.unwrap_or_default();
	
	Ok(Item { mut0, object, name, wrap, chain, pass, build, props, back })
}

pub(crate) fn expand(
	Item { mut0, object, name, wrap, chain, pass, build, props, back }: Item,
	  objects: &mut TokenStream,
	 builders: &mut Vec<TokenStream>,
	 settings: &mut TokenStream,
	 bindings: &mut TokenStream,
	     root: &[&syn::Ident],
	mut attrs: Vec<syn::Attribute>,
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
	
	if field { // TODO builder?
		if let Some(back) = back { back.do_not_use(objects) }
		settings.extend(quote![#(#attrs)* #(#root.)* #ident = #set #chain;])
	} else if let Some(index) = builder {
		if let Some(back) = back { back.do_not_use(objects) }
		builders[index].extend(quote![.#ident #(::#gens)* (#args #set #chain, #rest)])
	} else if let Some(common::Back { battrs, mut0, back, build, props, .. }) = back {
		attrs.extend(battrs);
		
		let (semi, index) = if build.is_some() {
			builders.push(TokenStream::new());
			(None, Some(builders.len() - 1))
		} else {
			(Some(<syn::Token![;]>::default()), None)
		};
		
		settings.extend(quote! {
			#(#attrs)* let #mut0 #back = #(#root.)* #ident #(::#gens)* (#args #set #chain, #rest) #semi
		});
		
		props.into_iter().for_each(|keyword| content::expand(
			keyword, objects, builders, settings, bindings, &attrs, &[&back], index
		));
		
		if let Some(index) = index {
			let builder = builders.remove(index);
			settings.extend(quote![#builder;])
		}
	} else {
		settings.extend(quote![#(#attrs)* #(#root.)* #ident #(::#gens)* (#args #set #chain, #rest);])
	}
}
