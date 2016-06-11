use std::cmp::max;

use grid::*;

#[derive(Clone, Debug)]
pub enum HexDir {
    North,
    NorthEast,
    SouthEast,
    South,
    SouthWest,
    NorthWest
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct HexVector {
    pub x : isize,
    pub y : isize
}

impl Vector for HexVector {
    type Direction = HexDir;
    fn distance(&self, other : &HexVector) -> isize {
        let xdist = (self.x - other.x).abs();
        let ydist = (self.y - other.y).abs();
        let zdist = ((self.x + self.y) - (other.x + other.y)).abs();
        return max(max(xdist, ydist), zdist);
    }
    fn neighbour(&self, direction : HexDir) -> HexVector {
        match direction {
            HexDir::North     => HexVector {x : self.x    , y : self.y - 1},
            HexDir::NorthEast => HexVector {x : self.x + 1, y : self.y - 1},
            HexDir::SouthEast => HexVector {x : self.x + 1, y : self.y    },
            HexDir::South     => HexVector {x : self.x    , y : self.y + 1},
            HexDir::SouthWest => HexVector {x : self.x - 1, y : self.y + 1},
            HexDir::NorthWest => HexVector {x : self.x - 1, y : self.y    }
        }
    }
}

#[derive(Clone, Debug)]
pub struct HexGrid {
    width : isize,
    height : isize
}

impl Grid for HexGrid {
    type Vector = HexVector;
    fn dimensions(&self) -> Vec<isize> {
        vec![self.width, self.height]
    }
    fn is_within_bounds(&self, v : HexVector) -> bool {
        v.x >= 0 && v.x < self.width && v.y >= 0 && v.y < self.height
    }
}

#[cfg(test)]
mod tests {
    use quickcheck::{Gen, Arbitrary, quickcheck};
    use super::*;
    use grid::Vector;

    impl Arbitrary for HexVector {
        fn arbitrary<G : Gen>(g : &mut G) -> HexVector {
            let (x, y) = Arbitrary::arbitrary(g);
            return HexVector{x : x, y : y};
        }
    }

    impl Arbitrary for HexDir {
        fn arbitrary<G : Gen>(g : &mut G) -> HexDir {
            let i : u32 = g.gen_range(0, 6);
            match i {
                0 => HexDir::North,
                1 => HexDir::NorthEast,
                2 => HexDir::SouthEast,
                3 => HexDir::South,
                4 => HexDir::SouthWest,
                5 => HexDir::NorthWest,
                _ => unreachable!()
            }
        }
    }

    fn identity_of_indescernibles_prop(v : HexVector) -> bool {
        v.distance(&v) == 0
    }

    #[test]
    fn identity_of_indescernibles() {
        quickcheck(identity_of_indescernibles_prop as fn(HexVector) -> bool);
    }

    fn triangle_inequality_prop(u : HexVector, v : HexVector, w : HexVector) -> bool {
        u.distance(&w) <= u.distance(&v) + v.distance(&w)
    }

    #[test]
    fn triangle_inequality() {
        quickcheck(triangle_inequality_prop as fn(HexVector, HexVector, HexVector) -> bool);
    }

    fn symmetry_prop(v : HexVector, w : HexVector) -> bool {
        v.distance(&w) == w.distance(&v)
    }

    #[test]
    fn symmetry() {
        quickcheck(symmetry_prop as fn(HexVector, HexVector) -> bool);
    }

    fn neighbour_adjacency_prop(v : HexVector, d : HexDir) -> bool {
        v.distance(&v.neighbour(d)) == 1
    }

    #[test]
    fn neighbour_adjacency() {
        quickcheck(neighbour_adjacency_prop as fn(HexVector, HexDir) -> bool);
    }
}
               
