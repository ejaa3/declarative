/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use super::{common, content::{self, Content}, item};

pub(crate) struct Prop<T> {
	attrs: Vec<syn::Attribute>,
	 prop: syn::Ident,
	 gens: Option<(syn::Token![::], syn::AngleBracketedGenericArguments)>,
	 args: Punctuated<syn::Expr, syn::Token![,]>,
	 rest: Option<TokenStream>,
	value: T,
}

impl<T: common::ParseReactive> common::ParseReactive for Prop<T> {
	fn parse(input: syn::parse::ParseStream,
	         attrs: Option<Vec<syn::Attribute>>,
	      reactive: bool,
	) -> syn::Result<Prop<T>> {
		let attrs = attrs.unwrap_or_default();
		let prop = input.parse()?;
		
		let gens = input.parse::<syn::Token![::]>().ok()
			.map(|sep| Ok::<_, syn::Error>((sep, input.parse()?))).transpose()?;
		
		let mut args = Punctuated::new();
		let mut rest = None;
		
		if input.peek(syn::token::Bracket) {
			let brackets; syn::bracketed!(brackets in input);
			
			while !brackets.is_empty() {
				if brackets.parse::<syn::Token![@]>().is_ok() {
					brackets.parse::<syn::Token![,]>()?;
					rest = Some(brackets.parse()?);
					break;
				}
				
				args.push_value(brackets.parse()?);
				if brackets.is_empty() { break; }
				args.push_punct(brackets.parse()?);
			}
		}
		
		if !args.empty_or_trailing() {
			args.push_punct(Default::default());
		}
		
		Ok(Prop { attrs, prop, gens, args, rest, value: T::parse(input, None, reactive)? })
	}
}

pub(crate) enum Value {
	ItemCall(item::Item),
	ItemField(item::Item),
	Expr(Expr),
}

impl common::ParseReactive for Value {
	fn parse(input: syn::parse::ParseStream,
	        _attrs: Option<Vec<syn::Attribute>>,
	      reactive: bool,
	) -> syn::Result<Self> {
		if input.peek(syn::Token![=>]) {
			input.parse::<syn::Token![=>]>()?;
			Ok(Value::ItemCall(item::parse(input, reactive)?))
		} else if input.peek(syn::Token![->]) && !input.peek3(syn::token::Brace) {
			input.parse::<syn::Token![->]>()?;
			Ok(Value::ItemField(item::parse(input, reactive)?))
		} else {
			Ok(Value::Expr(Expr::parse(input, None, reactive)?))
		}
	}
}

pub(crate) fn expand_value(
	Prop { mut attrs, prop, gens, args, rest, value }: Prop<Value>,
	 objects: &mut TokenStream,
	builders: &mut Vec<crate::Builder>,
	settings: &mut TokenStream,
	bindings: &mut TokenStream,
	  pattrs: &[syn::Attribute],
	    name: &[&syn::Ident],
	 builder: Option<usize>,
) {
	match value {
		Value::ItemCall(item) => {
			common::extend_attributes(&mut attrs, pattrs);
			item::expand(
				item, objects, builders, settings, bindings,
				&name, attrs, prop, gens, args, rest, builder, false
			);
		}
		Value::ItemField(item) => {
			common::extend_attributes(&mut attrs, pattrs);
			item::expand(
				item, objects, builders, settings, bindings,
				&name, attrs, prop, gens, args, rest, builder, true
			);
		}
		Value::Expr(value) => {
			let prop = Prop { attrs, prop, gens, args, rest, value };
			
			let Some(expr) = expand_expr(
				prop, objects, builders, settings,
				bindings, pattrs, name, builder.is_some()
			) else { return };
			
			let Some(index) = builder else { return settings.extend(expr) };
			
			#[cfg(feature = "builder-mode")]
			builders[index].1.extend(expr);
			
			#[cfg(not(feature = "builder-mode"))]
			builders[index].extend(expr);
		}
	}
}

