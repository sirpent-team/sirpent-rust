#![feature(custom_derive, plugin)]
#![plugin(serde_macros)]

extern crate uuid;
extern crate rand;
extern crate serde;
#[cfg(test)]
extern crate quickcheck;

pub mod grid;
mod hexagon_grid;
mod square_grid;
mod triangle_grid;
pub mod snake;
pub mod player;

pub use grid::*;
pub use snake::*;
pub use player::*;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
