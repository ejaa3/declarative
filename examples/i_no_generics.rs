/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{clone, construct, view};
use gtk::{glib, prelude::*};
use std::{cell::{OnceCell, RefCell, RefMut}, rc::Rc};

// let's make the previous example work without using
// generics as in the example `e_no_generics` (using templates)

struct Child { // now there are no generics
	  count: RefCell<u8>,
	    nth: &'static str,
	 parent: Rc<Parent>, // here neither
	widgets: OnceCell<Widgets>, // here neither
}

#[view]
impl Child { // here neither
	fn new(nth: &'static str, parent: Rc<Parent>) -> Rc<Self> {
		let this = Rc::new(Self {
			nth, parent, count: RefCell::new(0), widgets: OnceCell::new()
		});
		
		expand_view_here! { }
		
		this.widgets.set(Widgets { root, label }).unwrap_or_else(|_| panic!());
		this
	}
	
	view! {
		struct Widgets { }
		
		gtk::Box ref root { // we export the root widget
			orientation: gtk::Orientation::Vertical
			~spacing: 6
			
			append: &_ @ gtk::Label ref label { // we also export widgets that refresh
				label: &format!("This is the {nth} child")
				
				// we can use `self` here because of the position of `bindings!` (below):
				'bind set_label: &format!("The {} count is: {}", self.nth, self.count.borrow())
			}
			append: &_ @ gtk::Button::with_label("Increase") {
				connect_clicked: clone![this; move |_| {
					this.update(|mut count| *count = count.wrapping_add(1));
					this.parent.notify_child_update(nth);
				}]
			}
			append: &_ @ gtk::Button::with_label("Decrease") {
				connect_clicked: clone![this; move |_| {
					this.update(|mut count| *count = count.wrapping_sub(1));
					this.parent.notify_child_update(nth);
				}]
			}
		}
	}
	
	fn refresh(&self) { // we only destructure the refreshable widgets:
		let Widgets { label, .. } = self.widgets.get().unwrap();
		bindings! { }
	}
	
	fn update(&self, update_count: fn(RefMut<u8>)) {
		update_count(self.count.borrow_mut());
		self.refresh();
	}
	
	fn reset(&self) {
		*self.count.borrow_mut() = 0;
		self.parent.notify_child_update(self.nth);
		self.refresh();
	}
}

struct Parent { label: OnceCell<gtk::Label> } // parent has only one refreshable widget

#[view]
impl Parent {
	fn start(app: &gtk::Application) {
		let this = Rc::new(Parent { label: OnceCell::new() });
		let first_child = Child::new("First", this.clone());
		
		expand_view_here! { }
		
		this.label.set(label).unwrap_or_else(|_| panic!());
		window.present()
	}
	
	view![ gtk::ApplicationWindow window {
		application: app
		title: "Components"
		titlebar: &gtk::HeaderBar::new()
		
		child: &_ @ gtk::Box {
			orientation: gtk::Orientation::Vertical
			spacing: 6
			margin_top: 6
			margin_bottom: 6
			margin_start: 6
			~margin_end: 6
			
			// we compose a bit differently than before:
			append: &first_child.widgets.get().unwrap().root
			
			append: &_.widgets.get().unwrap().root @
				Child::new("Second", this.clone()) second_child { }
			
			append: &_ @ gtk::Label label {
				label: "Waiting for message…"
				'bind set_label: &format!("{nth} child updated")
			}
			append: &_ @ gtk::Button::with_label("Reset first child") {
				connect_clicked: move |_| first_child.reset()
			}
			append: &_ @ gtk::Button::with_label("Reset second child") {
				connect_clicked: move |_| second_child.reset()
			}
		}
	} ];
	
	fn notify_child_update(&self, nth: &str) {
		let label = self.label.get().unwrap();
		bindings! { }
	}
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(Parent::start);
	app.run()
}
