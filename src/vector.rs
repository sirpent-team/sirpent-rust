use direction::*;
use square_grid::*;
use hexagon_grid::*;
use triangle_grid::*;

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub enum Vector {
    #[serde(rename = "square")]
    Square(SquareVector),
    #[serde(rename = "hexagon")]
    Hexagon(HexagonVector),
    #[serde(rename = "triangle")]
    Triangle(TriangleVector),
}

impl Vector {
    pub fn square(x: isize, y: isize) -> Vector {
        Vector::Square(SquareVector { x: x, y: y })
    }

    pub fn hexagon(x: isize, y: isize) -> Vector {
        Vector::Hexagon(HexagonVector { x: x, y: y })
    }

    pub fn triangle(u: isize, v: isize, r: bool) -> Vector {
        Vector::Triangle(TriangleVector { u: u, v: v, r: r })
    }

    pub fn distance(&self, other: &Vector) -> usize {
        match (*self, *other) {
            (Vector::Square(v1), Vector::Square(v2)) => v1.distance(&v2),
            (Vector::Hexagon(v1), Vector::Hexagon(v2)) => v1.distance(&v2),
            (Vector::Triangle(v1), Vector::Triangle(v2)) => v1.distance(&v2),
            _ => unimplemented!(),
        }
    }

    pub fn neighbour(&self, direction: &Direction) -> Vector {
        match (*self, *direction) {
            (Vector::Square(v), Direction::Square(d)) => Vector::Square(v.neighbour(&d)),
            (Vector::Hexagon(v), Direction::Hexagon(d)) => Vector::Hexagon(v.neighbour(&d)),
            (Vector::Triangle(v), Direction::Triangle(d)) => Vector::Triangle(v.neighbour(&d)),
            _ => unimplemented!(),
        }
    }

    pub fn neighbours(&self) -> Vec<Self> {
        match *self {
            Vector::Square(v) => v.neighbours().iter().map(map_to_vector_square).collect(),
            Vector::Hexagon(v) => v.neighbours().iter().map(map_to_vector_hexagon).collect(),
            Vector::Triangle(v) => v.neighbours().iter().map(map_to_vector_triangle).collect(),
        }
    }

    pub fn directions(&self) -> Vec<Direction> {
        match *self {
            Vector::Square(_) => Direction::squares(),
            Vector::Hexagon(_) => Direction::hexagons(),
            Vector::Triangle(_) => Direction::triangles(),
        }
    }
}

fn map_to_vector_square(v: &SquareVector) -> Vector {
    Vector::Square(*v)
}

fn map_to_vector_hexagon(v: &HexagonVector) -> Vector {
    Vector::Hexagon(*v)
}

fn map_to_vector_triangle(v: &TriangleVector) -> Vector {
    Vector::Triangle(*v)
}
