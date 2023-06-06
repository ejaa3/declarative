/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{builder_mode, clone, view};
use gtk::{glib, prelude::*};
use std::{cell::{OnceCell, RefCell, RefMut}, rc::Rc};

// let's take advantage of the previous example, where we dedicate a generic for a closure that
// refreshed the view, but now we're going to avoid it, which has its advantages and disadvantages

struct State { count: i32 }

// we need a struct that contains the widgets with “properties” marked with 'bind:
struct Widgets { window: gtk::ApplicationWindow, label: gtk::Label }
// in the future declarative should be able to create structures automatically

struct View { // now there are no generics
	  state: RefCell<State>,
	widgets: OnceCell<Widgets>, // we use `Widgets` instead of a generic
}

#[view]
impl View {
	fn refresh(&self) {
		// we destructure `Widgets` due to the `bindings!` expansion:
		let Widgets { window, label } = self.widgets.get().unwrap();
		let state = self.state.borrow(); // the state does not change
		bindings! { }
	}
	
	fn start(app: &gtk::Application) {
		let state = RefCell::new(State { count: 0 });
		let view = Rc::new(Self { state, widgets: OnceCell::new() });
		
		expand_view_here! { }
		
		// we give the refreshable widgets to `view`:
		view.widgets.set(Widgets { window, label }).unwrap_or(());
		view.refresh(); // initial view refresh
		view.widgets.get().unwrap().window.present() // we show the window
	}
	
	fn update(&self, update_state: fn(RefMut<State>)) {
		update_state(self.state.borrow_mut());
		// some application logic here
		self.refresh() // remember to refresh after updating the state
	}
	
	view! {
		// we name the refreshable widgets the same as the fields:
		gtk::ApplicationWindow window !{
			application: app
			
			gtk::HeaderBar #titlebar(&#) { }
			
			'bind match state.count % 2 == 0 {
				true  => set_title: Some("The value is even")
				false => set_title: Some("The value is odd")
			}
			
			gtk::Grid #child(&#) !{
				column_spacing: 6
				row_spacing: 6
				margin_top: 6
				margin_bottom: 6
				margin_start: 6
				~margin_end: 6
				
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
		}
	}
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(View::start);
	app.run()
}
