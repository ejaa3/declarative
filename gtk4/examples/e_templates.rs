/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative_gtk4::Composable;
use gtk::{glib, prelude::*};

// the second (not the first) thing to do is this structure:
struct BoxTemplate {
	 root: gtk::Box, // the main widget
	label: gtk::Label, // the rest are widgets that you want to “publish”
}

// to edit any widget in the scope of this object:
impl std::ops::Deref for BoxTemplate {
	type Target = gtk::Box;
	
	fn deref(&self) -> &Self::Target {
		&self.root // in this case the root widget
	}
	
	declarative::view! {
		BoxTemplate { // with Deref
			set_spacing: 10
			gtk::Label { } // “component assignment” is possible
		}
		BoxTemplate { // without Deref
			root -> { // root -> { } required
				set_spacing: 10
				gtk::Label { }
			}
		} .. // the macro does nothing if nothing is added here
	}
}

declarative::view! { // now yes, the first thing to do is a view
	gtk::Box root !{ // I prefer to use the same names as the fields
		orientation: gtk::Orientation::Vertical
		spacing: 6
		margin_top: 6
		margin_bottom: 6
		margin_start: 6
		margin_end: 6
		build! ..
		
		gtk::Label label { } // I want to publish this widget
		
		gtk::Button::with_label("Increase") { // this is private
			connect_clicked: 'clone sender
				move |_| send!(Msg::Increase => sender)
		}
	} ..
	
	// if Default were implemented, there would be no need
	// to write new::() but there would be no parameters
	
	impl BoxTemplate {
		// could also be unassociated function:
		fn new(sender: &glib::Sender<Msg>) -> Self {
			expand_view_here!();
			Self { root, label }
		}
	}
} // now let's use the template:

#[derive(Debug)]
enum Msg { Increase, Decrease } // Elm again

struct State { count: i32 }

fn update_state(state: &mut State, msg: Msg) {
	match msg {
		Msg::Increase => state.count = state.count.wrapping_add(1),
		Msg::Decrease => state.count = state.count.wrapping_sub(1),
	}
}

declarative::view! {
	gtk::ApplicationWindow window !{
		application: app
		title: "Templates"
		build! ..
		set_titlebar => gtk::HeaderBar 'wrap Some { }
		
		gtk::Box !{
			orientation: gtk::Orientation::Vertical
			spacing: 6
			build!
			
			// BoxTemplate is not a widget but its root field is;
			// use 'dot to specify that you are adding root to the gtk::Box:
			BoxTemplate::new(&sender_1) 'dot root {
				// you can edit a field with: field -> { /* edit */ }
				label -> {
					set_label: "This is the first view:"
					
					'bind_only set_label:
						&format!("The first count is: {}", state_1.count)
				}
				
				gtk::Button::with_label("Decrease") {
					connect_clicked: move |_| send!(Msg::Decrease => sender_1)
				}
				
				// be careful editing a template after creating a binding closure that updates it:
				'binding update_view_1: move |state_1: &State| { bindings!(); }
				// at this point the entire template has moved to the binding closure
			}
			
			gtk::Separator { }
			
			BoxTemplate::new(&sender_2) 'dot root { // almost the same code as above
				label -> {
					set_label: "This is the second view:"
					
					'bind_only set_label:
						&format!("The second count is: {}", state_2.count)
				}
				
				gtk::Button::with_label("Decrease") {
					connect_clicked: move |_| send!(Msg::Decrease => sender_2)
				}
				
				'binding update_view_2: move |state_2: &State| { bindings!(); }
			}
		}
	} ..
	
	// we create states and channels for both “templated components”:
	fn window(app: &gtk::Application) -> gtk::ApplicationWindow {
		let (sender_1, receiver_1) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
		let (sender_2, receiver_2) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
		
		let mut state_1 = State { count: 0 };
		let mut state_2 = State { count: 0 };
		
		expand_view_here!();
		
		receiver_1.attach(None, move |msg| {
			update_state(&mut state_1, msg);
			update_view_1(&state_1);
			glib::Continue(true)
		});
		
		receiver_2.attach(None, move |msg| {
			update_state(&mut state_2, msg);
			update_view_2(&state_2);
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

macro_rules! send {
	($expr:expr => $sender:ident) => {
		$sender.send($expr).unwrap_or_else(
			move |error| glib::g_critical!("e_templates", "{error}")
		)
	};
}

use send;
