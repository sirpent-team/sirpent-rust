use rand::Rng;
use std::marker;
use std::fmt::Debug;
use serde::{Serialize, Deserialize};

#[cfg(feature = "hexagon")]
pub use hexagon_grid::*;
#[cfg(feature = "hexagon")]
pub type Direction = HexagonDir;
#[cfg(feature = "hexagon")]
pub type Vector = HexagonVector;
#[cfg(feature = "hexagon")]
pub type Grid = HexagonGrid;

#[cfg(feature = "square")]
pub use square_grid::*;
#[cfg(feature = "square")]
pub type Direction = SquareDir;
#[cfg(feature = "square")]
pub type Vector = SquareVector;
#[cfg(feature = "square")]
pub type Grid = SquareGrid;

#[cfg(feature = "triangle")]
pub use triangle_grid::*;
#[cfg(feature = "triangle")]
pub type Direction = TriangleDir;
#[cfg(feature = "triangle")]
pub type Vector = TriangleVector;
#[cfg(feature = "triangle")]
pub type Grid = TriangleGrid;

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

pub trait GridTrait
    : PartialEq + Eq + Copy + Serialize + Deserialize + Clone + Debug {
    type Vector: VectorTrait;

    fn dimensions(&self) -> Vec<isize>;
    fn is_within_bounds(&self, v: Self::Vector) -> bool;
    fn cells(&self) -> Vec<Self::Vector>;
    fn random_cell<R: Rng>(&self) -> Self::Vector;
}
