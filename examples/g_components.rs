/*
 * SPDX-FileCopyrightText: 2024 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{clone, construct, view};
use gtk::{glib, prelude::*};

// a component is a template with its own states and channels (usually one of each)
// quite useful for splitting a user interface into different files for easy maintenance

enum Msg { Increase, Decrease, Reset } // messages for child components

macro_rules! send { [$msg:expr => $tx:expr] => [$tx.send_blocking($msg).unwrap()] }

struct Child { // basic structure of a component
	root: gtk::Box, // the main widget of this component
	  tx: async_channel::Sender<Msg> // a transmitter to send messages to this component
} // we could have declared the structure in the view with just the `tx` field

#[view]
impl Child {
	// `nth` will be the child number ("First" or "Second") and we will communicate
	// with the parent component through a reference to its transmitter (parent_tx):
	fn new(nth: &'static str, parent_tx: async_channel::Sender<&'static str>) -> Self {
		let (tx, rx) = async_channel::bounded(1);
		let mut count = 0; // the state
		
		expand_view_here! { }
		
		let update = move |count: &mut u8, msg| match msg {
			Msg::Increase => *count = count.wrapping_add(1),
			Msg::Decrease => *count = count.wrapping_sub(1),
			Msg::Reset  => { *count = 0; send!(nth => parent_tx) },
		};
		
		glib::spawn_future_local(async move {
			while let Ok(msg) = rx.recv().await {
				update(&mut count, msg);
				bindings! { } // we can refresh the view like this
			}
		});
		
		Self { root, tx }
	}
	
	view![ gtk::Box root {
		orientation: gtk::Orientation::Vertical
		spacing: 6
		~
		append: &_ @ gtk::Label {
			label: glib::gformat!("This is the {nth} child")
			'bind set_label: &format!("The {nth} count is: {count}")
		}
		append: &_ @ gtk::Button::with_label("Increase") {
			// for several clones use commas:
			connect_clicked: clone![tx, parent_tx; move |_| {
				send!(Msg::Increase => tx);
				send!(nth => parent_tx);
			}]
		}
		append: &_ @ gtk::Button::with_label("Decrease") {
			connect_clicked: clone![tx, parent_tx; move |_| {
				send!(Msg::Decrease => tx);
				send!(nth => parent_tx);
			}]
		}
	} ];
}

#[view[ gtk::ApplicationWindow window { // this is the parent component (the composite)
	application: app
	title: "Components"
	titlebar: &gtk::HeaderBar::new()
	
	child: &_ @ gtk::Box {
		orientation: gtk::Orientation::Vertical
		spacing: 6
		margin_top: 6
		margin_bottom: 6
		margin_start: 6
		margin_end: 6
		~
		// remember that the component widget is the `root` field:
		append: &_.root @ Child::new("First", tx.clone()) first_child { }
		// we use composition just to give a variable name
		
		append: &second_child.root // or use an argument or variable before view expansion
		
		append: &_ @ gtk::Label {
			label: "Waiting for message…"
			'bind set_label: &format!("{nth} child updated")
		}
		append: &_ @ gtk::Button::with_label("Reset first child") {
			// sending messages to a child component is as simple as using its own transmitter:
			connect_clicked: move |_| send!(Msg::Reset => first_child.tx) // (its `tx` field)
			// just in case `clone!` has a convenient syntax for cloning fields like `tx` in this case
		}
		append: &_ @ gtk::Button::with_label("Reset second child") {
			connect_clicked: move |_| send!(Msg::Reset => second_child.tx)
		}
	}
} ]]

fn start(app: &gtk::Application) {
	let (tx, rx) = async_channel::bounded(1);
	let second_child = Child::new("Second", tx.clone());
	
	expand_view_here! { }
	
	glib::spawn_future_local(async move {
		while let Ok(nth) = rx.recv().await { bindings!() }
	});
	window.present()
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(start);
	app.run()
}
