use rand::Rng;
use std::collections::HashMap;
use std::fmt;

use state::*;
use state::grids::*;

mod spectators;

pub use self::spectators::*;

#[derive(Debug, PartialEq, Clone)]
pub enum State {
    Start,
    Round,
    End,
    InvalidTransition(Box<State>, Event),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Event {
    Turn(HashMap<String, Direction>),
}

pub struct Game {
    state: State,
    rng: Box<Rng>,
    grid: Grid,
    game_state: GameState,
    round_state: RoundState,
}

impl Game {
    pub fn new(rng: Box<Rng>, grid: Grid) -> Self {
        let mut game = Game {
            state: State::Start,
            rng: rng,
            grid: grid,
            game_state: GameState::new(grid),
            round_state: RoundState::default(),
        };

        // @TODO: Alter API to avoid this juggling.
        let mut round_state = RoundState::default();
        game.manage_food(&mut round_state);
        game.round_state = round_state;

        game
    }

    pub fn add_player(&mut self, desired_name: String) -> String {
        // Find an unused name based upon the desired_name.
        let mut final_name = desired_name;
        while self.game_state.players.contains(&final_name) {
            final_name += "_";
        }
        // Reserve the new name.
        self.game_state.players.insert(final_name.clone());
        // Generate and insert a snake.
        let head = self.grid.random_cell(&mut self.rng);
        let snake = Snake::new(vec![head]);
        self.round_state
            .snakes
            .insert(final_name.clone(), snake);

        final_name
    }

    pub fn next(&mut self, event: Event) {
        self.state = match (self.state.clone(), event) {
            (State::Start, Event::Turn(directions)) |
            (State::Round, Event::Turn(directions)) => {
                self.advance_round(directions);
                if self.concluded() {
                    State::End
                } else {
                    State::Round
                }
            }
            (s, e) => State::InvalidTransition(Box::new(s), e),
        };
    }

    pub fn concluded(&self) -> bool {
        let number_of_living_snakes = self.round_state.snakes.len();
        match number_of_living_snakes {
            0 | 1 => true,
            _ => false,
        }
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn game_state(&self) -> &GameState {
        &self.game_state
    }

    pub fn round_state(&self) -> &RoundState {
        &self.round_state
    }

    fn advance_round(&mut self, moves: HashMap<String, Direction>) -> RoundState {
        let mut next_round: RoundState = self.round_state.clone();

        // N.B. does not free memory.
        next_round.eaten.clear();
        next_round.directions.clear();
        next_round.casualties.clear();

        // Apply movement and remove snakes that did not move.
        self.snake_movement(&mut next_round, moves);
        self.remove_snakes(&mut next_round);

        // Grow snakes whose heads collided with a food.
        self.snake_eating(&mut next_round);
        self.manage_food(&mut next_round);

        // Detect collisions with snakes and remove colliding snakes.
        self.snake_collisions(&mut next_round);
        self.remove_snakes(&mut next_round);

        // Detect snakes outside grid and remove them.
        // @TODO: I think it is sound to move this to being straight after applying movement,
        // so long as snakes are not removed before collision detection.
        self.snake_grid_bounds(&mut next_round);
        self.remove_snakes(&mut next_round);

        next_round.round_number += 1;

        self.round_state = next_round.clone();
        next_round
    }

    fn snake_movement(&mut self,
                      next_round: &mut RoundState,
                      mut moves: HashMap<String, Direction>) {
        // Apply movement and remove snakes that did not move.
        // Snake plans are Result<Direction, MoveError>. MoveError = String.
        // So we can specify an underlying error rather than just omitting any move.
        // Then below if no snake plan is set, we use a default error message.
        // While intricate this very neatly leads to CauseOfDeath.

        for (name, snake) in &mut next_round.snakes {
            match moves.remove(name) {
                Some(direction) => {
                    snake.step_in_direction(direction);
                    next_round.directions.insert(name.clone(), direction);
                }
                _ => {
                    let cause_of_death = CauseOfDeath::NoMoveMade;
                    next_round
                        .casualties
                        .insert(name.clone(), cause_of_death);
                }
            }
        }
    }

    fn snake_eating(&mut self, next_round: &mut RoundState) {
        for (name, snake) in &mut next_round.snakes {
            if next_round.food.contains(&snake.segments[0]) {
                // Remove this food only after the full loop, such that N snakes colliding on top of a
                // food all grow. They immediately die but this way collision with growth of both snakes
                // is possible.
                snake.grow();
                next_round.eaten.insert(name.clone(), snake.segments[0]);
            }
        }
    }

    fn snake_collisions(&mut self, next_round: &mut RoundState) {
        for (name, snake) in &next_round.snakes {
            for coll_snake in next_round.snakes.values() {
                if snake != coll_snake && snake.has_collided_into(coll_snake) {
                    next_round
                        .casualties
                        .insert(name.clone(), CauseOfDeath::CollidedWithSnake);
                    break;
                }
            }
        }
    }

    fn snake_grid_bounds(&mut self, next_round: &mut RoundState) {
        for (name, snake) in &next_round.snakes {
            for &segment in &snake.segments {
                if !self.grid.is_within_bounds(segment) {
                    next_round
                        .casualties
                        .insert(name.clone(), CauseOfDeath::CollidedWithBounds);
                }
            }
        }
    }

    fn remove_snakes(&mut self, next_round: &mut RoundState) {
        // N.B. At one point we .drain()ed the dead_snakes Set. This was removed so it
        // can be used to track which players were killed.
        for name in next_round.casualties.keys() {
            // Kill snake if not already killed, and drop food at non-head segments within the grid.
            // @TODO: This code is much cleaner than the last draft but still lots goes on here.
            if let Some(dead_snake) = next_round.snakes.remove(name) {
                // Get segments[1..] safely. Directly slicing panics if the Vec had <2 elements.
                if let Some((_, headless_segments)) = dead_snake.segments.split_first() {
                    // Only retain segments if within grid.
                    // @TODO: Move this to food management?
                    let corpse_food: Vec<&Vector> = headless_segments
                        .iter()
                        .filter(|&s| self.grid.is_within_bounds(*s))
                        .collect();
                    next_round.food.extend(corpse_food);
                }
            }
        }
    }

    fn manage_food(&mut self, next_round: &mut RoundState) {
        for food in next_round.eaten.values() {
            next_round.food.remove(food);
        }

        if next_round.food.len() < 1 {
            let new_food = self.grid.random_cell(&mut self.rng);
            next_round.food.insert(new_food);
        }
    }
}

impl fmt::Debug for Game {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Game")
            .field("state", &self.state)
            .field("grid", &self.grid)
            .field("game_state", &self.game_state)
            .field("round_state", &self.round_state)
            .finish()
    }
}
