/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{block as view, clone, construct};
use gtk::{glib, prelude::*};

enum Msg { Increase, Decrease }

// syntactic sugar for sending messages:
macro_rules! send { [$msg:expr => $tx:expr] => [$tx.send($msg).unwrap()] }

fn start(app: &gtk::Application) {
    let (tx, rx) = glib::MainContext::channel(glib::Priority::DEFAULT);
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
            ~margin_end: 6

            append: &_ @ gtk::Label {
                label: "Count unchanged"
                'bind set_label: &format!("The count is: {count}")
            }
            append: &_ @ gtk::Button {
                ~label: "Increase"
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

    rx.attach(None, move |msg| {
        update(&mut count, msg); // the state is updated
        refresh(count); // now the view is refreshed
        glib::ControlFlow::Continue
    });

    window.present()
}

fn main() -> glib::ExitCode {
    let app = gtk::Application::default();
    app.connect_activate(start);
    app.run()
}
