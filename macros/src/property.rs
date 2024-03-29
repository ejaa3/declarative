/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use proc_macro2::{Delimiter, Group, Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{punctuated::Punctuated, visit_mut::VisitMut};
use crate::{content, item, Assignee, Attributes, ConstrError, Construction, Mode};

pub struct Property {
	 attrs: Vec<syn::Attribute>,
	  path: crate::Path,
	by_ref: Option<syn::Token![&]>,
	  mut_: Option<syn::Token![mut]>,
	  mode: Mode,
	  args: Punctuated<syn::Expr, syn::Token![,]>,
	 items: Vec<item::Item>,
	  back: Option<Box<item::Back>>,
}

pub fn parse(input: syn::parse::ParseStream, attrs: Vec<syn::Attribute>) -> syn::Result<Box<Property>> {
	let rest = |callable| {
		let args = if callable { crate::parse_unterminated(input)? }
			else { Punctuated::from_iter([input.parse::<syn::Expr>()?]) };
		
		let mut items = vec![];
		while input.peek(syn::Token![@]) { items.push(item::parse(input, None)?) }
		
		let _ = items.is_empty().then(|| input.parse::<syn::Token![;]>());
		let back = if callable { item::parse_back(input)? } else { None };
		Ok::<_, syn::Error>((args, items, back))
	};
	
	let path: crate::Path = input.parse()?;
	
	let (by_ref, mut_) = if path.is_long() {
		let by_ref: Option<_> = input.parse()?;
		(by_ref, by_ref.and_then(|_| input.parse().ok()))
	} else { (None, None) };
	
	syn::custom_punctuation!(ColonEq, :=);
	syn::custom_punctuation!(SemiSemi, ;;);
	
	let (mode, (args, items, back)) = if let Ok(eq) = input.parse::<syn::Token![=]>() {
		(Mode::Field(eq.span), rest(false)?)
	} else if let Ok(colon_eq) = input.parse::<ColonEq>() {
		(Mode::FnField(colon_eq.spans[1]), rest(true)?)
	} else if let Ok(colon) = input.parse::<syn::Token![:]>() {
		(Mode::Method(colon.span), rest(true)?)
	} else if let Ok(semis) = input.parse::<SemiSemi>() {
		(Mode::FnField(semis.spans[1]), (Punctuated::new(), vec![], item::parse_back(input)?))
	} else if let Ok(semi) = input.parse::<syn::Token![;]>() {
		(Mode::Method(semi.span), (Punctuated::new(), vec![], item::parse_back(input)?))
	} else { Err(input.error("expected `=`, `:`, `:=`, `;` or `;;`"))? };
	
	Ok(Box::new(Property { attrs, path, by_ref, mut_, mode, args, items, back }))
}

fn check_property(
	attrs: Option<&[syn::Attribute]>, path: &crate::Path, mode: Mode, back: Option<Box<item::Back>>
) -> syn::Result<Span> {
	if path.is_long() { Err(syn::Error::new_spanned(path, ConstrError("cannot use long path")))? }
	
	if let Some(attrs) = attrs {
		if !attrs.is_empty() {
			Err(syn::Error::new_spanned(quote![#(#attrs)*], ConstrError("cannot use attributes")))?
		}
	} else if match path {
		crate::Path::Type(path) => path.path.get_ident().is_none(),
		crate::Path::Field { gens, .. } => gens.is_some(),
	} { Err(syn::Error::new_spanned(path, "cannot give generics to struct fields"))? }
	
	if let Some(back) = back {
		Err(syn::Error::new(back.token.span(), ConstrError("cannot use 'back")))?
	}
	match mode {
		Mode::Method(span) => Ok(span),
		Mode::Field(span) | Mode::FnField(span) =>
			Err(syn::Error::new(span, ConstrError("can only use colon or single semicolon")))
	}
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expand(
	Property { mut attrs, path, by_ref, mut_, mode, mut args, items, back }: Property,
	 objects: &mut TokenStream,
	 constrs: &mut Vec<Construction>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	  fields: &mut Option<&mut Punctuated<syn::Field, syn::Token![,]>>,
	  pattrs: crate::Attributes<&[syn::Attribute]>,
	assignee: Assignee,
	  constr: Option<usize>,
) -> syn::Result<()> {
	let no_assignee = {
		let mut items = items.iter().map(item::Item::as_assignee as _);
		let mut assignee = Some(assignee);
		
		for expr in &mut args {
			let mut visitor = crate::Visitor::Ok {
				      items: Some(&mut items),
				   assignee: &mut assignee,
				placeholder: "bindings",
				     stream: &mut bindings.stream,
			};
			visitor.visit_expr_mut(expr);
			if visitor.stream_is_empty()? { bindings.spans.clear() }
		}
		
		if let Some(item) = items.next() {
			Err(syn::Error::new_spanned(item, "an underscore is missing for this item"))?
		}
		assignee.is_none()
	};
	
	for mut item in items {
		item.set_attrs(attrs.clone());
		item::expand(item, objects, constrs, settings, bindings, fields, pattrs)?;
	}
	
	let (right, back) = 'tuple: {
		if no_assignee {
			let span = match mode {
				Mode::Method(span) => span,
				Mode::Field(span) | Mode::FnField(span) => Err(syn::Error::new(span, "use `:` instead of `=` or `:=`"))?
			};
			if let Some(mut_) = mut_ {
				Err(crate::Range(by_ref.unwrap().span, mut_.span).error("cannot use `&mut` with an extra underscore"))?
			} else if let Some(by_ref) = by_ref {
				Err(syn::Error::new(by_ref.span, "cannot use `&` with an extra underscore"))?
			} else { break 'tuple (quote_spanned![span => #path(#args)], back) }
		}
		
		macro_rules! set_field {
			($fields:ident, $span:ident) => {
				if args.len() > 1 {
					return Err(syn::Error::new_spanned(args, "cannot give multiple arguments"))
				}
				let args = args.iter();
				$fields.extend(quote_spanned![*$span => #path #(: #args)*,]);
				return Ok(())
			}
		}
		
		match constr.map(|index| &mut constrs[index]) {
			Some(Construction::BuilderPattern { right, span, .. }) => {
				check_property(Some(&attrs), &path, mode, back)?;
				right.extend(quote_spanned![*span => .#path(#args)]);
				return Ok(())
			}
			Some(Construction::StructLiteral { fields, span, .. }) => {
				check_property(None, &path, mode, back)?;
				set_field!(fields, span);
			}
			None => ()
		}
		
		match mode {
			Mode::Field(span) => {
				let (assignee, pattrs) = (assignee.spanned_to(span), pattrs.get(fields));
				settings.extend(quote_spanned![span => #(#pattrs)* #(#attrs)* #(#assignee.)* #path = #args;]);
				return Ok(())
			}
			Mode::Method(span) => {
				let assignee = assignee.spanned_to(span);
				
				if path.is_long() {
					(quote_spanned![span => #path(#by_ref #mut_ #(#assignee).*, #args)], back)
				} else {
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
				(quote_spanned![path.span() => #field (#args)], back)
			}
		}
	};
	
	let pattrs = pattrs.get(fields);
	
	let Some(back) = back else {
		return Ok(settings.extend(quote![#(#pattrs)* #(#attrs)* #right;]))
	};
	
	crate::extend_attributes(&mut attrs, pattrs);
	item::expand_back(*back, objects, constrs, settings, bindings, fields, Attributes::Some(attrs), right)
}

pub struct Edit {
	attrs: Vec<syn::Attribute>,
	 edit: Punctuated<syn::Ident, syn::Token![.]>,
	 body: Vec<content::Content>,
}

pub fn parse_edit(
	input: syn::parse::ParseStream,
	attrs: Vec<syn::Attribute>,
) -> syn::Result<Box<Edit>> {
	let edit = crate::parse_unterminated(input)?;
	let (_, body) = content::parse_vec(input)?;
	Ok(Box::new(Edit { attrs, edit, body }))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expand_edit(
	Edit { mut attrs, edit, body }: Edit,
	 objects: &mut TokenStream,
	 constrs: &mut Vec<Construction>,
	settings: &mut TokenStream,
	bindings: &mut crate::Bindings,
	  fields: &mut Option<&mut Punctuated<syn::Field, syn::Token![,]>>,
	  pattrs: crate::Attributes<&[syn::Attribute]>,
	assignee: Assignee,
) -> syn::Result<()> {
	crate::extend_attributes(&mut attrs, pattrs.get(fields));
	
	let assignee = Assignee::Field(Some(&assignee), &edit);
	settings.extend(quote![#(#attrs)* let _ = #assignee;]);
	
	for content in body { content::expand(
		content, objects, constrs, settings, bindings, fields,
		crate::Attributes::Some(&attrs), assignee, None
	)? }
	
	Ok(())
}
