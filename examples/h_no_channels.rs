/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{builder_mode, clone, view};
use gtk::{glib, prelude::*};
use once_cell::unsync::OnceCell;
use std::{cell::{RefCell, RefMut}, rc::Rc};

// let's make the previous example work without using
// channels as in the example `d_no_channels` (using generics)

// P and R are for the closures that refresh the parent
// component of this one, and this component respectively:
struct Child<P = (), R = ()> { // generic default types are unnecessary
	count: RefCell<u8>, // a mutable state for the child
	  nth: &'static str, // the child number (first or second)
	
	// we need a reference to the parent component if we want to...
	parent: Rc<Parent<P>>, // communicate with it outside of `view!`
	// there may be other ways to communicate, but we do it this way for demo purposes
	
	// the main widget plus the closure that refreshes the view of this component:
	data: OnceCell<(gtk::Box, R)>,
}

#[view]
// since by default the generics are `()`, we can associate a `new()` function conveniently:
impl Child<(), ()> { // now we don't have to specify P and R in `Child::<P, R>::new()`
	fn new(nth: &'static str,
	    parent: Rc<Parent<impl Fn(&str) + 'static>>, // now we must always specify the trait that...
	) -> Rc<Child<impl Fn(&str), impl Fn(u8)>> { // implements the closure that refreshes the parent
		let this = Rc::new(Child {
			nth, parent, count: RefCell::new(0), data: OnceCell::new()
		});
		
		expand_view_here! { }
		
		let data = (root, move |count| bindings!());
		this.data.set(data).unwrap_or_else(|_| panic!());
		this
	} // of course this could be an unassociated function
	
	view! {
		gtk::Box root !{
			orientation: gtk::Orientation::Vertical
			~spacing: 6
			
			gtk::Label #append(&#) !{
				label: &format!("This is the {nth} child")
				'bind set_label: &format!("The {nth} count is: {count}")
			}
			
			gtk::Button::with_label("Increase") #append(&#) {
				// now instead of sending two messages, we call two methods:
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
	}
}

// https://doc.rust-lang.org/rust-by-example/fn/closures/anonymity.html
impl<P, U> Child<P, U> where P: Fn(&str), U: Fn(u8) { // one more time
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

struct Parent<U> { refresh: OnceCell<U> } // has no state

#[view]
impl<U> Parent<U> {
	fn start(app: &gtk::Application) {
		let this = Rc::new(Parent { refresh: OnceCell::new() });
		let first_child = Child::new("First", this.clone());
		// we have not written `Child::<(), ()>::new(...)`
		
		expand_view_here! { }
		
		this.refresh.set(move |nth: &_| bindings!()).unwrap_or_else(|_| panic!());
		window.present()
	}
	
	fn notify_child_update(&self, nth: &str) where U: Fn(&str) {
		self.refresh.get().unwrap() (nth)
	}
	
	view! {
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
				~margin_end: 6
				
				// interpolation is a bit more verbose:
				ref first_child { #append(&#.data.get().unwrap().0) }
				
				Child::new("Second", this.clone()) second_child {
					// verbose interpolations should be in scope:
					#append(&#.data.get().unwrap().0)
				}
				
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
		}
	}
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(Parent::<()>::start);
	app.run()
}
