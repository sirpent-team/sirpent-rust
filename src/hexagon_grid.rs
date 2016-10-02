use std::cmp::max;
use rand::Rng;

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub enum HexagonDirection {
    #[serde(rename = "north")]
    North,
    #[serde(rename = "northeast")]
    NorthEast,
    #[serde(rename = "southeast")]
    SouthEast,
    #[serde(rename = "south")]
    South,
    #[serde(rename = "southwest")]
    SouthWest,
    #[serde(rename = "northwest")]
    NorthWest,
}

impl HexagonDirection {
    pub fn variants() -> &'static [HexagonDirection] {
        static VARIANTS: &'static [HexagonDirection] = &[HexagonDirection::North,
                                                         HexagonDirection::NorthEast,
                                                         HexagonDirection::SouthEast,
                                                         HexagonDirection::South,
                                                         HexagonDirection::SouthWest,
                                                         HexagonDirection::NorthWest];
        VARIANTS
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub struct HexagonVector {
    pub x: isize,
    pub y: isize,
}

impl HexagonVector {
    pub fn distance(&self, other: &HexagonVector) -> usize {
        let xdist = (self.x - other.x).abs();
        let ydist = (self.y - other.y).abs();
        let zdist = ((self.x + self.y) - (other.x + other.y)).abs();
        return max(max(xdist, ydist), zdist) as usize;
    }

    pub fn neighbour(&self, direction: &HexagonDirection) -> HexagonVector {
        match *direction {
            HexagonDirection::North => {
                HexagonVector {
                    x: self.x,
                    y: self.y - 1,
                }
            }
            HexagonDirection::NorthEast => {
                HexagonVector {
                    x: self.x + 1,
                    y: self.y - 1,
                }
            }
            HexagonDirection::SouthEast => {
                HexagonVector {
                    x: self.x + 1,
                    y: self.y,
                }
            }
            HexagonDirection::South => {
                HexagonVector {
                    x: self.x,
                    y: self.y + 1,
                }
            }
            HexagonDirection::SouthWest => {
                HexagonVector {
                    x: self.x - 1,
                    y: self.y + 1,
                }
            }
            HexagonDirection::NorthWest => {
                HexagonVector {
                    x: self.x - 1,
                    y: self.y,
                }
            }
        }
    }

    pub fn neighbours(&self) -> Vec<Self> {
        let mut neighbours = vec![];
        for variant in HexagonDirection::variants() {
            neighbours.push(self.neighbour(variant));
        }
        neighbours
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub struct HexagonGrid {
    pub radius: usize,
}

impl HexagonGrid {
    pub fn new(radius: usize) -> HexagonGrid {
        HexagonGrid { radius: radius }
    }
}

impl HexagonGrid {
    pub fn dimensions(&self) -> Vec<isize> {
        vec![self.radius as isize]
    }

    pub fn is_within_bounds(&self, v: HexagonVector) -> bool {
        // @TODO: Calculate a more efficient bounding rule.
        HexagonVector { x: 0, y: 0 }.distance(&v) <= self.radius
    }

    pub fn cells(&self) -> Vec<HexagonVector> {
        unimplemented!();
    }

    pub fn random_cell<R: Rng>(&self) -> HexagonVector {
        unimplemented!();
    }
}

#[cfg(test)]
mod tests {
    use quickcheck::{Gen, Arbitrary, quickcheck};
    use super::*;

    impl Arbitrary for HexagonVector {
        fn arbitrary<G: Gen>(g: &mut G) -> HexagonVector {
            let (x, y) = Arbitrary::arbitrary(g);
            return HexagonVector { x: x, y: y };
        }
    }

    impl Arbitrary for HexagonDirection {
        fn arbitrary<G: Gen>(g: &mut G) -> HexagonDirection {
            let i: usize = g.gen_range(0, 6);
            HexagonDirection::variants()[i].clone()
        }
    }

    fn identity_of_indescernibles_prop(v: HexagonVector) -> bool {
        v.distance(&v) == 0
    }

    #[test]
    fn identity_of_indescernibles() {
        quickcheck(identity_of_indescernibles_prop as fn(HexagonVector) -> bool);
    }

    fn triangle_inequality_prop(u: HexagonVector, v: HexagonVector, w: HexagonVector) -> bool {
        u.distance(&w) <= u.distance(&v) + v.distance(&w)
    }

    #[test]
    fn triangle_inequality() {
        quickcheck(triangle_inequality_prop as fn(HexagonVector, HexagonVector, HexagonVector)
                                                  -> bool);
    }

    fn symmetry_prop(v: HexagonVector, w: HexagonVector) -> bool {
        v.distance(&w) == w.distance(&v)
    }

    #[test]
    fn symmetry() {
        quickcheck(symmetry_prop as fn(HexagonVector, HexagonVector) -> bool);
    }

    fn neighbour_adjacency_prop(v: HexagonVector, d: HexagonDirection) -> bool {
        v.distance(&v.neighbour(&d)) == 1
    }

    #[test]
    fn neighbour_adjacency() {
        quickcheck(neighbour_adjacency_prop as fn(HexagonVector, HexagonDirection) -> bool);
    }
}
