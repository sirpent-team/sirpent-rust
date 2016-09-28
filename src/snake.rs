use uuid::Uuid;

use grid::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snake<V: Vector> {
    pub alive: bool,
    pub uuid: Uuid,
    pub segments: Vec<V>,
}

impl<V: Vector> Snake<V> {
    pub fn new(segments: Vec<V>) -> Snake<V> {
        Snake::<V> {
            alive: true,
            uuid: Uuid::new_v4(),
            segments: segments,
        }
    }

    pub fn is_head_at(&self, v: &V) -> bool {
        self.segments.len() > 0 && self.segments[0] == *v
    }

    pub fn has_segment_at(&self, v: &V) -> bool {
        self.segments.iter().any(|x| x == v)
    }

    pub fn has_collided_into(&self, other: &Snake<V>) -> bool {
        let my_head = self.segments[0];
        let mut next_candidate = my_head.distance(&other.segments[0]);
        while let Some(here) = other.segments.get(next_candidate) {
            if my_head == *here {
                return true;
            }
            next_candidate += my_head.distance(&here);
        }
        return false;
    }

    pub fn step_in_direction(&mut self, dir: V::Direction) {
        if self.segments.len() == 0 {
            return;
        }
        for i in (1..self.segments.len()).rev() {
            self.segments[i] = self.segments[i - 1];
        }
        self.segments[0] = self.segments[0].neighbour(&dir);
    }
}

#[cfg(test)]
mod tests {
    use quickcheck::{Arbitrary, Gen, quickcheck};
    use super::*;
    use hexagon_grid::*;
    use grid::Vector;
    use uuid::Uuid;

    impl Arbitrary for Snake<HexagonVector> {
        fn arbitrary<G: Gen>(g: &mut G) -> Snake<HexagonVector> {
            let alive = Arbitrary::arbitrary(g);
            let size = {
                let s = g.size();
                g.gen_range(0, s)
            };
            let head: HexagonVector = Arbitrary::arbitrary(g);
            let segments = (0..size)
                .scan(head, |state, _| {
                    let dir = Arbitrary::arbitrary(g);
                    *state = (*state).neighbour(&dir);
                    Some(*state)
                })
                .collect();
            return Snake {
                alive: alive,
                segments: segments,
                uuid: Uuid::nil(),
            };
        }

        fn shrink(&self) -> Box<Iterator<Item = Snake<HexagonVector>>> {
            let mut shrinks = Vec::new();
            for i in 0..self.segments.len() {
                shrinks.push(Snake {
                    alive: self.alive,
                    segments: self.segments[..i].to_vec(),
                    uuid: Uuid::nil(),
                })
            }
            return Box::new(shrinks.into_iter());
        }
    }

    fn snake_is_connected_prop(snake: Snake<HexagonVector>) -> bool {
        snake.segments.windows(2).all(|x| x[0].distance(&x[1]) == 1)
    }

    #[test]
    fn snake_is_connected() {
        // this is really to test the Arbitrary instance for Snake
        quickcheck(snake_is_connected_prop as fn(Snake<HexagonVector>) -> bool);
    }

    fn step_preserves_connectedness_prop(snake: Snake<HexagonVector>, dir: HexagonDir) -> bool {
        let mut snake = snake.clone();
        snake.step_in_direction(dir);
        return snake_is_connected_prop(snake);
    }

    #[test]
    fn step_preserves_connectedness() {
        quickcheck(step_preserves_connectedness_prop as fn(Snake<HexagonVector>, HexagonDir) -> bool);
    }

    fn head_is_at_head_prop(snake: Snake<HexagonVector>) -> bool {
        snake.segments.len() == 0 || snake.is_head_at(&snake.segments[0])
    }

    #[test]
    fn head_is_at_head() {
        quickcheck(head_is_at_head_prop as fn(Snake<HexagonVector>) -> bool);
    }

    fn only_head_is_at_head_prop(snake: Snake<HexagonVector>) -> bool {
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
        quickcheck(only_head_is_at_head_prop as fn(Snake<HexagonVector>) -> bool);
    }
}
