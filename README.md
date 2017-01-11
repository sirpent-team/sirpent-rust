## Synopsis
Sirpent is a multiplayer snake server for playing AI games across a network. **Under active development.**

## Motivation
This implemention is to learn safe low-level systems programming in [Rust](https://www.rust-lang.org), grow familiarity with network programming and explore [Futures-based](https://github.com/alexcrichton/futures-rs/) [asynchronous programming in Rust](https://tokio.rs).

## Installation

Built upon Rust nightly which can be chosen and installed by [rustup](https://www.rustup.rs/). Download the sirpent server with git: `git clone https://github.com/sirpent-team/sirpent-rust.git`.

Run the server with `cargo run`.

<!--
These need testing before advertising them:

Sirpent can use any of the [Regular Tilings](https://en.wikipedia.org/wiki/Euclidean_tilings_by_convex_regular_polygons#Regular_Tilings) for its Grid. Rust's type system is a little too limited to choose at runctime, so it's a compile-time option.

``` sh
cargo run --no-default-features --features square
```

``` sh
cargo run --no-default-features --features triangle
```
-->

<!--
## API Reference

Depending on the size of the project, if it is small and simple enough the reference docs can be added to the README. For medium size to larger projects it is important to at least provide a link to where the API reference docs live.

## Tests

Describe and show how to run the tests with code examples.

## Contributors

Let people know how they can dive into the project, include important links to things like issue trackers, irc, twitter accounts if applicable.
-->

## License

`sirpent-rust` is primarily distributed under the terms of both the MIT license and the Apache License (Version 2.0), with portions covered by various BSD-like licenses.

See LICENSE-APACHE, and LICENSE-MIT for details.
