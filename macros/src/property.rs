/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::{Span, TokenStream};
use quote::quote;
use crate::content;

pub(crate) struct Prop {
	 attrs: Vec<syn::Attribute>,
	  prop: syn::Path,
	by_ref: Option<syn::Token![&]>,
	  mut0: Option<syn::Token![mut]>,
	  mode: Mode,
}

enum Mode {
	Edit    (Vec<content::Content>),
	Field   (Box<syn::Expr>),
	Method  { span: Span, args: Vec<syn::Expr>, back: Option<Box<Back>> },
	Closure { args: Vec<syn::Expr>, back: Option<Box<Back>> },
}

impl crate::ParseReactive for Prop {
	fn parse(input: syn::parse::ParseStream,
	         attrs: Option<Vec<syn::Attribute>>,
	      reactive: bool,
	) -> syn::Result<Prop> {
		let attrs = attrs.unwrap_or_default();
		let prop: syn::Path = input.parse()?;
		
		let invokable = || {
			let mut args = vec![input.parse()?];
			while input.parse::<syn::Token![,]>().is_ok() {
				args.push(input.parse()?);
			}
			input.parse::<Option<syn::Token![;]>>()?;
			let back = parse_back(input, reactive)?;
			Ok::<_, syn::Error>((args, back))
		};
		
		if prop.get_ident().is_none() {
			let (by_ref, mut0) = if prop.segments.len() == 1 {
				(None, None)
			} else {
				(input.parse()?, input.parse()?)
			};
			
			let (span, (args, back)) =
				if let Ok(colon) = input.parse::<syn::Token![:]>() {
					(colon.span, invokable()?)
				} else if let Ok(semi) = input.parse::<syn::Token![;]>() {
					(semi.span, (vec![], parse_back(input, reactive)?))
				} else { Err(input.error("expected `:` or `;`"))? };
			
			let mode = Mode::Method { span, args, back };
			return Ok(Prop { attrs, prop, by_ref, mut0, mode })
		}
		
		let mode = if input.parse::<syn::Token![=>]>().is_ok() {
			let braces; syn::braced!(braces in input);
			Mode::Edit(content::parse_vec(&braces, reactive)?)
		} else if input.parse::<syn::Token![=]>().is_ok() {
			Mode::Field(input.parse()?)
		} else if let Ok(colon) = input.parse::<syn::Token![:]>() {
			if input.parse::<syn::Token![=]>().is_ok() {
				let (args, back) = invokable()?;
				Mode::Closure { args, back }
			} else {
				let (args, back) = invokable()?;
				Mode::Method { span: colon.span, args, back }
			}
		} else if let Ok(semi) = input.parse::<syn::Token![;]>() {
			if input.parse::<syn::Token![;]>().is_ok() {
				let back = parse_back(input, reactive)?;
				Mode::Closure { args: vec![], back }
			} else {
				let back = parse_back(input, reactive)?;
				Mode::Method { span: semi.span, args: vec![], back }
			}
		} else { Err(input.error("expected `=>`, `=`, `:`, `:=`, `;` or `;;`"))? };
		
		Ok(Prop { attrs, prop, by_ref: None, mut0: None, mode })
	}
}

pub(crate) struct Back {
	pub   token: syn::Lifetime,
	pub    mut0: Option<syn::Token![mut]>,
	pub    back: syn::Ident,
	pub   build: Option<syn::Token![!]>,
	pub content: Vec<content::Content>,
}

impl Back {
	pub(crate) fn do_not_use(self, stream: &mut TokenStream) {
		let error = syn::Error::new(self.token.span(), "cannot use 'back in builder mode");
		stream.extend(error.into_compile_error())
	}
}

