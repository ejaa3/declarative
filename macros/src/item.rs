/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::{Delimiter, Group, Punct, Spacing, Span, TokenStream};
use quote::{quote, ToTokens};
use syn::spanned::Spanned;
use crate::{content, property};

pub struct Item {
	  attrs: Vec<syn::Attribute>,
	 object: Object,
	  spans: [Span; 3],
	  inter: Option<Box<Inter>>,
	  build: Option<syn::Token![!]>,
	content: Vec<content::Content>,
}

pub fn parse(
	   input: syn::parse::ParseStream,
	   attrs: Vec<syn::Attribute>,
	reactive: bool,
	    root: bool,
) -> syn::Result<Item> {
	let mut spans = [input.span(), Span::call_site(), Span::call_site()];
	let object = input.parse()?;
	let pound = input.parse::<syn::Token![#]>();
	let name = Name::from(&object);
	
	if let Ok(pound) = pound {
		if root { Err(syn::Error::new(pound.span, "cannot #interpolate here"))? }
		spans[2] = pound.span;
	}
	
	spans[1] = input.span();
	let mut inter = pound.is_ok().then(|| parse_inter(input, &name, reactive)).transpose()?;
	let body = inter.as_ref().map(|inter| inter.back.is_none()).unwrap_or(true);
	let build = if body { input.parse()? } else { None };
	let mut content = vec![];
	
	if body {
		let braces; syn::braced!(braces in input);
		
		while !braces.is_empty() {
			if inter.is_none() {
				if braces.peek(syn::Token![#]) && (
					braces.peek2(syn::Ident) || braces.peek2(syn::Token![<])
				) {
					let pound = braces.parse::<syn::Token![#]>()?;
					if root { Err(syn::Error::new(pound.span, "cannot #interpolate here"))? }
					spans[2] = pound.span;
					inter = Some(parse_inter(&braces, &name, reactive)?);
					continue
				}
				content.push(crate::ParseReactive::parse(&braces, None, reactive)?)
			} else { content::parse_vec(&mut content, &braces, reactive)? }
		}
	}
	Ok(Item { attrs, object, spans, inter, build, content })
}

enum Object {
	Expr (Box<syn::Expr>, Option<syn::Token![mut]>, syn::Ident),
	Type (Box<syn::TypePath>, Option<syn::Token![mut]>, syn::Ident),
	 Ref (Vec<syn::Ident>),
}

impl ToTokens for Object {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		match self {
			Object::Expr(expr, mut_, name) => {
				expr.to_tokens(tokens);
				mut_.to_tokens(tokens);
				name.to_tokens(tokens);
			}
			Object::Type(ty, mut_, name) => {
				  ty.to_tokens(tokens);
				mut_.to_tokens(tokens);
				name.to_tokens(tokens);
			}
			Object::Ref(idents) => for ident in idents { ident.to_tokens(tokens) }
		}
	}
}

impl syn::parse::Parse for Object {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		if input.parse::<syn::Token![ref]>().is_ok() {
			let mut idents = vec![input.parse()?];
			while input.parse::<syn::Token![.]>().is_ok() {
				idents.push(input.parse()?);
			}
			return Ok(Object::Ref(idents))
		}
		
		let ahead = input.fork();
		
		Ok(if ahead.parse::<syn::TypePath>().is_ok() && (
			   ahead.peek(syn::Token![mut])
			|| ahead.peek(syn::Ident)
			|| ahead.peek(syn::Token![#])
			|| ahead.peek(syn::Token![!])
			|| ahead.peek(syn::token::Brace)
		) {
			Object::Type(input.parse()?, input.parse()?, input.parse::<Option<_>>()?
				.unwrap_or_else(|| syn::Ident::new(&crate::count(), Span::call_site())))
		} else {
			Object::Expr(input.parse()?, input.parse()?, input.parse::<Option<_>>()?
				.unwrap_or_else(|| syn::Ident::new(&crate::count(), Span::call_site())))
		})
	}
}

fn expand_object(
	  object: &Object,
	 objects: &mut TokenStream,
	builders: &mut Vec<crate::Builder>,
	   attrs: &[syn::Attribute],
	    let_: syn::Ident,
	 builder: Option<syn::Token![!]>,
) -> Option<usize> {
	let Some(_builder) = builder else {
		objects.extend(match object {
			Object::Type(path, mut_, name) => quote![#(#attrs)* #let_ #mut_ #name = #path::default();],
			Object::Expr(expr, mut_, name) => quote![#(#attrs)* #let_ #mut_ #name = #expr;],
			Object::Ref(_) => return None,
		});
		return None
	};
	
	builders.push(match object {
		#[cfg(feature = "builder-mode")]
		Object::Type(ty, mut_, name) => crate::Builder {
			 left: quote![#(#attrs)* #let_ #mut_ #name =],
			right: quote![#ty =>],
			 span: _builder.span,
			tilde: None,
		},
		#[cfg(not(feature = "builder-mode"))]
		Object::Type(ty, mut_, name) => quote![#(#attrs)* #let_ #mut_ #name = #ty::default()],
		
		#[cfg(feature = "builder-mode")]
		Object::Expr(expr, mut_, name) => crate::Builder {
			 left: quote![#(#attrs)* #let_ #mut_ #name =],
			right: quote![#expr],
			 span: _builder.span,
			tilde: None,
		},
		#[cfg(not(feature = "builder-mode"))]
		Object::Expr(expr, mut_, name) => quote![#(#attrs)* #let_ #mut_ #name = #expr],
		
		Object::Ref(_) => return None,
	});
	
	Some(builders.len() - 1)
}

struct Inter {
	 inter: syn::TypePath,
	by_ref: Option<syn::Token![&]>,
	  mut_: Option<syn::Token![mut]>,
	  mode: InterMode,
	tokens: TokenStream,
	  back: Option<Box<property::Back>>,
}

enum InterMode {
	  Field (syn::token::Brace),
	FnField (syn::token::Bracket),
	 Method (syn::token::Paren),
}

fn parse_inter(
	   input: syn::parse::ParseStream,
	    name: &[&syn::Ident],
	reactive: bool,
) -> syn::Result<Box<Inter>> {
	let inter: syn::TypePath = input.parse()?;
	let buffer;
	
	let (by_ref, mut_) = if inter.path.segments.len() > 1 || inter.qself.is_some() {
		(input.parse()?, input.parse()?)
	} else { (None, None) };
	
	let mode = if input.peek(syn::token::Paren) {
		InterMode::Method(syn::parenthesized!(buffer in input))
	} else if input.peek(syn::token::Brace) {
		InterMode::Field(syn::braced!(buffer in input))
	} else {
		InterMode::FnField(syn::bracketed!(buffer in input))
	};
	
	let tokens = buffer.step(|cursor| {
		let mut rest = *cursor;
		let mut stream = TokenStream::new();
		
		crate::find_pound(&mut rest, &mut stream, name)
			.then(|| (stream, syn::buffer::Cursor::empty()))
			.ok_or_else(|| cursor.error("no single `#` found around here"))
	})?;
	
	let back = if let InterMode::FnField(..) | InterMode::Method(..) = &mode {
		property::parse_back(input, reactive)?
	} else { None };
	
	Ok(Box::new(Inter { inter, by_ref, mut_, mode, tokens, back }))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expand(
	Item { mut attrs, object, spans, inter, build, content }: Item,
	 objects: &mut TokenStream,
	builders: &mut Vec<crate::Builder>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	  pattrs: &[syn::Attribute],
	assignee: Option<&[&syn::Ident]>,
	 builder: Option<usize>,
) {
	let let_ = syn::Ident::new("let", spans[2]);
	crate::extend_attributes(&mut attrs, pattrs);
	
	if let Object::Ref(ref idents) = object {
		settings.extend(quote![#(#attrs)* #let_ _ = #(#idents).*;])
	}
	
	let new_builder = expand_object(&object, objects, builders, &attrs, let_, build);
	
	for content in content { content::expand(
		content, objects, builders, settings, bindings, &attrs, &Name::from(&object), new_builder
	) }
	
	let Some(assignee) = assignee else { return };
	
	let Some(inter) = inter else {
		let mut start = Punct::new('<', Spacing::Alone); start.set_span(spans[0]);
		let mut   end = Punct::new('>', Spacing::Alone);   end.set_span(spans[1]);
		let error = syn::Error::new_spanned(quote![#start #end], "missing #interpolation").into_compile_error();
		return objects.extend(error)
	};
	
	let Inter { inter, by_ref, mut_, mode, tokens, back } = *inter;
	
	if let Some(index) = builder {
		if let Some(back) = back { return back.do_not_use(objects) }
		
		if inter.path.segments.len() == 1 || inter.qself.is_none() {
			let group = Group::new(Delimiter::Parenthesis, tokens);
			
			#[cfg(feature = "builder-mode")]
			builders[index].right.extend(quote![.#inter #group]);
			
			#[cfg(not(feature = "builder-mode"))]
			builders[index].extend(quote![.#inter #group]);
		} else {
			objects.extend(syn::Error::new_spanned(
				inter, "cannot use long path in builder mode"
			).into_compile_error());
		}
		return
	}
	
	let right = match mode {
		InterMode::Field(brace) => {
			let assignee = crate::span_to(assignee, brace.span.join());
			
			let mut group = Group::new(Delimiter::Brace, tokens);
			group.set_span(brace.span.join());
			
			if back.is_some() { quote![#(#assignee.)* #inter = #group] }
			else { quote![#(#attrs)* #(#assignee.)* #inter = #group;] }
		}
		InterMode::FnField (bracket) => {
			let assignee = crate::span_to(assignee, bracket.span.join());
			
			let mut field = Group::new(Delimiter::Parenthesis, quote![#(#assignee.)* #inter]);
			field.set_span(inter.span());
			
			let mut group = Group::new(Delimiter::Parenthesis, tokens);
			group.set_span(bracket.span.join());
			
			if back.is_some() { quote![#field #group] }
			else { quote![#(#attrs)* #field #group;] }
		}
		InterMode::Method(paren) => {
			let assignee = crate::span_to(assignee, paren.span.join());
			
			if inter.path.segments.len() > 1 || inter.qself.is_some() {
				let group = quote![#by_ref #mut_ #(#assignee).*, #tokens];
				
				let mut group = Group::new(Delimiter::Parenthesis, group);
				group.set_span(paren.span.join());
				
				if back.is_some() { quote![#inter #group] }
				else { quote![#(#attrs)* #inter #group;] }
			} else {
				let mut group = Group::new(Delimiter::Parenthesis, tokens);
				group.set_span(paren.span.join());
				
				if back.is_some() { quote![#(#assignee.)* #inter #group] }
				else { quote![#(#attrs)* #(#assignee.)* #inter #group;] }
			}
		}
	};
	
	if let Some(back) = back {
		property::expand_back(*back, objects, builders, settings, bindings, attrs, right)
	} else { settings.extend(right); }
}

enum Name<'a> { Slice([&'a syn::Ident; 1]), Vec(Vec<&'a syn::Ident>) }

impl<'a> From <&'a Object> for Name<'a> {
	fn from(object: &'a Object) -> Self {
		match &object {
			Object::Expr(.., name) => Name::Slice([name]),
			Object::Type(.., name) => Name::Slice([name]),
			Object::Ref(ref_) => Name::Vec(ref_.iter().collect()),
		}
	}
}

impl<'a> std::ops::Deref for Name<'a> {
	type Target = [&'a syn::Ident];
	
	fn deref(&self) -> &Self::Target {
		match self {
			Name::Slice(slice) => slice,
			Name::Vec(vec) => vec.as_slice(),
		}
	}
}
