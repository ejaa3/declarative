/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative_gtk4::{Composable, builder_mode};
use gtk::prelude::*;
use once_cell::unsync::OnceCell;
use std::{cell::{RefCell, RefMut, Ref}, rc::Rc};

struct State { count: i32 }

// let's try to update the view from the view itself
// avoiding the application logic in the view:
struct View<Updater> {
	  state: RefCell<State>, // state must be mutable, and so RefCell
	updater: OnceCell<Updater>, // with this we will update the view
} // this approach requires sharing (as Rc<Component>)

impl<Updater: Fn(Ref<State>)> View<Updater> {
	fn new() -> Self {
		let state = State { count: 0 }.into();
		Self { state, updater: OnceCell::new() }
	}
	
	fn update(&self, state_updater: fn(RefMut<State>)) {
		state_updater(self.state.borrow_mut()); // with this we update the state
		
		// application logic here
		
		// now let's update the view:
		self.updater.get().unwrap() (self.state.borrow())
	}
}

declarative::view! {
	gtk::ApplicationWindow window !{ // builder mode
		application: app
		
		// 'bind_only because there is no state in the startup scope:
		'bind_only if state.count % 2 == 0 {
			set_title: Some("The value is even")
		} else { // builder mode does not affect 'bind and the like
			set_title: Some("The value is odd")
		} #
		
		set_titlebar => gtk::HeaderBar 'wrap Some { }
		
		gtk::Grid !{
			column_spacing: 6
			row_spacing: 6
			margin_top: 6
			margin_bottom: 6
			margin_start: 6
			margin_end: 6
			
			gtk::Label my_label 'with (0, 0, 2, 1) {
				set_hexpand: true
				'bind_only set_label: &format!("The count is: {}", state.count)
			}
			
			gtk::Button::with_label("Increase") 'with (0, 1, 1, 1) {
				// now instead of sending messages we have to do:
				connect_clicked: 'clone view move |_| view.update(
					|mut state| state.count = state.count.wrapping_add(1)
				)
			}
			
			gtk::Button::with_label("Decrease") 'with (1, 1, 1, 1) {
				// we clone `view` again to move the clone to the
				// closure and thus be able to assign the updater to it
				connect_clicked: 'clone view move |_| view.update(
					|mut state| state.count = state.count.wrapping_sub(1)
				)
			}
		}
		
		'binding updater: 'clone window move |state: Ref<State>| { bindings!(); }
	} ..
	
	fn window(app: &gtk::Application) -> gtk::ApplicationWindow {
		let view = Rc::from(View::new()); // we create shareable `view`
		
		expand_view_here!(); // `view` is shared here
		updater(view.state.borrow()); // initial update
		
		// we give the binding closure to the state:
		view.updater.set(updater).unwrap_or(());
		window
	}
}

fn main() -> gtk::glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(move |app| window(app).present());
	app.run()
}
