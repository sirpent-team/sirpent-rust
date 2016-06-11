pub trait Vector : Eq + Copy {
    type Direction;
    fn distance(&self, other : &Self) -> isize;
    fn neighbour(&self, direction : Self::Direction) -> Self;
}

pub trait Grid {
    type Vector : Vector;
    fn dimensions(&self) -> Vec<isize>;
    fn is_within_bounds(&self, v : Self::Vector) -> bool;
}
