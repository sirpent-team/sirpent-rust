use rand::Rng;

use super::traits::*;

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub enum SquareDirection {
    #[serde(rename = "north")]
    North,
    #[serde(rename = "east")]
    East,
    #[serde(rename = "south")]
    South,
    #[serde(rename = "west")]
    West,
}

impl DirectionTrait for SquareDirection {
    fn variants() -> &'static [SquareDirection] {
        static VARIANTS: &'static [SquareDirection] = &[SquareDirection::North,
                                                        SquareDirection::East,
                                                        SquareDirection::South,
                                                        SquareDirection::West];
        VARIANTS
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub struct SquareVector {
    pub x: isize,
    pub y: isize,
}

impl VectorTrait for SquareVector {
    type Direction = SquareDirection;

    fn distance(&self, other: &SquareVector) -> usize {
        let xdist = (self.x - other.x).abs();
        let ydist = (self.y - other.y).abs();
        (xdist + ydist) as usize
    }

    fn neighbour(&self, direction: &SquareDirection) -> SquareVector {
        match *direction {
            SquareDirection::North => {
                SquareVector {
                    x: self.x,
                    y: self.y - 1,
                }
            }
            SquareDirection::East => {
                SquareVector {
                    x: self.x + 1,
                    y: self.y,
                }
            }
            SquareDirection::South => {
                SquareVector {
                    x: self.x,
                    y: self.y + 1,
                }
            }
            SquareDirection::West => {
                SquareVector {
                    x: self.x - 1,
                    y: self.y,
                }
            }
        }
    }

    fn neighbours(&self) -> Vec<Self> {
        SquareDirection::variants().into_iter().map(|d| self.neighbour(d)).collect()
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub struct SquareGrid {
    pub width: usize,
    pub height: usize,
}

impl SquareGrid {
    #[allow(dead_code)]
    pub fn new(width: usize, height: usize) -> SquareGrid {
        SquareGrid {
            width: width,
            height: height,
        }
    }
}

impl GridTrait for SquareGrid {
    type Vector = SquareVector;

    fn dimensions(&self) -> Vec<isize> {
        vec![self.width as isize, self.height as isize]
    }

    fn is_within_bounds(&self, v: SquareVector) -> bool {
        v.x >= 0 && v.x < (self.width as isize) && v.y >= 0 && v.y < (self.height as isize)
    }

    fn cells(&self) -> Vec<SquareVector> {
        unimplemented!();
    }

    fn random_cell<R: Rng>(&self, rng: &mut R) -> SquareVector {
        let isize_width = self.width as isize;
        let isize_height = self.height as isize;
        SquareVector {
            x: rng.gen_range(0, isize_width),
            y: rng.gen_range(0, isize_height),
        }
    }
}

#[cfg(test)]
mod tests {
    use quickcheck::{Gen, Arbitrary, quickcheck};
    use super::*;
    use rand::OsRng;

    impl Arbitrary for SquareVector {
        fn arbitrary<G: Gen>(g: &mut G) -> SquareVector {
            let (x, y) = Arbitrary::arbitrary(g);
            return SquareVector { x: x, y: y };
        }
    }

    impl Arbitrary for SquareDirection {
        fn arbitrary<G: Gen>(g: &mut G) -> SquareDirection {
            let i: usize = g.gen_range(0, 4);
            SquareDirection::variants()[i].clone()
        }
    }

    impl Arbitrary for SquareGrid {
        fn arbitrary<G: Gen>(g: &mut G) -> SquareGrid {
            let (mut width, mut height) = Arbitrary::arbitrary(g);
            if width == 0 {
                width = 1;
            }
            if height == 0 {
                height = 1;
            }
            return SquareGrid {
                width: width,
                height: height,
            };
        }
    }

    fn identity_of_indescernibles_prop(v: SquareVector) -> bool {
        v.distance(&v) == 0
    }

    #[test]
    fn identity_of_indescernibles() {
        quickcheck(identity_of_indescernibles_prop as fn(SquareVector) -> bool);
    }

    fn triangle_inequality_prop(u: SquareVector, v: SquareVector, w: SquareVector) -> bool {
        u.distance(&w) <= u.distance(&v) + v.distance(&w)
    }

    #[test]
    fn triangle_inequality() {
        quickcheck(triangle_inequality_prop as fn(SquareVector, SquareVector, SquareVector) -> bool);
    }

    fn symmetry_prop(v: SquareVector, w: SquareVector) -> bool {
        v.distance(&w) == w.distance(&v)
    }

    #[test]
    fn symmetry() {
        quickcheck(symmetry_prop as fn(SquareVector, SquareVector) -> bool);
    }

    fn neighbour_adjacency_prop(v: SquareVector, d: SquareDirection) -> bool {
        v.distance(&v.neighbour(&d)) == 1
    }

    #[test]
    fn neighbour_adjacency() {
        quickcheck(neighbour_adjacency_prop as fn(SquareVector, SquareDirection) -> bool);
    }

    fn random_cells_within_bounds_prop(g: SquareGrid) -> bool {
        let mut osrng = OsRng::new().unwrap();
        for _ in 0..1000 {
            let random_cell = g.random_cell(&mut osrng);
            if !g.is_within_bounds(random_cell) {
                return false;
            }
        }
        return true;
    }

    #[test]
    fn random_cells_within_bounds() {
        quickcheck(random_cells_within_bounds_prop as fn(SquareGrid) -> bool);
    }
}
