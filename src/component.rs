/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::TokenStream;
use crate::{content::{self, Content}, common};

pub(crate) struct Component {
	 attrs: Vec<syn::Attribute>,
	  pass: common::Pass,
	object: Option<common::Object>,
	  mut0: Option<syn::Token![mut]>,
	  name: syn::Ident,
	   dot: Option<TokenStream>,
	  with: Option<syn::Expr>,
	 build: Option<syn::Token![!]>,
	 props: Vec<Content>,
	  back: Option<common::Back>,
}

pub(crate) fn parse(
	   input: syn::parse::ParseStream,
	   attrs: Vec<syn::Attribute>,
	reactive: bool,
	    root: bool,
) -> syn::Result<Component> {
	let pass = common::parse_pass(input, root)?;
	let use0 = input.parse::<syn::Token![use]>().is_ok();
	let object = (!use0).then(|| input.parse()).transpose()?;
	let mut0 = if !use0 { input.parse()? } else { None };
	let name = if use0 { Some(input.parse()?) } else { input.parse()? }
		.unwrap_or_else(|| syn::Ident::new(&common::count(), input.span()));
	
	let mut dot = None;
	let mut with = None;
	
	let (build, (props, back)) = 'back: {
		for _ in 0..3 {
			let Ok(keyword) = input.parse::<syn::Lifetime>() else { break };
			
			if root {
				return Err(syn::Error::new_spanned(keyword, "cannot use 'keywords here"))
			}
			
			match keyword.ident.to_string().as_str() {
				"back" => break 'back (None, (vec![], Some(
					common::parse_back(input, keyword, vec![], reactive)?
				))),
				
				"dot" => if dot.is_some() {
					Err(syn::Error::new_spanned(keyword, "expected a single 'dot"))?
				} else { dot = Some(common::dot(input)?) }
				
				"with" => if with.is_some() {
					Err(syn::Error::new_spanned(keyword, "expected a single 'with"))?
				} else { with = Some(input.parse()?) }
				
				_ => Err(syn::Error::new_spanned(keyword, "expected 'back, 'dot or 'with"))?
			}
		}
		
		(input.parse()?, common::object_content(input, reactive, root)?)
	};
	
	Ok(Component { attrs, pass, object, mut0, name, dot, with, build, props, back })
}

pub(crate) fn expand(
	Component { mut attrs, pass, object, mut0, name, dot, with, build, props, back }: Component,
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
	
	let Some(composable) = composable else { return };
	let with = with.unwrap_or_else(|| syn::Expr::Verbatim(quote::quote!(())));
	let common::Pass(pass) = pass;
	
	if let Some(common::Back { token: _, battrs, mut0, back, build, props }) = back {
		attrs.extend(battrs);
		
		let (semi, index) = if build.is_some() {
			builders.push(TokenStream::new());
			(None, Some(builders.len() - 1))
		} else {
			(Some(<syn::Token![;]>::default()), None)
		};
		
		settings.extend(quote::quote! {
			#(#attrs)*
			let #mut0 #back = #(#composable.)* as_composable_add_component(
				#pass #name #dot, #with
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
			#(#composable.)* as_composable_add_component(#pass #name #dot, #with);
		});
	}
}
