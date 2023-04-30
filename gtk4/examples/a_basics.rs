/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: (Apache-2.0 or MIT)
 */

fn main() {
	let greet = "Hello world!";
	
	declarative::view! { // inner syntax
		String::from(greet) string { } // this is an “object”
	}
	
	println!("[INNER]\n{string}\n");
	
	outer();
	assignments();
}

declarative::view! { // outer syntax
	// writing only the type constructs it with Type::default()
	String { } // no need to name
	
	// you can add more objects here
	String::new() mut string { /* can mutate here */ }
	
	.. // the double dot ends the view and starts the code where it will be expanded
	
	fn outer() { // preferably a small function
		println!("[OUTER]");
		
		expand_view_here!(); // here we insert the strings
		string.push_str("Mutable string\n");
		
		println!("{string}");
	}
}

declarative::view! { // assignments
	String::new() mut string { // this is a “composable object”
		push_str: "Now I " // this is a “property assignment”
		// although push_str() is not a setter, but let's assume
		
		push_str => String mut { // this is an “object assignment”
			push_str: "will say: "
		}
		
		// this is a “component assignment” (only works with “composable objects”):
		move String mut 'with "Goodbye" { // this syntax is explained below
			push_str: " world!"
		}
	} ..
	
	fn assignments() {
		println!("[ASSIGNMENTS]");
		expand_view_here!();
		println!("{string}");
	}
}

// for “component assignment” to work, the “composable object”
// must have a method called “as_composable_add_component”:
impl ComposableString for String {
	fn as_composable_add_component(&mut self, string: String, with: &str) {
		self.push_str(with);
		self.push_str(&string);
	}
}

trait ComposableString {
	/**
		## Composable object
		
		If the first parameter is `&mut self`,
		the “composable object” must be declared mutable like this:
		~~~
		declarative::view! {
			Composable mut { }
		}
		~~~
		If the first parameter is `&self`,
		the “composable object” must be inmutable:
		~~~
		declarative::view! {
			Composable { }
		}
		~~~
		
		## Component object
		
		If the second parameter is not a reference,
		the “component” must have `move` before it:
		~~~
		declarative::view! {
			Composable {
				move Component { }
			}
		}
		~~~
		If the second parameter is a mutable reference,
		the “component” must have `mut` before it:
		~~~
		declarative::view! {
			Composable {
				mut Component { }
			}
		}
		~~~
		If the second parameter is an inmutable reference,
		the “component” must not have `move` or `mut` before it:
		~~~
		declarative::view! {
			Composable {
				Component { }
			}
		}
		~~~
		If the third parameter is not of type `()` (unit type),
		the “component” must have `'with <argument>` after it.
		
		For example, for a parameter of type `&str`, the “component” would be declared like this:
		~~~
		declarative::view! {
			String mut { // composable
				move String mut 'with "world!" { // component with argument
					push_str: "Hello "
				}
			}
		}
		~~~
	*/ fn as_composable_add_component(&mut self, string: String, with: &str);
}
