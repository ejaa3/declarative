/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{builder_mode, clone};
use gtk::{glib, prelude::*};
use once_cell::unsync::OnceCell;
use std::{cell::{Ref, RefCell, RefMut}, rc::Rc};

struct State { count: i32 }

// let's try to update the state from the view itself
// avoiding the application logic in the view:
struct View<U> { // `U` is for the closure that [U]pdates the view
	  state: RefCell<State>, // `state` must be mutable, and so `RefCell`
	updater: OnceCell<U>, // with this we will update the view
} // this approach requires sharing (as `Rc<View>` or a static view)

// view does not mutate `state` while “refreshing”...
impl<U> View<U> where U: Fn(Ref<State>) { // and so `Ref<State>`
	fn update(&self, state_updater: fn(RefMut<State>)) {
		// the state is mutated from the view itself:
		state_updater(self.state.borrow_mut());
		
		// application logic here
		
		// now let's update the view (state does not mutate):
		self.updater.get().unwrap() (self.state.borrow())
	}
}

#[declarative::view { // you may prefer to see the function first
	gtk::ApplicationWindow window !{
		application: app
		
		gtk::HeaderBar #titlebar(&#) { }
		
		'bind if state.count % 2 == 0 {
			set_title: Some("The value is even")
		} else {
			set_title: Some("The value is odd")
		}
		
		gtk::Grid #child(&#) !{
			column_spacing: 6
			row_spacing: 6
			margin_top: 6
			margin_bottom: 6
			margin_start: 6
			margin_end: 6 #:
			
			gtk::Label my_label #attach(&#, 0, 0, 2, 1) !{
				hexpand: true
				'bind set_label: &format!("The count is: {}", state.count)
			}
			
			gtk::Button::with_label("Increase") #attach(&#, 0, 1, 1, 1) {
				// now instead of sending messages we have to do:
				connect_clicked: clone![view; move |_| view.update(
					|mut state| state.count = state.count.wrapping_add(1)
				)]
			}
			
			gtk::Button::with_label("Decrease") #attach(&#, 1, 1, 1, 1) {
				// we clone `view` again to move the clone to the
				// closure and thus be able to assign the updater to it
				connect_clicked: clone![view; move |_| view.update(
					|mut state| state.count = state.count.wrapping_sub(1)
				)]
			}
		}
		
		@updater = { clone![window]; move |state: Ref<State>| bindings!() }
	}
}]

fn start(app: &gtk::Application) {
	let state = RefCell::new(State { count: 0 }); // `state` is initialized
	let view = Rc::new(View { state, updater: OnceCell::new() });
	
	expand_view_here! { } // `view` is shared here
	
	updater(view.state.borrow()); // initial update
	
	// we give the `updater` closure to the state:
	view.updater.set(updater).unwrap_or_else(|_| panic!());
	window.present()
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(start);
	app.run()
}
