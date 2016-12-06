#![feature(proc_macro)]

// UUID is used to give unique identifiers to each game.
extern crate uuid;
// Rand is used to generate OS-level random numbers.
extern crate rand;
// Rayon's par_iter() is used to do things in parallel.
extern crate rayon;
// Serde is used to Serialise/Deserialise game data.
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
// Quickcheck is used for property-based testing.
#[cfg(test)]
extern crate quickcheck;

mod net;
mod grid;
mod grids;
mod snake;
mod player;
mod protocol;
mod game_state;
mod game_engine;

pub use net::*;
pub use grid::*;
pub use snake::*;
pub use player::*;
pub use protocol::*;
pub use game_state::*;
pub use game_engine::*;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
