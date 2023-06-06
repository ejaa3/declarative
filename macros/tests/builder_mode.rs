/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

#![allow(unused_variables, dead_code)]

#[derive(Default)]
struct Test { field: Option<Box<Test>> }

impl Test {
	fn builder() -> Builder { Builder }
	fn method(&self) { }
	fn start(self) -> Self { self }
}

struct Builder;

impl Builder {
	fn building(self) -> Self { self }
	fn build(self) -> Test { Test { field: None } }
}

#[test]
#[cfg(not(feature = "builder-mode"))]
fn builder_mode() {
	#[allow(unused_macros)]
	macro_rules! builder_mode { [$($_:tt)*] => [Test::default()] }
	
	declarative_macros::block! {
		Test::builder() !{ ~build; }
		Test::builder() !{ ~build; method; }
		Test::builder() !{ building; ~build; }
		
		Test::builder() !{ ~~build; } // the same as the above three
		Test::builder() !{ ~~build; method; }
		Test::builder() !{ building; ~~build; }
		
		Test struct_1 ~{ field: None }
		Test struct_2 ~{ ~~field: None } // same as previous
		Test struct_3 ~{ ~~field: None; method; }
		Test struct_4 ~{ field: None; ~start; }
		Test struct_5 ~{ field: None; ~start; method; }
		
		Test inter_1 ~{ Test #field(Some(#.into())) { } }
		Test inter_2 ~{ ~~Test #field(Some(#.into())) { } } // same as previous
		Test inter_3 ~{ ~~Test #field(Some(#.into())) { } method; }
		Test inter_4 ~{ Test #field(Some(#.into())) { } ~start; }
		Test inter_5 ~{ Test #field(Some(#.into())) { } ~start; method; }
	}
}

#[test]
#[cfg(feature = "builder-mode")]
fn builder_mode() {
	macro_rules! builder_mode {
		(~($struct:expr)) => { $struct };
		( ($struct:expr)) => { $struct.start() };
		
		(~$expr:expr) => { $expr };
		( $expr:expr) => { $expr.build() };
		
		(~$type:ty => $($methods:tt)*) => { <$type>::builder() $($methods)* };
		( $type:ty => $($methods:tt)*) => { <$type>::builder() $($methods)*.build() };
	}
	
	declarative_macros::block! {
		Test::builder() end_1 !{ ~~build; }
		Test end_2 !{ ~~build; method; }
		Test end_3 !{ building; ~~build; }
		
		Test::builder() auto_1 !{ ~building; }
		Test auto_2 !{ ~building; method; }
		Test auto_3 !{ building; }
		
		Test struct_1 ~{ field: None } // start
		Test struct_2 ~{ ~field: None } // same as previous
		Test struct_3 ~{ ~field: None; method; } // start
		Test struct_4 ~{ ~~field: None } // does not start
		Test struct_5 ~{ ~~field: None; method; } // does not start
		
		Test inter_1 ~{ Test #field(Some(#.into())) { } } // start
		Test inter_2 ~{ ~Test #field(Some(#.into())) { } } // same as previous
		Test inter_3 ~{ ~Test #field(Some(#.into())) { } method; } // start
		Test inter_4 ~{ ~~Test #field(Some(#.into())) { } } // does not start
		Test inter_5 ~{ ~~Test #field(Some(#.into())) { } method; } // does not start
	}
}
