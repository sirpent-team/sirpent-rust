use std::cmp::max;
use grid::*;

#[derive(Clone, Debug)]
pub enum HexagonDir {
    North,
    NorthEast,
    SouthEast,
    South,
    SouthWest,
    NorthWest
}

impl Direction for HexagonDir {
    fn variants() -> &'static [HexagonDir] {
        static VARIANTS: &'static [HexagonDir] = &[HexagonDir::North, HexagonDir::NorthEast, HexagonDir::SouthEast, HexagonDir::South, HexagonDir::SouthWest, HexagonDir::NorthWest];
        VARIANTS
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
pub struct HexagonVector {
    pub x : isize,
    pub y : isize
}

impl Vector for HexagonVector {
    type Direction = HexagonDir;

    fn distance(&self, other : &HexagonVector) -> usize {
        let xdist = (self.x - other.x).abs();
        let ydist = (self.y - other.y).abs();
        let zdist = ((self.x + self.y) - (other.x + other.y)).abs();
        return max(max(xdist, ydist), zdist) as usize;
    }

    fn neighbour(&self, direction : &HexagonDir) -> HexagonVector {
        match *direction {
            HexagonDir::North     => HexagonVector {x : self.x    , y : self.y - 1},
            HexagonDir::NorthEast => HexagonVector {x : self.x + 1, y : self.y - 1},
            HexagonDir::SouthEast => HexagonVector {x : self.x + 1, y : self.y    },
            HexagonDir::South     => HexagonVector {x : self.x    , y : self.y + 1},
            HexagonDir::SouthWest => HexagonVector {x : self.x - 1, y : self.y + 1},
            HexagonDir::NorthWest => HexagonVector {x : self.x - 1, y : self.y    }
        }
    }

    fn neighbours(&self) -> Vec<Self> {
        let mut neighbours = vec![];
        for variant in HexagonDir::variants() {
            neighbours.push(self.neighbour(variant));
        }
        neighbours
    }
}

pub struct HexagonGrid {
    radius : usize,
}

impl Grid for HexagonGrid {
    type Vector = HexagonVector;

    fn dimensions(&self) -> Vec<isize> {
        vec![self.radius as isize]
    }

    fn is_within_bounds(&self, v : HexagonVector) -> bool {
        // @TODO: Calculate a more efficient bounding rule.
        HexagonVector{x : 0, y : 0}.distance(&v) <= self.radius
    }
}

impl HexagonGrid {
    fn new(radius : usize) -> HexagonGrid {
        HexagonGrid{radius : radius}
    }
}

#[cfg(test)]
mod tests {
    use quickcheck::{Gen, Arbitrary, quickcheck};
    use super::*;
    use grid::Direction;
    use grid::Vector;

    impl Arbitrary for HexagonVector {
        fn arbitrary<G : Gen>(g : &mut G) -> HexagonVector {
            let (x, y) = Arbitrary::arbitrary(g);
            return HexagonVector{x : x, y : y};
        }
    }

    impl Arbitrary for HexagonDir {
        fn arbitrary<G : Gen>(g : &mut G) -> HexagonDir {
            let i : usize = g.gen_range(0, 6);
            HexagonDir::variants()[i].clone()
        }
    }

    fn identity_of_indescernibles_prop(v : HexagonVector) -> bool {
        v.distance(&v) == 0
    }

    #[test]
    fn identity_of_indescernibles() {
        quickcheck(identity_of_indescernibles_prop as fn(HexagonVector) -> bool);
    }

    fn triangle_inequality_prop(u : HexagonVector, v : HexagonVector, w : HexagonVector) -> bool {
        u.distance(&w) <= u.distance(&v) + v.distance(&w)
    }

    #[test]
    fn triangle_inequality() {
        quickcheck(triangle_inequality_prop as fn(HexagonVector, HexagonVector, HexagonVector) -> bool);
    }

    fn symmetry_prop(v : HexagonVector, w : HexagonVector) -> bool {
        v.distance(&w) == w.distance(&v)
    }

    #[test]
    fn symmetry() {
        quickcheck(symmetry_prop as fn(HexagonVector, HexagonVector) -> bool);
    }

    fn neighbour_adjacency_prop(v : HexagonVector, d : HexagonDir) -> bool {
        v.distance(&v.neighbour(&d)) == 1
    }

    #[test]
    fn neighbour_adjacency() {
        quickcheck(neighbour_adjacency_prop as fn(HexagonVector, HexagonDir) -> bool);
    }
}
