/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{builder_mode, clone};
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

#[declarative::view { // component factory (similar to a template)
	gtk::Box root !{
		orientation: gtk::Orientation::Vertical
		spacing: 6 #:
		
		gtk::Label #append(&#) !{
			label: &format!("This is the {nth} component")
			'bind set_label: &format!("The {nth} count is: {}", state.count)
		} // at this point the `gtk::Label` is appended to the `gtk::Box`, so...
		
		// we consume the bindings here so as not to clone the `gtk::Label`:
		@update_view = move |state: &State| bindings!()
		
		gtk::Button::with_label("Increase") #append(&#) {
			// for several clones use commas:
			connect_clicked: clone![sender, parent; move |_| {
				send!(Msg::Increase => sender);
				send!(nth => parent);
			}]
		}
		
		gtk::Button::with_label("Decrease") #append(&#) {
			connect_clicked: clone![sender, parent; move |_| {
				send!(Msg::Decrease => sender);
				send!(nth => parent);
			}]
		}
	}
}]

fn component(nth: &'static str, parent: &glib::Sender<&'static str>) -> gtk::Box {
	let mut state = State { count: 0 };
	let (sender, receiver) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
	
	expand_view_here! { }
	
	receiver.attach(None, move |msg| {
		update_state(&mut state, msg);
		update_view(&state);
		glib::Continue(true)
	});
	
	root
}

#[declarative::view { // this is the composite
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
			
			// you can call the function here:
			component("First", &sender) #append(&#) { /* edit */ }
			
			// or use a local variable or argument:
			ref second_component #append(&#) { /* edit */ }
			
			// and of course (this way you cannot edit here):
			append: &component("Third", &sender)
			
			gtk::Label #append(&#) !{
				label: "Waiting for message…"
				'bind set_label: &format!("{nth} component updated")
			}
		}
		
		@update_view = move |nth| bindings!()
	}
}]

fn window(app: &gtk::Application) -> gtk::ApplicationWindow {
	let (sender, receiver) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
	
	let second_component = component("Second", &sender);
	
	expand_view_here! { }
	
	receiver.attach(None, move |nth| {
		update_view(nth);
		glib::Continue(true)
	});
	
	window
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(move |app| window(app).present());
	app.run()
}

macro_rules! send {
	($expr:expr => $sender:ident) => {
		$sender.send($expr).unwrap_or_else(
			move |error| glib::g_critical!("f_components", "{error}")
		)
	};
}

use send;
