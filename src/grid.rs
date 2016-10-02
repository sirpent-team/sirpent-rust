pub use square_grid::*;
pub use hexagon_grid::*;
pub use triangle_grid::*;
pub use vector::*;
pub use direction::*;

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub enum Grid {
    #[serde(rename = "square")]
    Square(SquareGrid),
    #[serde(rename = "hexagon")]
    Hexagon(HexagonGrid),
    #[serde(rename = "triangle")]
    Triangle(TriangleGrid),
}

impl Grid {
    pub fn square(width: isize, height: isize) -> Grid {
        Grid::Square(SquareGrid::new(width, height))
    }

    pub fn hexagon(radius: usize) -> Grid {
        Grid::Hexagon(HexagonGrid::new(radius))
    }

    pub fn triangle(radius: usize) -> Grid {
        Grid::Triangle(TriangleGrid::new(radius))
    }

    pub fn directions(&self) -> Vec<Direction> {
        match *self {
            Grid::Square(_) => Direction::squares(),
            Grid::Hexagon(_) => Direction::hexagons(),
            Grid::Triangle(_) => Direction::triangles(),
        }
    }
}

// pub trait DirectionTrait
// : PartialEq + Eq + Copy + Serialize + Deserialize + Clone + Debug
// {
// fn variants() -> &'static [Direction];
// }
//
// pub trait VectorTrait
// : PartialEq + Eq + Copy + Serialize + Deserialize + Clone + Debug {
// fn distance(&self, other: &VectorTrait) -> usize;
// fn neighbour(&self, direction: &Direction) -> VectorTrait;
// fn neighbours(&self) -> Vec<VectorTrait>;
// }
//
