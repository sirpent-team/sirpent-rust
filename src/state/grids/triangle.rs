use rand::Rng;

use super::traits::*;

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub enum TriangleDirection {
    #[serde(rename = "east")]
    East,
    #[serde(rename = "south")]
    South,
    #[serde(rename = "west")]
    West,
}

impl DirectionTrait for TriangleDirection {
    fn variants() -> &'static [TriangleDirection] {
        static VARIANTS: &'static [TriangleDirection] =
            &[TriangleDirection::East, TriangleDirection::South, TriangleDirection::West];
        VARIANTS
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub struct TriangleVector {
    pub u: isize,
    pub v: isize,
    pub r: bool,
}

impl VectorTrait for TriangleVector {
    type Direction = TriangleDirection;

    fn distance(&self, other: &TriangleVector) -> usize {
        // http://simblob.blogspot.co.uk/2007/06/distances-on-triangular-grid.html
        // distance = abs(Δu) + abs(Δv) + abs(Δ(u+v+R))
        let du = (self.u - other.u).abs();
        let dv = (self.v - other.v).abs();
        let d3 = ((self.u + self.v + (self.r as isize)) - (other.u + other.v + (other.r as isize)))
            .abs();
        (du + dv + d3) as usize
    }

    fn neighbour(&self, direction: &TriangleDirection) -> TriangleVector {
        match (self.r, *direction) {
            (true, TriangleDirection::East) => {
                TriangleVector {
                    u: self.u + 1,
                    v: self.v,
                    r: false,
                }
            }
            (true, TriangleDirection::South) => {
                TriangleVector {
                    u: self.u,
                    v: self.v + 1,
                    r: false,
                }
            }
            (true, TriangleDirection::West) => {
                TriangleVector {
                    u: self.u,
                    v: self.v,
                    r: false,
                }
            }
            (false, TriangleDirection::East) => {
                TriangleVector {
                    u: self.u,
                    v: self.v,
                    r: true,
                }
            }
            (false, TriangleDirection::South) => {
                TriangleVector {
                    u: self.u,
                    v: self.v - 1,
                    r: true,
                }
            }
            (false, TriangleDirection::West) => {
                TriangleVector {
                    u: self.u - 1,
                    v: self.v,
                    r: true,
                }
            }
        }
    }

    fn neighbours(&self) -> Vec<Self> {
        TriangleDirection::variants().into_iter().map(|d| self.neighbour(d)).collect()
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
pub struct TriangleGrid {
    pub radius: usize,
}

impl TriangleGrid {
    #[allow(dead_code)]
    pub fn new(radius: usize) -> TriangleGrid {
        TriangleGrid { radius: radius }
    }
}

impl GridTrait for TriangleGrid {
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

    fn random_cell<R: Rng>(&self, _: &mut R) -> TriangleVector {
        unimplemented!();
    }
}

#[cfg(test)]
mod tests {
    use quickcheck::{Gen, Arbitrary, quickcheck};
    use super::*;
    use rand::OsRng;

    impl Arbitrary for TriangleVector {
        fn arbitrary<G: Gen>(g: &mut G) -> TriangleVector {
            let (u, v, r) = Arbitrary::arbitrary(g);
            return TriangleVector { u: u, v: v, r: r };
        }
    }

    impl Arbitrary for TriangleDirection {
        fn arbitrary<G: Gen>(g: &mut G) -> TriangleDirection {
            let i: usize = g.gen_range(0, 3);
            TriangleDirection::variants()[i].clone()
        }
    }

    impl Arbitrary for TriangleGrid {
        fn arbitrary<G: Gen>(g: &mut G) -> TriangleGrid {
            let radius = Arbitrary::arbitrary(g);
            return TriangleGrid { radius: radius };
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
        quickcheck(triangle_inequality_prop as
                   fn(TriangleVector, TriangleVector, TriangleVector) -> bool);
    }

    fn symmetry_prop(v: TriangleVector, w: TriangleVector) -> bool {
        v.distance(&w) == w.distance(&v)
    }

    #[test]
    fn symmetry() {
        quickcheck(symmetry_prop as fn(TriangleVector, TriangleVector) -> bool);
    }

    fn neighbour_adjacency_prop(v: TriangleVector, d: TriangleDirection) -> bool {
        v.distance(&v.neighbour(&d)) == 1
    }

    #[test]
    fn neighbour_adjacency() {
        quickcheck(neighbour_adjacency_prop as fn(TriangleVector, TriangleDirection) -> bool);
    }

    fn random_cells_within_bounds_prop(g: TriangleGrid) -> bool {
        let mut osrng = OsRng::new().unwrap();
        for _ in 0..1000 {
            let random_cell = g.random_cell(&mut osrng);
            if !g.is_within_bounds(random_cell) {
                return false;
            }
        }
        return true;
    }

    //#[test]
    fn random_cells_within_bounds() {
        quickcheck(random_cells_within_bounds_prop as fn(TriangleGrid) -> bool);
    }
}
