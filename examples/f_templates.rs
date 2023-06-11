/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{builder_mode, clone, view};
use gtk::{glib, prelude::*};

enum Msg { Increase, Decrease, Reset } // channels again

macro_rules! send { [$msg:expr => $tx:expr] => [$tx.send($msg).unwrap()] }

#[view]
impl BoxTemplate { // we are implementing a struct generated semi-automatically by the view
	view! { // specifically this is the struct:
		struct BoxTemplate { } // it is also possible to change the visibility or add more fields if needed
		// each struct must be followed by an item (we will generate its fields from the items as follows)
		
		// to “export” this widget in the template, you must give it a name followed by a colon:
		gtk::Box root: !{ // the type is assumed to be `gtk::Box`, but an incorrect assumption is possible
			// in such case the correct type should be specified after the colon (only type paths are supported)
			// it is also possible to change the visibility with `pub` before the `mut` or the name
			
			orientation: gtk::Orientation::Vertical
			spacing: 6
			margin_top: 6
			margin_bottom: 6
			margin_start: 6
			~margin_end: 6
			
			gtk::Label label: #append(&#) !{ // we also export this widget, so it has a name followed by a colon
				label: glib::gformat!("This is the {nth} view")
			}
			
			gtk::Button::with_label("Increase") #append(&#) { // we do not export this (not even named)
				connect_clicked: clone![tx; move |_| send!(Msg::Increase => tx)]
			}
		}
		// we will also export this widget although it is independent of the root:
		gtk::Button reset: !{ // we must do something with it in the main view so that it is not lost
			~label: glib::gformat!("Reset {nth}")
			connect_clicked: clone![tx; move |_| send!(Msg::Reset => tx)]
		}
	}
	
	// if `Default` were implemented, there would be no need to write `::new()` but there
	// would be no parameters (we could also have defined an unassociated function):
	fn new(nth: &str, tx: &glib::Sender<Msg>) -> Self {
		expand_view_here! { }
		Self { root, label, reset }
	}
}

// if we had used `#[view]` for a `mod`, the struct would be created inside it
//
// for the rest (`impl`, `trait`, `fn`, etc.) it is created in the same scope (here for the above case)

impl std::ops::Deref for BoxTemplate { // views get along with `Deref`
	type Target = gtk::Box;
	
	fn deref(&self) -> &Self::Target {
		&self.root // let's try `Deref` with the root widget
	}
}

#[view]
mod example { // now let's use the template:
	use super::*;
	
	// let's create two states and two channels for two templates:
	pub fn start(app: &gtk::Application) {
		let (tx_1, rx_1) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
		let (tx_2, rx_2) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
		let (mut count_1, mut count_2) = (0, 0); // the states
		
		expand_view_here! { }
		
		let update = |count: &mut u8, msg| match msg {
			Msg::Increase => *count = count.wrapping_add(1),
			Msg::Decrease => *count = count.wrapping_sub(1),
			Msg::Reset    => *count = 0,
		}; // this closure does not capture anything (could be a function)
		
		rx_1.attach(None, move |msg| {
			update(&mut count_1, msg);
			refresh_template_1(count_1);
			glib::Continue(true)
		});
		
		rx_2.attach(None, move |msg| {
			update(&mut count_2, msg);
			refresh_template_2(count_2);
			glib::Continue(true)
		});
		
		window.present()
	}
	
	view! {
		gtk::ApplicationWindow window !{
			application: app
			title: "Templates"
			
			gtk::HeaderBar #titlebar(&#) { }
			
			gtk::Box #child(&#) !{
				orientation: gtk::Orientation::Vertical
				~spacing: 6
				
				BoxTemplate::new("first", &tx_1) first {
					#append(&#.root) // `BoxTemplate` is not a widget but its `root` field is
					
					label => { // we are editing the `label` field
						'bind set_label: &format!("The first count is: {count}")
					}
					
					// we can #interpolate with just the method thanks to `Deref`;
					// otherwise we would have to edit the `root` field as `label`:
					gtk::Button::with_label("Decrease") #append(&#) {
						connect_clicked: move |_| send!(Msg::Decrease => tx_1)
					}
					
					// be careful editing a template after creating a closure that refreshes it:
					@refresh_template_1 = move |count| bindings!()
					// at this point the template has partially moved to the closure,
					// so `Deref` can no longer be used (you can edit `like => { this; }`)
				}
				
				gtk::Separator #append(&#) { }
				
				// almost the same code as above:
				BoxTemplate::new("second", &tx_2) second #append(&#.root) {
					// if the field is only edited once, it is not necessary to use braces:
					'bind label.set_label: &format!("The second count is: {count}")
					
					// if `Deref` is not implemented, it is possible to #interpolate like this:
					gtk::Button::with_label("Decrease") #root.append(&#) {
						connect_clicked: move |_| send!(Msg::Decrease => tx_2)
					}
					
					@refresh_template_2 = move |count| bindings!()
				}
				
				gtk::Separator #append(&#) { }
				
				gtk::Box #append(&#) !{
					margin_bottom: 6
					margin_end: 6
					margin_start: 6
					~spacing: 6
					
					// we put the independent widget (the reset button)
					// of the `first` and `second` templates here:
					ref  first.reset #append(&#) { set_hexpand: true }
					ref second.reset #append(&#) { set_hexpand: true }
				}
			}
		}
	}
}

// in the `g_components` example we will avoid manually
// creating states and channels for each template

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(example::start);
	app.run()
}
