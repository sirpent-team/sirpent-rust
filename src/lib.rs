#![feature(proc_macro, associated_consts)]

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
#[macro_use]
extern crate futures;
#[macro_use]
extern crate tokio_core;

pub mod grids;
pub mod snake;
pub mod clients;
pub mod net;
pub mod protocol;
pub mod game;
pub mod game_future;

pub use grids::*;
pub use snake::*;
pub use clients::*;
pub use net::*;
pub use protocol::*;
pub use game::*;
pub use game_future::*;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
