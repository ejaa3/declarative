<!--
	SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
	
	SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Declarative

[![REUSE status](https://api.reuse.software/badge/github.com/ejaa3/declarative)](https://api.reuse.software/info/github.com/ejaa3/declarative)

A framework-agnostic procedural macro for creating complex reactive views declaratively and quickly.

To use it, add to your Cargo.toml:

~~~ toml
[dependencies.declarative]
git = 'https://github.com/ejaa3/declarative'

# For GTK (and libadwaita) implementation:
package = 'declarative-gtk4'
# see the features in ./gtk4/Cargo.toml
~~~

This package does not re-export gtk4 or libadwaita, so you should specify them in your Cargo.toml and preferably synchronize the features of both.

To learn how to use this macro, it is best to clone this repository, read the source code of the examples in alphabetical order and run them like this:

~~~ bash
cargo run -p declarative-gtk4 --example EXAMPLE_NAME
~~~

The examples depend on [gtk-rs], so you should familiarize yourself with [gtk-rs] first:  
https://gtk-rs.org/gtk4-rs/stable/latest/book/

[gtk-rs]: https://gtk-rs.org

However, the macro is generic. In the first example (`a_basics`) I don't use GTK at all, but implement it for `String`. The implementation consists only of a method called `as_composable_add_component()` (the first example explains its signature).

## Application example

In the following I manually implement the Elm pattern. The macro does not require any specific pattern.

![Light theme app screenshot](light.png)
![Dark theme app screenshot](dark.png)

~~~ rust
use declarative_gtk4::{Composable, builder_mode};
use gtk::{glib, prelude::*};

#[derive(Debug)]
enum Msg { Increase, Decrease }

struct State { count: i32 }

fn update_state(state: &mut State, msg: Msg) {
	match msg {
		Msg::Increase => state.count = state.count.wrapping_add(1),
		Msg::Decrease => state.count = state.count.wrapping_sub(1),
	}
}

declarative::view! {
	gtk::ApplicationWindow window !{
		application: app
		title: "My Application" #
		set_titlebar => gtk::HeaderBar 'wrap Some { }
		
		gtk::Box !{
			orientation: gtk::Orientation::Vertical
			spacing: 6
			margin_top: 6
			margin_bottom: 6
			margin_start: 6
			margin_end: 6 #
			
			gtk::Label {
				'bind set_label: &format!("The count is: {}", state.count)
			}
			
			gtk::Button::with_label("Increase") {
				connect_clicked: 'clone sender
					move |_| send!(Msg::Increase => sender)
			}
			
			gtk::Button::with_label("Decrease") {
				connect_clicked: move |_| send!(Msg::Decrease => sender)
			}
			
			'binding update_view: move |state: &State| { bindings!(); }
		}
	} ..
	
	fn window(app: &gtk::Application) -> gtk::ApplicationWindow {
		let mut state = State { count: 0 };
		let (sender, receiver) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
		
		expand_view_here!();
		
		receiver.attach(None, move |msg| {
			update_state(&mut state, msg);
			update_view(&state);
			glib::Continue(true)
		});
		
		window
	}
}

fn main() -> glib::ExitCode {
	let app = gtk::Application::default();
	app.connect_activate(move |app| window(app).present());
	app.run()
}

macro_rules! send {
	($expr:expr => $sender:ident) => {
		$sender.send($expr).unwrap_or_else(
			move |error| glib::g_critical!("example", "{error}")
		)
	};
}

use send;
~~~

To execute, run:

~~~ bash
cargo run -p declarative-gtk4
~~~

## License

Licensed under either of

* Apache License, Version 2.0 ([Apache-2.0.txt](LICENSES/Apache-2.0.txt) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([MIT.txt](LICENSES/MIT.txt) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
