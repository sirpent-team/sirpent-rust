use std::marker;
use rustc_serialize::{Encodable, Decodable};

use hexagon_grid::*;
use square_grid::*;
use triangle_grid::*;

#[derive(RustcEncodable, RustcDecodable)]
pub enum Grids {
    HexagonGrid(HexagonGrid),
    SquareGrid(SquareGrid),
    TriangleGrid(TriangleGrid),
}

pub trait Direction where Self: marker::Sized {
    fn variants() -> &'static [Self];
}

pub trait Vector : Eq + Copy {
    type Direction;// : Direction;
    fn distance(&self, other : &Self) -> usize;
    fn neighbour(&self, direction : &Self::Direction) -> Self;
    fn neighbours(&self) -> Vec<Self>;
    /*fn ball_around(&self, radius : usize) -> Vec<Self>;
    fn rand_within<R : Rng>(rng : &mut R, radius : usize) -> Self;*/
}

pub trait Grid : Encodable + Decodable {
    type Vector;// : Vector;
    fn dimensions(&self) -> Vec<isize>;
    fn is_within_bounds(&self, v : Self::Vector) -> bool;
    fn name(&self) -> String;
}
