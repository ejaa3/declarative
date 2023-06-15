/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::{Delimiter, Group, Punct, Spacing, Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::punctuated::Punctuated;
use crate::{content, Assignee, Attributes, Builder, Mode};

pub struct Property {
	 attrs: Vec<syn::Attribute>,
	  path: crate::Path,
	by_ref: Option<syn::Token![&]>,
	  mut_: Option<syn::Token![mut]>,
	  mode: Mode,
	  args: Punctuated<Expr, syn::Token![,]>,
	  back: Option<Box<Back>>,
}

pub struct Expr(Option<syn::Token![@]>, syn::Expr);

impl syn::parse::Parse for Expr {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		Ok(Self(input.parse()?, input.parse()?))
	}
}

impl quote::ToTokens for Expr {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		self.1.to_tokens(tokens)
	}
}

pub(crate) fn parse(
	input: syn::parse::ParseStream, attrs: Vec<syn::Attribute>, path: crate::Path
) -> syn::Result<Box<Property>> {
	let callable = || {
		let args = crate::parse_unterminated(input)?;
		let    _ = input.parse::<syn::Token![;]>();
		let back = parse_back(input)?;
		Ok::<_, syn::Error>((args, back))
	};
	
	let (by_ref, mut_) = if path.is_long() {
		(input.parse()?, input.parse()?)
	} else { (None, None) };
	
	syn::custom_punctuation!(ColonEq, :=);
	syn::custom_punctuation!(SemiSemi, ;;);
	
	let (mode, (args, back)) = if let Ok(eq) = input.parse::<syn::Token![=]>() {
		(Mode::Field(eq.span), (Punctuated::from_iter([input.parse::<Expr>()?]), None))
	} else if let Ok(colon_eq) = input.parse::<ColonEq>() {
		(Mode::FnField(colon_eq.spans[1]), callable()?)
	} else if let Ok(colon) = input.parse::<syn::Token![:]>() {
		(Mode::Method(colon.span), callable()?)
	} else if let Ok(semis) = input.parse::<SemiSemi>() {
		(Mode::FnField(semis.spans[1]), (Punctuated::new(), parse_back(input)?))
	} else if let Ok(semi) = input.parse::<syn::Token![;]>() {
		(Mode::Method(semi.span), (Punctuated::new(), parse_back(input)?))
	} else { Err(input.error("expected `=>`, `=`, `:`, `:=`, `;` or `;;`"))? };
	
	Ok(Box::new(Property { attrs, path, by_ref, mut_, mode, args, back }))
}

pub struct Back {
	token: syn::Lifetime,
	field: crate::Field,
	build: Option<syn::Token![!]>,
	 body: Vec<content::Content>,
}

pub fn parse_back(input: syn::parse::ParseStream) -> syn::Result<Option<Box<Back>>> {
	let token = if input.fork().parse::<syn::Lifetime>()
		.map(|keyword| keyword.ident == "back").unwrap_or(false) {
			input.parse::<syn::Lifetime>()?
		} else { return Ok(None) };
	
	let (mut field, build) = (crate::parse_field(None, input)?, input.parse()?);
	
	let braces;
	let brace = syn::braced!(braces in input);
	
	if field.auto { field.name.set_span(brace.span.join()) }
	
	let mut body = vec![];
	while !braces.is_empty() { body.push(braces.parse()?) }
	
	Ok(Some(Box::new(Back { token, field, build, body })))
}

