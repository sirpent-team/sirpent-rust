use uuid::Uuid;

use grid::*;

#[derive(Clone, Debug)]
pub struct Snake<V : Vector> {
    pub growing : bool,
    pub uuid : Uuid,
    pub segments : Vec<V>
}

impl<V : Vector> Snake<V> {
    pub fn is_head_at(&self, v : &V) -> bool {
        self.segments.len() > 0 && self.segments[0] == *v
    }

    pub fn has_segment_at(&self, v : &V) -> bool {
        self.segments.iter().any(|x| x == v)
    }

    pub fn has_collided_into(&self, other : &Snake<V>) -> bool {
        self.segments.len() > 0 && other.has_segment_at(&self.segments[0])
    }

    pub fn step_in_direction(&mut self, dir : V::Direction) {
        if self.segments.len() == 0 {
            return;
        }
        for i in (1..self.segments.len()).rev() {
            self.segments[i] = self.segments[i-1];
        }
        self.segments[0] = self.segments[0].neighbour(&dir);
    }
}

#[cfg(test)]
mod tests {
    use quickcheck::{Arbitrary, Gen, quickcheck};
    use super::*;
    use hexgrid::*;
    use grid::Vector;

    impl Arbitrary for Snake<HexVector> {
        fn arbitrary<G : Gen>(g : &mut G) -> Snake<HexVector> {
            let dead = Arbitrary::arbitrary(g);
            let size = {let s = g.size(); g.gen_range(0, s)};
            let head : HexVector = Arbitrary::arbitrary(g);
            let segments = (0..size).scan(head, |state, _| {
                let dir = Arbitrary::arbitrary(g);
                *state = (*state).neighbour(&dir);
                Some(*state)
            }).collect();
            return Snake{dead : dead, segments : segments};
        }

        fn shrink(&self) -> Box<Iterator<Item=Snake<HexVector>>> {
            let mut shrinks = Vec::new();
            for i in 0..self.segments.len() {
                shrinks.push(Snake{dead : self.dead, segments : self.segments[..i].to_vec()})
            }
            return Box::new(shrinks.into_iter());
        }
    }

    fn snake_is_connected_prop(snake : Snake<HexVector>) -> bool {
        snake.segments.windows(2).all(|x| x[0].distance(&x[1]) == 1)
    }

    #[test]
    fn snake_is_connected() {
        // this is really to test the Arbitrary instance for Snake
        quickcheck(snake_is_connected_prop as fn(Snake<HexVector>) -> bool);
    }

    fn step_preserves_connectedness_prop(snake : Snake<HexVector>, dir : HexDir) -> bool {
        let mut snake = snake.clone();
        snake.step_in_direction(dir);
        return snake_is_connected_prop(snake);
    }

    #[test]
    fn step_preserves_connectedness() {
        quickcheck(step_preserves_connectedness_prop as fn(Snake<HexVector>, HexDir) -> bool);
    }

    fn head_is_at_head_prop(snake : Snake<HexVector>) -> bool {
        snake.segments.len() == 0 || snake.is_head_at(&snake.segments[0])
    }

    #[test]
    fn head_is_at_head() {
        quickcheck(head_is_at_head_prop as fn(Snake<HexVector>) -> bool);
    }

    fn only_head_is_at_head_prop(snake : Snake<HexVector>) -> bool {
        if snake.segments.len() == 0 {
            return true;
        }
        let head_position = snake.segments[0];
        for x in snake.segments.clone() {
            if x != head_position && snake.is_head_at(&x) {
                return false;
            }
        }
        return true;
    }

    #[test]
    fn only_head_is_at_head() {
        quickcheck(only_head_is_at_head_prop as fn(Snake<HexVector>) -> bool);
    }
}
