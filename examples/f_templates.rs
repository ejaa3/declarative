/*
 * SPDX-FileCopyrightText: 2025 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{clone, construct, view};
use gtk::{glib, prelude::*};

enum Msg { Increase, Decrease, Reset } // channels again

macro_rules! send { [$msg:expr => $tx:expr] => [$tx.send_blocking($msg).unwrap()] }

#[view] // we are implementing a struct generated automatically by the view
impl BoxTemplate { // visibility could be changed, like `#[view(pub)]`
	// more fields could be added, such as `#[view(integer: i32, pub float: f32)]`
	// or both, like `#[view(pub, integer: i32, pub float: f32)]`
	
	view! {
		// to reference this widget as a template field, you must give it a name preceded by `ref` (same as `pub(self)`)
		// or other visibility; if no widgets were exported, no struct would be automatically generated
		gtk::Box ref root { // the type is assumed to be `gtk::Box`, but an incorrect assumption is possible
			// in such case the correct type should be specified with `as Type` after the name (only type paths supported)
			// for public visibility use `pub` or similar instead of `ref` or `pub(self)`
			
			orientation: gtk::Orientation::Vertical
			spacing: 6
			margin_top: 6
			margin_bottom: 6
			margin_start: 6
			margin_end: 6
			~
			append: &_ @ gtk::Label ref label { // we also export this widget
				label: glib::gformat!("This is the {nth} view")
			}
			append: &_ @ gtk::Button::with_label("Increase") { // we do not export this (not even named)
				connect_clicked: clone![tx; move |_| send!(Msg::Increase => tx)]
			}
		}
		gtk::Button ref reset { // we will also export this widget although it is independent of the root
			label: glib::gformat!("Reset {nth}") ~
			connect_clicked: clone![tx; move |_| send!(Msg::Reset => tx)]
		} // we must do something with it in the main view so that it is not lost
	}
	
	// if `Default` were implemented, `!` after braces would suffice instead of writing "::new()"
	// but there would be no parameters (we could also have defined an unassociated function):
	fn new(nth: &str, tx: &async_channel::Sender<Msg>) -> Self {
		expand_view_here! { }
		Self { root, label, reset }
	}
}

// if we had used `#[view]` for a `mod`, the struct would be created inside it
//
// for the rest (`impl`, `trait`, `fn`, etc.) it is created
// in the macro invocation scope (here for the above case)

impl std::ops::Deref for BoxTemplate { // views get along with `Deref`
	type Target = gtk::Box;
	fn deref(&self) -> &Self::Target { &self.root } // let's try with the root widget
}

#[view]
mod example { // now let's use the template:
	use super::*;
	
	// let's create two states and two channels for two templates:
	pub fn start(app: &gtk::Application) {
		let (tx_1, rx_1) = async_channel::bounded(1);
		let (tx_2, rx_2) = async_channel::bounded(1);
		let (mut count_1, mut count_2) = (0, 0); // the states
		
		expand_view_here! { }
		
		let update = |count: &mut u8, msg| match msg {
			Msg::Increase => *count = count.wrapping_add(1),
			Msg::Decrease => *count = count.wrapping_sub(1),
			Msg::Reset    => *count = 0,
		}; // this closure does not capture anything (could be a function)
		
		glib::spawn_future_local(async move {
			while let Ok(msg) = rx_1.recv().await {
				update(&mut count_1, msg);
				refresh_template_1(count_1);
			}
		});
		
		glib::spawn_future_local(async move {
			while let Ok(msg) = rx_2.recv().await {
				update(&mut count_2, msg);
				refresh_template_2(count_2);
			}
		});
		
		window.present()
	}
	
	view![ gtk::ApplicationWindow window {
		application: app
		title: "Templates"
		titlebar: &gtk::HeaderBar::new()
		
		child: &_ @ gtk::Box {
			orientation: gtk::Orientation::Vertical
			spacing: 6
			~
			// `BoxTemplate` is not a widget but its `root` field is:
			append: &_.root @ BoxTemplate::new("first", &tx_1) first {
				ref label { // this `ref` is not an item, but we are editing the `label` field
					'bind set_label: &format!("The first count is: {count}")
				}
				// we can compose with just the method thanks to `Deref`:
				append: &_ @ gtk::Button::with_label("Decrease") {
					connect_clicked: move |_| send!(Msg::Decrease => tx_1)
				}
				// be careful editing a template after creating a closure that refreshes it:
				'consume refresh_template_1 = move |count| bindings!()
				// at this point the template has partially moved to the closure,
				// so `Deref` can no longer be used (you can edit like `ref field { edit; }`)
			}
			append: &gtk::Separator::default()
			
			// the same code as above but with dot (as `field.method`) instead of `ref` and `Deref`:
			append: &_.root @ BoxTemplate::new("second", &tx_2) second {
				'bind label.set_label: &format!("The second count is: {count}")
				
				root.append: &_ @ gtk::Button::with_label("Decrease") {
					connect_clicked: move |_| send!(Msg::Decrease => tx_2)
				}
				'consume refresh_template_2 = move |count| bindings!()
			}
			append: &gtk::Separator::default()
			
			append: &_ @ gtk::Box {
				margin_bottom: 6
				margin_end: 6
				margin_start: 6
				spacing: 6
				~
				// we put the independent widget (the reset button)
				// of the `first` and `second` templates here:
				append: &_ @ ref  first.reset { set_hexpand: true }
				append: &_ @ ref second.reset { set_hexpand: true }
			}
		}
	} ];
}

// in the `g_components` example we will avoid manually
// creating states and channels for each template

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(example::start);
	app.run()
}
