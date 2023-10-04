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
fn construct() {
	macro_rules! construct {
		(? $type:ty) => { <$type>::default() };
		
		(? ~$struct_literal:expr) => { $struct_literal };
		(?  $struct_literal:expr) => { $struct_literal.start() };
		
		(~$builder:expr) => { $builder };
		( $builder:expr) => { $builder.build() };
		
		(~$type:ty => $($methods:tt)*) => { <$type>::builder() $($methods)* };
		( $type:ty => $($methods:tt)*) => { <$type>::builder() $($methods)*.build() };
	}
	
	declarative_macros::block! {
		Test default_1 { method; method; }!
		Test::default() default_2 { method; method; }
		
		Test::builder() end_1 { ~~build; }!
		Test end_2 { ~~build; method; }
		Test end_3 { building; ~~build; }
		
		Test::builder() auto_1 { ~building; }!
		Test auto_2 { ~building; method; }
		Test auto_3 { building; }
		
		Test struct_1 { field: None }? // start
		Test struct_2 { ~field: None }? // same as previous
		Test struct_3 { ~field: None; method; }? // start
		Test struct_4 { ~~field: None }? // does not start
		Test struct_5 { ~~field: None; method; }? // does not start
		
		Test inter_1 { field: Some(_.into()) @ Test { } }? // start
		Test inter_2 { ~field: Some(_.into()) @ Test { } }? // same as previous
		Test inter_3 { ~field: Some(_.into()) @ Test { } method; }? // start
		Test inter_4 { ~~field: Some(_.into()) @ Test { } }? // does not start
		Test inter_5 { ~~field: Some(_.into()) @ Test { } method; }? // does not start
	}
}
