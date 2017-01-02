use grid::*;
use protocol::*;

#[derive(PartialEq, Eq, Clone, Hash, Debug, Serialize, Deserialize)]
pub struct Snake {
    pub alive: bool,
    pub segments: Vec<Vector>,
    #[serde(skip_serializing, skip_deserializing)]
    previous_tail: Option<Vector>,
}

impl Snake {
    pub fn new(segments: Vec<Vector>) -> Snake {
        Snake {
            alive: true,
            segments: segments,
            previous_tail: None,
        }
    }

    pub fn is_head_at(&self, v: &Vector) -> bool {
        self.segments.len() > 0 && self.segments[0] == *v
    }

    pub fn has_segment_at(&self, v: &Vector) -> bool {
        self.segments.iter().any(|x| x == v)
    }

    pub fn has_collided_into(&self, other: &Snake) -> bool {
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

    pub fn step_in_direction(&mut self, dir: Direction) {
        self.previous_tail = self.segments.last().cloned();
        if !self.segments.is_empty() {
            for i in (1..self.segments.len()).rev() {
                self.segments[i] = self.segments[i - 1];
            }
            self.segments[0] = self.segments[0].neighbour(&dir);
        }
    }

    pub fn grow(&mut self) {
        if let Some(previous_tail) = self.previous_tail {
            self.segments.push(previous_tail);
            self.previous_tail = None;
        }
    }
}

// Useful for debugging and statistics.
// CauseOfDeath converts MoveError to a String in order to be serialisable/deserialisable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CauseOfDeath {
    NoMoveMade(String),
    CollidedWithSnake(String),
    CollidedWithBounds(Vector),
}

impl From<ProtocolError> for CauseOfDeath {
    fn from(err: ProtocolError) -> CauseOfDeath {
        CauseOfDeath::NoMoveMade(format!("{}", err))
    }
}

#[cfg(test)]
mod tests {
    use quickcheck::{Arbitrary, Gen, quickcheck};
    use super::*;

    impl Arbitrary for Snake {
        fn arbitrary<G: Gen>(g: &mut G) -> Snake {
            let alive = Arbitrary::arbitrary(g);
            let size = {
                let s = g.size();
                g.gen_range(0, s)
            };
            let head: Vector = Arbitrary::arbitrary(g);
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
                previous_tail: None,
            };
        }

        fn shrink(&self) -> Box<Iterator<Item = Snake>> {
            let mut shrinks = Vec::new();
            for i in 0..self.segments.len() {
                shrinks.push(Snake {
                    alive: self.alive,
                    segments: self.segments[..i].to_vec(),
                    previous_tail: None,
                })
            }
            return Box::new(shrinks.into_iter());
        }
    }

    fn snake_is_connected_prop(snake: Snake) -> bool {
        snake.segments.windows(2).all(|x| x[0].distance(&x[1]) == 1)
    }

    #[test]
    fn snake_is_connected() {
        // this is really to test the Arbitrary instance for Snake
        quickcheck(snake_is_connected_prop as fn(Snake) -> bool);
    }

    fn step_preserves_connectedness_prop(snake: Snake, dir: Direction) -> bool {
        let mut snake = snake.clone();
        snake.step_in_direction(dir);
        return snake_is_connected_prop(snake);
    }

    #[test]
    fn step_preserves_connectedness() {
        quickcheck(step_preserves_connectedness_prop as fn(Snake, Direction) -> bool);
    }

    fn head_is_at_head_prop(snake: Snake) -> bool {
        snake.segments.len() == 0 || snake.is_head_at(&snake.segments[0])
    }

    #[test]
    fn head_is_at_head() {
        quickcheck(head_is_at_head_prop as fn(Snake) -> bool);
    }

    fn only_head_is_at_head_prop(snake: Snake) -> bool {
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
        quickcheck(only_head_is_at_head_prop as fn(Snake) -> bool);
    }
}
