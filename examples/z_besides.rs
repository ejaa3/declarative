/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

// this “example” shows some features that the macro supports
// (not exhaustively) but have not been properly exemplified:

#![allow(unused)]

use declarative::clone;
use std::rc::Rc;

struct Besides {
	inner: Option<Box<Besides>>,
	funny: fn() -> Option<()>,
	funny_arg: fn(Option<()>) -> Option<()>,
	string: Rc<String>,
}

impl Default for Besides {
	fn default() -> Self {
		Self {
			inner: Default::default(),
			funny: || Some(()),
			funny_arg: |arg| arg,
			string: Rc::from(String::new()),
		}
	}
}

impl Besides {
	fn method(&self) -> Option<()> { Some(()) }
	fn method_arg(&self, arg: Option<()>) -> Option<()> { arg }
	fn method_generic<T: Default>(&self) -> Option<T> { Some(T::default()) }
	fn method_generic_arg<T>(&self, arg: T) -> T { arg }
	fn append(&self, other: Option<&Besides>) { }
}

#[declarative::view {
	// a way to define a struct:
	(Besides { ..Default::default() }) mut object {
		// to assign fields use =
		inner = None
		
		// to assign items to fields, interpolate with braces:
		Besides #inner { Some(#.into()) } { }
		
		// to assign items to functional fields, interpolate with brackets:
		Some(()) #funny_arg [#] { unwrap; }
		
		// ;; to call functional fields without arguments:
		funny;; 'back { unwrap; }
		method; 'back { unwrap; }
		
		// := to call functional fields with arguments:
		funny_arg := Some(()) 'back { unwrap; }
		method_arg: Some(()) 'back { unwrap; }
		
		// the following are not full paths:
		method_generic::<()>; 'back { unwrap; }
		method_generic_arg::<Option<()>>: Some(()) 'back { unwrap; }
		
		// you can clone without `.clone()`
		// `String` is clonable like `Rc`, so be explicit:
		string = clone![second as Rc::clone(&first); second]
		
		// if you need many clones:
		string = clone! {
			 first, // as `first.clone()` (it is not explicit)
			second as String::clone(&first),
			 third as first.to_string(),
			triple as [first.as_str(), &second, &third].join(" ");
			triple.into() // final value
		}
	}
	
	// external attributes apply internally (doc comments are attributes):
	/// outer object
	Besides mut outer {
		/// outer property
		append: None
		
		/// inner object
		Besides mut inner #append(Some(&#)) {
			/// inner property
			append: None
			
			/// bind colon
			'bind: if "this".is_empty() {
				append: None
				
				/// bind colon property
				append: None
			}
			
			/// bind outer if
			'bind! if "this".is_empty() {
				append: None
				
				/// bind inner if
				if false { append: None } else {
					/// bind inner if prop
					append: None
				}
			} else if true { append: None } else {
				/// bind outer if prop
				append: None
			}
		}
		
		/// binding
		'binding closure = move || bindings!()
		
		/// outer if
		if "this".is_empty() { // conditional initialization (you can also `match`)
			append: None // no reactivity possible on conditional initialization
			
			/// outer if property
			append: None
			
			/// outer if inner object
			Besides mut inner #append(Some(&#)) {
				/// outer if inner property
				append: None
			}
		}
	}
}]

fn main() {
	let first = Rc::from(String::from("Hello world!"));
	expand_view_here! { }
	println!("{}", object.string)
}
