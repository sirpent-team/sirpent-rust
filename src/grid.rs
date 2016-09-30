use rand::Rng;
use std::marker;
use std::fmt::Debug;
use serde::{Serialize, Deserialize};

pub use hexagon_grid::*;
//pub use square_grid::*;
//pub use triangle_grid::*;

pub trait DirectionTrait
    : PartialEq + Eq + Copy + Serialize + Deserialize + Clone + Debug
    where Self: marker::Sized
{
    fn variants() -> &'static [Self];
}

pub trait VectorTrait
    : PartialEq + Eq + Copy + Serialize + Deserialize + Clone + Debug {
    type Direction: DirectionTrait;

    fn distance(&self, other: &Self) -> usize;
    fn neighbour(&self, direction: &Self::Direction) -> Self;
    fn neighbours(&self) -> Vec<Self>;
}

pub trait GridTrait: PartialEq + Eq + Copy + Serialize + Deserialize + Clone + Debug {
    type Vector: VectorTrait;

    fn dimensions(&self) -> Vec<isize>;
    fn is_within_bounds(&self, v: Self::Vector) -> bool;
    fn cells(&self) -> Vec<Self::Vector>;
    fn random_cell<R: Rng>(&self) -> Self::Vector;
}

pub type Direction = HexagonDir;
pub type Vector = HexagonVector;
pub type Grid = HexagonGrid;
