// https://pascalhertleif.de/artikel/good-practices-for-writing-rust-libraries/
#![cfg_attr(feature = "dev", allow(unstable_features))]
#![cfg_attr(feature = "dev", feature(plugin))]
#![cfg_attr(feature = "dev", plugin(clippy))]
#![deny(trivial_numeric_casts,
        unsafe_code,
        unused_import_braces, unused_qualifications)]

// `error_chain!` can recurse deeply
#![recursion_limit = "1024"]

// UUID is used to give unique identifiers to each game.
extern crate uuid;
// Rand is used to generate OS-level random numbers.
extern crate rand;
// Serde is used to Serialise/Deserialise game data.
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
// Quickcheck is used for property-based testing.
#[cfg(test)]
extern crate quickcheck;
extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_timer;
extern crate bytes;
#[macro_use]
extern crate error_chain;
extern crate comms;

pub mod state;
pub mod engine;
pub mod net;
pub mod utils;
mod errors;

pub use errors::*;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
