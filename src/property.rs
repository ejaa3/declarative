/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use super::{common, content::{self, Content}, item};

pub(crate) struct Prop<T: Parse> {
	attrs: Vec<syn::Attribute>,
	ident: syn::Ident,
	 gens: Option<syn::AngleBracketedGenericArguments>,
	 args: Punctuated<syn::Expr, syn::Token![,]>,
	 rest: Option<TokenStream>,
	value: T,
}

impl<T: Parse> Parse for Prop<T> {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let attrs = input.call(syn::Attribute::parse_outer)?;
		let ident = input.parse()?;
		
		let gens = if input.peek(syn::Token![::]) {
			input.parse::<syn::Token![::]>()?;
			Some(input.parse()?)
		} else { None };
		
		let mut args = Punctuated::new();
		let mut rest = None;
		
		if input.peek(syn::token::Bracket) {
			let brackets; syn::bracketed!(brackets in input);
			loop {
				if brackets.is_empty() { break; }
				
				if brackets.peek(syn::Token![..]) {
					brackets.parse::<syn::Token![..]>()?;
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
		
		Ok(Prop { attrs, ident, gens, args, rest, value: input.parse()? })
	}
}

pub(crate) enum Value<const B: bool> {
	ItemCall(item::Item<B>),
	ItemField(item::Item<B>),
	Expr(Expr<B>),
}

impl<const B: bool> Parse for Value<B> {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let ahead = input.lookahead1();
		
		if  ahead.peek (syn::Token![:])
		||  ahead.peek (syn::Token![!])
		||  input.peek (syn::Token![->])
		&&  input.peek3(syn::token::Brace)
		||  input.peek (syn::Token![=])
		&& !input.peek2(syn::Token![>]) {
			Ok(Value::Expr(input.parse()?))
		} else if ahead.peek(syn::Token![=>]) {
			input.parse::<syn::Token![=>]>()?;
			Ok(Value::ItemCall(input.parse()?))
		} else if ahead.peek(syn::Token![->]) {
			input.parse::<syn::Token![->]>()?;
			Ok(Value::ItemField(input.parse()?))
		} else {
			Err(ahead.error())
		}
	}
}

pub(crate) enum Expr<const B: bool> {
	Call {
		clones: Punctuated<common::Clone, syn::Token![,]>,
		 value: syn::Expr,
		  back: Option<common::Back<B>>,
	},
	Invoke { back: Option<common::Back<B>> },
	Field(Punctuated<common::Clone, syn::Token![,]>, syn::Expr),
	Edit(Vec<Content<B>>),
}

impl<const B: bool> Parse for Expr<B> {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let ahead = input.lookahead1();
		
		let back = ||
			if let Ok("back") = input.fork().parse::<syn::Lifetime>()
				.map(|keyword| keyword.ident.to_string()).as_deref() {
					input.parse::<syn::Lifetime>()?;
					Ok(Some(common::back(input)?))
				} else { Ok::<_, syn::Error>(None) };
		
		if ahead.peek(syn::Token![:]) {
			input.parse::<syn::Token![:]>()?;
			
			Ok(Expr::Call {
				clones: if input.peek(syn::Lifetime) {
					let error = input.fork();
					
					if input.parse::<syn::Lifetime>()?.ident.to_string().as_str() == "clone" {
						common::parse_clones(input)?
					} else { Err(error.error("expected 'clone"))? }
				} else { Punctuated::new() },
				
				value: input.parse()?,
				 back: back()?
			})
		} else if ahead.peek(syn::Token![!]) {
			input.parse::<syn::Token![!]>()?;
			Ok(Expr::Invoke { back: back()? })
		} else if ahead.peek(syn::Token![=]) {
			input.parse::<syn::Token![=]>()?;
			
			if !input.peek(syn::Lifetime) {
				Ok(Expr::Field(Default::default(), input.parse()?))
			} else {
				let error = input.fork();
				if input.parse::<syn::Lifetime>()?.ident != "clone" {
					return Err(error.error("expected 'clone"))
				}
				Ok(Expr::Field(common::parse_clones(input)?, input.parse()?))
			}
		} else if ahead.peek(syn::Token![->]) {
			input.parse::<syn::Token![->]>()?;
			let braces; syn::braced!(braces in input);
			Ok(Expr::Edit(common::content(&braces)?))
		} else {
			Err(ahead.error())
		}
	}
}

pub(crate) fn expand_expr<const B: bool>(
	Prop { mut attrs, ident, gens, args, rest, value }: Prop<Expr<B>>,
	 objects: &mut TokenStream,
	builders: &mut Vec<TokenStream>,
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
		field.push(&ident);
		
		content.into_iter().for_each(|content| content::expand(
			content, objects, builders, settings, bindings, &attrs, &field, None
		)); // TODO builder mode?
		return None
	}
	
	let gens = gens.into_iter();
	
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
			
			return Some(quote![#(#pattrs)* #(#attrs)* #(#name.)* #ident = #value;])
		},
		Expr::Invoke { back } => (quote![], back),
		Expr::Edit(..) => unreachable!(),
	};
	
	if let Some(common::Back { mut0, name: back, build, props }) = back {
		let (semi, index) = if build.is_some() {
			builders.push(TokenStream::new());
			(None, Some(builders.len() - 1))
		} else {
			(Some(<syn::Token![;]>::default()), None)
		};
		
		settings.extend(quote::quote! {
			#(#pattrs)* #(#attrs)*
			let #mut0 #back = #(#name.)* #ident #(::#gens)* (#args #assigned #rest) #semi
		});
		
		common::extend_attributes(&mut attrs, pattrs);
		props.into_iter().for_each(|keyword| content::expand(
			keyword, objects, builders, settings, bindings, &attrs, &[&back], index
		));
		
		if let Some(index) = index {
			let builder = builders.remove(index);
			settings.extend(quote::quote![#builder;])
		}
		
		return None
	} else {
		if build { return Some(quote![.#ident #(::#gens)* (#args #assigned #rest)]) }
		
		Some(quote![#(#pattrs)* #(#attrs)* #(#name.)* #ident #(::#gens)* (#args #assigned #rest);])
	}
}

pub(crate) fn expand_value<const B: bool>(
	Prop { mut attrs, ident, gens, args, rest, value }: Prop<Value<B>>,
	 objects: &mut TokenStream,
	builders: &mut Vec<TokenStream>,
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
				&name, attrs, ident, gens, args, rest, builder, false
			);
		}
		Value::ItemField(item) => {
			common::extend_attributes(&mut attrs, pattrs);
			item::expand(
				item, objects, builders, settings, bindings,
				&name, attrs, ident, gens, args, rest, builder, true
			);
		}
		Value::Expr(value) => {
			let prop = Prop { attrs, ident, gens, args, rest, value };
			
			let Some(expr) = expand_expr(
				prop, objects, builders, settings,
				bindings, pattrs, name, builder.is_some()
			) else { return };
			
			if let Some(index) = builder {
				builders[index].extend(expr);
			} else {
				settings.extend(expr);
			}
		}
	}
}
