/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{builder_mode, clone, view};
use gtk::{glib, prelude::*};

enum Msg { Increase, Decrease, Reset } // channels again

macro_rules! send { [$msg:expr => $tx:expr] => [$tx.send($msg).unwrap()] }

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

#[view]
// if `Default` were implemented, there would be no need
// to write `::new()` but there would be no parameters:
impl BoxTemplate {
	// could also be unassociated function:
	fn new(nth: &str, tx: &glib::Sender<Msg>) -> Self {
		expand_view_here! { }
		Self { root, label, reset }
	}
	
	view! { // now yes, the first thing to do is a view
		gtk::Box root !{ // it is convenient to name the widgets the same as the fields
			orientation: gtk::Orientation::Vertical
			spacing: 6
			margin_top: 6
			margin_bottom: 6
			margin_start: 6
			~margin_end: 6
			
			gtk::Label label #append(&#) !{ // we will publish this widget and so we name it
				label: glib::gformat!("This is the {nth} view")
			}
			
			gtk::Button::with_label("Increase") #append(&#) { // this is private (not even named)
				connect_clicked: clone![tx; move |_| send!(Msg::Increase => tx)]
			}
		}
		// we will also export this widget although it is independent of the root:
		gtk::Button reset !{ // we must do something with it in the main view so that it is not lost
			~label: glib::gformat!("Reset {nth}")
			connect_clicked: clone![tx; move |_| send!(Msg::Reset => tx)]
		}
	}
} // now let's use the template:

#[view]
mod example {
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
					
					// we can interpolate here thanks to `Deref`;
					// otherwise we would have to edit the `root` field as `label`:
					gtk::Button::with_label("Decrease") #append(&#) {
						connect_clicked: move |_| send!(Msg::Decrease => tx_1)
					}
					
					// be careful editing a template after creating a binding closure that updates it:
					@refresh_template_1 = move |count| bindings!()
					// at this point the template has partially moved to the binding closure,
					// so `Deref` can no longer be used (you can edit `like => { this; }`)
				}
				
				gtk::Separator #append(&#) { }
				
				// almost the same code as above:
				BoxTemplate::new("second", &tx_2) second #append(&#.root) {
					// if the field is only edited once, it is not necessary to use braces:
					label => 'bind set_label: &format!("The second count is: {count}")
					
					gtk::Button::with_label("Decrease") #append(&#) {
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
