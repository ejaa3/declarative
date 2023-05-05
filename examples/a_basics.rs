/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use gtk::{glib, prelude::*};

fn main() -> glib::ExitCode {
	let greet = "Hello world!";
	
	declarative::block! { // inner syntax
		String::from(greet) var_name { } // let's call this “item”
	} // logically the block expands here
	
	println!("[INNER]\n{var_name}\n");
	
	outer();
	builder_mode()
}

#[declarative::view { // outer syntax
	// writing only the type constructs it with `Type::default()`:
	String mut my_string { // with `mut` you can mutate here
		
		// this is a method call:
		push_str: &first // `first` is another item (see below)
		// although `push_str()` is not a setter, but let's assume
		
		// this is a composition:
		String mut { // no need to name items (a name is generated)
			
			// for composition, you must #interpolate how to append it to the parent:
			#push_str(#.as_ref()) // specifically this means `#parent_method(arguments)`
			// in the `(arguments)` the first match of # will be replaced by the item name;
			// if you need a #, you can avoid the replacement by typing ## (if there was a replacement, #)
			
			push_str: "Second, "
		}
		
		// also valid to interpolate before the brace:
		String mut #push_str(&#) { push_str: "Third, " }
		
		String mut { // a full path can also be used (useful for disambiguating traits):
			String::push_str &mut: "Fourth, " // delete `&mut` and you will have a nice error
			// `&mut:` because `push_str()` requires `&mut self` as the first argument
			
			// if you delete `&mut` here you will get an error in the whole macro:
			#String::push_str &mut (&#) // you can interpolate anywhere in this scope
			// I have not been able to display the error near the parenthesis (among others)
		}
		
		// to interpolate and edit an argument or variable before view expansion, use `ref`:
		ref pre_view #push_str(&#) { push_str: "Sixth, " }
		
		// by coincidence we can do the same with `first`:
		ref first #push_str(&#) { // (this is also a composition)
			clear; // you can call a method without arguments with semicolon
			push_str: "Fifth, "
		}
	}
	
	// you can add more items here:
	String::from("First, ") mut first { }
}]

fn outer() {
	println!("[OUTER]");
	
	let mut pre_view = String::new();
	expand_view_here! { } // here we insert the string items
	my_string.push_str("End;"); // logically you can edit after view
	
	println!("{my_string}");
}

// you can use the “builder mode” with the exclamation mark before the brace,
// like this: `Type !{ }`
//
// if only a type is specified, the function `Type::default()` is assumed, but it
// is possible to change the associated function and even call a last method with
// the `builder-mode` feature, which requires a `builder_mode!` macro in the scope
//
// the `gtk-rs` feature already activates this one plus the macro to import;
// however, let's not include the macro to explain it here:

macro_rules! builder_mode {
	// when only a type is specified and the mode is
	// terminated without an auto-invoked last method (with #.)
	(.$type:ty => $($token:tt)+) => { <$type>::builder() $($token)+ };
	
	// when only a type is specified and the mode is
	// terminated with an auto-invoked last method (with #..)
	( $type:ty => $($token:tt)+) => { <$type>::builder() $($token)+.build() };
	
	// when an expression is specified and the mode is
	// terminated without an auto-invoked last method (with #.)
	(.$($expr:expr)+) => { $($expr)+ };
	
	// when an expression is specified and the mode is
	// terminated with an auto-invoked last method (with #..)
	( $($expr:expr)+) => { $($expr)+.build() };
}

// let's exemplify the second and third case:
#[declarative::view { // (the first and the last are almost the same)
	gtk::ApplicationWindow window !{ // outer builder mode (type only)
		application: app
		title: "Title"
		
		// the interpolation calls a builder method:
		gtk::HeaderBar #titlebar(&#) { }
		
		gtk::Box::builder() !{ // inner builder mode (expression)
			orientation: gtk::Orientation::Vertical
			spacing: 6
			margin_top: 6
			margin_bottom: 6
			margin_start: 6
			margin_end: 6
			
			// for ease, interpolation is not allowed after finishing builder mode
			#child(&#) // so we interpolate before
			
			build; #. // inner builder mode ends (third case)
			// gtk-rs requires calling `build()` in most of its builders
			
			gtk::Button #append(&#) !{ label: "First" } // if you do not put a #. or #.., it is as if
			gtk::Button #append(&#) !{ label: "Second" } // you had put a #.. at the end of the scope
		} #.. // outer builder mode ends (second case)
		
		present; // we show the window
	}
}]

fn builder_mode() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(|app| expand_view_here!());
	app.run()
}
