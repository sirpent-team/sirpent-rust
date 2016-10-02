use square_grid::*;
use hexagon_grid::*;
use triangle_grid::*;

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub enum Direction {
    #[serde(rename = "square")]
    Square(SquareDirection),
    #[serde(rename = "hexagon")]
    Hexagon(HexagonDirection),
    #[serde(rename = "triangle")]
    Triangle(TriangleDirection),
}

impl Direction {
    pub fn squares() -> Vec<Direction> {
        SquareDirection::variants().iter().map(map_to_direction_square).collect()
    }

    pub fn hexagons() -> Vec<Direction> {
        HexagonDirection::variants().iter().map(map_to_direction_hexagon).collect()
    }

    pub fn triangles() -> Vec<Direction> {
        TriangleDirection::variants().iter().map(map_to_direction_triangle).collect()
    }
}

fn map_to_direction_square(d: &SquareDirection) -> Direction {
    Direction::Square(*d)
}

fn map_to_direction_hexagon(d: &HexagonDirection) -> Direction {
    Direction::Hexagon(*d)
}

fn map_to_direction_triangle(d: &TriangleDirection) -> Direction {
    Direction::Triangle(*d)
}
