/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

//! Generic DSL macros for easy view code manipulation.

#![warn(missing_docs)]

pub use declarative::{block, view};

#[macro_export]
/// A default implementation for a macro called by [`block!`] and [`view!`].
/// Must be in scope. Ignore if another implementation is required.
macro_rules! construct {
	(? $type:ty) => { <$type>::default() };
	
	(? ~$struct_literal:expr) => { $struct_literal };
	(?  $struct_literal:expr) => { $struct_literal.start() };
	
	(~$builder:expr) => { $builder };
	( $builder:expr) => { $builder.build() };
	
	(~$type:ty => $($methods:tt)*) => { <$type>::builder() $($methods)* };
	( $type:ty => $($methods:tt)*) => { <$type>::builder() $($methods)*.build() };
}

#[macro_export]
/// A macro for frequent cloning, especially when moving to closures.
///
/// ## Examples
///
/// In the following `shared` is a clone with the same name as the variable
/// and `custom` is a custom clone (not `shared.clone()`) with a custom name:
/// ~~~
/// use {std::rc::Rc, declarative::clone};
///
/// let shared = Rc::new(2);
///
/// let closure = clone![shared, custom = Rc::clone(&shared); move || {
///     println!("{shared} + {custom} = {}", *shared + *custom)
/// }];
///
/// closure(); // 2 + 2 = 4
/// Rc::strong_count(&shared); // `shared` is still usable here
/// ~~~
///
/// In the following everything is cloned as `something.clone()`:
/// - `renamed` is a clone of `shared` with a custom name.
/// - `number.field` is cloned with the same field name.
/// - `number.field` is cloned again with a custom name.
///
/// ~~~
/// use {std::rc::Rc, declarative::clone};
///
/// struct Number { field: Rc<i32> }
///
/// let shared = Rc::new(2);
/// let number = Number { field: Rc::clone(&shared) };
///
/// let closure = {
///     clone![shared as renamed, number.field, number.field as other];
///     let sum = *renamed + *field + *other;
///     move || println!("{renamed} + {field} + {other} = {sum}")
/// };
///
/// closure(); // 2 + 2 + 2 = 6
/// Rc::strong_count(&shared); // `shared` is still usable here
/// Rc::strong_count(&number.field); // `number.field` too
/// ~~~
macro_rules! clone {
	[if [$($_:tt)+] { $($foo:tt)* } else { $($bar:tt)* }] => { $($foo)* };
	[if [         ] { $($foo:tt)* } else { $($bar:tt)* }] => { $($bar)* };
	
	($last:expr => $($tt:tt)*) => {{ $($tt)* $last }};
	(           => $($tt:tt)*) =>  { $($tt)* };
	
	[.$field:ident] => { $field };
	[.$field:ident $(.$rest:ident)+] => { $crate::clone![$(.$rest)+] };
	
	[$($let:ident $(.$field:ident)* $(as $name:ident)? $(= $expr:expr)?),+ $(,)? $(; $last:expr)?] => {
		$crate::clone!($($last)? => $($crate::clone! {
			if [$($field)* $($name)?] {
				$crate::clone![if [$($expr)?] {
					compile_error!("cannot use fields or `as` while custom cloning");
				} else {
					let $crate::clone! {
						if [$($name)?] { $($name)? } else { $crate::clone![$(.$field)*] }
					} = $let $(.$field)* .clone();
				}]
			} else {
				let $let = $crate::clone! {
					if [$($expr)?] { $($expr)? } else { $let.clone() }
				};
			}
		})+)
	};
}
