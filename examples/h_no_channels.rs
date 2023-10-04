/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{clone, construct, view};
use gtk::{glib, prelude::*};
use std::{cell::{OnceCell, RefCell, RefMut}, rc::Rc};

// let's make the previous example work without using
// channels as in the example `d_no_channels` (using generics)

// P and R are for the closures that refresh the parent
// component of this one, and this component respectively:
struct Child <P = (), R = ()> { // generic default types are unnecessary
	count: RefCell<u8>, // a mutable state for the child
	  nth: &'static str, // the child number (first or second)
	
	// we need a reference to the parent component if we want to...
	parent: Rc<Parent<P>>, // communicate with it outside of `view!`
	// there may be other ways to communicate, but we do it this way for demo purposes
	
	// the main widget plus the closure that refreshes the view of this component:
	data: OnceCell<(gtk::Box, R)>,
}

#[view]      // since by default the generics are `()`, we don't have
impl Child { // to specify P and R here and in `Child::<P, R>::new()`
	fn new(nth: &'static str,
	    parent: Rc<Parent<impl Fn(&str) + 'static>> // now we must always specify the trait that...
	) -> Rc<Child<impl Fn(&str), impl Fn(u8)>> { // implements the closure that refreshes the parent
		let this = Rc::new(Child {
			nth, parent, count: RefCell::new(0), data: OnceCell::new()
		});
		
		expand_view_here! { }
		
		let data = (root, move |count| bindings!());
		this.data.set(data).unwrap_or_else(|_| panic!());
		this
	} // of course this could be an unassociated function
	
	view![ gtk::Box root {
		orientation: gtk::Orientation::Vertical
		~spacing: 6
		
		append: &_ @ gtk::Label {
			label: &format!("This is the {nth} child")
			'bind set_label: &format!("The {nth} count is: {count}")
			// the binding closure got a reference to `nth` due to the previous binding
		}
		append: &_ @ gtk::Button::with_label("Increase") {
			// now instead of sending two messages, we call two methods:
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
	} ];
}

// https://doc.rust-lang.org/rust-by-example/fn/closures/anonymity.html
impl<P, R> Child<P, R> where P: Fn(&str), R: Fn(u8) { // one more time
	fn refresh(&self) {
		self.data.get().unwrap().1 (*self.count.borrow())
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

struct Parent<R> { refresh: OnceCell<R> } // has no state

#[view]
impl<R> Parent<R> {
	fn start(app: &gtk::Application) {
		let this = Rc::new(Parent { refresh: OnceCell::new() });
		let first_child = Child::new("First", this.clone());
		// we have not written `Child::<(), ()>::new(...)`
		
		expand_view_here! { }
		
		this.refresh.set(move |nth: &_| bindings!()).unwrap_or_else(|_| panic!());
		window.present()
	}
	
	fn notify_child_update(&self, nth: &str) where R: Fn(&str) {
		self.refresh.get().unwrap() (nth)
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
			
			append: &first_child.data.get().unwrap().0
			
			append: &_.data.get().unwrap().0 @
				Child::new("Second", this.clone()) second_child { }
			
			append: &_ @ gtk::Label {
				label: "Waiting for message…"
				'bind set_label: &format!("{nth} child updated")
			}
			append: &_ @ gtk::Button::with_label("Reset first child") {
				connect_clicked: move |_| first_child.reset();
			}
			append: &_ @ gtk::Button::with_label("Reset second child") {
				connect_clicked: move |_| second_child.reset();
			}
		}
	} ];
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(Parent::<()>::start);
	app.run()
}
