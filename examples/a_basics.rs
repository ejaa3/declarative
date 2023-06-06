/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

fn main() -> glib::ExitCode {
	let greet = "Hello world!";
	
	// use `block!` to create a simple inner view:
	declarative::block! {
		// type an expression followed by a variable name to create one (braces required)
		String::from(greet) main_string { } // let's call this “item”
	} // logically the view block expands here
	
	println!("[block]\n{main_string}\n");
	
	// you might like to rename `block!` to `view!` like so:
	// use declarative::block as view;
	
	view_attributes_example();
	some_module::example();
	builder_mode_example()
}

use declarative::view;

// you can use the #[view] attribute to separate the view from the logic
// (the attributed function); its content is the same as `block!`:
#[view {
	// writing a type instead of an expression will construct it as `Type::default()`:
	String mut main_string { // with `mut` you can mutate here
		push_str: "Hello " // this is a method call with an argument
	}
}]

// a second view (you can use the attribute multiple times):
#[view(ref main_string { // this (`ref something`) is also an “item”
	// we keep editing `main_string` from the view above because of the `ref` keyword
	// `ref` is useful for editing arguments or variables before the expansion of this item or view
	
	push_str: &{
		// the `expand_view_here!` placeholder macro can only consume one view:
		expand_view_here! { } // here the third view is expanded
		world_string // the function will explain how views are expanded
	}
})]

// Rust allows to delimit the content of an attribute between
// parentheses, brackets or braces just like a functional macro:
#[view[ String::from("world!") world_string { } ]]
// so don't think that previous views work differently than this one

fn view_attributes_example() {
	// first we expand the first view:
	expand_view_here! { } // `main_string` is created
	
	// now we expand the second view, which also consumes and expands the...
	expand_view_here! { } // third view due to an internal `expand_view_here!`
	// the first view is expanded first, and the last is expanded last (FIFO)
	
	println!("[view_attributes]\n{main_string}\n");
}

#[view] // can also be used without content for a `mod`, `impl` or `trait`
// views are now written as functional macros and can be placed after the function where they are expanded
// views are expanded in FIFO order with the same `expand_view_here!` placeholder macro
// views cannot be expanded inside other views with `expand_view_here!` for technical reasons
mod some_module {
	pub fn example() {
		println!("[view_module]");
		
		let mut string_before_view = String::new();
		expand_view_here! { } // first `view!` will expand here
		main_string.push_str("9"); // logically you can edit after view
		
		println!("{main_string}");
	}
	
	// this function is a “chunk” of view (let's call it "extension"):
	fn add_five_and_six(string: &mut String) {
		expand_view_here! { } // last `view!` will expand here
	} // we will use it in the following view:
	
	view! {
		String mut main_string {
			push_str: &first // `first` is another item at the end of this view
			
			// this is a composition:
			String mut { // no need to name items (a name is generated)
				
				// for composition, you must #interpolate how to append it to the parent:
				#push_str(#.as_ref()) // specifically this means `#parent_method(arguments)`
				// in the `(arguments)` the first match of # will be replaced by the item name;
				// if you need a #, you can avoid the replacement by typing ## (if there was a replacement, #)
				
				push_str: "2, "
			} // at this point the interpolation takes effect
			
			// also valid to interpolate before the brace:
			String mut #push_str(&#) { push_str: "3, " }
			
			String mut { // a full path can also be used (useful for disambiguating traits):
				String::push_str &mut: "4, "
				// `&mut:` because `push_str()` requires `&mut self` as the first argument
				
				#String::push_str &mut (&#) // you can interpolate anywhere in this scope
			}
			
			// `@extensions(#)` are useful for sharing a view edit; we are extending `main_string`:
			@add_five_and_six(&mut #) // remember the `add_five_and_six()` function above
			// unlike an #interpolation, it cannot go before a brace
			
			// you can also compose with `ref`:
			ref string_before_view #push_str(&#) { push_str: "7, " }
			
			// we can use `ref` with `first` because items are placed at the beginning...
			ref first #push_str(&#) { // and their content at the end at expansion time
				clear; // you can call a method without arguments with semicolon
				push_str: "8, "
			}
		}
		
		String::from("1, ") mut first { } // you can add more items here
		// we were able to use this item even though it was declared last in the view
	}
	
