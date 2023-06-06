/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{builder_mode, clone, view}; // we need to clone (see its doc)
use gtk::{glib, prelude::*};

// let's try to create a reactive view using channels

// this is what our app will do (increase or decrease a counter):
enum Msg { Increase, Decrease } // as in Elm architecture

// syntactic sugar for sending messages:
macro_rules! send { [$msg:expr => $tx:expr] => [$tx.send($msg).unwrap()] }

struct State { count: i32 } // a simple counting state

#[view]
impl State {
	fn update(&mut self, msg: Msg) {
		match msg { // state is updated according to the message
			Msg::Increase => self.count = self.count.wrapping_add(1),
			Msg::Decrease => self.count = self.count.wrapping_sub(1),
		}
	}
	
	fn start(app: &gtk::Application) {
		let mut state = Self { count: 0 }; // we create the state
		
		// about the following: https://docs.gtk.org/glib/struct.MainContext.html
		let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
		// looks like https://doc.rust-lang.org/book/ch16-02-message-passing.html
		
		expand_view_here! { } // here a closure named `refresh` was created
		
		rx.attach(None, move |msg| { // `state` and `refresh` lives in this closure
			state.update(msg); // we update the state
			refresh(&state); // and now we refresh the view
			glib::Continue(true) // this is for glib to keep this closure alive
		});
		
		window.present()
	}
	
	view! {
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
				~margin_end: 6
				
				gtk::Label #attach(&#, 0, 0, 2, 1) {
					set_hexpand: true
					'bind @set_label: &format!("The count is: {}", state.count)
				}
				
				gtk::Button::with_label("Increase") #attach(&#, 0, 1, 1, 1) {
					// we clone `tx` to be able to use it with the other button:
					connect_clicked: clone![tx; move |_| send!(Msg::Increase => tx)]
					// you can see that `clone![]` allows you to put the expression
					// to be assigned (in this case a closure) after the semicolon
				}
				
				gtk::Button::with_label("Decrease") #attach(&#, 1, 1, 1, 1) {
					connect_clicked: move |_| send!(Msg::Decrease => tx)
				}
			}
			
			// the following closure requires `window` because of the 'bind above:
			@refresh = {
				// we clone `window` to move the clone to the closure and thus be able to present it:
				clone![window]; move |state: &Self| bindings!()
				// this time the closure is outside of `clone!` so that `bindings!` can be expanded
			}
		}
	}
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(State::start);
	app.run()
}
