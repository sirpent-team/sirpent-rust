#![feature(question_mark, rustc_macro, structural_match, rustc_attrs)]

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
mod vector;
mod direction;
mod hexagon_grid;
mod square_grid;
mod triangle_grid;
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
