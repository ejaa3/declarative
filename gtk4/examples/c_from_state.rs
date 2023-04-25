/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative_gtk4::Composable;
use gtk::prelude::*;
use once_cell::unsync::OnceCell;
use std::{cell::Cell, rc::Rc};

struct State { // let's try to update the view from the state itself
	  count: Cell<i32>, // fields must be mutable, and so Cell
	binding: OnceCell<Box<dyn Fn(&Self)>> // with this we will update
} // this approach requires state sharing (as Rc<State>)

impl State {
	fn update(&self, self_updater: fn(&Self)) {
		self_updater(self); // with this we update the state
		// application logic here?
		
		// now let's update the view:
		if let Some(ref closure) = self.binding.get() { closure(self) }
	}
}

declarative::view! {
	gtk::ApplicationWindow window !{
		application: app
		title: "Count unchanged"
		titlebar => gtk::HeaderBar { }
		build!
		
		// now we have to use .get() because of Cell:
		'bind_only if state.count.get() % 2 == 0 {
			set_title: Some("The value is even")
		} else { // builder mode does not affect 'bind and the like
			set_title: Some("The value is odd")
		}
		
		gtk::Grid !{ // builder mode
			column_spacing: 6
			row_spacing: 6
			margin_top: 6
			margin_bottom: 6
			margin_start: 6
			margin_end: 6
			
			// builder mode does not affect “component assignments”;
			// if it were to affect it, it would be a breaking change:
			gtk::Label my_label 'with (0, 0, 2, 1) {
				set_hexpand: true
				'bind set_label: &format!("The count is: {}", state.count.get())
			}
			
			build! // breaking change would only affect builders that require a final call like build!
			// however, if called without first assigning components, the code would be immune to breaking change
			
			gtk::Button::with_label("Increase") 'with (0, 1, 1, 1) {
				// now instead of sending messages we have to do:
				connect_clicked: 'clone state move |_| state.update(
					|state| state.count.set(state.count.get().wrapping_add(1))
				) // you may prefer a macro instead of writing 'clone state move |_| state.update(|state| { })
			}
			
			gtk::Button::with_label("Decrease") 'with (1, 1, 1, 1) {
				// we clone the state again to move the clone to the
				// closure and thus be able to assign the binding to it
				connect_clicked: 'clone state move |_| state.update(
					|state| state.count.set(state.count.get().wrapping_sub(1))
				)
			}
		}
		
		'binding update_view: 'clone window move |state: &State| { bindings!(); }
	} ..
	
	fn window(app: &gtk::Application) -> gtk::ApplicationWindow {
		let state: Rc<_> = State {
			  count: 0.into(),
			binding: OnceCell::new(),
		}.into();
		
		expand_view_here!();
		
		state.binding.set(Box::new(update_view)).unwrap_or(());
		window
	}
}

fn main() -> gtk::glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(move |app| window(app).present());
	app.run()
}
