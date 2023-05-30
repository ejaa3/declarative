/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::{Delimiter, Group, Punct, Spacing, Span, TokenStream};
use quote::{TokenStreamExt, ToTokens, quote};
use syn::spanned::Spanned;
use crate::content;

syn::custom_punctuation!(ColonEq, :=);
syn::custom_punctuation!(SemiSemi, ;;);

pub struct Prop {
	 attrs: Vec<syn::Attribute>,
	  prop: syn::TypePath,
	by_ref: Option<syn::Token![&]>,
	  mut_: Option<syn::Token![mut]>,
	  mode: Mode,
}

enum Mode {
	   Edit (Vec<content::Content>),
	  Field { span: Span, at: Option<syn::Token![@]>, value: Box<syn::Expr> },
	 Method { span: Span, args: Vec<(Option<syn::Token![@]>, syn::Expr)>, back: Option<Box<Back>> },
	FnField { span: Span, args: Vec<(Option<syn::Token![@]>, syn::Expr)>, back: Option<Box<Back>> },
}

impl crate::ParseReactive for Prop {
	fn parse(input: syn::parse::ParseStream,
	         attrs: Option<Vec<syn::Attribute>>,
	      reactive: bool,
	) -> syn::Result<Self> {
		let attrs = attrs.unwrap_or_default();
		let prop: syn::TypePath = input.parse()?;
		let at = || {
			let at = input.parse::<Option<syn::Token![@]>>()?;
			if let (Some(at), false) = (at, reactive) {
				Err(syn::Error::new(at.span, "cannot consume bindings here"))?
			}
			Ok::<_, syn::Error>(at)
		};
		
		let callable = || {
			let mut args = vec![(at()?, input.parse()?)];
			while input.parse::<syn::Token![,]>().is_ok() {
				args.push((at()?, input.parse()?));
			}
			input.parse::<Option<syn::Token![;]>>()?;
			let back = parse_back(input, reactive)?;
			Ok::<_, syn::Error>((args, back))
		};
		
		if prop.path.get_ident().is_none() || prop.qself.is_some() {
			let (by_ref, mut_) = if prop.path.segments.len() > 1 || prop.qself.is_some() {
				(input.parse()?, input.parse()?)
			} else { (None, None) };
			
			let (span, (args, back)) =
				if let Ok(colon) = input.parse::<syn::Token![:]>() {
					(colon.span, callable()?)
				} else if let Ok(semi) = input.parse::<syn::Token![;]>() {
					(semi.span, (vec![], parse_back(input, reactive)?))
				} else { Err(input.error("expected `:` or `;`"))? };
			
			let mode = Mode::Method { span, args, back };
			return Ok(Prop { attrs, prop, by_ref, mut_, mode })
		}
		
		let mode = if input.parse::<syn::Token![=>]>().is_ok() {
			if input.peek(syn::token::Brace) {
				let braces; syn::braced!(braces in input);
				let mut content = vec![]; content::parse_vec(&mut content, &braces, reactive)?;
				Mode::Edit(content)
			} else {
				Mode::Edit(vec![crate::ParseReactive::parse(input, None, reactive)?])
			}
		} else if let Ok(eq) = input.parse::<syn::Token![=]>() {
			Mode::Field { span: eq.span, at: at()?, value: input.parse()? }
		} else if let Ok(colon_eq) = input.parse::<ColonEq>() {
			let (args, back) = callable()?;
			Mode::FnField { span: colon_eq.spans[1], args, back }
		} else if let Ok(colon) = input.parse::<syn::Token![:]>() {
			let (args, back) = callable()?;
			Mode::Method { span: colon.span, args, back }
		} else if let Ok(semi) = input.parse::<SemiSemi>() {
			let back = parse_back(input, reactive)?;
			Mode::FnField { span: semi.spans[1], args: vec![], back }
		} else if let Ok(semi) = input.parse::<syn::Token![;]>() {
			let back = parse_back(input, reactive)?;
			Mode::Method { span: semi.span, args: vec![], back }
		} else { Err(input.error("expected `=>`, `=`, `:`, `:=`, `;` or `;;`"))? };
		
		Ok(Prop { attrs, prop, by_ref: None, mut_: None, mode })
	}
}

impl crate::ParseReactive for Box<Prop> {
	fn parse(input: syn::parse::ParseStream,
	         attrs: Option<Vec<syn::Attribute>>,
	      reactive: bool,
	) -> syn::Result<Self> {
		Ok(Box::new(crate::ParseReactive::parse(input, attrs, reactive)?))
	}
}

pub struct Back {
	pub   token: syn::Lifetime,
	pub    mut_: Option<syn::Token![mut]>,
	pub    back: syn::Ident,
	pub   build: Option<syn::Token![!]>,
	pub content: Vec<content::Content>,
}

impl Back {
	pub fn do_not_use(self, stream: &mut TokenStream) {
		let error = syn::Error::new(self.token.span(), "cannot use 'back in builder mode");
		stream.extend(error.into_compile_error())
	}
}

