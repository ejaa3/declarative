/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{builder_mode, clone};
use gtk::{glib, prelude::*};
use once_cell::unsync::OnceCell;
use std::{cell::{RefCell, RefMut}, rc::Rc};

// let's make the previous example work without using
// channels as in the example `d_from_view` (using generics)

// P and U are for the closures that update the parent
// component of this one, and this component respectively:
struct Child<P = (), U = ()> { // generic default types are unnecessary
	 count: RefCell<u8>, // a mutable state for the child
	   nth: &'static str, // the child number (first or second)
	parent: Rc<Parent<P>>, // a reference to the parent component
	
	// the main widget plus the closure that updates the view of this component:
	data: OnceCell<(gtk::Box, U)>,
}

#[declarative::view { // component factory
	gtk::Box root !{
		orientation: gtk::Orientation::Vertical
		spacing: 6 #:
		
		gtk::Label #append(&#) !{
			label: &format!("This is the {nth} child")
			'bind set_label: &format!("The {nth} count is: {count}")
		}
		
		@updater = move |count| bindings!()
		
		gtk::Button::with_label("Increase") #append(&#) {
			connect_clicked: clone![this; move |_| {
				this.update(|mut count| *count = count.wrapping_add(1));
				this.parent.notify_child_update(nth);
			}]
		}
		
		gtk::Button::with_label("Decrease") #append(&#) {
			connect_clicked: clone![this; move |_| {
				this.update(|mut count| *count = count.wrapping_sub(1));
				this.parent.notify_child_update(nth);
			}]
		}
	}
}]

// since by default the generics are `()`, we can associate a `new()` function conveniently:
impl Child<(), ()> { // now we don't have to specify P and U in `Child::<P, U>::new()`
	fn new(nth: &'static str,
	    parent: Rc<Parent<impl Fn(&str) + 'static>>,
	) -> Rc<Child<impl Fn(&str), impl Fn(u8)>> {
		let this = Rc::new(Child {
			nth, parent, count: RefCell::new(0), data: OnceCell::new()
		});
		
		expand_view_here! { }
		
		this.data.set((root, updater)).unwrap_or_else(|_| panic!());
		this
	} // of course this could be an unassociated function
}

impl<P, U> Child<P, U> where P: Fn(&str), U: Fn(u8) {
	fn update_view(&self) {
		self.data.get().unwrap().1 (*self.count.borrow())
	}
	
	fn update(&self, count_updater: fn(RefMut<u8>)) {
		count_updater(self.count.borrow_mut());
		self.update_view();
	}
	
	fn reset(&self) {
		*self.count.borrow_mut() = 0;
		self.parent.notify_child_update(self.nth);
		self.update_view();
	}
}

struct Parent<U = ()> { updater: OnceCell<U> } // has no state

#[declarative::view {
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
			
			Child::new("First", this.clone()) first_child {
				#append(&#.data.get().unwrap().0) // interpolation is a bit more verbose
			}
			
			ref second_child { #append(&#.data.get().unwrap().0) }
			
			gtk::Label #append(&#) !{
				label: "Waiting for message…"
				'bind set_label: &format!("{nth} child updated")
			}
			
			gtk::Button::with_label("Reset first child") #append(&#) {
				connect_clicked: move |_| first_child.reset();
			}
			
			gtk::Button::with_label("Reset second child") #append(&#) {
				connect_clicked: move |_| second_child.reset();
			}
		}
		
		@updater = move |nth: &_| bindings!()
	}
}]

impl Parent<()> {
	fn start(app: &gtk::Application) {
		let this = Rc::new(Parent { updater: OnceCell::new() });
		let second_child = Child::new("Second", this.clone());
		
		expand_view_here! { }
		
		this.updater.set(updater).unwrap_or_else(|_| panic!());
		window.present()
	}
}

impl<U> Parent<U> where U: Fn(&str) {
	fn notify_child_update(&self, nth: &str) {
		self.updater.get().unwrap() (nth)
	}
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(Parent::start);
	app.run()
}
