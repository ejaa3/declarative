/*
 * SPDX-FileCopyrightText: 2024 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

// this “example” shows some features that the macro supports
// (not exhaustively) but have not been properly exemplified:

#![allow(unused_doc_comments, unused_variables, dead_code)]

use declarative::{block as view, construct};
use gtk::{glib, prelude::*};

struct Besides {
	     inner: Option<Box<Besides>>,
	functional: fn() -> Option<()>,
	fn_and_arg: fn(Option<()>) -> Option<()>,
}

impl Default for Besides {
	fn default() -> Self {
		Self { inner: Default::default(), functional: || Some(()), fn_and_arg: |arg| arg }
	}
}

impl Besides {
	fn method(&self) -> Option<()> { Some(()) }
	fn method_arg(&self, arg: Option<()>) -> Option<()> { arg }
	
	fn generic_method<T: Default>(&self) -> Option<T> { Some(T::default()) }
	fn generic_method_arg<T>(&self, arg: T) -> T { arg }
	
	fn append<'a>(&self, other: Option<&'a Besides>) -> Option<&'a Besides> { other }
}

fn main() -> glib::ExitCode {
	let functional = || Some(()); // this variable will be bound to a struct field
	
	view! {
		Besides mut object {
			functional; // if no argument is given, it will bind to a previous variable with the same field name
			fn_and_arg: |arg| arg
			inner: None
			~~
			inner = None // `=` to assign fields
			
			functional;; 'back { unwrap; ~~ } // `;;` to call functional fields without arguments
			method; 'back { unwrap; ~~ }
			
			inner = Some(Box::new(_)) @ Besides { // composition with fields
				fn_and_arg: |arg| arg
				inner: Some(Box::new(_)) @ Besides { }! // composition with a struct field
				functional: || None; ~~
			}?
			
			fn_and_arg := Some(()) 'back { unwrap; ~~ } // `:=` to call functional fields with arguments
			method_arg: Some(()) 'back { unwrap; ~~ }
			
			fn_and_arg := _ @ Some(()) { unwrap; } // composition with a functional field
			
			// the following are not long paths (only generics are given to methods):
			generic_method::<()>; 'back { unwrap; ~~ }
			generic_method_arg::<Option<()>>: Some(()) 'back { unwrap; ~~ } // below is the same
			generic_method_arg: Some(()) 'back { unwrap; ~~ } // the generic was inferred
		}?
	}
	
	view! { // external attributes apply internally (doc comments are attributes):
		/** some struct */ struct Struct<'a> { }
		// struct fields inherit attributes from items
		
		/** outer object */ Besides ref outer {
			/** outer property */ append: None
			/** outer extension */ Besides::method: &_
			
			/** inner object */ append: Some(&_) @ Besides ref inner {
				/** inner property */ append: Some(&outer) 'back ref deep as Option<&'a Besides> { }!
				// we have generated another struct field with the above `'back`
				
				/** inner ref */ append: _.as_deref() @ ref outer.inner { }
				/** inner extension */ Besides::generic_method_arg::<Option<()>>: &_, Some(())
				
				/** bind colon */ 'bind: if "this".is_empty() {
					append: None 'back { }!
					/** bind colon property */ append: None
				}
				
				/** bind pound outer if */ 'bind #if "this".is_empty() {
					append: None
					
					/** bind at inner if */ if false { append: None } else {
						/** bind at inner if property */ append: None
					}
				} else if true { append: None } else {
					/** bind at outer if property */ append: None
				}
			}!
			
			/** consume */ 'consume closure = move || bindings!()
			
			/** outer if */ if "this".is_empty() {
				append: None // conditional initialization (you can also `match`)
				
				/** outer if bind at */ 'bind #append: None
				// bindings created in an inner scope can only be consumed there
				
				/** outer if property */ append: { bindings!(); None }
				
				/** inner match */ match true {
					/** inner match arm */ true => append: None
					false => /** inner match arm property */ append: None
				}
				
				/** outer if inner object */ append: Some(&_) @ Besides inner {
					/** outer if inner property */ append: None
				}!
				
				/** outer if ref */ append: _.as_deref() @ ref outer.inner { }
			}
		}!
	}
	
	let app = gtk::Application::default();
	app.connect_activate(start);
	app.run()
}

fn start(app: &gtk::Application) { // composing in a binding
	let count = std::cell::Cell::new(0);
	
	view![ gtk::ApplicationWindow {
		application: app
		title: "Besides"
		default_height: 240
		
		child: &_ @ gtk::ScrolledWindow {
			child: &_ @ gtk::Box {
				margin_bottom: 6
				margin_top: 6
				orientation: gtk::Orientation::Vertical
				
				'bind #append: &_ @ gtk::Label {
					label: glib::gformat!("Child: {}", count.get())
				}
			}
		}
		titlebar: &_ @ gtk::HeaderBar {
			pack_start: &_ @ gtk::Button::with_label("Add child") {
				connect_clicked: move |_| { count.set(count.get() + 1); bindings!(); }
			}
		}! ~
		present;
	} ];
}