pub fn parse_back(
	input: syn::parse::ParseStream, reactive: bool,
) -> syn::Result<Option<Box<Back>>> {
	let token = if input.fork().parse::<syn::Lifetime>()
		.map(|keyword| keyword.ident == "back").unwrap_or(false) {
			input.parse::<syn::Lifetime>()?
		} else { return Ok(None) };
	
	let mut_ = input.parse()?;
	let back = input.parse()
		.unwrap_or_else(|_| syn::Ident::new(&crate::count(), input.span()));
	
	let build = input.parse()?;
	
	let braces; syn::braced!(braces in input);
	let mut content = vec![];
	
	while !braces.is_empty() {
		content.push(crate::ParseReactive::parse(&braces, None, reactive)?)
	}
	
	Ok(Some(Box::new(Back { token, mut_, back, build, content })))
}

pub(crate) fn expand_back(
	Back { token, mut_, back, build, content }: Back,
	 objects: &mut TokenStream,
	builders: &mut Vec<crate::Builder>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	   attrs: Vec<syn::Attribute>,
	   right: TokenStream,
) {
	let let_ = syn::Ident::new("let", token.span());
	let left = quote![#(#attrs)* #let_ #mut_ #back =];
	
	let index = if let Some(_build) = build {
		#[cfg(feature = "builder-mode")]
		builders.push(crate::Builder { left, right, span: _build.span(), tilde: None });
		
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
		builders.remove(index).to_tokens(settings);
		settings.append(Punct::new(';', Spacing::Alone));
	}
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expand(
	Prop { mut attrs, prop, by_ref, mut_, mode }: Prop,
	 objects: &mut TokenStream,
	builders: &mut Vec<crate::Builder>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	  pattrs: &[syn::Attribute],
	assignee: &[&syn::Ident],
	 builder: Option<usize>,
) -> Option<TokenStream> {
	if builder.is_some() {
		if !attrs.is_empty() {
			objects.extend(syn::Error::new_spanned(
				quote![#(#attrs)*], "cannot use attributes in builder mode"
			).into_compile_error());
			return None
		}
		
		let Mode::Method { span: _, args, back } = mode else {
			objects.extend(syn::Error::new_spanned(
				prop, "can only call methods in builder mode"
			).into_compile_error());
			return None
		};
		
		if prop.path.segments.len() > 1 || prop.qself.is_some() {
			objects.extend(syn::Error::new_spanned(
				prop, "cannot use long path in builder mode"
			).into_compile_error());
			return None
		}
		
		if let Some(back) = back { back.do_not_use(objects); return None }
		let args = try_bind(objects, bindings, args);
		return Some(quote![.#prop(#(#args),*)])
	}
	
	let (right, back) = match mode {
		Mode::Edit(content) => if let Some(prop) = prop.path.get_ident() {
			crate::extend_attributes(&mut attrs, pattrs);
			
			let mut field = Vec::with_capacity(assignee.len() + 1);
			field.extend_from_slice(assignee);
			field.push(prop);
			
			for content in content { content::expand(
				content, objects, builders, settings, bindings, &attrs, &field, None
			) } // TODO builder mode?
			
			return None
		} else {
			objects.extend( // NOTE this seems dead code
				syn::Error::new_spanned(prop, "must be a field name").into_compile_error()
			);
			return None
		}
		Mode::Field { span, at, mut value } => {
			let assignee = crate::span_to(assignee, span);
			if let Some(at) = at { crate::try_bind(at, objects, bindings, &mut value) }
			return Some(quote![#(#pattrs)* #(#attrs)* #(#assignee.)* #prop = #value;])
		}
		Mode::Method { span, args, back } => {
			let assignee = crate::span_to(assignee, span);
			
			if prop.path.segments.len() > 1 || prop.qself.is_some() {
				let args = try_bind(objects, bindings, args);
				(quote![#prop(#by_ref #mut_ #(#assignee).*, #(#args),*)], back)
			} else {
				let args = try_bind(objects, bindings, args);
				let mut group = Group::new(Delimiter::Parenthesis, quote![#(#args),*]);
				group.set_span(prop.span());
				(quote![#(#assignee.)* #prop #group], back)
			}
		}
		Mode::FnField { span, args, back } => {
			let assignee = crate::span_to(assignee, span);
			let mut field = Group::new(Delimiter::Parenthesis, quote![#(#assignee.)* #prop]);
			field.set_span(prop.span());
			
			let args = try_bind(objects, bindings, args);
			(quote![#field (#(#args),*)], back) // WARNING #prop must be a field name
		}
	};
	
	let Some(back_) = back else {
		return Some(quote![#(#pattrs)* #(#attrs)* #right;])
	};
	
	crate::extend_attributes(&mut attrs, pattrs);
	expand_back(*back_, objects, builders, settings, bindings, attrs, right);
	None
}

fn try_bind<'a>(
	 objects: &'a mut TokenStream,
	bindings: &'a mut crate::Bindings,
	    args: Vec<(Option<syn::Token![@]>, syn::Expr)>
) -> std::iter::Map <
	std::vec::IntoIter<(Option<syn::Token![@]>, syn::Expr)>,
	impl FnMut((Option<syn::Token![@]>, syn::Expr)) -> syn::Expr + 'a
> {
	args.into_iter().map(|(at, mut arg)| {
		if let Some(at) = at {
			crate::try_bind(at, objects, bindings, &mut arg);
		} arg
	})
}
