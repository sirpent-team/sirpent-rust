pub mod traits;
pub mod square;
pub mod hexagon;
pub mod triangle;

pub use self::traits::*;

#[cfg(feature = "hexagon")]
pub use self::hexagon::*;
#[cfg(feature = "hexagon")]
pub type Direction = HexagonDirection;
#[cfg(feature = "hexagon")]
pub type Vector = HexagonVector;
#[cfg(feature = "hexagon")]
pub type Grid = HexagonGrid;

#[cfg(feature = "square")]
pub use self::square::*;
#[cfg(feature = "square")]
pub type Direction = SquareDirection;
#[cfg(feature = "square")]
pub type Vector = SquareVector;
#[cfg(feature = "square")]
pub type Grid = SquareGrid;

#[cfg(feature = "triangle")]
pub use self::triangle::*;
#[cfg(feature = "triangle")]
pub type Direction = TriangleDirection;
#[cfg(feature = "triangle")]
pub type Vector = TriangleVector;
#[cfg(feature = "triangle")]
pub type Grid = TriangleGrid;
