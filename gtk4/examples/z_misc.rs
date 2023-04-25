/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

// this “example” shows some features that the macro supports
// (not exhaustively) but have not been properly exemplified:

#![allow(unused)]

use std::rc::Rc;

declarative::view! { // conditionals:
	gtk::Box {
		if "this".is_empty() {
			set_orientation: gtk::Orientation::Vertical
			gtk::Label { set_label: "Hello" }
		} else {
			set_orientation: gtk::Orientation::Horizontal
			gtk::Label { set_label: "World!" }
		}
		
		match "something" {
			"Hello"  => set_spacing: 10
			"World!" => gtk::Label { set_label: "Hello" }
			_ => {
				set_spacing: 10
				gtk::Label { set_label: "World!" }
			}
		}
		
		// 'bind, 'bind_only and 'bind_now allow conditionals,
		// but do not allow assigning objects or components
		//
		// 'bind only allows if without else and without inner ifs
	} ..
}

struct Type {
	closure: Box<dyn Fn() -> Option<()>>,
	closure_arg: Box<dyn Fn(Option<()>) -> Option<()>>,
	inner: Option<Box<Type>>,
	string: Rc<String>,
}

impl Default for Type {
	fn default() -> Self {
		Self {
			closure: Box::new(|| None),
			closure_arg: Box::new(|arg| arg),
			inner: Default::default(),
			string: Rc::from(String::new()),
		}
	}
}

impl Type {
	fn method(&self) -> Option<()> { None }
	fn method_with_arg(&self, arg: Option<()>) -> Option<()> { arg }
	fn method_with_generic<T: Default>(&self) -> T { T::default() }
	fn method_with_generic_and_arg<T: Default>(&self, arg: T) -> T { arg }
	fn append(&self, other: Option<&Type>) { }
}

declarative::view! {
	// a way to define a struct:
	{ Type { ..Default::default() } } mut object {
		// to assign fields use =
		inner = None
		
		method!
		// closure! // does not work
		method_with_arg: None
		// closure_with_arg: None // does not work
		method_with_generic::<i32>!
		method_with_generic_and_arg::<i32>: 32
		
		// to assign objects to fields use -> ObjectType:
		//
		// if multiple wrappers are needed, write them
		// in parentheses in inner-to-outer order:
		inner -> move Type mut 'wrap (Box::from, Some) {
			inner = None
		}
		
		// 'chain accepts methods().and_fields.2
		inner -> move Type 'wrap Box::from 'chain into() { }
		
		// you can clone without .clone()
		// String is clonable like Rc, so be explicit:
		string = 'clone second as Rc::clone(&first) second
		
		// if you need many clones:
		string = 'clone {
			 first, // as first.clone() (it is not explicit)
			second as String::clone(&first),
			 third as first.to_string(),
			triple as [first.as_str(), &second, &third].join(" ")
		} triple.into()
	}
	
	// external attributes apply internally:
	#[cfg(feature = "first")]
	Type mut first_object {
		#[cfg(feature = "second")]
		append => Type mut second_object 'wrap Some {
			#[cfg(feature = "third")]
			append: None
			
			#[cfg(feature = "bind")]
			'bind {
				append: None
				append: None
			}
			
			#[cfg(feature = "bind_now")]
			'bind_now if !"this".is_empty() { // or bind_only
				#[cfg(feature = "inner_if")]
				if false {
					#[cfg(feature = "inner_prop")]
					append: None
				}
				append: None
			} else if true {
				#[cfg(feature = "else")]
				append: None
			}
		}
		
		#[cfg(feature = "binding")]
		'binding closure: move || { bindings!(); }
	} ..
	
	fn main() {
		let first = Rc::from(String::from("Hello world!"));
		expand_view_here!();
		println!("{}", object.string)
	}
}