fn builds(content: Option<&content::Content>) -> bool {
	content.map(|content| match content {
		content::Content::Built(built) => built.rest.is_empty(),
		
		| content::Content::Bind(_)
		| content::Content::BindColon(_)
		| content::Content::Edit(_)
		| content::Content::Extension(_)
		| content::Content::If(_)
		| content::Content::Match(_) => false,
		
		| content::Content::Binding(_)
		| content::Content::Item(_)
		| content::Content::Property(_) => true
	}).unwrap_or(true)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expand_back(
	Back { token, field, build, body }: Back,
	 objects: &mut TokenStream,
	builders: &mut Vec<Builder>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	  fields: &mut Option<&mut Punctuated<syn::Field, syn::Token![,]>>,
	   attrs: Attributes<Vec<syn::Attribute>>,
	   right: TokenStream,
) -> syn::Result<()> {
	let pattrs = attrs.get(fields);
	let let_ = syn::Ident::new("let", token.span());
	let crate::Field { vis, mut_, name, colon, ty, auto } = field;
	
	let left = if auto && build.is_some() && builds(body.first()) && builds(body.last())
		{ quote![#(#pattrs)*] } else { quote![#(#pattrs)* #let_ #mut_ #name =] };
	
	let index = if let Some(build) = build {
		#[cfg(feature = "builder-mode")]
		builders.push(Builder::Builder { left, right, span: build.span, tilde: None });
		
		#[cfg(not(feature = "builder-mode"))]
		builders.push(Builder::Builder(quote![#left #right], build.span));
		
		Some(builders.len() - 1)
	} else { settings.extend(quote![#left #right;]); None };
	
	let mut setup = TokenStream::new();
	
	for content in body { content::expand(
		content, objects, builders, &mut setup, bindings,
		fields, attrs.as_slice(), Assignee::Ident(None, &name), index
	)? }
	
	if let Some(colon) = colon {
		let fields = fields.as_deref_mut().ok_or_else(
			|| syn::Error::new_spanned(quote![#vis #colon #ty], crate::NO_FIELD)
		)?;
		
		let ty = ty.ok_or_else(|| syn::Error::new_spanned(quote![#name #colon], crate::NO_TYPE))?;
		
		let attrs = match attrs {
			Attributes::Some(attrs) => attrs,
			Attributes::None(index) => fields.iter().nth(index).unwrap().attrs.clone()
		};
		
		fields.push(syn::Field {
			attrs, vis, ty: syn::Type::Path(*ty),
			    mutability: syn::FieldMutability::None,
			         ident: Some(name.clone()),
			   colon_token: Some(colon),
		});
	}
	
	if let Some(index) = index {
		if builders.get(index).is_some() {
			builders.remove(index).extend_into(settings);
		}
	}
	settings.extend(setup); Ok(())
}

fn try_bind(args: &mut Punctuated<Expr, syn::Token![,]>,
        bindings: &mut crate::Bindings) -> syn::Result<()> {
	for Expr(at, expr) in args {
		let Some(at) = at else { continue };
		crate::try_bind(*at, bindings, expr)?
	} Ok(())
}

pub(crate) fn check(
	attrs: Option<&[syn::Attribute]>, path: &crate::Path, mode: Mode, inter: bool, back: Option<Box<Back>>
) -> syn::Result<Span> {
	if path.is_long() { Err(syn::Error::new_spanned(path, "cannot use long path in builder mode"))? }
	
	if let Some(attrs) = attrs {
		if !attrs.is_empty() {
			Err(syn::Error::new_spanned(quote![#(#attrs)*], "cannot use attributes for chained methods"))?
		}
	} else if match path {
		crate::Path::Type(path) => path.path.get_ident().is_none(),
		crate::Path::Field { gens, .. } => gens.is_some(),
	} { Err(syn::Error::new_spanned(path, "cannot give generics to struct fields"))? }
	
	if let Some(back) = back {
		Err(syn::Error::new(back.token.span(), "cannot use 'back in builder mode"))?
	}
	match mode {
		Mode::Method(span) => Ok(span),
		Mode::Field(span) | Mode::FnField(span) =>
			Err(syn::Error::new(span, match inter {
				true  => "only parentheses can be used in builder mode",
				false => "can only use colon or single semicolon in builder mode",
			}))
	}
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expand(
	Property { mut attrs, path, by_ref, mut_, mode, mut args, back }: Property,
	 objects: &mut TokenStream,
	builders: &mut Vec<Builder>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	  fields: &mut Option<&mut Punctuated<syn::Field, syn::Token![,]>>,
	  pattrs: crate::Attributes<&[syn::Attribute]>,
	assignee: Assignee,
	 builder: Option<usize>,
) -> syn::Result<()> {
	macro_rules! set_field {
		($fields:ident, $span:ident) => {
			if args.len() > 1 {
				return Err(syn::Error::new_spanned(args, "cannot give multiple arguments"))
			}
			let args = { try_bind(&mut args, bindings)?; args.iter() };
			$fields.extend(quote_spanned![*$span => #path #(: #args)*,]);
			return Ok(())
		}
	}
	
	match builder.map(|index| &mut builders[index]) {
		#[cfg(not(feature = "builder-mode"))]
		Some(Builder::Builder(stream, span)) => {
			check(Some(&attrs), &path, mode, false, back)?;
			try_bind(&mut args, bindings)?;
			stream.extend(quote_spanned![*span => .#path(#args)]);
			return Ok(())
		}
		#[cfg(feature = "builder-mode")]
		Some(Builder::Builder { right, span, .. }) => {
			check(Some(&attrs), &path, mode, false, back)?;
			try_bind(&mut args, bindings)?;
			right.extend(quote_spanned![*span => .#path(#args)]);
			return Ok(())
		}
		#[cfg(not(feature = "builder-mode"))]
		Some(Builder::Struct { ty: _, fields, call, span }) => {
			check(call.is_some().then_some(&attrs), &path, mode, false, back)?;
			
			let Some(call) = call else { set_field!(fields, span); };
			try_bind(&mut args, bindings)?;
			call.extend(quote_spanned![*span => .#path(#args)]);
			return Ok(())
		}
		#[cfg(feature = "builder-mode")]
		Some(Builder::Struct { fields, span, .. }) => {
			check(None, &path, mode, false, back)?;
			set_field!(fields, span);
		}
		None => ()
	}
	
	let pattrs = pattrs.get(fields);
	
	let (right, back) = match mode {
		Mode::Field(span) => {
			let assignee = assignee.spanned_to(span);
			try_bind(&mut args, bindings)?;
			settings.extend(quote_spanned![span => #(#pattrs)* #(#attrs)* #(#assignee.)* #path = #args;]);
			return Ok(())
		}
		Mode::Method(span) => {
			let assignee = assignee.spanned_to(span);
			
			if path.is_long() {
				try_bind(&mut args, bindings)?;
				(quote_spanned![span => #path(#by_ref #mut_ #(#assignee).*, #args)], back)
			} else {
				try_bind(&mut args, bindings)?;
				let mut args = Group::new(Delimiter::Parenthesis, quote![#args]);
				args.set_span(path.span());
				
				(quote_spanned![span => #(#assignee.)* #path #args], back)
			}
		}
		Mode::FnField(span) => {
			let assignee = assignee.spanned_to(span);
			let field = quote_spanned![span => #(#assignee.)* #path];
			let mut field = Group::new(Delimiter::Parenthesis, field);
			field.set_span(path.span());
			
			try_bind(&mut args, bindings)?;
			(quote_spanned![path.span() => #field (#args)], back)
		}
	};
	
	let Some(back) = back else {
		return Ok(settings.extend(quote![#(#pattrs)* #(#attrs)* #right;]))
	};
	
	crate::extend_attributes(&mut attrs, pattrs);
	expand_back(*back, objects, builders, settings, bindings, fields, Attributes::Some(attrs), right)
}

pub struct Edit {
	attrs: Vec<syn::Attribute>,
	 edit: Punctuated<syn::Ident, syn::Token![.]>,
	arrow: syn::Token![=>],
	 body: Vec<content::Content>,
}

pub fn parse_edit(
	input: syn::parse::ParseStream,
	attrs: Vec<syn::Attribute>,
	 edit: Punctuated<syn::Ident, syn::Token![.]>,
) -> syn::Result<Box<Edit>> {
	let arrow = input.parse()?;
	let (_, body) = content::parse_vec(input)?;
	Ok(Box::new(Edit { attrs, edit, arrow, body }))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expand_edit(
	Edit { mut attrs, edit, arrow, body }: Edit,
	 objects: &mut TokenStream,
	builders: &mut Vec<Builder>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	  fields: &mut Option<&mut Punctuated<syn::Field, syn::Token![,]>>,
	  pattrs: crate::Attributes<&[syn::Attribute]>,
	assignee: Assignee,
) -> syn::Result<()> {
	crate::extend_attributes(&mut attrs, pattrs.get(fields));
	let assignee = Assignee::Field(Some(&assignee), &edit);
	
	let let_ = syn::Ident::new("let", arrow.spans[1]);
	let mut eq = Punct::new('=', Spacing::Alone); eq.set_span(arrow.spans[0]);
	settings.extend(quote![#(#attrs)* #let_ _ #eq #assignee;]);
	
	for content in body { content::expand(
		content, objects, builders, settings, bindings, fields,
		crate::Attributes::Some(&attrs), assignee, None
	)? }
	
	Ok(())
}
