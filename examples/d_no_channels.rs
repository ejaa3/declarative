/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{clone, construct, view};
use gtk::{glib, prelude::*};
use std::{cell::{OnceCell, Ref, RefCell, RefMut}, rc::Rc};

// let's try to update the state and refresh the view without using channels

struct State { count: i32 }

struct View<R> { // `R` is for the closure that [R]efreshes the view
	  state: RefCell<State>, // `state` must be mutable, and so `RefCell`
	refresh: OnceCell<R>, // with this we will refresh the view
} // this approach requires sharing (as `Rc<View>` or a static view)

#[view]
impl<R> View<R> {
	fn start(app: &gtk::Application) {
		let state = RefCell::new(State { count: 0 }); // `state` is initialized
		let view = Rc::new(View { state, refresh: OnceCell::new() });
		
		expand_view_here! { } // `view` is shared here
		
		// if there are unconsumed bindings in the view, they can be consumed outside:
		let refresh = { clone![window]; move |state: Ref<State>| bindings!() };
		// for multiple views with unconsumed bindings, `bindings!()` would consume in FIFO order
		
		refresh(view.state.borrow()); // initial refresh
		
		// we give the `refresh` closure to the state:
		view.refresh.set(refresh).unwrap_or_else(|_| panic!());
		window.present()
	}
	
	fn update(&self, update_state: fn(RefMut<State>))
	where R: Fn(Ref<State>) { // view does not mutate `state` while refreshing, and so `Ref<State>`
		update_state(self.state.borrow_mut()); // the state is mutated from the view itself
		
		// some application logic here
		
		// now let's refresh the view (state does not mutate):
		self.refresh.get().unwrap() (self.state.borrow())
	}
	
	view![ gtk::ApplicationWindow window {
		application: app
		titlebar: &gtk::HeaderBar::new()
		
		'bind match state.count % 2 == 0 {
			true  => set_title: Some("The value is even")
			false => set_title: Some("The value is odd")
		}
		
		child: &_ @ gtk::Grid {
			column_spacing: 6
			row_spacing: 6
			margin_top: 6
			margin_bottom: 6
			margin_start: 6
			~margin_end: 6
			
			attach: &_, 0, 0, 2, 1 @ gtk::Label my_label {
				hexpand: true
				'bind set_label: &format!("The count is: {}", state.count)
			}
			attach: &_, 0, 1, 1, 1 @ gtk::Button::with_label("Increase") {
				// now instead of sending messages we have to do:
				connect_clicked: clone![view; move |_| view.update(
					|mut state| state.count = state.count.wrapping_add(1)
				)]
			}
			attach: &_, 1, 1, 1, 1 @ gtk::Button::with_label("Decrease") {
				// clone `view` again to move the clone to the closure
				// so we can give the closure `refresh` to `view`
				connect_clicked: clone![view; move |_| view.update(
					|mut state| state.count = state.count.wrapping_sub(1)
				)]
			}
		}
	} ];
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(View::<()>::start);
	app.run()
}
