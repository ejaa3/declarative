/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use gtk::{glib, prelude::*};

// declarative allows a view to be able to react to state changes,
// but you decide where and when that reaction occurs

#[declarative::view]
mod example {
	use {std::cell::Cell, declarative::builder_mode, super::*};
	
	pub fn start(app: &gtk::Application) {
		// this will be the state and it starts with an odd number:
		let main_count = Cell::new(1_u8);
		// will mutate on the view inside a closure that does not implement `FnMut`
		
		// we get the count before the view is expanded for a reason...
		let count = main_count.get(); // explained in it (line 52)
		
		expand_view_here! { }
		window.present()
	}
	
	view! {
		gtk::ApplicationWindow window !{
			application: app
			title: "Reactivity"
			
			gtk::Box #child(&#) !{
				orientation: gtk::Orientation::Vertical
				// semicolons are optional but good separators:
				spacing: 6; margin_top: 6; ~margin_bottom: 6
				
				gtk::Label #append(&#) !{
					label: "Waiting for a change…" // 'bind does not initialize (read below)
					
					// to make a method call react to changes, use 'bind:
					'bind set_label: &format!("Count: {count}") // is immune to builder mode
				}
				gtk::Label #append(&#) {
					// if you want 'bind to initialize, prepend @ to what will be bound:
					'bind @{ // multiple method calls inside braces are also valid
						set_label: &format!("Count with tooltip: {count}")
						set_tooltip_text: Some(&format!("Count: {count}"))
					} // although you could use 'bind on each instead
					
					// since we referred to the count with `count` instead of `main_count.get()`,
					// we needed the `count` variable before `expand_view_here!` because we are @initializing
				}
				
				// use @ to consume the above bindings in a closure to refresh the view appropriately
				// (this @ is not an extension; `@extensions(#)` look like `#interpolations(#)`):
				@refresh_first_two_labels = move |count: u8| bindings!()
				// you can see that it has one parameter, although there could be several, but
				// their names must match what is supposed to change in what is marked with 'bind
				//
				// you can also see the `bindings!` placeholder macro which consumes and expands
				// everything marked with 'bind before @that point, regardless of scope, at that position
				// within the closure, preventing another closure from refreshing the same as the actual
				
				gtk::Label #append(&#) {
					// with colon refreshes conditionally, but initializes unconditionally
					'bind: if count % 2 == 0 { // mandatory condition, only `if` (and thus `if let`)
						set_label: &format!("Even count: {count}") // at the beginning this is a lie
						// more method calls are allowed, conditionally or not
						// (only outermost condition is ignored on initialization)
					} // there cannot be an `else`
				}
				gtk::Label #append(&#) !{
					label: "Waiting for an even number…"
					
					'bind @if count % 2 == 0 { // @ can initialize conditionally
						set_label: &format!("Even count (really): {count}")
						// more method calls are allowed, conditionally or not
					} // `else if` and `else` are allowed
				}
				
				// unlike the previous closure, this receives a `Cell<u8>` instead of `u8`:
				@refresh_second_two_labels = move |count: Cell<u8>| {
					// however, the 'bind calls above still use `count` instead of `count.get()`
					let count = count.get(); // so we get the count here
					bindings! { } // and now we can consume the bindings without problems
				}
				
				gtk::Label #append(&#) {
					'bind @match count % 2 == 0 { // matching is also possible
						true  => set_label: "The count is even" // commas are not allowed
						false => set_label: "The count is odd"; // (semicolons are)
						#[allow(unreachable_patterns)]
						_ => { /* more method calls are allowed, conditionally or not */ }
					}
				}
				gtk::Label #append(&#) {
					// the above is more or less equivalent to:
					'bind @set_label: match count % 2 == 0 {
						true  => "The count is even", // comma required
						false => "The count is odd" // (this is normal Rust)
					}
				}
			}
			
			gtk::HeaderBar #titlebar(&#) {
				gtk::Button::with_label("Increase") #pack_start(&#) {
					// if @ is prepended to an argument, bindings can be consumed in its expression:
					connect_clicked: @move |_| {
						// we mutate the count:
						main_count.set(main_count.get().wrapping_add(1));
						// the first closure requires a `u8`:
						refresh_first_two_labels(main_count.get());
						// the second requires a `Cell<u8>`:
						refresh_second_two_labels(main_count.clone());
						
						// `count` here so that the one in `main()` is not used:
						let count = main_count.get();
						// now we refresh the last two labels:
						bindings! { }
					}
				}
			}
			
			// SUMMARY
			//
			// 'bind:  initializes unconditionally but refreshes conditionally
			//         (mandatory an `if` without `else`)
			// 'bind @ initializes and refreshes, conditionally or not (`if` or `match`)
			// 'bind   does not initialize but refreshes, conditionally or not (`if` or `match`)
			//
			// usually you would use `bind without `@` or `:` plus an initial update
		}
	}
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(example::start);
	app.run()
}
