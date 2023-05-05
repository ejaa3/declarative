/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::builder_mode;
use gtk::{glib, prelude::*};

#[declarative::view {
	gtk::ApplicationWindow window !{
		application: app
		default_height: 300
		
		gtk::HeaderBar #titlebar(&#) { }
		
		gtk::Box #child(&#) {
			gtk::StackSidebar #append(&#) { set_stack: &stack }
			
			gtk::Stack stack #append(&#) {
				connect_visible_child_notify: move |_| send![() => sender]
				
				// some methods return something (in this case a `BindingBuilder`):
				bind_property: "visible-child-name", &window, "title" // separate arguments with comma
					// you can edit the return with 'back, even in builder mode:
					'back !{ sync_create; }
				
				#[allow(unused_mut)] // note that attributes affect everything inside
				gtk::Label !{
					label: "With body"
					
					// the method of this interpolation returns a `gtk::StackPage`:
					#add_titled(&#, None, "First")
						// in this case you edit the return with 'back inside the scope,
						// and also give it a variable name and make it mutable as well:
						'back mut returned_page { set_name: "first_name" }
				}
				
				gtk::Label::new(Some("Without body")) // if only need to edit return:
				#add_child(&#) 'back { set_name: "second_name"; set_title: "Second" }
				
				// the above is equivalent to:
				add_child: &gtk::Label::new(Some("As an assignment"))
				'back { set_name: "third_name"; set_title: "Third" }
				
				gtk::Label !{
					#add_child(&#) 'back {
						set_name: "fourth_name"
						'bind! set_title: &format!("Changes: {changes}")
					}
					label: "'back supports reactivity!"
				}
			}
			
			'binding update_view = move |changes: u8| bindings!()
		}
		// it is possible that in the future 'back may also destructure, or call a
		// returned functional, or assign a returned mutable reference, and so on
	}
}]

fn window(app: &gtk::Application) -> gtk::ApplicationWindow {
	let (sender, receiver) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
	let mut changes: u8 = 0;
	
	expand_view_here! { }
	
	receiver.attach(None, move |_| {
		changes = changes.wrapping_add(1);
		update_view(changes);
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
			move |error| glib::g_critical!("g_returns", "{error}")
		)
	};
}

use send;
