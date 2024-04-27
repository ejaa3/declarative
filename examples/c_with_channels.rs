/*
 * SPDX-FileCopyrightText: 2024 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{clone, construct, view}; // we need to clone (see its doc)
use gtk::{glib, prelude::*};

// let's try to create a reactive view using channels

// this is what our app will do (increase or decrease a counter):
enum Msg { Increase, Decrease } // as in Elm architecture

// syntactic sugar for sending messages:
macro_rules! send { [$msg:expr => $tx:expr] => [$tx.send_blocking($msg).unwrap()] }

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
		let (tx, rx) = async_channel::bounded(1);
		
		expand_view_here! { } // here a closure named `refresh` was created
		
		glib::spawn_future_local(async move { // `state` and `refresh` lives in this async block
			while let Ok(msg) = rx.recv().await {
				state.update(msg); // we update the state
				refresh(&state); // and now we refresh the view
			}
		});
		
		window.present()
	}
	
	view![ gtk::ApplicationWindow window {
		application: app
		title: "Count unchanged"
		titlebar: &gtk::HeaderBar::new()
		
		'bind match state.count % 2 == 0 {
			true  => set_title: Some("The value is even")
			false => set_title: Some("The value is odd")
		}
		
		child: &_ @ gtk::Grid {
			column_spacing: 6
			row_spacing: 6
			margin_top: 6
			margin_bottom: 6
			margin_start: 6
			margin_end: 6
			~
			attach: &_, 0, 0, 2, 1 @ gtk::Label {
				hexpand: true
				'bind #set_label: &format!("The count is: {}", state.count)
			}
			attach: &_, 0, 1, 1, 1 @ gtk::Button::with_label("Increase") {
				// we clone `tx` to be able to use it with the other button:
				connect_clicked: clone![tx; move |_| send!(Msg::Increase => tx)]
				// you can see that `clone![]` allows you to put the expression
				// to be assigned (in this case a closure) after the semicolon
			}
			attach: &_, 1, 1, 1, 1 @ gtk::Button::with_label("Decrease") {
				connect_clicked: move |_| send!(Msg::Decrease => tx)
			}
		}
		'consume refresh = { // this closure requires `window` because of the 'bind above
			// we clone `window` to move the clone to the closure and thus be able to present it:
			clone![window]; move |state: &Self| bindings!()
			// this time the closure is outside of `clone!` so that `bindings!` can be expanded
		}
	} ];
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(State::start);
	app.run()
}