pub(crate) enum Expr {
	Call {
		clones: Punctuated<common::Clone, syn::Token![,]>,
		 value: syn::Expr,
		  back: Option<common::Back>,
	},
	Invoke (Option<common::Back>),
	Field  (Punctuated<common::Clone, syn::Token![,]>, syn::Expr),
	Edit   (Vec<Content>),
}

impl common::ParseReactive for Expr {
	fn parse(input: syn::parse::ParseStream,
	        _attrs: Option<Vec<syn::Attribute>>,
	      reactive: bool,
	) -> syn::Result<Self> {
		let back = || {
			let Ok(keyword) = input.fork().parse::<syn::Lifetime>()
				else { return Ok::<_, syn::Error>(None) };
			
			if keyword.ident == "back" {
				input.parse::<syn::Lifetime>()?;
				Ok(Some(common::parse_back(input, keyword, vec![], reactive)?))
			} else { Ok(None) }
		};
		
		if input.parse::<syn::Token![:]>().is_ok() {
			let clones = common::parse_clones(input)?;
			let  value = input.parse()?;
			let   back = back()?;
			input.parse::<Option<syn::Token![;]>>()?;
			Ok(Expr::Call { clones, value, back })
		} else if input.parse::<syn::Token![=]>().is_ok() {
			let expr = Expr::Field(common::parse_clones(input)?, input.parse()?);
			input.parse::<Option<syn::Token![;]>>()?;
			Ok(expr)
		} else if input.parse::<syn::Token![->]>().is_ok() {
			let braces; syn::braced!(braces in input);
			Ok(Expr::Edit(common::content(&braces, reactive)?))
		} else if input.parse::<syn::Token![;]>().is_ok() {
			Ok(Expr::Invoke(None))
		} else {
			Ok(Expr::Invoke(back()?))
		}
	}
}

pub(crate) fn expand_expr(
	Prop { mut attrs, prop, gens, args, rest, value }: Prop<Expr>,
	 objects: &mut TokenStream,
	builders: &mut Vec<crate::Builder>,
	settings: &mut TokenStream,
	bindings: &mut TokenStream,
	  pattrs: &[syn::Attribute],
	    name: &[&syn::Ident],
	   build: bool,
) -> Option<TokenStream> {
	if let Expr::Edit(content) = value {
		common::extend_attributes(&mut attrs, pattrs);
		
		let mut field = Vec::with_capacity(name.len() + 1);
		field.extend_from_slice(name);
		field.push(&prop);
		
		content.into_iter().for_each(|content| content::expand(
			content, objects, builders, settings, bindings, &attrs, &field, None
		)); // TODO builder mode?
		return None
	}
	
	let (sep, gens) = gens.unzip();
	
	let (assigned, back) = match value {
		Expr::Call { clones, value, back } =>
			if clones.is_empty() {
				(quote![#value,], back)
			} else {
				let clones = clones.into_iter();
				(quote![{ #(let #clones;)* #value },], back)
			}
		Expr::Field(clones, value) => {
			let value = if !clones.is_empty() {
				let clones = clones.into_iter();
				quote![{ #(let #clones;)* #value }]
			} else { quote![#value] };
			
			return Some(quote![#(#pattrs)* #(#attrs)* #(#name.)* #prop = #value;])
		},
		Expr::Invoke(back) => (quote![], back),
		Expr::Edit(..) => unreachable!(),
	};
	
	if build {
		if let Some(back) = back { back.do_not_use(objects); return None }
		return Some(quote![.#prop #sep #gens (#args #assigned #rest)])
	}
	
	let Some(back0) = back else { return quote! {
		#(#pattrs)* #(#attrs)* #(#name.)* #prop #sep #gens (#args #assigned #rest);
	}.into() };
	
	let common::Back { mut0, back, .. } = &back0;
	let left  = quote! { #(#pattrs)* #(#attrs)* let #mut0 #back = };
	let right = quote! { #(#name.)* #prop #sep #gens (#args #assigned #rest) };
	
	common::extend_attributes(&mut attrs, pattrs);
	common::expand_back(back0, objects, builders, settings, bindings, attrs, left, right);
	None
}
