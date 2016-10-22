pub use grids::traits::*;

#[cfg(feature = "hexagon")]
pub use grids::hexagon::*;
#[cfg(feature = "hexagon")]
pub type Direction = HexagonDirection;
#[cfg(feature = "hexagon")]
pub type Vector = HexagonVector;
#[cfg(feature = "hexagon")]
pub type Grid = HexagonGrid;

#[cfg(feature = "square")]
pub use grids::square::*;
#[cfg(feature = "square")]
pub type Direction = SquareDirection;
#[cfg(feature = "square")]
pub type Vector = SquareVector;
#[cfg(feature = "square")]
pub type Grid = SquareGrid;

#[cfg(feature = "triangle")]
pub use grids::triangle::*;
#[cfg(feature = "triangle")]
pub type Direction = TriangleDirection;
#[cfg(feature = "triangle")]
pub type Vector = TriangleVector;
#[cfg(feature = "triangle")]
pub type Grid = TriangleGrid;
