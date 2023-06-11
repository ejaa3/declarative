/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

// this “example” shows some features that the macro supports
// (not exhaustively) but have not been properly exemplified:

#![allow(unused_doc_comments, unused_variables, dead_code)]

use declarative::{block as view, builder_mode};

struct Besides {
	inner: Option<Box<Besides>>,
	fun_1: fn() -> Option<()>,
	fun_2: fn(Option<()>) -> Option<()>,
}

impl Default for Besides {
	fn default() -> Self {
		Self { inner: Default::default(), fun_1: || Some(()), fun_2: |arg| arg }
	}
}

impl Besides {
	// you should create a function like this for components with many parameters:
	fn start(self) -> Self { self } // body and return type should be different
	// and use the parameters as struct fields (view allows to initialize fields)
	
	fn method(&self) -> Option<()> { Some(()) }
	fn method_arg(&self, arg: Option<()>) -> Option<()> { arg }
	
	fn generic_method<T: Default>(&self) -> Option<T> { Some(T::default()) }
	fn generic_method_arg<T>(&self, arg: T) -> T { arg }
	
	fn append<'a>(&self, other: Option<&'a Besides>) -> Option<&'a Besides> { other }
}

fn main() {
	let fun_1 = || Some(()); // this variable will be bound to a struct field
	
	view! { // a way to define a struct, useful for starting components with many parameters:
		Besides mut object ~{ // tilde before the brace (works similar to using `!`)
			fun_1; // if no argument is given, it will bind to a previous variable with the same field name
			fun_2: |arg| arg
			~inner: None // `start()` method is chained due to the `builder_mode!` macro implementation
			// if the mode was not explicitly terminated (with tilde), it would also be chained
			// if the `builder-mode` feature were inactive, the above would be equivalent to adding `~start;`
			// in both cases the double tilde allows defining structures without chained methods
			
			// use = to assign fields
			inner = None
			
			// to assign items to fields, #interpolate with braces:
			Besides #inner { Some(Box::new(#)) } ~{
				fun_1: || None
				fun_2: |arg| arg
				
				Besides #inner(Some(Box::new(#))) { } // we have #interpolated into a structure field
			}
			
			// to assign items to functional fields, interpolate with brackets:
			Some(()) #fun_2[#] { unwrap; }
			
			// use ;; to call functional fields without arguments:
			fun_1;; 'back !{ ~~unwrap; }
			method; 'back !{ ~~unwrap; }
			
			// use := to call functional fields with arguments:
			fun_2 := Some(()) 'back !{ ~~unwrap; }
			method_arg: Some(()) 'back !{ ~~unwrap; }
			
			// the following is not a long path (only generics are given to method):
			generic_method::<()>; 'back !{ ~~unwrap; }
			generic_method_arg::<Option<()>>: Some(()) 'back !{ ~~unwrap; } // below is the same
			generic_method_arg: Some(()) 'back !{ ~~unwrap; } // the generic was inferred
		}
	}
	
	view! { // external attributes apply internally (doc comments are attributes):
		/// some struct
		struct Struct<'a> { } // struct fields inherit attributes from items
		
		/// outer object
		Besides outer: {
			/// outer property
			append: None
			
			/// outer extension
			@Besides::method(&#)
			
			/// inner object
			Besides inner: #append(Some(&#)) {
				/// inner property
				append: Some(&outer) 'back deep: Option<&'a Besides> { }
				// we have generated another struct field with the above `'back`
				
				/// inner ref
				ref outer.inner #append(#.as_deref()) { }
				
				/// inner extension
				@Besides::generic_method_arg::<Option<()>>(&#, Some(()))
				
				/// bind colon
				'bind: if "this".is_empty() {
					append: None 'back { }
					
					/// bind colon property
					append: None
				}
				
				/// bind at outer if
				'bind @if "this".is_empty() {
					append: None
					
					/// bind at inner if
					if false { append: None } else {
						/// bind at inner if property
						append: None
					}
				} else if true { append: None } else {
					/// bind at outer if property
					append: None
				}
			}
			
			/// bindings
			@closure = move || bindings!()
			
			/// outer if
			if "this".is_empty() { // conditional initialization (you can also `match`)
				append: None
				
				/// outer if bind at
				'bind @append: None // bindings created in an `if` or `match`...
				// scope can only be consumed there, not even in an inner scope
				
				/// outer if property
				append: @{ bindings!(); None }
				
				/// inner match
				match true {
					/// inner match arm
					true  => append: None
					false => /// inner match arm property
						append: None
				}
				
				/// outer if inner object
				Besides inner #append(Some(&#)) {
					/// outer if inner property
					append: None
				}
				
				/// outer if ref
				ref outer.inner #append(#.as_deref()) { }
			}
		}
	}
}
