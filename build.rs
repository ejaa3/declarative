/*
 * SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
 *
 * SPDX-License-Identifier: Unlicense
 */

fn main() {
	println!("cargo:rerun-if-changed=logo.svg");
	
	std::fs::copy("logo.svg", "target/doc/logo.svg")
		.expect("failed to copy logo when building documentation");
}
