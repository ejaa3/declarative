/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{construct, view};
use gtk::{glib, prelude::*};

fn become_the_third(page: &gtk::StackPage) {
	page.set_name("third_page");
	page.set_title("Third");
}

fn add_label(name: &str, title: &str, stack: &gtk::Stack) -> gtk::Label {
	let label = gtk::Label::default();
	stack.add_titled(&label, Some(name), title);
	label
}

#[view]
impl Template {
	pub fn start(app: &gtk::Application) {
		let changes = std::cell::Cell::new(0_u8);
		expand_view_here! { }
		window.present()
	}
	
	view! {
		#[allow(unused)]
		struct Template { }
		
		gtk::ApplicationWindow window {
			application: app
			default_height: 300
			default_width: 360
			titlebar: &gtk::HeaderBar::new()
			
			child: &_ @ gtk::Box {
				append: &_ @ gtk::StackSidebar { stack: &stack }
				append: &_ @ gtk::Stack stack {
					hexpand: true
					margin_bottom: 12
					margin_end: 12
					margin_start: 12
					~margin_top: 12
					
					// some methods return something, in this case a `BindingBuilder`:
					bind_property: "visible-child-name", &window, "title"
						// 'back edits the return of the function or method called back
						'back { sync_create; } // by default edits under the builder pattern
						// the tildes explained in the first example work the same with 'back,
						// specifically the fourth and fifth arm of the macro `construct!`
					
					add_titled: &_, None, "First" // this method returns a `gtk::StackPage`
						@ gtk::Label { label: "Composition" } // 'back must be placed after items
						'back { set_name: "first_page" }! // with `!` does not edit under the builder pattern
					
					// the above is equivalent to:
					add_titled: &gtk::Label::new(Some("Non-composition")), None, "Second"
						// we can export the return to the `Template` struct like this:
						'back ref returned_page as gtk::StackPage { set_name: "second_page" }!
					
					add_child: &_ @ gtk::Label { label: "'back supports reactivity!" } 'back {
						become_the_third: &_ // a function defined above within the scope of 'back
						'bind set_title: &format!("Changes: {}", changes.get())
					}!
					
					#[allow(unused_mut)] // note that attributes affect everything inside items and 'back
					add_label: "fourth_page", "Fourth", &_ // 'back with a function defined above
						'back mut { set_label: "Mutable return" }! // `mut` if the return needs to be mutable
					
					add_label: "fifth_page", "Fifth", &_ // even reactively
						'back { 'bind set_label: &format!("Changes (again): {}", changes.get()) }!
					
					connect_visible_child_notify: move |_| {
						changes.set(changes.get().wrapping_add(1));
						bindings! { }
					}
				}
			}!
		}
	}
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(Template::start);
	app.run()
}