pub(crate) fn parse_back(
	input: syn::parse::ParseStream, reactive: bool,
) -> syn::Result<Option<Box<Back>>> {
	let token = if input.fork().parse::<syn::Lifetime>()
		.map(|keyword| keyword.ident == "back").unwrap_or(false) {
			input.parse::<syn::Lifetime>()?
		} else { return Ok(None) };
	
	let mut0 = input.parse()?;
	let back = input.parse()
		.unwrap_or_else(|_| syn::Ident::new(&crate::count(), input.span()));
	
	let build = input.parse()?;
	
	let braces; syn::braced!(braces in input);
	let mut content = vec![];
	
	while !braces.is_empty() {
		content.push(crate::ParseReactive::parse(&braces, None, reactive)?)
	}
	
	Ok(Some(Back { token, mut0, back, build, content }.into()))
}

pub(crate) fn expand_back(
	Back { token: _, mut0, back, build, content }: Back,
	 objects: &mut TokenStream,
	builders: &mut Vec<crate::Builder>,
	settings: &mut TokenStream,
	bindings: &mut TokenStream,
	   attrs: Vec<syn::Attribute>,
	   right: TokenStream,
) {
	let left = quote![#(#attrs)* let #mut0 #back =];
	
	let index = if build.is_some() {
		#[cfg(feature = "builder-mode")]
		builders.push(crate::Builder(left, right, None));
		
		#[cfg(not(feature = "builder-mode"))]
		builders.push(quote!(#left #right));
		
		Some(builders.len() - 1)
	} else {
		settings.extend(quote!(#left #right;));
		None
	};
	
	for content in content { content::expand(
		content, objects, builders, settings, bindings, &attrs, &[&back], index
	) }
	
	if let Some(index) = index {
		let builder = builders.remove(index);
		settings.extend(quote![#builder;])
	}
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expand(
	Prop { mut attrs, prop, by_ref, mut0, mode }: Prop,
	 objects: &mut TokenStream,
	builders: &mut Vec<crate::Builder>,
	settings: &mut TokenStream,
	bindings: &mut TokenStream,
	  pattrs: &[syn::Attribute],
	assignee: &[&syn::Ident],
	   build: Option<usize>,
) -> Option<TokenStream> {
	if build.is_some() {
		let Mode::Method { span: _, args, back } = mode else {
			objects.extend(syn::Error::new_spanned(
				prop, "can only call methods in builder mode"
			).into_compile_error());
			
			return None
		};
		
		if prop.segments.len() > 1 {
			objects.extend(syn::Error::new_spanned(
				prop, "cannot use long path in builder mode"
			).into_compile_error());
			
			return None
		}
		
		if let Some(back) = back { back.do_not_use(objects); return None }
		return Some(quote![.#prop(#(#args),*)])
	}
	
	let (right, back) = match mode {
		Mode::Edit(content) => if let Some(prop) = prop.get_ident() {
			crate::extend_attributes(&mut attrs, pattrs);
			
			let mut field = Vec::with_capacity(assignee.len() + 1);
			field.extend_from_slice(assignee);
			field.push(prop);
			
			for content in content { content::expand(
				content, objects, builders, settings, bindings, &attrs, &field, None
			) } // TODO builder mode?
			
			return None
		} else {
			objects.extend(
				syn::Error::new_spanned(prop, "must be a field name").into_compile_error()
			);
			return None
		}
		Mode::Field(value) => {
			return Some(quote![#(#pattrs)* #(#attrs)* #(#assignee.)* #prop = #value;])
		},
		Mode::Method { span, args, back } => if prop.segments.len() == 1 {
			(quote![#(#assignee.)* #prop(#(#args),*)], back)
		} else {
			let assignee = crate::span_to(assignee, span);
			(quote![#prop(#by_ref #mut0 #(#assignee).*, #(#args),*)], back)
		}
		Mode::Closure { args, back } => (quote![(#(#assignee.)* #prop) (#(#args),*)], back), // WARNING #prop must be a field name
	};
	
	let Some(back0) = back else {
		return quote![#(#pattrs)* #(#attrs)* #right;].into()
	};
	
	crate::extend_attributes(&mut attrs, pattrs);
	expand_back(*back0, objects, builders, settings, bindings, attrs, right);
	None
}
