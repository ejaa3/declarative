/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative_gtk4::Composable;
use gtk::{glib, prelude::*};

// you can use the “builder mode” with the exclamation mark before the brace,
// like this: Type !{ }
//
// if only a type is specified, the function Type::default() is assumed, but it
// is possible to change the associated function and even call a last method with
// the “builder-mode” feature, which requires a "builder_mode" macro in the scope
//
// declarative_gtk4 already has the feature active plus the macro to import;
// however, let's not include the macro to explain it here:

macro_rules! builder_mode {
	// when only a type is specified and the mode is
	// terminated without an auto-invoked last method (with #!)
	(!$type:ty => $($token:tt)+) => { <$type>::builder() $($token)+ };
	
	// when only a type is specified and the mode is
	// terminated with an auto-invoked last method (with #)
	( $type:ty => $($token:tt)+) => { <$type>::builder() $($token)+.build() };
	
	// when an expression is specified and the mode is
	// terminated without an auto-invoked last method (with #!)
	(!$($expr:expr)+) => { $($expr)+ };
	
	// when an expression is specified and the mode is
	// terminated with an auto-invoked last method (with #)
	( $($expr:expr)+) => { $($expr)+.build() };
}

// let's exemplify the second and third case:
declarative::view! { // (the first and the last are almost the same)
	gtk::ApplicationWindow window !{ // builder mode (type only)
		application: app
		title: "Count unchanged" # // mode end (second case)
		set_titlebar => gtk::HeaderBar 'wrap Some { }
		
		gtk::Box::builder() !{ // builder mode (expression)
			orientation: gtk::Orientation::Vertical
			spacing: 6
			margin_top: 6
			margin_bottom: 6
			margin_start: 6
			margin_end: 6
			
			// currently builder mode does not affect “component assignments”:
			gtk::Button !{ label: "First" } // if you do not put a # or #!, it is as if
			gtk::Button !{ label: "Second" } // you had put a # at the end of the scope
			
			// if I were to implement “component assignments” in builder mode,
			// I would make it an optional feature to keep the macro framework-agnostic,
			// but if you enable it, you will probably have to reorder your code
			//
			// For now it is best to assign components after # or #!
			// if you intend to enable this feature if it exists
			//
			// I would say that gtk-rs users should not worry about this because gtk4 has
			// private builders (they cannot be composable), but there are no guarantees;
			// if they become public, I may also add an optional feature to declarative_gtk4
			
			// you can call a method without arguments with semicolon:
			build; #! // mode end (third case)
			// GTK requires calling `build()` in most of its builders
		}
	} ..
	
	fn window(app: &gtk::Application) -> gtk::ApplicationWindow {
		expand_view_here!();
		window
	}
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(move |app| window(app).present());
	app.run()
}
