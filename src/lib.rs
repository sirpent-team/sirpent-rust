// #![feature(rustc_macro, structural_match, rustc_attrs, custom_derive)]
#![feature(proc_macro)]

extern crate uuid;
extern crate rand;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
#[macro_use(chan_select)]
extern crate chan;
#[cfg(test)]
extern crate quickcheck;

mod net;
mod grid;
mod grids;
mod snake;
mod player;
mod protocol;
mod game_state;

pub use net::*;
pub use grid::*;
pub use snake::*;
pub use player::*;
pub use protocol::*;
pub use game_state::*;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
