/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{builder_mode, clone}; // we need to clone
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
			move |error| glib::g_critical!("c_from_messages", "{error}")
		)
	};
}

#[declarative::view {
	gtk::ApplicationWindow window !{
		application: app
		title: "Count unchanged"
		
		'bind match state.count % 2 == 0 {
			true  => set_title: Some("The value is even")
			false => set_title: Some("The value is odd")
		}
		
		gtk::HeaderBar #titlebar(&#) { }
		
		gtk::Grid #child(&#) !{
			column_spacing: 6
			row_spacing: 6
			margin_top: 6
			margin_bottom: 6
			margin_start: 6
			margin_end: 6 #..
			
			gtk::Label my_label #attach(&#, 0, 0, 2, 1) {
				set_hexpand: true
				'bind! set_label: &format!("The count is: {}", state.count)
			}
			
			gtk::Button::with_label("Increase") #attach(&#, 0, 1, 1, 1) {
				// we clone the sender to be able to use it with the other button:
				connect_clicked: clone![sender; move |_| send!(Msg::Increase => sender)]
				// you can see that `clone![]` allows you to put the expression
				// to be assigned (in this case a closure) after the semicolon
			}
			
			gtk::Button::with_label("Decrease") #attach(&#, 1, 1, 1, 1) {
				connect_clicked: move |_| send!(Msg::Decrease => sender)
			}
		}
		
		// the following binding closure requires `window` because of the 'bind above:
		'binding update_view = { // this brace is an expression
			// we clone `window` to move the clone to the closure and thus be able to return `window`:
			clone![window]; move |state: &State| bindings!()
			// this time the closure is outside of `clone![]` so that `bindings!()` can be expanded
		}
	}
}]

fn window(app: &gtk::Application) -> gtk::ApplicationWindow {
	let mut state = State { count: 0 }; // we create the state
	
	// https://docs.gtk.org/glib/struct.MainContext.html
	let (sender, receiver) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
	// looks like https://doc.rust-lang.org/book/ch16-02-message-passing.html
	
	expand_view_here! { }
	
	receiver.attach(None, move |msg| { // `state` lives in this closure
		update_state(&mut state, msg); // we update the state
		update_view(&state); // and now the view
		glib::Continue(true) // this is for glib to keep this closure alive
	});
	
	window
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(move |app| window(app).present());
	app.run()
}
