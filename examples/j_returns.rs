/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{builder_mode, view};
use gtk::{glib, prelude::*};

// two small extensions for demo purposes:

fn inner_extension(page: &gtk::StackPage) {
	page.set_name("fourth_name");
	page.set_title("Fourth");
}

fn outer_extension(stack: &gtk::Stack, name: &str, title: &str) -> gtk::Label {
	let label = gtk::Label::default();
	stack.add_titled(&label, Some(name), title);
	label
}

#[view]
mod example {
	use super::*;
	
	pub fn start(app: &gtk::Application) {
		let changes = std::cell::Cell::new(0_u8);
		expand_view_here! { }
		window.present()
	}
	
	view! {
		gtk::ApplicationWindow window !{
			application: app
			default_height: 300
			default_width: 360
			
			gtk::HeaderBar #titlebar(&#) { }
			
			gtk::Box #child(&#) {
				gtk::StackSidebar #append(&#) { set_stack: &stack }
				
				gtk::Stack stack #append(&#) !{
					hexpand: true
					margin_bottom: 12
					margin_end: 12
					margin_start: 12
					~margin_top: 12
					
					// some methods return something (in this case a `BindingBuilder`):
					bind_property: "visible-child-name", &window, "title" // separate arguments with comma
						// 'back edits the return of the method or interpolation that was called back
						'back !{ sync_create; } // even in builder mode (a `.build()` is chained)
					
					#[allow(unused_mut)] // note that attributes affect everything inside
					gtk::Label !{
						label: "With body"
						
						// the method of this interpolation returns a `gtk::StackPage`:
						#add_titled(&#, None, "First")
							// in this case you edit the return with 'back within the scope,
							// and also give it a variable name and make it mutable as well:
							'back mut returned_page { set_name: "first_name" }
					}
					
					gtk::Label::new(Some("Without body")) #add_child(&#)
					// if you want to edit the return of the interpolation without
					// editing the item, you can avoid extra braces with 'back:
					'back { set_name: "second_name"; set_title: "Second" }
					
					// the above is equivalent to:
					add_child: &gtk::Label::new(Some("As an argument"))
						'back { set_name: "third_name"; set_title: "Third" }
					
					gtk::Label !{
						#add_child(&#) 'back {
							@inner_extension(&#) // you can extend what `add_child()` returns
							'bind set_title: &format!("Changes: {}", changes.get())
						}
						label: "'back supports reactivity!"
					}
					
					// you can also edit the return of an `@extension(#)` with 'back:
					@outer_extension(&#, "fifth_name", "Fifth")
						'back { set_label: "From an extension" }
					
					@outer_extension(&#, "sixth_name", "Sixth") 'back {
						'bind set_label: &format!("Changes (from an extension): {}", changes.get())
					} // even reactively
					
					connect_visible_child_notify: @move |_| {
						changes.set(changes.get().wrapping_add(1));
						bindings! { }
					}
				}
			}
		}
	}
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(example::start);
	app.run()
}
