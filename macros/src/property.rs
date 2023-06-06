/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::{Delimiter, Group, Punct, Spacing, TokenStream};
use quote::{TokenStreamExt, ToTokens, quote};
use syn::{punctuated::Punctuated, spanned::Spanned};
use crate::{content, Assignee, Builder, Mode};

pub struct Prop {
	 attrs: Vec<syn::Attribute>,
	  prop: crate::Path,
	by_ref: Option<syn::Token![&]>,
	  mut_: Option<syn::Token![mut]>,
	  mode: Mode,
	  args: Vec<(Option<syn::Token![@]>, syn::Expr)>,
	  back: Option<Box<Back>>,
}

impl crate::ParseReactive for Box<Prop> {
	fn parse(input: syn::parse::ParseStream,
	         attrs: Option<Vec<syn::Attribute>>,
	      reactive: bool,
	) -> syn::Result<Self> {
		let attrs = attrs.unwrap_or_default();
		let prop: crate::Path = input.parse()?;
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
		
		let (by_ref, mut_) = if prop.is_long() {
			(input.parse()?, input.parse()?)
		} else { (None, None) };
		
		syn::custom_punctuation!(ColonEq, :=);
		syn::custom_punctuation!(SemiSemi, ;;);
		
		let (mode, (args, back)) = if let Ok(eq) = input.parse::<syn::Token![=]>() {
			(Mode::Field(eq.span), (vec![(at()?, input.parse()?)], None))
		} else if let Ok(colon_eq) = input.parse::<ColonEq>() {
			(Mode::FnField(colon_eq.spans[1]), callable()?)
		} else if let Ok(colon) = input.parse::<syn::Token![:]>() {
			(Mode::Method(colon.span), callable()?)
		} else if let Ok(semi) = input.parse::<SemiSemi>() {
			(Mode::FnField(semi.spans[1]), (vec![], parse_back(input, reactive)?))
		} else if let Ok(semi) = input.parse::<syn::Token![;]>() {
			(Mode::Method(semi.span), (vec![], parse_back(input, reactive)?))
		} else { Err(input.error("expected `=>`, `=`, `:`, `:=`, `;` or `;;`"))? };
		
		Ok(Box::new(Prop { attrs, prop, by_ref, mut_, mode, args, back }))
	}
}

