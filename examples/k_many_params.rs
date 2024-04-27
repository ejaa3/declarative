/*
 * SPDX-FileCopyrightText: 2024 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{construct, view};
use gtk::{glib, pango, prelude::*};

// this example is like `g_components` but with a child component that requires many
// parameters to be initialized, which can be inconvenient to specify in a function
//
// however we can use a struct and put the parameters as fields since
// declarative allows param structs to be initialized consistently

macro_rules! send { [$msg:expr => $tx:expr] => [$tx.send_blocking($msg).unwrap()] }

struct Child {
	        name: &'static str,
	       count: u8, // the state as a parameter
	 third_param: &'static str,
	fourth_param: &'static str,
	   parent_tx: async_channel::Sender<&'static str>,
}

#[view]
impl Child {
	// we name the function `start` due to the `construct!` macro implementation
	fn start(mut self) -> ChildTemplate { // `mut self` because it has a state that should be mutable
		let (tx, rx) = async_channel::bounded(1);
		
		expand_view_here! { }
		
		glib::spawn_future_local(async move {
			while let Ok(add) = rx.recv().await {
				self.count = if add { u8::wrapping_add } else { u8::wrapping_sub } (self.count, 1);
				bindings! { }
			}
		});
		
		ChildTemplate { tx, first_label, root }
	}
	
	view! {
		struct ChildTemplate { tx: async_channel::Sender<bool> }
		
		gtk::Frame ref root {
			label: self.name
			child: &_ @ gtk::Box {
				margin_bottom: 6
				margin_end: 6
				margin_start: 6
				margin_top: 6
				orientation: gtk::Orientation::Vertical
				spacing: 6
				~
				append: &_ @ gtk::Label ref first_label { label: self.third_param }
				append: &_ @ gtk::Label { label: self.fourth_param }
				append: &_ @ gtk::Label { 'bind #set_label: &format!("Count: {}", self.count) }
				append: &_ @ gtk::Button::with_label("Greet") {
					connect_clicked: move |_| send!(self.name => self.parent_tx)
				}
			}
		}
	}
}

struct Parent;

#[view]
impl Parent {
	fn start(app: &gtk::Application) {
		let (tx, rx) = async_channel::bounded(1);
		
		expand_view_here! { }
		
		glib::spawn_future_local(async move {
			while let Ok(child) = rx.recv().await { bindings!() }
		});
	}
	
	view![ gtk::ApplicationWindow {
		application: app
		titlebar: &_ @ gtk::HeaderBar {
			pack_start: &_ @ gtk::Button::with_label("Count") {
				connect_clicked: move |_| {
					send!(false => first_child.tx);
					send!(true => second_child.tx);
				}
			}
		}!
		child: &_ @ gtk::Grid {
			column_spacing: 6
			margin_bottom: 6
			margin_end: 6
			margin_start: 6
			margin_top: 6
			row_spacing: 6
			~
			// to compose with an item that should expand to a struct literal...
			attach: &_.root, 0, 0, 1, 1 @ Child first_child { // `?` is added after braces
				name: "First Child"
				third_param: "Underline"
				fourth_param: "Hello World"
				count: 255
				parent_tx: tx // with a single tilde the current `construct!` macro appends...
				// `.start()` to the struct literal, but with double tilde it would not append it
				~
				// at this point we edit what `Child::start()` returns (a `ChildTemplate`);
				// if there were a double tilde we would be directly editing a `Child` instance:
				first_label.set_attributes: Some(&_) @ pango::AttrList {
					insert: pango::AttrInt::new_underline(pango::Underline::Single)
				}!
			}? // although it may not seem like it, `first_child` and `second_child` are of type...
			attach: &_.root, 1, 0, 1, 1 @ Child second_child { // `ChildTemplate` instead of `Child`
				name: "Second Child"
				count: 0
				third_param: "Third Param"
				fourth_param: "Fourth Param"
				// we clone `tx` here and not in the previous child for a reason...
				parent_tx: tx.clone() // explained at the end of the first example
			}? // since there is no tilde, "construct!" also appends `.start()`
			attach: &_, 0, 1, 2, 1 @ gtk::Label {
				hexpand: true
				label: "Waiting for greetings"
				'bind set_label: &format!("Greeting from {child}")
			}
		} ~
		present;
	} ];
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(Parent::start);
	app.run()
}
