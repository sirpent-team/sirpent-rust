use std::fmt::Debug;

use grid::*;

#[derive(Clone, Copy, Debug)]
pub struct SnakeSegment<G : Grid> {
    pub position : G::Vector
}

#[derive(Clone, Debug)]
pub struct Snake<G : Grid + Debug> {
    pub dead : bool,
    pub segments : Vec<SnakeSegment<G>>
}

impl<G : Grid + Debug> Snake<G> {
    pub fn is_head_at(&self, v : &G::Vector) -> bool {
        self.segments.len() > 0 && self.segments[0].position == *v
    }

    pub fn has_segment_at(&self, v : &G::Vector) -> bool {
        self.segments.iter().any(|x| &x.position == v)
    }

    pub fn has_collided_into(&self, other : &Snake<G>) -> bool {
        self.segments.len() > 0 && other.has_segment_at(&self.segments[0].position)
    }

    pub fn step_in_direction(&mut self, dir : <<G as Grid>::Vector as Vector>::Direction) {
        if self.segments.len() == 0 {
            return;
        }
        for i in (1..self.segments.len()).rev() {
            self.segments[i].position = self.segments[i-1].position;
        }
        self.segments[0].position = self.segments[0].position.neighbour(dir);
    }
}

#[cfg(test)]
mod tests {
    use quickcheck::{Arbitrary, Gen, quickcheck};
    use super::*;
    use hexgrid::*;
    use grid::Vector;

    impl Arbitrary for Snake<HexGrid> {
        fn arbitrary<G : Gen>(g : &mut G) -> Snake<HexGrid> {
            let dead = Arbitrary::arbitrary(g);
            let size = {let s = g.size(); g.gen_range(0, s)};
            let head : HexVector = Arbitrary::arbitrary(g);
            let segments = (0..size).scan(head, |state, _| {
                let dir = Arbitrary::arbitrary(g);
                *state = (*state).neighbour(dir);
                Some(SnakeSegment{position : *state})
            }).collect();
            return Snake{dead : dead, segments : segments};
        }

        fn shrink(&self) -> Box<Iterator<Item=Snake<HexGrid>>> {
            let mut shrinks = Vec::new();
            for i in 0..self.segments.len() {
                shrinks.push(Snake{dead : self.dead, segments : self.segments[..i].to_vec()})
            }
            return Box::new(shrinks.into_iter());
        }
    }

    fn snake_is_connected_prop(snake : Snake<HexGrid>) -> bool {
        snake.segments.windows(2).all(|x| x[0].position.distance(&x[1].position) == 1)
    }

    #[test]
    fn snake_is_connected() {
        // this is really to test the Arbitrary instance for Snake
        quickcheck(snake_is_connected_prop as fn(Snake<HexGrid>) -> bool);
    }

    fn step_preserves_connectedness_prop(snake : Snake<HexGrid>, dir : HexDir) -> bool {
        let mut snake = snake.clone();
        snake.step_in_direction(dir);
        return snake_is_connected_prop(snake);
    }

    #[test]
    fn step_preserves_connectedness() {
        quickcheck(step_preserves_connectedness_prop as fn(Snake<HexGrid>, HexDir) -> bool);
    }

    fn head_is_at_head_prop(snake : Snake<HexGrid>) -> bool {
        snake.segments.len() == 0 || snake.is_head_at(&snake.segments[0].position)
    }

    #[test]
    fn head_is_at_head() {
        quickcheck(head_is_at_head_prop as fn(Snake<HexGrid>) -> bool);
    }

    fn only_head_is_at_head_prop(snake : Snake<HexGrid>) -> bool {
        if snake.segments.len() == 0 {
            return true;
        }
        let head_position = snake.segments[0].position;
        for x in snake.segments.clone() {
            if x.position != head_position && snake.is_head_at(&x.position) {
                return false;
            }
        }
        return true;
    }

    #[test]
    fn only_head_is_at_head() {
        quickcheck(only_head_is_at_head_prop as fn(Snake<HexGrid>) -> bool);
    }
}