pub struct Back {
	pub   token: syn::Lifetime,
	pub    mut_: Option<syn::Token![mut]>,
	pub    back: syn::Ident,
	pub   build: Option<syn::Token![!]>,
	pub content: Vec<content::Content>,
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
	builders: &mut Vec<Builder>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	   attrs: Vec<syn::Attribute>,
	   right: TokenStream,
) {
	let let_ = syn::Ident::new("let", token.span());
	let left = quote![#(#attrs)* #let_ #mut_ #back =];
	
	let index = if let Some(_build) = build {
		#[cfg(feature = "builder-mode")]
		builders.push(Builder::Builder { left, right, span: _build.span, tilde: None });
		
		#[cfg(not(feature = "builder-mode"))]
		builders.push(Builder::Builder(quote![#left #right]));
		
		Some(builders.len() - 1)
	} else {
		settings.extend(quote![#left #right;]);
		None
	};
	
	for content in content { content::expand(
		content, objects, builders, settings, bindings, &attrs, Assignee::Ident(&back), index
	) }
	
	if let Some(index) = index {
		builders.remove(index).to_tokens(settings);
		settings.append(Punct::new(';', Spacing::Alone));
	}
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
		let Some(at) = at else { return arg };
		crate::try_bind(at, objects, bindings, &mut arg);
		arg
	})
}

pub(crate) fn check(
	stream: &mut TokenStream,
	 attrs: Option<&[syn::Attribute]>,
	  path: &crate::Path,
	  mode: Mode,
	 inter: bool,
	  back: Option<Box<Back>>,
) -> Result<(), ()> {
	if path.is_long() {
		Err(stream.extend(syn::Error::new_spanned(
			path, "cannot use long path in builder mode"
		).into_compile_error()))?
	}
	if let Some(attrs) = attrs {
		if !attrs.is_empty() {
			Err(stream.extend(syn::Error::new_spanned(
				quote![#(#attrs)*], "cannot use attributes for chained methods"
			).into_compile_error()))?
		}
	} else if match path {
		crate::Path::Type(path) => path.path.get_ident().is_none(),
		crate::Path::Field { gens, .. } => gens.is_some(),
	} {
		Err(stream.extend(syn::Error::new_spanned(
			path, "cannot give generics to struct fields"
		).into_compile_error()))?
	}
	if let Mode::Field(span) = mode {
		Err(stream.extend(syn::Error::new(span, match inter {
			true  => "currently only parentheses can be used",
			false => "only use a colon or a single semicolon",
		}).into_compile_error()))?
	}
	if let Some(back) = back {
		Err(stream.extend(syn::Error::new(
			back.token.span(), "cannot use 'back in builder mode"
		).into_compile_error()))?
	}
	Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expand(
	Prop { mut attrs, prop, by_ref, mut_, mode, mut args, back }: Prop,
	 objects: &mut TokenStream,
	builders: &mut Vec<Builder>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	  pattrs: &[syn::Attribute],
	assignee: Assignee,
	 builder: Option<usize>,
) {
	macro_rules! set_field {
		($fields:ident) => {
			if args.len() > 1 {
				let args = args.into_iter().map(|(_, arg)| arg);
				return objects.extend(syn::Error::new_spanned(
					quote![#(#args)*], "cannot give multiple arguments"
				).into_compile_error());
			}
			let args = try_bind(objects, bindings, args);
			$fields.get_mut().extend(quote![#prop #(: #args)*,])
		};
	}
	
	match builder.map(|index| &mut builders[index]) {
		#[cfg(not(feature = "builder-mode"))]
		Some(Builder::Builder(stream)) =>
			return if check(objects, Some(&attrs), &prop, mode, false, back).is_ok() {
				let args = try_bind(objects, bindings, args);
				return stream.extend(quote![.#prop(#(#args),*)])
			},
		
		#[cfg(feature = "builder-mode")]
		Some(Builder::Builder { right, .. }) =>
			return if check(objects, Some(&attrs), &prop, mode, false, back).is_ok() {
				let args = try_bind(objects, bindings, args);
				right.extend(quote![.#prop(#(#args),*)])
			},
		
		#[cfg(not(feature = "builder-mode"))]
		Some(Builder::Struct { ty: _, fields, call }) => return if check(
			objects, call.is_some().then_some(&attrs), &prop, mode, false, back
		).is_ok() {
			if let Some(call) = call {
				let args = try_bind(objects, bindings, args);
				call.extend(quote![.#prop(#(#args),*)])
			} else { set_field!(fields); }
		},
		
		#[cfg(feature = "builder-mode")]
		Some(Builder::Struct { fields, .. }) =>
			return if check(objects, None, &prop, mode, false, back).is_ok() {
				set_field!(fields);
			},
		
		None => ()
	}
	
	let (right, back) = match mode {
		Mode::Field(span) => {
			let (assignee, (at, value)) = (assignee.spanned_to(span), &mut args[0]);
			if let Some(at) = at { crate::try_bind(*at, objects, bindings, value) }
			return settings.extend(quote![#(#pattrs)* #(#attrs)* #(#assignee.)* #prop = #value;])
		}
		Mode::Method(span) => {
			let assignee = assignee.spanned_to(span);
			
			if prop.is_long() {
				let args = try_bind(objects, bindings, args);
				(quote![#prop(#by_ref #mut_ #(#assignee).*, #(#args),*)], back)
			} else {
				let args = try_bind(objects, bindings, args);
				let mut group = Group::new(Delimiter::Parenthesis, quote![#(#args),*]);
				group.set_span(prop.span());
				
				(quote![#(#assignee.)* #prop #group], back)
			}
		}
		Mode::FnField(span) => {
			let assignee = assignee.spanned_to(span);
			let mut field = Group::new(Delimiter::Parenthesis, quote![#(#assignee.)* #prop]);
			field.set_span(prop.span());
			
			let args = try_bind(objects, bindings, args);
			(quote![#field (#(#args),*)], back)
		}
	};
	
	let Some(back) = back else {
		return settings.extend(quote![#(#pattrs)* #(#attrs)* #right;])
	};
	
	crate::extend_attributes(&mut attrs, pattrs);
	expand_back(*back, objects, builders, settings, bindings, attrs, right)
}

pub struct Edit {
	  attrs: Vec<syn::Attribute>,
	   edit: Punctuated<syn::Ident, syn::Token![.]>,
	  arrow: syn::Token![=>],
	content: Vec<content::Content>,
}

pub fn parse_edit(
	   input: syn::parse::ParseStream,
	   attrs: Vec<syn::Attribute>,
	reactive: bool,
) -> syn::Result<Box<Edit>> {
	let edit = crate::parse_unterminated(input)?;
	let arrow = input.parse()?;
	let (_, content) = crate::parse_vec(input, reactive)?;
	Ok(Box::new(Edit { attrs, edit, arrow, content }))
}

pub(crate) fn expand_edit(
	Edit { mut attrs, edit, arrow, content }: Edit,
	 objects: &mut TokenStream,
	builders: &mut Vec<Builder>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	  pattrs: &[syn::Attribute],
	assignee: Assignee,
) {
	crate::extend_attributes(&mut attrs, pattrs);
	let punctuated;
	
	let assignee = match assignee {
		Assignee::Field(field) => {
			punctuated = Punctuated::from_iter(field.iter().cloned().chain(edit.into_iter()));
			Assignee::Field(&punctuated)
		}
		Assignee::Ident(ident) => {
			punctuated = Punctuated::from_iter(std::iter::once(ident.clone()).chain(edit.into_iter()));
			Assignee::Field(&punctuated)
		}
		Assignee::None => Assignee::Field(&edit)
	};
	
	let let_ = syn::Ident::new("let", arrow.spans[1]);
	let mut eq = Punct::new('=', Spacing::Alone);
	eq.set_span(arrow.spans[0]);
	settings.extend(quote![#(#attrs)* #let_ _ #eq #assignee;]);
	
	for content in content { content::expand(
		content, objects, builders, settings, bindings, &attrs, assignee, None
	) }
}
