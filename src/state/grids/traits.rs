use rand::Rng;
use std::fmt::Debug;
use serde::{Serialize, Deserialize};

use super::hexagon::*;
use super::square::*;
use super::triangle::*;

pub trait DirectionTrait
    : PartialEq + Eq + Copy + Serialize + Deserialize + Clone + Debug {
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
    fn random_cell<R: Rng>(&self, rng: &mut R) -> Self::Vector;
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "tiling", rename_all = "snake_case")]
pub enum GridEnum {
    Hexagon(HexagonGrid),
    Square(SquareGrid),
    Triangle(TriangleGrid),
}

impl From<HexagonGrid> for GridEnum {
    fn from(grid: HexagonGrid) -> GridEnum {
        GridEnum::Hexagon(grid)
    }
}

impl From<SquareGrid> for GridEnum {
    fn from(grid: SquareGrid) -> GridEnum {
        GridEnum::Square(grid)
    }
}

impl From<TriangleGrid> for GridEnum {
    fn from(grid: TriangleGrid) -> GridEnum {
        GridEnum::Triangle(grid)
    }
}
