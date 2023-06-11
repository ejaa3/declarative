<!--
	SPDX-FileCopyrightText: 2023 Eduardo Javier Alvarado Aarón <eduardo.javier.alvarado.aaron@gmail.com>
	
	SPDX-License-Identifier: CC-BY-SA-4.0
-->

# <img src="../logo.svg" width="96" align="left"/> `declarative-macros`

[![REUSE status](https://api.reuse.software/badge/github.com/ejaa3/declarative)](https://api.reuse.software/info/github.com/ejaa3/declarative)
[![declarative-macros on crates.io](https://img.shields.io/crates/v/declarative-macros.svg)](https://crates.io/crates/declarative-macros)
[![Matrix](https://img.shields.io/matrix/declarative-rs:matrix.org?color=6081D4&label=matrix)](https://matrix.to/#/#declarative-rs:matrix.org)

A proc-macro library for creating complex reactive views declaratively and quickly.

To use it, add to your Cargo.toml:

~~~ toml
[dependencies.declarative]
package = 'declarative-macros'
version = '0.5.2'

# for a custom builder mode:
features = ['builder-mode']
~~~

## Main crate

* https://crates.io/crates/declarative
* https://lib.rs/crates/declarative

## License

Licensed under either of

* Apache License, Version 2.0 ([Apache-2.0.txt](../LICENSES/Apache-2.0.txt) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([MIT.txt](../LICENSES/MIT.txt) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
