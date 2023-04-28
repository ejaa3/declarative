/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative_gtk4::Composable;
use gtk::{glib, prelude::*};
use std::cell::Cell;

declarative::view! { // first look at the window() function below
	gtk::ApplicationWindow::new(app) window { // and then come back here
		set_title: Some("Reactivity")
		
		gtk::Box::new(gtk::Orientation::Vertical, 6) {
			set_margin_top: 6
			set_margin_bottom: 6
			
			gtk::Label {
				// to make a “property assignment” react to changes, use 'bind:
				'bind set_label: &format!("['bind] Count: {count}") // but `bind also initializes
				// since we referred to the count with `count` instead of
				// `main_count.get()`, we needed the `count` variable before the view
			}
			gtk::Label {
				'bind { // multiple “property assignments” within braces are also valid
					set_label: &format!("['bind] Count with tooltip: {count}")
					set_tooltip_text: Some(&format!("['bind] Count: {count}"))
				} // although you could use 'bind' on each instead
			}
			gtk::Label {
				// 'bind can update conditionally, but always initializes unconditionally:
				'bind if count % 2 == 0 { // only `if` (and thus `if let`) is allowed
					set_label: &format!("['bind] Even count: {count}") // at the beginning this is a lie
					// more “property assignments” are accepted but no conditions
				} // there cannot be an `else`
			}
			
			// use 'binding to create a binding closure to update the view appropriately:
			'binding update_first_labels: move |count: u8| { bindings!(); }
			// you can see that it has one parameter although there could be several, but their names
			// must match what is supposed to change in reactive assignments (with 'bind and the like)
			//
			// you can also see the statement `bindings!();` which consumes everything
			// declared with 'bind and the like before this point, regardless of scope,
			// and expands them at that position within the binding closure which prevents
			// a future new binding closure from updating the same as the current one
			
			gtk::Label {
				set_label: "['bind_now] Waiting for an even number…"
				
				// unlike 'bind, 'bind_now initializes conditionally:
				'bind_now if count % 2 == 0 {
					set_label: &format!("['bind_now] Even count (really): {count}")
				} // and allows `else if` and `else`
			}
			gtk::Label {
				'bind_now match count % 2 == 0 { // and allows `match`
					true  => set_label: "['bind_now] The count is even" // commas are not allowed
					false => set_label: "['bind_now] The count is odd"; // (semicolons are)
					#[allow(unreachable_patterns)]
					_ => { /* between braces is valid various property assignments and inner conditionals */ }
				}
			}
			gtk::Label {
				// the above is more or less equivalent to:
				'bind set_label: match count % 2 == 0 {
					true  => "['bind] The count is even", // comma required
					false => "['bind] The count is odd" // (this is normal Rust)
				}
			}
			gtk::Label {
				set_label: "['bind_only] Waiting for a change…"
				
				// 'bind_only is like 'bind_now, but does not initialize (only reacts):
				'bind_only set_label: &format!("['bind_only] Count: {}", count)
			}
			
			// unlike the previous binding closure, this receives a Cell<u8> instead of u8:
			'binding update_latest_labels: move |count: Cell<u8>| {
				// however, the reactive assignments above still use `count` instead of `count.get()`
				let count = count.get(); // so we get the count here
				bindings!(); // and now we can consume the bindings without problems
			}
		}
		
		// if it is necessary to wrap the assigned object,
		// such as Some(object) in this case, use 'wrap:
		set_titlebar => gtk::HeaderBar 'wrap Some {
			pack_start => gtk::Button::with_label("Increase") {
				connect_clicked: move |_| {
					// we mutate the count:
					main_count.set(main_count.get().wrapping_add(1));
					// the first binding closure requires a u8:
					update_first_labels(main_count.get());
					// the second requires a Cell<u8>:
					update_latest_labels(main_count.clone());
				}
			}
		}
	} ..
	
	fn window(app: &gtk::Application) -> gtk::ApplicationWindow {
		// this will mutate from a closure in
		// the view that does not implement FnMut:
		let main_count = Cell::new(1_u8); // we put an odd number
		
		// we get the count here for a reason explained above:
		let count = main_count.get();
		
		expand_view_here!();
		window
	}
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(move |app| window(app).present());
	app.run()
}
