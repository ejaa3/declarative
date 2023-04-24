/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

//! Implementation of the "as_composable_add_component()" method for more
//! convenient syntax when using the macro with the gtk4 and libadwaita crates.

#![cfg_attr(feature = "deprecated", allow(deprecated))]

pub use declarative::view;

use gtk::{gio, glib::IsA};
use gtk::traits::{
	BoxExt, ButtonExt, FlowBoxChildExt, FrameExt,
	GridExt, GtkWindowExt, ListBoxRowExt, PopoverExt
};

#[cfg(any(feature = "gtk_v4_8", feature = "dox"))]
use gtk::traits::CheckButtonExt;

macro_rules! fallback {
	(@($($foo:tt)*) $($bar:tt)+) => { $($bar)+ };
	(@($($foo:tt)*)) => { $($foo)* };
}

/* macro_rules! exclusive {
	($trait:ident for $impl:ty { [$self:ident, $component:ident: $ty:ty $(, $with:tt: $with_ty:ty)?] $($ret:ty)? $block:block }) => {
		pub trait $trait {
			fn as_composable_add_component(&self, $component: &$ty, with: fallback!(@(()) $($with_ty)?)) -> fallback!(@(()) $($ret)?);
		}
		
		impl $trait for $impl {
			fn as_composable_add_component(
				&$self, $component: &$ty, fallback!(@(_) $($with)?): fallback!(@(()) $($with_ty)?)
			) -> fallback!(@(()) $($ret)?) $block
		}
	};
} */

macro_rules! composable {
	($parent:ty $(: $child:ty)?, $method:ident($($with:ident: $ty:ty),*) $(-> $ret:ty)? $(, $some:ident)?) => {
		impl<T> Composable<&T, ($($ty),*), fallback!(@(()) $($ret)?)> for $parent
		where T: IsA<fallback!(@(gtk::Widget) $($child)?)> {
			fn as_composable_add_component(&self, component: &T, ($($with),*): ($($ty),*)) -> fallback!(@(()) $($ret)?) {
				self.$method(fallback![@(component) $($some(component))?], $($with),*)
			}
		}
	};
	($parent:ty => $child:ty, $method:ident($($with:ident: $ty:ty),*) $(-> $ret:ty)? $(, $some:ident)?) => {
		impl Composable<$child, ($($ty),*), fallback!(@(()) $($ret)?)> for $parent {
			fn as_composable_add_component(&self, component: $child, ($($with),*): ($($ty),*)) -> fallback!(@(()) $($ret)?) {
				self.$method(fallback![@(component) $($some(component))?], $($with),*)
			}
		}
	};
}

/** With this trait you can write:
~~~ rust
declarative::view! {
	gtk::Box {
		gtk::Label { }
	} ..
}
~~~
Instead of:
~~~ rust
declarative::view! {
	gtk::Box {
		append => gtk::Label { }
	} ..
}
~~~ */
pub trait Composable<Component, With, Return> {
	/// Method called by the [view!] macro when a "component assignment" is performed.
	fn as_composable_add_component(&self, component: Component, with: With) -> Return;
}

composable!(gio::Menu => &gio::MenuItem, append_item());

composable!(gtk::ActionBar, set_center_widget(), Some); // NOTE also has pack_start and pack_end
composable!(gtk::ApplicationWindow, set_child(), Some);
composable!(gtk::Box, append());
composable!(gtk::Button, set_child(), Some);
composable!(gtk::Expander, set_child(), Some); // NOTE also has set_label_widget
composable!(gtk::FlowBoxChild, set_child(), Some);
composable!(gtk::Frame, set_child(), Some);
composable!(gtk::Grid, attach(column: i32, row: i32, width: i32, height: i32));
composable!(gtk::HeaderBar, set_title_widget(), Some); // NOTE also has {pack_start,pack_end}
composable!(gtk::ListBox, append());
composable!(gtk::ListBoxRow, set_child(), Some);
composable!(gtk::Overlay, set_child(), Some);
composable!(gtk::PasswordEntry: gio::MenuModel, set_extra_menu(), Some);
composable!(gtk::Popover, set_child(), Some); // NOTE also has set_default_widget
composable!(gtk::Revealer, set_child(), Some);
composable!(gtk::ScrolledWindow, set_child(), Some);
composable!(gtk::SearchBar, set_child(), Some);
composable!(gtk::ShortcutController => gtk::Shortcut, add_shortcut());
composable!(gtk::SpinButton: gtk::Adjustment, set_adjustment());
composable!(gtk::Stack, add_child() -> gtk::StackPage);
composable!(gtk::StackSidebar => &gtk::Stack, set_stack());
composable!(gtk::StackSwitcher => &gtk::Stack, set_stack(), Some);
composable!(gtk::ToggleButton, set_child(), Some);
composable!(gtk::Window, set_child(), Some);

#[cfg(any(feature = "gtk_v4_8", feature = "dox"))]
composable!(gtk::CheckButton, set_child(), Some);

