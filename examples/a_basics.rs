/*
 * SPDX-FileCopyrightText: 2025 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

fn main() -> glib::ExitCode {
	let hello = "Hello";
	
	// use `block!` to create a simple declarative scope (a view):
	declarative::block! {
		// call a function followed by a variable name to create one
		String::from(hello) first_item { } // braces should be written...
		String::from("world!") second_item { } // even if they are empty
	}
	
	println!("[block!]\n{first_item} {second_item}\n");
	
	// you might like to rename `block!` to `view!` like so:
	// use declarative::block as view;
	
	view_attributes_example();
	abc_module::example();
	construct_example()
}

use declarative::view;

// you can use the `#[view]` attribute to separate the view from the logic,
// in this case a function; its content is the same as `block!`:
#[view {
	// writing a type instead of a function call will invoke a `construct!`
	// macro, which should expand into code that instantiates it:
	String mut main_string { // with `mut` you can mutate inside the braces
		push_str: "Hello " // this is a method call with an argument
	}! // because `!` the `construct!` macro expands into `String::default()` (explained later)
}]

// a second view (you can use the attribute multiple times):
#[view[ ref main_string { // `ref` has three usages, but in this case it is as an item
	// `ref` as an item refers to an existing variable rather than instantiate
	// in this case we keep editing `main_string` from the view above because of `ref`
	
	push_str: &{ // &{this} is normal Rust code like "this string"
		// the `expand_view_here!` placeholder macro can only consume one view:
		expand_view_here! { } // here the third view is expanded
		last_view_string // the function will explain how views are expanded
	}
} ]]

// Rust allows to delimit the content of an attribute between
// parentheses, brackets or braces just like a functional macro:
#[view[ String::from("world!") last_view_string { } ]]
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
mod abc_module {
	use super::construct; // explained later
	
	pub fn example() {
		println!("[abc_module]");
		
		let mut string_before_view = String::new();
		expand_view_here! { } // first `view!` will expand here
		first_view_string.push_str("!"); // logically you can edit after view
		
		println!("{first_view_string}");
	}
	
	// we can share view code to reduce boilerplate code with
	// functions whose last argument should be the item to edit:
	fn push_two_str(first: &str, second: &str, target: &mut String) {
		expand_view_here! { } // the second `view!` will expand here
	}
	
	view! {
		String mut first_view_string {
			// the previous function is called by using an underscore where the
			// variable name of this item should be located (the last argument):
			push_two_str: "a", "b", &mut _ // arguments are separated with commas
			
			// we can use another underscore to simulate composition:
			push_two_str: "c", &_, &mut _ // an `@` is used for each item to be composed
				// in the first underscore the name of the following string will be located:
				@ String mut { push_str: "d" }! // no need to name items (a name is generated)
			
			// full paths can be used:
			super::abc_module::push_two_str: &_, _, &mut _ // we can add underscores as necessary
				@ String mut { push_str: "e" }! // item names are placed in FIFO order
				@ str::as_ref("f") { } // the last underscore is always for the parent item
			
			// when the number of underscores matches the number of items, a method of...
			push_str: _.as_ref() @ String mut { push_str: "g" }! // the parent item is called
			
			// full paths to methods can also be used (useful for disambiguating traits):
			String::push_str &mut: &_ // `&mut:` because `push_str()` requires `&mut self` as the first argument
				@ String mut { String::push_str &mut: "h" }!
			
			// remember that `ref` can be an item:
			push_str: &_ @ ref string_before_view { push_str: "i" }
			
			push_str: &last_string_item // "j"
			
			// we can use `ref` with `last_string_item` because items are defined at the beginning...
			push_str: &_ @ ref last_string_item { // and their function calls later at expansion time
				clear; // you can call a method without arguments with semicolon
				push_str: "k"
			}
		}!
		
		// we were able to use this item even though it was declared last in the view
		String::from("j") mut last_string_item { } // `mut` because it is mutated in the previous `ref`
	}
	
	view!(ref target { // the view of `push_two_str()` function
		push_str: first
		push_str: _ @ ref second { }
	});
}

// declarative depends on a `construct!` macro in scope to sugar coat the view a bit,
// but first note that the initial content of an item expands differently if instead
// of a type you use a function call, or if you add `!` or `?` after the braces
//
// approximate expansions when using types:
// - Type { method; }! -> let var = Type::default(); var.method();
// - Type {  field; }? -> let var = Type { field };
// - Type { method; }  -> let var = Type::builder().method();
//
// approximate expansions when using function calls:
// - call() { method; }! -> let var = call().method();
// - call() { method; }  -> let var = call(); var.method();
//
// you can see that using or not `!` in function calls is the opposite in types
//
// the main crate contains this implementation which integrates well with gtk-rs:
macro_rules! construct {
	// this arm will match for items with a type and `!` after braces
	(? $type:ty) => { <$type>::default() };
	// example: `Type { }!` and `Type { six: 6 }!` expands to `Type::default()`
	
	// for items with a type, `~~` or `~~>` after the last field and `?` after braces
	(? ~$struct_literal:expr) => { $struct_literal };
	// `Type { two: 2; field; ~~ six: 6 }?` and
	// `Type { two: 2; field; ~~ }?` expands to `Type { two: 2, field }`
	
	// for items with a type, `~` or `~>` after the last field (optional) and `?` after braces
	(?  $struct_literal:expr) => { $struct_literal.start() };
	// `Type { }?` expands to `Type { }.start()`
	//
	// `Type { field; two: 2 ~ six: 6 }?` and
	// `Type { field; two: 2 }?` expands to `Type { field, two: 2 }.start()`
	
	// for items with a function call, `~~` or `~~>` after the last method and `!` after braces
	(~$builder:expr) => { $builder };
	// `Type::builder() { two: 2; method; ~~ six: 6 }!` and
	// `Type::builder() { two: 2; method; ~~ }!` expands to `Type::builder().two(2).method()`
	
	// for items with a function call, `~` or `~>` after the last method (optional) and `!` after braces
	( $builder:expr) => { $builder.build() };
	// `Type::builder() { }!` expands to `Type::builder().build()`
	//
	// `Type::builder() { method; two: 2; ~ six: 6 }!` and
	// `Type::builder() { method; two: 2; }!` expands to `Type::builder().method().two(2).build()`
	
	// for items with a type and `~~` or `~~>` after the last method
	(~$type:ty => $($methods:tt)*) => { <$type>::builder() $($methods)* };
	// `Type { two: 2; method; ~~ }!` expands to `Type::builder().two(2).method()`
	
	// for items with a type and `~` or `~>` after the last method (optional)
	( $type:ty => $($methods:tt)*) => { <$type>::builder() $($methods)*.build() };
	// `Type { }` expands to `Type::builder().build()`
	//
	// `Type { method; two: 2 ~ six: 6 }` and
	// `Type { method; two: 2 }` expands to `Type::builder().method().two(2).build()`
}

use gtk::{glib, prelude::*}; // let's illustrate the above with gtk-rs:

// the following item consists of a type, single tilde after the last method and no `!` or `?` after
// braces, so it will expand to something like: `gtk::ApplicationWindow::builder().methods().build()`
#[view[ gtk::ApplicationWindow {
	application: app
	title: "Title"
	titlebar: &_ @ gtk::HeaderBar { } // composition works with builder methods or struct fields
	
	// the following item consists of a function call, double tilde and `!` after braces,
	// so it will expand to something like: `gtk::Box::builder().methods().last_method()`
	child: &_ @ gtk::Box::builder() {
		orientation: gtk::Orientation::Vertical
		spacing: 6
		margin_top: 6
		margin_bottom: 6
		margin_start: 6
		margin_end: 6
		build; // we call `build()` manually because gtk-rs requires it (the last method)
		~~ // now we can compose this item with non-builder methods:
		append: &_ @ gtk::Button { label: "First" } // these items are expanded as the root item like this:
		append: &_ @ gtk::Button { label: "Second" } // gtk::Button::builder().label("Text").build()
	}!
	~ // now we can call non-builder methods:
	present; // we show the window
} ]]

fn construct_example() -> glib::ExitCode {
	let app = gtk::Application::default();
	// if you use `expand_view_here!` as an expression, it will wrap itself in braces:
	app.connect_activate(|app| expand_view_here!());
	app.run()
}

// declarative expands builder items or struct literal items in the reverse order they were defined
// if you need to expand any item in the usual order, you must add `>` to the tildes (ie `~>` or `~~>`)
// this would result in not being able to compose child items with builder methods of the parent

use construct;
