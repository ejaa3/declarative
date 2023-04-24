/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative_gtk4::Composable;
use gtk::{glib, prelude::*};

#[derive(Debug)]
// let's try to create a reactive view using messages:
enum Msg { Increase, Decrease } // or the Elm architecture

struct State { count: i32 } // a simple counter state

fn update_state(state: &mut State, msg: Msg) {
	match msg { // state is updated according to the message
		Msg::Increase => state.count = state.count.wrapping_add(1),
		Msg::Decrease => state.count = state.count.wrapping_sub(1),
	}
}

macro_rules! send { // a macro to log send errors
	($expr:expr => $sender:ident) => {
		$sender.send($expr).unwrap_or_else(
			move |error| glib::g_critical!("c_reactivity", "{error}")
		)
	};
}

declarative::view! {
	gtk::ApplicationWindow::new(app) window {
		set_title: Some("Count unchanged")
		set_titlebar => gtk::HeaderBar 'wrap Some { }
		
		// use 'bind_only to react to changes (do not assign at startup):
		'bind_only if state.count % 2 == 0 {
			set_title: Some("The value is even")
		} else {
			set_title: Some("The value is odd")
		}
		
		// you can start the builder mode with the exclamation mark before the brace:
		// if only a type is specified, the function Type::builder() is assumed:
		gtk::Grid !{
			column_spacing: 6
			row_spacing: 6
			margin_top: 6
			margin_bottom: 6
			margin_start: 6
			margin_end: 6
			
			// methods without arguments are called with an exclamation mark:
			build! // build is a GTK method to finish a builder
			
			.. // the double dot ends the builder mode
			
			// “property or object assignments” only assign the first argument of a method;
			// if the method requires multiple arguments, you can pass the rest in square brackets:
			attach[.. 0, 0, 2, 1] => gtk::Label my_label {
				set_hexpand: true
				// Use 'bind to react to changes (assign at startup):
				'bind set_label: &format!("The count is: {}", state.count)
			}
			
			// since gtk::Grid is a “composable object”, we better use this syntax:
			gtk::Button::with_label("Increase") 'with (0, 1, 1, 1) {
				// we clone the sender to be able to use it with the other button:
				connect_clicked: 'clone sender move |_| send!(Msg::Increase => sender)
			}
			
			gtk::Button::with_label("Decrease") 'with (1, 1, 1, 1) {
				connect_clicked: move |_| send!(Msg::Decrease => sender)
			}
		}
		
		// use 'binding to create a closure to update the view appropriately:
		'binding update_view: 'clone window move |state: &State| { bindings!(); }
		// we clone the window to move the clone to the closure so that the window can be returned
		// the closure requires the window due to the 'bind_only above
	} ..
	
	fn window(app: &gtk::Application) -> gtk::ApplicationWindow {
		let mut state = State { count: 0 };
		let (sender, receiver) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
		
		expand_view_here!(); // insert the view code here
		
		receiver.attach(None, move |msg| {
			update_state(&mut state, msg);
			update_view(&state);
			glib::Continue(true)
		});
		
		window
	}
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(move |app| window(app).present());
	app.run()
}
