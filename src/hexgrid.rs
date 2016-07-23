use std::cmp::max;
use std::collections::hash_map::{HashMap, RandomState};

use rand::Rng;
use uuid::Uuid;

use grid::*;
use snake::*;

#[derive(Clone, Debug)]
pub enum HexDir {
    North,
    NorthEast,
    SouthEast,
    South,
    SouthWest,
    NorthWest
}

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
pub struct HexVector {
    pub x : isize,
    pub y : isize
}

impl Vector for HexVector {
    type Direction = HexDir;
    fn distance(&self, other : &HexVector) -> usize {
        let xdist = (self.x - other.x).abs();
        let ydist = (self.y - other.y).abs();
        let zdist = ((self.x + self.y) - (other.x + other.y)).abs();
        return max(max(xdist, ydist), zdist) as usize;
    }
    fn subtract(&self, other : &HexVector) -> HexVector {
        HexVector{x : self.x - other.x, y : self.y - other.y}
    }
    fn neighbour(&self, direction : &HexDir) -> HexVector {
        match *direction {
            HexDir::North     => HexVector {x : self.x    , y : self.y - 1},
            HexDir::NorthEast => HexVector {x : self.x + 1, y : self.y - 1},
            HexDir::SouthEast => HexVector {x : self.x + 1, y : self.y    },
            HexDir::South     => HexVector {x : self.x    , y : self.y + 1},
            HexDir::SouthWest => HexVector {x : self.x - 1, y : self.y + 1},
            HexDir::NorthWest => HexVector {x : self.x - 1, y : self.y    }
        }
    }
    fn ball_around(&self, radius : usize) -> Vec<Self> {
        let mut r = Vec::new();
        let sradius = radius as isize;
        for x in -sradius..sradius + 1 {
            for y in -sradius..sradius + 1 {
                let z : isize = x + y;
                if z.abs() <= sradius {
                    r.push(HexVector{x : x + self.x, y : y + self.y});
                }
            }
        }
        return r;
    }
    fn rand_within<R : Rng>(rng : &mut R, radius : usize) -> Self {
        let zero = HexVector{x : 0, y : 0};
        loop {
            let x = rng.gen_range(-(radius as isize), radius as isize);
            let y = rng.gen_range(-(radius as isize), radius as isize);
            let hv = HexVector{x : x, y : y};
            if zero.distance(&hv) <= radius {
                return hv;
            }
        }
    }
}

pub struct HexGrid {
    width : isize,
    height : isize,
    view : usize,
    board : HashMap<HexVector, Cell, RandomState>
}

impl Grid for HexGrid {
    type Vector = HexVector;
    fn new(radius : usize) -> HexGrid {
        let width = 2 * radius as isize;
        let height = width;
        let view = 5;
        let mut board = HashMap::new();
        for i in (HexVector {x : 0, y : 0}).ball_around(radius) {
            board.insert(i, Cell::Empty);
        }
        return HexGrid{width : width, height : height, view : view, board : board};
    }
    fn dimensions(&self) -> Vec<isize> {
        vec![self.width, self.height]
    }
    fn is_within_bounds(&self, v : HexVector) -> bool {
        v.x >= 0 && v.x < self.width && v.y >= 0 && v.y < self.height
    }
    fn add_snake_at(&mut self, coord : HexVector) -> Option<Snake<Self::Vector>> {
        if self.board.get(&coord) != Some(&Cell::Empty) {
            return None;
        }
        let id = Uuid::new_v4();
        self.board.insert(coord, Cell::Segment(id));
        return Some(Snake{growing : false, uuid : id, segments : vec!(coord)});
    }
    fn get_local_map(&self, v : HexVector) -> HashMap<HexVector, Cell, RandomState> {
        let mut r = HashMap::new();
        for w in v.ball_around(self.view) {
            if let Some(cc) = self.board.get(&w) {
                r.insert(w.subtract(&v), cc.clone());
            }
        }
        return r;
    }
    fn get_cell_at(&self, v : HexVector) -> Option<Cell> {
        self.board.get(&v).cloned()
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
        v.distance(&v.neighbour(&d)) == 1
    }

    #[test]
    fn neighbour_adjacency() {
        quickcheck(neighbour_adjacency_prop as fn(HexVector, HexDir) -> bool);
    }

    fn ball_radius_prop(v : HexVector, r : usize) -> bool {
        for w in v.ball_around(r) {
            if v.distance(&w) > r {
                return false;
            }
        }
        return true;
    }

    #[test]
    fn ball_radius() {
        quickcheck(ball_radius_prop as fn(HexVector, usize) -> bool);
    }

    fn ball_point_count_prop(v : HexVector, r : usize) -> bool {
        v.ball_around(r).len() == 3 * r * (r + 1) + 1 // centered hexagonal number
    }

    #[test]
    fn ball_point_count() {
        quickcheck(ball_point_count_prop as fn(HexVector, usize) -> bool);
    }
}
               
