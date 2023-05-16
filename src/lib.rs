/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

//! A proc-macro library for creating complex reactive views declaratively and quickly.

#![doc(html_favicon_url = "../logo.svg")]
#![doc(html_logo_url = "../logo.svg")]
#![warn(missing_docs)]

pub use declarative::{block, view};

#[cfg(feature = "gtk-rs")]
#[macro_export]
/// Macro called by [`block!`] and [`view!`] macros when editing in builder mode (currently must be in scope).
macro_rules! builder_mode {
	(;$type:ty => $($token:tt)*) => { <$type>::builder() $($token)* };
	( $type:ty => $($token:tt)*) => { <$type>::builder() $($token)*.build() };
	(;$expr:expr) => { $expr };
	( $expr:expr) => { $expr.build() };
}

#[macro_export]
/// A small macro for frequent cloning, especially when moving to closures.
///
/// ## Example
/// ~~~
/// use declarative::clone;
/// 
/// fn example() {
///     let shared = std::rc::Rc::new(2);
///     
///     let closure = clone![shared, other as shared.clone(); move || {
///         println!("{shared} + {other} = {}", *shared + *other)
///     }];
///     
///     let another = {
///         clone![shared, other as shared.clone()];
///         move || println!("{shared} + {other} = {}", *shared + *other)
///     };
///     
///     closure();
///     another();
/// }
/// ~~~
macro_rules! clone {
	[fallback![$($foo:tt)+] $($bar:tt)+] => { $($bar)+ };
	[fallback![$($foo:tt)+]            ] => { $($foo)+ };
	
	[$($let:ident $(as $expr:expr)?),+] => {
		$(let $let = $crate::clone![fallback![$let.clone()] $($expr)?];)+
	};
	
	[$($let:ident $(as $expr:expr)?),+; $last:expr] => {{
		$(let $let = $crate::clone![fallback![$let.clone()] $($expr)?];)+
		$last
	}};
}
