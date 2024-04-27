<!--
	SPDX-FileCopyrightText: 2024 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
	
	SPDX-License-Identifier: CC-BY-SA-4.0
-->

# <img src="logo.svg" width="96" align="left"/> `declarative`

[![REUSE status]][reuse] [![On crates.io]][crate.io]

[REUSE status]: https://api.reuse.software/badge/github.com/ejaa3/declarative
[reuse]: https://api.reuse.software/info/github.com/ejaa3/declarative
[On crates.io]: https://img.shields.io/crates/v/declarative.svg?color=6081D4
[crate.io]: https://crates.io/crates/declarative

A proc-macro library that implements a generic [DSL] to create complex reactive view code easier to edit and maintain.

To use it, add to your Cargo.toml:

~~~ toml
[dependencies.declarative]
version = '0.7.0'
~~~

To learn how to use macros, currently the best way is to clone the repository, read the source code of the examples in alphabetical order and run them like this:

~~~ bash
cargo run --example EXAMPLE_NAME
~~~

The examples depend on [gtk-rs], so you should familiarize yourself with [gtk-rs] a bit before:  
https://gtk-rs.org/gtk4-rs/stable/latest/book/

In addition to macro features, the examples also show some usage patterns (templates, components, Elm, etc.). GTK has a pattern of its own due to its object orientation and `declarative` integrates well, but there is no example about it (it would be verbose and exclusive to GTK, while `declarative` is not GTK based).

## Counter application example

The following is an implementation of the Elm architecture with [gtk-rs]:

![Light theme app screenshot](light.png)
![Dark theme app screenshot](dark.png)

~~~ rust
use declarative::{block as view, clone, construct};
use gtk::{glib, prelude::*};

enum Msg { Increase, Decrease }

// syntactic sugar for sending messages:
macro_rules! send { [$msg:expr => $tx:expr] => [$tx.send_blocking($msg).unwrap()] }

fn start(app: &gtk::Application) {
    let (tx, rx) = async_channel::bounded(1);
    let mut count = 0; // the state

    view![ gtk::ApplicationWindow window {
        application: app
        title: "My Application"
        titlebar: &gtk::HeaderBar::new()

        child: &_ @ gtk::Box {
            orientation: gtk::Orientation::Vertical
            spacing: 6
            margin_top: 6
            margin_bottom: 6
            margin_start: 6
            margin_end: 6
            ~
            append: &_ @ gtk::Label {
                label: "Count unchanged"
                'bind set_label: &format!("The count is: {count}")
            }
            append: &_ @ gtk::Button {
                label: "Increase" ~
                connect_clicked: clone![tx; move |_| send!(Msg::Increase => tx)]
            }
            append: &_ @ gtk::Button::with_label("Decrease") {
                connect_clicked: move |_| send!(Msg::Decrease => tx)
            }
            'consume refresh = move |count| bindings!()
        }
    } ];

    let update = |count: &mut u8, msg| match msg {
        Msg::Increase => *count = count.wrapping_add(1),
        Msg::Decrease => *count = count.wrapping_sub(1),
    };

    glib::spawn_future_local(async move {
        while let Ok(msg) = rx.recv().await {
            update(&mut count, msg); // the state is updated
            refresh(count); // now the view is refreshed
        }
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
cargo run --example y_readme
~~~

## Basic maintenance

The following commands must be executed and must not give any problems:

~~~ bash
cargo check  -p declarative-macros
cargo clippy -p declarative-macros
cargo test   -p declarative-macros
cargo check
cargo clippy
cargo test
# and now run and check each example
~~~

If you need a changelog, maybe the commit log will help (the last ones try to have the most important details).

<br/>

#### License

<sub>Licensed under either of Apache License, Version 2.0 (<a href="LICENSES/Apache-2.0.txt">Apache-2.0.txt</a> or http://www.apache.org/licenses/LICENSE-2.0) or MIT license (<a href="LICENSES/MIT.txt">MIT.txt</a> or http://opensource.org/licenses/MIT) at your option.</sub>

<sub>Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.</sub>

[DSL]: https://en.wikipedia.org/wiki/Domain-specific_language
[gtk-rs]: https://gtk-rs.org
