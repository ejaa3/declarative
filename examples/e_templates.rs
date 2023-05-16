/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{builder_mode, clone};
use gtk::{glib, prelude::*};

// the second (not the first) thing to do is this structure:
struct BoxTemplate {
	 root: gtk::Box, // the main widget
	label: gtk::Label, // the rest are widgets that you want to “publish” or “export”
	reset: gtk::Button, // this widget will not be contained in the main one
}

// to edit any widget in the scope of this item:
impl std::ops::Deref for BoxTemplate {
	type Target = gtk::Box;
	
	fn deref(&self) -> &Self::Target {
		&self.root // in this case the root widget
	}
}

#[declarative::view { // now yes, the first thing to do is a view
	gtk::Box root !{ // I prefer to use the same names as the fields
		orientation: gtk::Orientation::Vertical
		spacing: 6
		margin_top: 6
		margin_bottom: 6
		margin_start: 6
		margin_end: 6 #:
		
		gtk::Label label #append(&#) !{ // I want to publish this widget
			label: glib::gformat!("This is the {nth} view")
		}
		
		gtk::Button::with_label("Increase") #append(&#) { // this is private
			connect_clicked: clone![sender; move |_| send!(Msg::Increase => sender)]
		}
	}
	// we will also export this widget although it is independent of the root:
	gtk::Button reset !{ // we must do something with it in the main view so that it is not lost
		label: glib::gformat!("Reset {nth}") #:
		connect_clicked: clone![sender; move |_| send!(Msg::Reset => sender)]
	}
}]

impl BoxTemplate {
	// if `Default` were implemented, there would be no need
	// to write `::new()` but there would be no parameters
	
	// could also be unassociated function:
	fn new(nth: &str, sender: &glib::Sender<Msg>) -> Self {
		expand_view_here! { }
		Self { root, label, reset }
	}
} // now let's use the template:

#[derive(Debug)]
enum Msg { Increase, Decrease, Reset } // Elm again

struct State { count: i32 }

fn update_state(state: &mut State, msg: Msg) {
	match msg {
		Msg::Increase => state.count = state.count.wrapping_add(1),
		Msg::Decrease => state.count = state.count.wrapping_sub(1),
		Msg::Reset    => state.count = 0,
	}
}

#[declarative::view {
	gtk::ApplicationWindow window !{
		application: app
		title: "Templates"
		
		gtk::HeaderBar #titlebar(&#) { }
		
		gtk::Box #child(&#) !{
			orientation: gtk::Orientation::Vertical
			spacing: 6 #:
			
			// `BoxTemplate` is not a widget but its `root` field is (#interpolate well):
			BoxTemplate::new("first", &sender1) first #append(&#.root) {
				label => { // we are editing the `label` field
					'bind set_label: &format!("The first count is: {}", state.count)
				}
				// we can interpolate here thanks to `Deref`;
				// otherwise we would have to edit the `root` field as `label`:
				gtk::Button::with_label("Decrease") #append(&#) {
					connect_clicked: move |_| send!(Msg::Decrease => sender1)
				}
				// be careful editing a template after creating a binding closure that updates it:
				@update1 = move |state: &State| bindings!()
				// at this point the template has partially moved to the binding closure,
				// so `Deref` can no longer be used (you can edit `like => { this; }`)
			}
			
			gtk::Separator #append(&#) { }
			
			// almost the same code as above:
			BoxTemplate::new("second", &sender2) second #append(&#.root) {
				// if the field is only edited once, it is not necessary to use braces:
				label => 'bind set_label: &format!("The second count is: {}", state.count)
				
				gtk::Button::with_label("Decrease") #append(&#) {
					connect_clicked: move |_| send!(Msg::Decrease => sender2)
				}
				@update2 = move |state: &State| bindings!()
			}
			
			gtk::Separator #append(&#) { }
			
			gtk::Box #append(&#) !{
				margin_bottom: 6
				margin_end: 6
				margin_start: 6
				spacing: 6 #:
				
				// we put the independent widget (the reset button) of the `first` and `second` templates here:
				ref  first.reset #append(&#) { set_hexpand: true }
				ref second.reset #append(&#) { set_hexpand: true }
			}
		}
	}
}] // we create states and channels for both “templated components”:

fn window(app: &gtk::Application) -> gtk::ApplicationWindow {
	let (sender1, receiver1) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
	let (sender2, receiver2) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
	
	let mut state1 = State { count: 0 };
	let mut state2 = State { count: 0 };
	
	expand_view_here! { }
	
	receiver1.attach(None, move |msg| {
		update_state(&mut state1, msg);
		update1(&state1);
		glib::Continue(true)
	});
	
	receiver2.attach(None, move |msg| {
		update_state(&mut state2, msg);
		update2(&state2);
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
			move |error| glib::g_critical!("e_templates", "{error}")
		)
	};
}

use send;
