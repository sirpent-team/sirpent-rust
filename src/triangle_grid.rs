use rand::Rng;

use grid::*;

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub enum TriangleDir {
    East,
    South,
    West,
}

impl Direction for TriangleDir {
    fn variants() -> &'static [TriangleDir] {
        static VARIANTS: &'static [TriangleDir] =
            &[TriangleDir::East, TriangleDir::South, TriangleDir::West];
        VARIANTS
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub struct TriangleVector {
    pub u: isize,
    pub v: isize,
    pub r: bool,
}

impl Vector for TriangleVector {
    type Direction = TriangleDir;

    fn distance(&self, other: &TriangleVector) -> usize {
        // http://simblob.blogspot.co.uk/2007/06/distances-on-triangular-grid.html
        // distance = abs(Δu) + abs(Δv) + abs(Δ(u+v+R))
        let du = (self.u - other.u).abs();
        let dv = (self.v - other.v).abs();
        let d3 = (((self.u + self.v + (self.r as isize))) -
                  ((other.u + other.v + (other.r as isize))))
            .abs();
        (du + dv + d3) as usize
    }

    fn neighbour(&self, direction: &TriangleDir) -> TriangleVector {
        match self.r {
            true => {
                match *direction {
                    TriangleDir::East => {
                        TriangleVector {
                            u: self.u + 1,
                            v: self.v,
                            r: false,
                        }
                    }
                    TriangleDir::South => {
                        TriangleVector {
                            u: self.u,
                            v: self.v + 1,
                            r: false,
                        }
                    }
                    TriangleDir::West => {
                        TriangleVector {
                            u: self.u,
                            v: self.v,
                            r: false,
                        }
                    }
                }
            }
            false => {
                match *direction {
                    TriangleDir::East => {
                        TriangleVector {
                            u: self.u,
                            v: self.v,
                            r: true,
                        }
                    }
                    TriangleDir::South => {
                        TriangleVector {
                            u: self.u,
                            v: self.v - 1,
                            r: true,
                        }
                    }
                    TriangleDir::West => {
                        TriangleVector {
                            u: self.u - 1,
                            v: self.v,
                            r: true,
                        }
                    }
                }
            }
        }
    }

    fn neighbours(&self) -> Vec<Self> {
        let mut neighbours = vec![];
        for variant in TriangleDir::variants() {
            neighbours.push(self.neighbour(variant));
        }
        neighbours
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub struct TriangleGrid {
    pub radius: usize,
}

impl TriangleGrid {
    pub fn new(radius: usize) -> TriangleGrid {
        TriangleGrid { radius: radius }
    }
}

impl Grid for TriangleGrid {
    type Vector = TriangleVector;

    fn dimensions(&self) -> Vec<isize> {
        vec![self.radius as isize]
    }

    fn is_within_bounds(&self, v: TriangleVector) -> bool {
        // @TODO: Calculate a more efficient bounding rule.
        TriangleVector {
                u: 0,
                v: 0,
                r: false,
            }
            .distance(&v) <= self.radius
    }

    fn cells(&self) -> Vec<TriangleVector> {
        unimplemented!();
    }

    fn random_cell<R: Rng>(&self) -> TriangleVector {
        unimplemented!();
    }
}

#[cfg(test)]
mod tests {
    use quickcheck::{Gen, Arbitrary, quickcheck};
    use super::*;
    use grid::Vector;
    use grid::Direction;

    impl Arbitrary for TriangleVector {
        fn arbitrary<G: Gen>(g: &mut G) -> TriangleVector {
            let (u, v, r) = Arbitrary::arbitrary(g);
            return TriangleVector { u: u, v: v, r: r };
        }
    }

    impl Arbitrary for TriangleDir {
        fn arbitrary<G: Gen>(g: &mut G) -> TriangleDir {
            let i: usize = g.gen_range(0, 3);
            TriangleDir::variants()[i].clone()
        }
    }

    fn identity_of_indescernibles_prop(v: TriangleVector) -> bool {
        v.distance(&v) == 0
    }

    #[test]
    fn identity_of_indescernibles() {
        quickcheck(identity_of_indescernibles_prop as fn(TriangleVector) -> bool);
    }

    fn triangle_inequality_prop(u: TriangleVector, v: TriangleVector, w: TriangleVector) -> bool {
        u.distance(&w) <= u.distance(&v) + v.distance(&w)
    }

    #[test]
    fn triangle_inequality() {
        quickcheck(triangle_inequality_prop as fn(TriangleVector, TriangleVector, TriangleVector)
                                                  -> bool);
    }

    fn symmetry_prop(v: TriangleVector, w: TriangleVector) -> bool {
        v.distance(&w) == w.distance(&v)
    }

    #[test]
    fn symmetry() {
        quickcheck(symmetry_prop as fn(TriangleVector, TriangleVector) -> bool);
    }

    fn neighbour_adjacency_prop(v: TriangleVector, d: TriangleDir) -> bool {
        v.distance(&v.neighbour(&d)) == 1
    }

    #[test]
    fn neighbour_adjacency() {
        quickcheck(neighbour_adjacency_prop as fn(TriangleVector, TriangleDir) -> bool);
    }
}
