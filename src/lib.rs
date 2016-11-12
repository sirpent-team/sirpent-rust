//#![feature(rustc_macro, structural_match, rustc_attrs, custom_derive)]
#![feature(proc_macro)]

extern crate uuid;
extern crate rand;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
#[cfg(test)]
extern crate quickcheck;

mod net;
mod game;
mod grid;
mod grids;
mod snake;
mod player;
mod protocol;

pub use net::*;
pub use game::*;
pub use grid::*;
pub use snake::*;
pub use player::*;
pub use protocol::*;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
