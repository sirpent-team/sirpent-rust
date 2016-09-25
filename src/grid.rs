use std::marker;
use serde::{Serialize, Deserialize};
use rand::Rng;

use hexagon_grid::*;
use square_grid::*;
use triangle_grid::*;

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub enum World {
    #[serde(rename = "hexagon_grid")]
    HexagonGrid(HexagonGrid),
    #[serde(rename = "square_grid")]
    SquareGrid(SquareGrid),
    #[serde(rename = "triangle_grid")]
    TriangleGrid(TriangleGrid),
}

pub trait Direction where Self: marker::Sized {
    fn variants() -> &'static [Self];
}

pub trait Vector : Eq + Copy + Serialize + Deserialize {
    type Direction;// : Direction;
    fn distance(&self, other : &Self) -> usize;
    fn neighbour(&self, direction : &Self::Direction) -> Self;
    fn neighbours(&self) -> Vec<Self>;
}

pub trait Grid : Serialize + Deserialize {
    type Vector;// : Vector;
    fn dimensions(&self) -> Vec<isize>;
    fn is_within_bounds(&self, v : Self::Vector) -> bool;
    fn cells(&self) -> Vec<Self::Vector>;
    fn random_cell<R : Rng>(&self) -> Self::Vector;
}
