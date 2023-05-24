/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative::{builder_mode, clone, view};
use gtk::{glib, prelude::*};

macro_rules! send { [$msg:expr => $tx:expr] => [$tx.send($msg).unwrap()] }

enum Msg { Increase, Decrease }

#[view {
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
            margin_end: 6 #:

            gtk::Label #append(&#) {
                'bind! set_label: &format!("The count is: {count}")
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
}]

fn start(app: &gtk::Application) {
    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
    let mut count = 0; // the state

    expand_view_here! { }

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
