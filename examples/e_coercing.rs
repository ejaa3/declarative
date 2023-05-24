/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{builder_mode, clone};
use gtk::{glib, prelude::*};
use once_cell::unsync::OnceCell;
use std::{cell::{Ref, RefCell, RefMut}, rc::Rc};

// let's take advantage of the previous example, where we dedicate a generic for a closure that
// updated the view, but now we're going to avoid it, which has its advantages and disadvantages

struct State { count: i32 }

// we need a struct that contains the widgets with “properties” marked with 'bind
// and a function pointer that will update those widgets based on the state:
struct Widgets {
	 window: gtk::ApplicationWindow,
	  label: gtk::Label,
	updater: fn(&Self, Ref<State>), // does not mutate state due to `Ref<State>`
}

struct View { // now there are no generics
	  state: RefCell<State>,
	widgets: OnceCell<Widgets>, // we use `Widgets` instead of a generic
}

#[declarative::view {
	gtk::ApplicationWindow window !{ // we name the updateable widgets the same as the fields
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
			
			gtk::Label label #attach(&#, 0, 0, 2, 1) !{ // the other widget
				hexpand: true
				'bind set_label: &format!("The count is: {}", state.count)
			}
			
			gtk::Button::with_label("Increase") #attach(&#, 0, 1, 1, 1) {
				connect_clicked: clone![view; move |_| view.update(
					|mut state| state.count = state.count.wrapping_add(1)
				)]
			}
			
			gtk::Button::with_label("Decrease") #attach(&#, 1, 1, 1, 1) {
				connect_clicked: clone![view; move |_| view.update(
					|mut state| state.count = state.count.wrapping_sub(1)
				)]
			}
		}
		
		// now this closure does not capture anything (nothing moves to it):
		@updater = | // thus we can coerce this closure to a function pointer
			// remember the parameters of the `updater` field of `Widgets`:
			Widgets { window, label, .. }: &_, state: Ref<State>,
			// we destructure `Widgets` due to the `bindings!()` expansion
		| bindings!()
	}
}]

impl View {
	fn start(app: &gtk::Application) {
		let state = RefCell::new(State { count: 0 });
		let view = Rc::new(Self { state, widgets: OnceCell::new() });
		
		expand_view_here! { }
		
		let widgets = Widgets { window, label, updater };
		updater(&widgets, view.state.borrow()); // initial update
		
		// we give `widgets` to `view`:
		view.widgets.set(widgets).unwrap_or(());
		view.widgets.get().unwrap().window.present() // we show the window
	}
	
	fn update(&self, state_updater: fn(RefMut<State>)) {
		state_updater(self.state.borrow_mut());
		
		// application logic here
		
		// we update the view widgets a little differently:
		let widgets = self.widgets.get().unwrap();
		(widgets.updater) (widgets, self.state.borrow());
	}
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(View::start);
	app.run()
}

// in short, generics can be avoided by coercing them to function pointers,
// which involves writing `Widgets` structs and destructuring them in the view
//
// this could be more verbose, but is useful if you
// need to have an accessible reference to each widget
//
// the other way (which I don't recommend) is to use `Box<dyn Fn(State)>`
// which avoids both generics and structs, but allocates on the heap
