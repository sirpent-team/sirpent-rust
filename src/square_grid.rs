use grid::*;

#[derive(Clone, Debug)]
pub enum SquareDir {
    North,
    East,
    South,
    West
}

impl Direction for SquareDir {
    fn variants() -> &'static [SquareDir] {
        static VARIANTS: &'static [SquareDir] = &[SquareDir::North, SquareDir::East, SquareDir::South, SquareDir::West];
        VARIANTS
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
pub struct SquareVector {
    pub x : isize,
    pub y : isize
}

impl Vector for SquareVector {
    type Direction = SquareDir;

    fn distance(&self, other : &SquareVector) -> usize {
        let xdist = (self.x - other.x).abs();
        let ydist = (self.y - other.y).abs();
        (xdist + ydist) as usize
    }

    fn neighbour(&self, direction : &SquareDir) -> SquareVector {
        match *direction {
            SquareDir::North => SquareVector {x : self.x    , y : self.y - 1},
            SquareDir::East  => SquareVector {x : self.x + 1, y : self.y    },
            SquareDir::South => SquareVector {x : self.x    , y : self.y + 1},
            SquareDir::West  => SquareVector {x : self.x - 1, y : self.y    },
        }
    }

    fn neighbours(&self) -> Vec<Self> {
        let mut neighbours = vec![];
        for variant in SquareDir::variants() {
            neighbours.push(self.neighbour(variant));
        }
        neighbours
    }
}

pub struct SquareGrid {
    width : isize,
    height : isize,
}

impl Grid for SquareGrid {
    type Vector = SquareVector;

    fn dimensions(&self) -> Vec<isize> {
        vec![self.width, self.height]
    }

    fn is_within_bounds(&self, v : SquareVector) -> bool {
        v.x >= 0 && v.x < self.width && v.y >= 0 && v.y < self.height
    }
}

impl SquareGrid {
    fn new(width : isize, height : isize) -> SquareGrid {
        SquareGrid{width : width, height : height}
    }
}

#[cfg(test)]
mod tests {
    use quickcheck::{Gen, Arbitrary, quickcheck};
    use super::*;
    use grid::Vector;
    use grid::Direction;

    impl Arbitrary for SquareVector {
        fn arbitrary<G : Gen>(g : &mut G) -> SquareVector {
            let (x, y) = Arbitrary::arbitrary(g);
            return SquareVector{x : x, y : y};
        }
    }

    impl Arbitrary for SquareDir {
        fn arbitrary<G : Gen>(g : &mut G) -> SquareDir {
            let i : usize = g.gen_range(0, 4);
            SquareDir::variants()[i].clone()
        }
    }

    fn identity_of_indescernibles_prop(v : SquareVector) -> bool {
        v.distance(&v) == 0
    }

    #[test]
    fn identity_of_indescernibles() {
        quickcheck(identity_of_indescernibles_prop as fn(SquareVector) -> bool);
    }

    fn triangle_inequality_prop(u : SquareVector, v : SquareVector, w : SquareVector) -> bool {
        u.distance(&w) <= u.distance(&v) + v.distance(&w)
    }

    #[test]
    fn triangle_inequality() {
        quickcheck(triangle_inequality_prop as fn(SquareVector, SquareVector, SquareVector) -> bool);
    }

    fn symmetry_prop(v : SquareVector, w : SquareVector) -> bool {
        v.distance(&w) == w.distance(&v)
    }

    #[test]
    fn symmetry() {
        quickcheck(symmetry_prop as fn(SquareVector, SquareVector) -> bool);
    }

    fn neighbour_adjacency_prop(v : SquareVector, d : SquareDir) -> bool {
        v.distance(&v.neighbour(&d)) == 1
    }

    #[test]
    fn neighbour_adjacency() {
        quickcheck(neighbour_adjacency_prop as fn(SquareVector, SquareDir) -> bool);
    }
}
