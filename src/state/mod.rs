mod game;
mod snake;
pub mod grids;

pub use self::game::*;
pub use self::snake::*;
use self::grids::*;

#[cfg(feature = "hexagon")]
pub type Direction = HexagonDirection;
#[cfg(feature = "hexagon")]
pub type Vector = HexagonVector;
#[cfg(feature = "hexagon")]
pub type Grid = HexagonGrid;

#[cfg(feature = "square")]
pub type Direction = SquareDirection;
#[cfg(feature = "square")]
pub type Vector = SquareVector;
#[cfg(feature = "square")]
pub type Grid = SquareGrid;

#[cfg(feature = "triangle")]
pub type Direction = TriangleDirection;
#[cfg(feature = "triangle")]
pub type Vector = TriangleVector;
#[cfg(feature = "triangle")]
pub type Grid = TriangleGrid;
