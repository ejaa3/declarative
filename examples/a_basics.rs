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

fn add_five(string: &mut String) {
	declarative::block! {
		// to edit an argument or variable before expansion, use `ref`:
		ref string { push_str: "5, " }
	}
}

#[declarative::view { // outer syntax
	// writing only the type constructs it with `Type::default()`:
	String mut main_string { // with `mut` you can mutate here
		
		// this is a method call:
		push_str: &first // `first` is another item (see below)
		// although `push_str()` is not a setter, but let's assume
		
		// this is a composition:
		String mut { // no need to name items (a name is generated)
			
			// for composition, you must #interpolate how to append it to the parent:
			#push_str(#.as_ref()) // specifically this means `#parent_method(arguments)`
			// in the `(arguments)` the first match of # will be replaced by the item name;
			// if you need a #, you can avoid the replacement by typing ## (if there was a replacement, #)
			
			push_str: "2, "
		}
		
		// also valid to interpolate before the brace:
		String mut #push_str(&#) { push_str: "3, " }
		
		String mut { // a full path can also be used (useful for disambiguating traits):
			String::push_str &mut: "4, " // delete `&mut` and you will have a nice error
			// `&mut:` because `push_str()` requires `&mut self` as the first argument
			
			// if you delete `&mut` here, you will get an error on the first token inside the parentheses:
			#String::push_str &mut (&#) // you can interpolate anywhere in this scope
			// I have not been able to display the error in the parentheses
		}
		
		// @extensions are useful for sharing a view edit:
		@add_five(&mut #) // remember the `add_five()` function, after `main()`
		// unlike an interpolation, it cannot go before a brace
		
		// you can also compose with `ref`:
		ref pre_view #push_str(&#) { push_str: "6, " }
		
		// by coincidence we can use `ref` with `first` although
		// the real purpose of `ref` is what lines 24 and 64 say:
		ref first #push_str(&#) {
			clear; // you can call a method without arguments with semicolon
			push_str: "7, "
		}
	}
	
	// you can add more items here:
	String::from("1, ") mut first { }
}]

#[declarative::view { // a second view
	String::from("9, ") mut end {
		push_str: {
			// the pseudo-macro `expand_view_here!` can only consume one view:
			expand_view_here! { } // here the third view is expanded (reason below)
			ten
		}
	}
}]

#[declarative::view { // a third view
	str::as_ref("10") ten { } // could be "10" without `str::as_ref()`
}]

fn outer() {
	println!("[OUTER]");
	
	let mut pre_view = String::new();
	expand_view_here! { } // here we put the string items of the first view
	main_string.push_str("8,"); // logically you can edit after view
	
	// here we expand the second view, which also expands and consumes the...
	expand_view_here! { } // third view due to an internal `expand_view_here!`
	// the first view is consumed first, and the last is consumed last
	
	println!("{main_string} {end}");
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
	// terminated without an auto-invoked last method (with #; or @;)
	(;$type:ty => $($token:tt)*) => { <$type>::builder() $($token)* };
	
	// when only a type is specified and the mode is
	// terminated with an auto-invoked last method (with #: or @:)
	( $type:ty => $($token:tt)*) => { <$type>::builder() $($token)*.build() };
	
	// when an expression is specified and the mode is
	// terminated without an auto-invoked last method (with #; or @;)
	(;$expr:expr) => { $expr };
	
	// when an expression is specified and the mode is
	// terminated with an auto-invoked last method (with #: or @:)
	( $expr:expr) => { $expr.build() };
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
			
			build; #; // inner builder mode ends (third case)
			// gtk-rs requires calling `build()` in most of its builders
			
			gtk::Button #append(&#) !{ label: "First" } // if you do not put a `#:` or `#;`, it is as if
			gtk::Button #append(&#) !{ label: "Second" } // you had put a `#:` at the end of the scope
		} #: // outer builder mode ends (second case)
		
		present; // we show the window
	}
}]

fn builder_mode() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(|app| expand_view_here!());
	app.run()
}

// builder mode expands inner items before outer items (the placement is reversed)
//
// if you want to place an outer item before the inner ones, you must
// end the mode with `@:` or `@;` instead of `#:` or `#;` respectively
//
// this would result in not being able to interpolate child items with builder methods of the parent
