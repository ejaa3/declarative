use declarative_gtk4::Composable;
use gtk::{glib, prelude::*};

declarative::view! {
	gtk::ApplicationWindow window !{
		application: app
		default_height: 300
		build! ..
		set_titlebar => gtk::HeaderBar 'wrap Some { }
		
		gtk::Box {
			gtk::StackSidebar { set_stack: &stack }
			
			gtk::Stack stack {
				// some methods return something (in this case a BindingBuilder):
				bind_property[.. &window, "title"]: "visible-child-name"
					// you can edit the return with 'back, even in builder mode:
					'back !{ sync_create! build! }
				
				connect_visible_child_notify: move |_| send!(() => sender)
				
				// this method returns a gtk::StackPage
				add_titled[.. None, "First"] => gtk::Label {
					set_label: "Expected" // no semicolon
					
					// in this case you edit the return with 'back inside the scope,
					// and also give it a variable name and make it mutable as well:
					#[allow(unused_mut)] // note that attributes affect everything inside
					'back mut returned_page { set_name: "first" }
				}
				
				add_child => gtk::Label {
					set_label: "From an object assignment";
					// the semicolon above is necessary because we do not want to
					// edit what set_label() returns, but what add_child() returns
					//
					// a semicolon was not used before because an #[attribute] followed
					'back { set_name: "second"; set_title: "Second" }
					// the semicolon inside 'back is not necessary, but it is
					// a good separator of several assignments on the same line
				}
				
				add_child => gtk::Label::new(
					Some("From an object assignment without a body")
				) 'back { set_name: "third"; set_title: "Third" }
				
				gtk::Label {
					set_label: "From a component assignment";
					'back { set_name: "fourth"; set_title: "Fourth" }
				}
				
				gtk::Label::new(
					Some("From a component assignment without a body")
				) 'back { set_name: "fifth"; set_title: "Fifth" }
				
				gtk::Label {
					'back { // can be in any position within the scope
						set_name: "sixth"
						'bind set_title: &format!("Changes: {changes}")
					}
					set_label: "'back supports reactivity!"
				}
			}
			
			'binding update_view: move |changes: u8| { bindings!(); }
		}
	} ..
	fn window(app: &gtk::Application) -> gtk::ApplicationWindow {
		let (sender, receiver) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
		let mut changes: u8 = 0;
		
		expand_view_here!();
		
		receiver.attach(None, move |_| {
			changes = changes.wrapping_add(1);
			update_view(changes);
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
			move |error| glib::g_critical!("f_returns", "{error}")
		)
	};
}

use send;