#[cfg(any(feature = "gtk_v4_6", feature = "dox"))]
composable!(gtk::FlowBox, append());

#[cfg(any(feature = "gtk_v4_6", feature = "dox"))]
composable!(gtk::MenuButton, set_child(), Some);

impl<T: IsA<gtk::Widget>> Composable<&T, (), u32> for gtk::Notebook {
	fn as_composable_add_component(&self, child: &T, _: ()) -> u32 {
		self.append_page(child, gtk::Widget::NONE)
	}
}

impl<T: IsA<gtk::Widget>> Composable<&T, (), ()> for gtk::CenterBox {
	fn as_composable_add_component(&self, component: &T, _: ()) {
		if self.center_widget().is_some() {
			if self.end_widget().is_none() {
				self.set_end_widget(Some(component));
			}
		} else if self.start_widget().is_some() {
			self.set_center_widget(Some(component));
		} else {
			self.set_start_widget(Some(component));
		}
	}
}

impl<T: IsA<gtk::Widget>> Composable<&T, (), ()> for gtk::Paned {
	fn as_composable_add_component(&self, component: &T, _: ()) {
		if self.start_child().is_some() {
			if self.end_child().is_none() {
				self.set_end_child(Some(component))
			}
		} else { self.set_start_child(Some(component)) }
	}
}

#[cfg(feature = "deprecated")]
mod deprecated {
	use gtk::prelude::{ComboBoxExt, DialogExt, TreeViewExt};
	use super::{ButtonExt, Composable, IsA};

	composable!(gtk::Assistant, append_page() -> i32);
	composable!(gtk::ComboBox, set_child(), Some);
	composable!(gtk::ComboBoxText, set_child(), Some);
	composable!(gtk::IconView => &gtk::TreeModel, set_model(), Some);
	composable!(gtk::InfoBar, add_child());
	composable!(gtk::LockButton, set_child(), Some);
	composable!(gtk::TreeView => &gtk::TreeViewColumn, append_column() -> i32);

	impl<T: IsA<gtk::Widget>> Composable<&T, gtk::ResponseType, ()> for gtk::Dialog {
		fn as_composable_add_component(&self, component: &T, response_type: gtk::ResponseType) {
			self.add_action_widget(component, response_type)
		}
	}
}

#[cfg(feature = "adw")]
mod libadwaita {
	use adw::traits::{
		AdwApplicationWindowExt, AdwWindowExt, BinExt, ComboRowExt,
		PreferencesGroupExt, PreferencesPageExt, PreferencesWindowExt
	};
	
	#[cfg(any(feature = "adw_v1_2", feature = "dox"))]
	use adw::traits::MessageDialogExt;

use super::*;
	// adw::{EntryRow,PasswordEntryRow} has add_{prefix,suffix}
	// adw::Flap has set_{content,flap,separator}
	
	composable!(adw::ApplicationWindow, set_content(), Some);
	composable!(adw::ActionRow, set_child(), Some);
	composable!(adw::Bin, set_child(), Some);
	composable!(adw::Carousel, append());
	composable!(adw::CarouselIndicatorDots => &adw::Carousel, set_carousel(), Some);
	composable!(adw::CarouselIndicatorLines => &adw::Carousel, set_carousel(), Some);
	composable!(adw::Clamp, set_child(), Some);
	composable!(adw::ClampScrollable, set_child(), Some);
	composable!(adw::ComboRow: gio::ListModel, set_model(), Some);
	composable!(adw::ExpanderRow, set_child(), Some);
	composable!(adw::HeaderBar, set_title_widget(), Some); // NOTE also has {pack_start,pack_end}
	composable!(adw::Leaflet, append() -> adw::LeafletPage);
	composable!(adw::PreferencesGroup, add());
	composable!(adw::PreferencesPage: adw::PreferencesGroup, add());
	composable!(adw::PreferencesWindow: adw::PreferencesPage, add());
	composable!(adw::SplitButton, set_child(), Some);
	composable!(adw::Squeezer, add() -> adw::SqueezerPage);
	composable!(adw::StatusPage, set_child(), Some);
	composable!(adw::TabBar => &adw::TabView, set_view(), Some);
	composable!(adw::TabView, append() -> adw::TabPage);
	composable!(adw::ToastOverlay, set_child(), Some);
	composable!(adw::ViewStack, add() -> adw::ViewStackPage);
	composable!(adw::ViewSwitcherBar => &adw::ViewStack, set_stack(), Some);
	composable!(adw::ViewSwitcherTitle => &adw::ViewStack, set_stack(), Some);
	composable!(adw::Window, set_content(), Some);
	
	#[cfg(any(feature = "adw_v1_2", feature = "dox"))]
	composable!(adw::MessageDialog, set_extra_child(), Some);
	
	#[cfg(any(feature = "adw_v1_3", feature = "dox"))]
	composable!(adw::TabOverview, set_child(), Some);
}
