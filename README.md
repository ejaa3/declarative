<!--
	SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
	
	SPDX-License-Identifier: CC-BY-SA-4.0
-->

# <img src="logo.svg" width="96" align="left"/> `declarative`

[![REUSE status](https://api.reuse.software/badge/github.com/ejaa3/declarative)](https://api.reuse.software/info/github.com/ejaa3/declarative)
[![declarative on crates.io](https://img.shields.io/crates/v/declarative.svg)](https://crates.io/crates/declarative)
[![Matrix](https://img.shields.io/matrix/declarative-rs:matrix.org?color=6081D4&label=matrix)](https://matrix.to/#/#declarative-rs:matrix.org)

A proc-macro library for creating complex reactive views declaratively and quickly.

To use it, add to your Cargo.toml:

~~~ toml
[dependencies.declarative]
version = '0.4.0'

# for a custom builder mode:
features = ['builder-mode']

# if you're going to use it with gtk-rs, you might want to:
features = ['gtk-rs'] # gives a suitable `builder_mode!` macro
~~~

To learn how to use the macros, it is best to clone the repository, read the source code of the examples in alphabetical order and run them like this:

~~~ bash
cargo run --features gtk-rs --example EXAMPLE_NAME
~~~

The examples depend on [gtk-rs], so you should familiarize yourself with [gtk-rs] a bit before:  
https://gtk-rs.org/gtk4-rs/stable/latest/book/

[gtk-rs]: https://gtk-rs.org

You may need to tell rust-analyzer that the examples depend on the `gtk-rs` feature to avoid false positives.
For example, with VS Code it is configured with the following JSON:

~~~ JSON
{ "rust-analyzer.cargo.features": ["gtk-rs"] }
~~~

## Counter application example

In the following I manually implement the Elm pattern. The macro does not require any specific pattern.

![Light theme app screenshot](light.png)
![Dark theme app screenshot](dark.png)

~~~ rust
use declarative::{block as view, builder_mode, clone};
use gtk::{glib, prelude::*};

macro_rules! send { [$msg:expr => $tx:expr] => [$tx.send($msg).unwrap()] }

enum Msg { Increase, Decrease }

fn start(app: &gtk::Application) {
    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
    let mut count = 0; // the state

    view! {
        gtk::ApplicationWindow window !{
            application: app
            title: "My Application"

            gtk::HeaderBar #titlebar(&#) { }

            gtk::Box #child(&#) !{
                orientation: gtk::Orientation::Vertical
                spacing: 6
                margin_top: 6
                margin_bottom: 6
                margin_start: 6
                ~margin_end: 6

                gtk::Label #append(&#) {
                    'bind @set_label: &format!("The count is: {count}")
                }

                gtk::Button::with_label("Increase") #append(&#) {
                    connect_clicked: clone![tx; move |_| send!(Msg::Increase => tx)]
                }

                gtk::Button::with_label("Decrease") #append(&#) {
                    connect_clicked: move |_| send!(Msg::Decrease => tx)
                }

                @update_view = move |count| bindings!()
            }
        }
    }

    let update_count = |count: &mut u8, msg| match msg {
        Msg::Increase => *count = count.wrapping_add(1),
        Msg::Decrease => *count = count.wrapping_sub(1),
    };

    rx.attach(None, move |msg| {
        update_count(&mut count, msg);
        update_view(count);
        glib::Continue(true)
    });

    window.present()
}

fn main() -> glib::ExitCode {
    let app = gtk::Application::default();
    app.connect_activate(start);
    app.run()
}
~~~

To execute, run:

~~~ bash
cargo run --features gtk-rs --example y_readme
~~~

## Basic maintenance

The following commands must be executed and must not give any problems:

~~~ bash
cargo test -p declarative-macros
cargo test -p declarative-macros --features builder-mode
cargo check -p declarative-macros
cargo check -p declarative-macros --features builder-mode
cargo clippy -p declarative-macros
cargo clippy -p declarative-macros --features builder-mode
cargo test --features gtk-rs
cargo check
cargo check --features gtk-rs
cargo clippy
cargo clippy --features gtk-rs
# and now run and check each example
~~~

If you need a changelog, maybe the commit log will help (the last ones try to have the most important details).

## License

Licensed under either of

* Apache License, Version 2.0 ([Apache-2.0.txt](LICENSES/Apache-2.0.txt) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([MIT.txt](LICENSES/MIT.txt) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
