/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

use declarative_gtk4::Composable;
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
		title: "My Application"
		build! ..
		set_titlebar => gtk::HeaderBar 'wrap Some { }
		
		gtk::Box !{
			orientation: gtk::Orientation::Vertical
			spacing: 6
			margin_top: 6
			margin_bottom: 6
			margin_start: 6
			margin_end: 6
			build!
			
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
