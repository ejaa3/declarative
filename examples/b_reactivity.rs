/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::builder_mode;
use gtk::{glib, prelude::*};
use std::cell::Cell;

#[declarative::view { // first look at the main function below
	gtk::ApplicationWindow window !{ // and then come back here
		application: app
		title: "Reactivity"
		
		gtk::Box #child(&#) !{
			orientation: gtk::Orientation::Vertical
			// semicolons are optional but good separators:
			spacing: 6; margin_top: 6; margin_bottom: 6 #..
			
			gtk::Label #append(&#) !{
				label: "Waiting for a change…" // 'bind does not initialize (read below)
				
				// to make a method call react to changes, use 'bind:
				'bind set_label: &format!("Count: {count}") // is immune to builder mode
			}
			gtk::Label #append(&#) {
				// if you want 'bind to initialize, use the exclamation mark:
				'bind! { // multiple method calls inside braces are also valid
					set_label: &format!("Count with tooltip: {count}")
					set_tooltip_text: Some(&format!("Count: {count}"))
				} // although you could use 'bind on each instead
				
				// since we referred to the count with `count` instead of `main_count.get()`,
				// we needed the `count` variable before the view because we are initializing
			}
			gtk::Label #append(&#) {
				// with colon updates conditionally, but initializes unconditionally
				'bind: if count % 2 == 0 { // mandatory condition, only `if` (and thus `if let`)
					set_label: &format!("Even count: {count}") // at the beginning this is a lie
					// more method calls are allowed, conditionally or not
					// (only outermost condition is ignored on initialization)
				} // there cannot be an `else`
			}
			
			// use 'binding to create a binding closure to update the view appropriately:
			'binding update_first_labels = move |count: u8| bindings!()
			// you can see that it has one parameter, although there could be several, but
			// their names must match what is supposed to change on bound calls (with 'bind)
			//
			// you can also see the pseudo-macro `bindings!()` which consumes everything
			// declared with 'bind before this point, regardless of scope, and
			// expands them at that position within the binding closure which prevents
			// a future new binding closure from updating the same as the current one
			
			gtk::Label #append(&#) !{
				label: "Waiting for an even number…"
				
				'bind! if count % 2 == 0 { // with ! initializes conditionally
					set_label: &format!("Even count (really): {count}")
					// more method calls are allowed, conditionally or not
				} // `else if` and `else` are allowed
			}
			gtk::Label #append(&#) {
				'bind! match count % 2 == 0 { // and `match` too
					true  => set_label: "The count is even" // commas are not allowed
					false => set_label: "The count is odd"; // (semicolons are)
					#[allow(unreachable_patterns)]
					_ => { /* more method calls are allowed, conditionally or not */ }
				}
			}
			gtk::Label #append(&#) {
				// the above is more or less equivalent to:
				'bind! set_label: match count % 2 == 0 {
					true  => "The count is even", // comma required
					false => "The count is odd" // (this is normal Rust)
				}
			}
			
			// unlike the previous binding closure, this receives a `Cell<u8>` instead of `u8`:
			'binding update_latest_labels = move |count: Cell<u8>| {
				// however, the 'bind calls above still use `count` instead of `count.get()`
				let count = count.get(); // so we get the count here
				bindings! { } // and now we can consume the bindings without problems
			}
		}
		
		// SUMMARY
		// 'bind: initializes unconditionally but updates conditionally
		//        (mandatory an `if` without `else`)
		// 'bind! initializes and updates, conditionally or not
		// 'bind  does not initialize but updates, conditionally or not
		
		gtk::HeaderBar #titlebar(&#) {
			gtk::Button::with_label("Increase") #pack_start(&#) {
				connect_clicked: move |_| {
					// we mutate the count:
					main_count.set(main_count.get().wrapping_add(1));
					// the first binding closure requires a `u8`:
					update_first_labels(main_count.get());
					// the second requires a `Cell<u8>`:
					update_latest_labels(main_count.clone());
				}
			}
		}
	}
}]

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	
	app.connect_activate(|app| {
		// this will mutate from a closure at the end
		// of the view that does not implement `FnMut`:
		let main_count = Cell::new(1_u8); // we put an odd number
		
		// we get the count here for a reason explained above:
		let count = main_count.get();
		
		expand_view_here! { }
		window.present()
	});
	
	app.run()
}
