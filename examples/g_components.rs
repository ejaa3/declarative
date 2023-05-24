/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{builder_mode, clone};
use gtk::{glib, prelude::*};

// a component is a template with its own states and channels (usually one of each)
// quite useful for splitting a user interface into different files for easy maintenance

enum Msg { Increase, Decrease, Reset } // messages for child components

macro_rules! send { [$msg:expr => $tx:expr] => [$tx.send($msg).unwrap()] }

struct Child { // basic structure of a component
	root: gtk::Box, // the main widget of this component
	  tx: glib::Sender<Msg> // a transmitter to send messages to this component
}

#[declarative::view { // component factory
	gtk::Box root !{
		orientation: gtk::Orientation::Vertical
		spacing: 6 #:
		
		gtk::Label #append(&#) !{
			label: &format!("This is the {nth} child")
			'bind set_label: &format!("The {nth} count is: {count}")
		} // at this point the `gtk::Label` is appended to the `gtk::Box`, so...
		
		// we consume the binding here so as not to clone the `gtk::Label`:
		@update_view = move |count| bindings!()
		
		gtk::Button::with_label("Increase") #append(&#) {
			// for several clones use commas:
			connect_clicked: clone![tx, parent_tx; move |_| {
				send!(Msg::Increase => tx);
				send!(nth => parent_tx);
			}]
		}
		
		gtk::Button::with_label("Decrease") #append(&#) {
			connect_clicked: clone![tx, parent_tx; move |_| {
				send!(Msg::Decrease => tx);
				send!(nth => parent_tx);
			}]
		}
	}
}]

impl Child {
	// let's communicate with the parent component via
	// a reference to its transmitter (parent_tx):
	fn new(nth: &'static str, parent_tx: glib::Sender<&'static str>) -> Self {
		let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
		let mut count = 0; // the state
		
		expand_view_here! { }
		
		let update_count = move |count: &mut u8, msg| match msg {
			Msg::Increase => *count = count.wrapping_add(1),
			Msg::Decrease => *count = count.wrapping_sub(1),
			Msg::Reset  => { *count = 0; send!(nth => parent_tx) },
		};
		
		rx.attach(None, move |msg| {
			update_count(&mut count, msg);
			update_view(count);
			glib::Continue(true)
		});
		
		Self { root, tx }
	} // this could be an unassociated function
}

#[declarative::view { // this is the parent component (the composite)
	gtk::ApplicationWindow window !{
		application: app
		title: "Components"
		
		gtk::HeaderBar #titlebar(&#) { }
		
		gtk::Box #child(&#) !{
			orientation: gtk::Orientation::Vertical
			spacing: 6
			margin_top: 6
			margin_bottom: 6
			margin_start: 6
			margin_end: 6 #:
			
			// you can add a child component here:
			Child::new("First", tx.clone()) first_child {
				#append(&#.root) // remember that the component widget is the `root` field
			}
			
			// or use a local variable or argument:
			ref second_child { #append(&#.root) }
			
			gtk::Label #append(&#) !{
				label: "Waiting for message…"
				'bind set_label: &format!("{nth} child updated")
			}
			
			gtk::Button::with_label("Reset first child") #append(&#) {
				// sending messages to a child component is as simple as using its own transmitter
				connect_clicked: move |_| send!(Msg::Reset => first_child.tx) // (its `tx` field)
				// just in case, `clone!` also allows to clone fields like `tx` in this case
			}
			
			gtk::Button::with_label("Reset second child") #append(&#) {
				connect_clicked: move |_| send!(Msg::Reset => second_child.tx)
			}
		}
		
		@update_view = move |nth| bindings!()
	}
}]

fn start(app: &gtk::Application) {
	let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
	let second_child = Child::new("Second", tx.clone());
	
	expand_view_here! { }
	
	rx.attach(None, move |nth| {
		update_view(nth);
		glib::Continue(true)
	});
	
	window.present()
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(start);
	app.run()
}
