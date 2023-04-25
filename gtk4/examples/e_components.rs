/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative_gtk4::Composable;
use gtk::{glib, prelude::*};

#[derive(Debug)]
enum Msg { Increase, Decrease } // Elm again

// basically each component has its own states (usually one):
struct State { count: i32 }

fn update_state(state: &mut State, msg: Msg) {
	match msg {
		Msg::Increase => state.count = state.count.wrapping_add(1),
		Msg::Decrease => state.count = state.count.wrapping_sub(1),
	}
}

declarative::view! { // component factory
	gtk::Box root !{
		spacing: 6
		orientation: gtk::Orientation::Vertical
		build!
		
		gtk::Label {
			set_label: &format!("This is the {nth} Component")
			'bind_only set_label: &format!("The {nth} count is: {}", state.count)
		} // at this point the gtk::Label is appended to the gtk::Box, so...
		
		// 'binding here to not clone the gtk::Label
		'binding update_view: move |state: &State| { bindings!(); }
		
		gtk::Button::with_label("Increase") {
			// for several clones use braces:
			connect_clicked: 'clone {sender, parent} move |_| {
				send!(Msg::Increase => sender);
				send!(nth => parent);
			}
		}
		
		gtk::Button::with_label("Decrease") {
			connect_clicked: 'clone {sender, parent} move |_| {
				send!(Msg::Decrease => sender);
				send!(nth => parent);
			}
		}
	} ..
	
	fn component(nth: &'static str, parent: &glib::Sender<&'static str>) -> gtk::Box {
		let mut state = State { count: 0 };
		let (sender, receiver) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
		
		expand_view_here!();
		
		receiver.attach(None, move |msg| {
			update_state(&mut state, msg);
			update_view(&state);
			glib::Continue(true)
		});
		
		root
	}
}

declarative::view! { // the main component
	gtk::ApplicationWindow window !{
		application: app
		title: "Components"
		titlebar => gtk::HeaderBar { }
		build!
		
		gtk::Box !{
			orientation: gtk::Orientation::Vertical
			spacing: 6
			margin_top: 6
			margin_bottom: 6
			margin_start: 6
			margin_end: 6
			build!
			
			// use 'use to use a variable defined before the view:
			'use first_component { }
			
			// or you can call the function here:
			component("Second", &sender) { }
			
			gtk::Label {
				set_label: "Waiting for message…"
				'bind_only set_label: &format!("{nth} component updated")
			}
		}
		
		'binding update_view: move |nth| { bindings!(); }
	} ..
	
	fn window(app: &gtk::Application) -> gtk::ApplicationWindow {
		let (sender, receiver) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
		
		let first_component = component("First", &sender);
		
		expand_view_here!();
		
		receiver.attach(None, move |nth| {
			update_view(nth);
			glib::Continue(true)
		});
		
		window
	}
}

fn main() {
	let app = gtk::Application::default();
	app.connect_activate(move |app| window(app).present());
	std::process::exit(app.run().value())
}

macro_rules! send {
	($expr:expr => $sender:ident) => {
		$sender.send($expr).unwrap_or_else(
			move |error| glib::g_critical!("e_components", "{error}")
		)
	};
}

use send;
