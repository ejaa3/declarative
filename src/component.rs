/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::TokenStream;
use quote::quote;
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
	let ref0 = input.parse::<syn::Token![ref]>().is_ok();
	let object = (!ref0).then(|| input.parse()).transpose()?;
	let mut0 = if !ref0 { input.parse()? } else { None };
	let name = if ref0 { Some(input.parse()?) } else { input.parse()? }
		.unwrap_or_else(|| syn::Ident::new(&common::count(), input.span()));
	
	let mut dot = None;
	let mut with = None;
	
	let (build, (props, back)) = 'back: {
		for _ in 0..3 {
			let Ok(keyword) = input.parse::<syn::Lifetime>() else { break };
			
			if root {
				return Err(syn::Error::new(keyword.span(), "cannot use 'keywords here"))
			}
			
			match keyword.ident.to_string().as_str() {
				"back" => break 'back (None, (vec![], Some(
					common::parse_back(input, keyword, vec![], reactive)?
				))),
				
				"dot" => if dot.is_some() {
					Err(syn::Error::new(keyword.span(), "expected a single 'dot"))?
				} else { dot = Some(common::dot(input)?) }
				
				"with" => if with.is_some() {
					Err(syn::Error::new(keyword.span(), "expected a single 'with"))?
				} else { with = Some(input.parse()?) }
				
				_ => Err(syn::Error::new(keyword.span(), "expected 'back, 'dot or 'with"))?
			}
		}
		
		(input.parse()?, common::object_content(input, reactive, root)?)
	};
	
	Ok(Component { attrs, pass, object, mut0, name, dot, with, build, props, back })
}

pub(crate) fn expand(
	Component { mut attrs, pass, object, mut0, name, dot, with, build, props, back }: Component,
	   objects: &mut TokenStream,
	  builders: &mut Vec<crate::Builder>,
	  settings: &mut TokenStream,
	  bindings: &mut TokenStream,
	    pattrs: &[syn::Attribute],
	composable: Option<&[&syn::Ident]>,
) {
	common::extend_attributes(&mut attrs, pattrs);
	
	let builder = object.map(|object| common::expand_object(
		object, objects, builders, &attrs, mut0, &name, build.is_some()
	)).unwrap_or(None);
	
	props.into_iter().for_each(|content| content::expand(
		content, objects, builders, settings, bindings, &attrs, &[&name], builder
	));
	
	let Some(composable) = composable else { return };
	let with = with.unwrap_or_else(|| syn::Expr::Verbatim(quote!(())));
	let common::Pass(pass) = pass;
	
	let Some(mut back0) = back else {
		return settings.extend(quote! {
			#(#attrs)*
			#(#composable.)* as_composable_add_component(#pass #name #dot, #with);
		});
	};
	
	let common::Back { battrs, mut0, back, .. } = &mut back0;
	attrs.extend(std::mem::take(battrs));
	
	let left  = quote! { #(#attrs)* let #mut0 #back = };
	let right = quote! {
		#(#composable.)* as_composable_add_component(#pass #name #dot, #with)
	};
	common::expand_back(back0, objects, builders, settings, bindings, attrs, left, right);
}