	view!(ref string { // the view of `add_five_and_six()` function
		push_str: "5, "
		str::as_ref("6, ") { #push_str(#) }
	});
}

// declarative allows method calls `to().be().chained()` with the exclamation mark before the
// brace, like so: `Type !{ }` or `expression() !{ }` (really only type paths are supported)
//
// if only a type is specified, the function `Type::default()` is assumed, but it is
// possible to change the associated function and even auto-chain a last method with
// the `builder-mode` feature, which requires a `builder_mode!` macro in the scope
//
// the `gtk-rs` feature already activates this one plus a macro to import;
// however, let's not include the macro to explain it here:

macro_rules! builder_mode {
	// 1) when an expression is specified and the mode is terminated
	//    without an auto-chained last method (with ~~ or ~~/)
	(~$expr:expr) => { $expr };
	
	// 2) the above but with an auto-chained last method (with ~ or ~/)
	( $expr:expr) => { $expr.build() };
	
	// 3) when a type is specified and the mode is terminated
	//    without an auto-chained last method (with ~~ or ~~/)
	(~$type:ty => $($methods:tt)*) => { <$type>::builder() $($methods)* };
	
	// 4) the above but with an auto-chained last method (with ~ or ~/)
	( $type:ty => $($methods:tt)*) => { <$type>::builder() $($methods)*.build() };
}

// two cases are still missing but they behave like the first two
//
// the `z_besides` example explains how it works, as for the
// macro content you can see the source code of `builder_mode!`

use gtk::{glib, prelude::*};

// let's exemplify the first and last case (the rest are almost the same):
#[view {
	// in the following we just specify a type, so it will autocomplete with `::builder()`
	gtk::ApplicationWindow !{ // could be case 3 or 4
		application: app
		title: "Title"
		
		// the #interpolation calls a builder method:
		gtk::HeaderBar #titlebar(&#) { }
		
		// we end the builder mode above with a single tilde, which means that a `.build()` will be
		// chained to the method that follows (if an item comes, then to its interpolation method)
		//
		// we also declare the child in builder mode, but with an expression instead of a type:
		~gtk::Box::builder() !{ // could be case 1 or 2
			orientation: gtk::Orientation::Vertical
			spacing: 6
			margin_top: 6
			margin_bottom: 6
			margin_start: 6
			margin_end: 6
			
			// currently it is not possible to interpolate after finishing builder mode:
			#child(&#) // so we interpolate before
			// we have interpolated with a builder method of the parent item; `.build()` will be chained to it
			// by the single tilde before this item (case 4 because a type was specified in the parent item)
			
			// we use double tilde to end the builder mode in this item without chaining a `.build()` at
			// the end of the following method (case 1 due to an expression was specified for this item):
			~~build; // we call `build()` manually because gtk-rs requires it
			
			// we can now interpolate the following items with non-builder methods:
			gtk::Button #append(&#) !{ label: "First" } // we have just created these...
			gtk::Button #append(&#) !{ label: "Second" } // items also in builder mode
			// case 2 or 4 will be assumed if the mode is not explicitly ended with tildes as in both
			// items above (a `.build()` will be chained to the last called or interpolated method)
		}
		
		// now we can use non-builder methods on the parent item:
		present; // we show the window
	}
}]

fn builder_mode_example() -> glib::ExitCode {
	let app = gtk::Application::default();
	// if you use `expand_view_here!` as an expression, it will wrap itself in braces:
	app.connect_activate(|app| expand_view_here!());
	app.run()
}

// builder mode expands inner items before outer items (the placement is reversed)
// if you need to place an outer item before the inner ones, you must add `/` to the tildes (ie ~/ or ~~/)
// this would result in not being able to interpolate child items with builder methods of the parent
