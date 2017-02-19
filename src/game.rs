use rand::Rng;
use uuid::Uuid;
use std::collections::{HashSet, HashMap};

use grids::*;
use snake::*;
use net::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameState {
    pub uuid: Uuid,
    pub grid: Grid,
    pub players: HashSet<String>,
}

impl GameState {
    pub fn new(grid: Grid) -> GameState {
        GameState {
            uuid: Uuid::new_v4(),
            grid: grid,
            players: HashSet::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurnState {
    pub turn_number: usize,
    pub food: HashSet<Vector>,
    pub eaten: HashMap<String, Vector>,
    pub snakes: HashMap<String, Snake>,
    pub directions: HashMap<String, Direction>,
    pub casualties: HashMap<String, CauseOfDeath>,
}

impl TurnState {
    pub fn new() -> TurnState {
        TurnState {
            turn_number: 0,
            food: HashSet::new(),
            eaten: HashMap::new(),
            snakes: HashMap::new(),
            directions: HashMap::new(),
            casualties: HashMap::new(),
        }
    }
}

pub struct Game<R: Rng> {
    pub rng: Box<R>,
    pub game_state: GameState,
    pub turn_state: TurnState,
}

impl<R: Rng> Game<R> {
    pub fn new(rng: R, grid: Grid) -> Self {
        let mut game = Game {
            rng: Box::new(rng),
            game_state: GameState::new(grid),
            turn_state: TurnState::new(),
        };

        // @TODO: Alter API to avoid this juggling.
        let mut turn_state = TurnState::new();
        game.manage_food(&mut turn_state);
        game.turn_state = turn_state;

        return game;
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
        let head = self.game_state.grid.random_cell(&mut *self.rng);
        let snake = Snake::new(vec![head]);
        self.turn_state.snakes.insert(final_name.clone(), snake);

        return final_name;
    }

    pub fn concluded(&self) -> bool {
        let number_of_living_snakes = self.turn_state.snakes.len();
        match number_of_living_snakes {
            0 => true,
            _ => false,
        }
    }

    pub fn advance_turn(&mut self, moves: HashMap<String, ProtocolResult<Direction>>) -> TurnState {
        let mut next_turn: TurnState = self.turn_state.clone();

        // N.B. does not free memory.
        next_turn.eaten.clear();
        next_turn.directions.clear();
        next_turn.casualties.clear();

        // Apply movement and remove snakes that did not move.
        self.snake_movement(&mut next_turn, moves);
        self.remove_snakes(&mut next_turn);

        // Grow snakes whose heads collided with a food.
        self.snake_eating(&mut next_turn);
        self.manage_food(&mut next_turn);

        // Detect collisions with snakes and remove colliding snakes.
        self.snake_collisions(&mut next_turn);
        self.remove_snakes(&mut next_turn);

        // Detect snakes outside grid and remove them.
        // @TODO: I think it is sound to move this to being straight after applying movement,
        // so long as snakes are not removed before collision detection.
        self.snake_grid_bounds(&mut next_turn);
        self.remove_snakes(&mut next_turn);

        next_turn.turn_number += 1;

        self.turn_state = next_turn.clone();
        return next_turn;
    }

    fn snake_movement(&mut self,
                      next_turn: &mut TurnState,
                      mut moves: HashMap<String, ProtocolResult<Direction>>) {
        // Apply movement and remove snakes that did not move.
        // Snake plans are Result<Direction, MoveError>. MoveError = String.
        // So we can specify an underlying error rather than just omitting any move.
        // Then below if no snake plan is set, we use a default error message.
        // While intricate this very neatly leads to CauseOfDeath.

        for (name, snake) in next_turn.snakes.iter_mut() {
            match moves.remove(name) {
                Some(Ok(direction)) => {
                    snake.step_in_direction(direction);
                    next_turn.directions.insert(name.clone(), direction);
                }
                Some(Err(e)) => {
                    let cause_of_death = CauseOfDeath::from(e);
                    next_turn.casualties.insert(name.clone(), cause_of_death);
                }
                None => {
                    let cause_of_death = CauseOfDeath::NoMoveMade("".to_string());
                    next_turn.casualties.insert(name.clone(), cause_of_death);
                }
            }
        }
    }

    fn snake_eating(&mut self, next_turn: &mut TurnState) {
        for (name, snake) in next_turn.snakes.iter_mut() {
            if next_turn.food.contains(&snake.segments[0]) {
                // Remove this food only after the full loop, such that N snakes colliding on top of a
                // food all grow. They immediately die but this way collision with growth of both snakes
                // is possible.
                snake.grow();
                next_turn.eaten.insert(name.clone(), snake.segments[0]);
            }
        }
    }

    fn snake_collisions(&mut self, next_turn: &mut TurnState) {
        for (name, snake) in next_turn.snakes.iter() {
            for (coll_player_name, coll_snake) in next_turn.snakes.iter() {
                if snake != coll_snake && snake.has_collided_into(coll_snake) {
                    next_turn.casualties
                        .insert(name.clone(),
                                CauseOfDeath::CollidedWithSnake(coll_player_name.clone()));
                    break;
                }
            }
        }
    }

    fn snake_grid_bounds(&mut self, next_turn: &mut TurnState) {
        for (name, snake) in next_turn.snakes.iter() {
            for &segment in snake.segments.iter() {
                if !self.game_state.grid.is_within_bounds(segment) {
                    next_turn.casualties
                        .insert(name.clone(), CauseOfDeath::CollidedWithBounds(segment));
                }
            }
        }
    }

    fn remove_snakes(&mut self, next_turn: &mut TurnState) {
        // N.B. At one point we .drain()ed the dead_snakes Set. This was removed so it
        // can be used to track which players were killed.
        for (name, _) in next_turn.casualties.iter() {
            // Kill snake if not already killed, and drop food at non-head segments within the grid.
            // @TODO: This code is much cleaner than the last draft but still lots goes on here.
            if let Some(dead_snake) = next_turn.snakes.remove(name) {
                // Get segments[1..] safely. Directly slicing panics if the Vec had <2 elements.
                if let Some((_, headless_segments)) = dead_snake.segments.split_first() {
                    // Only retain segments if within grid.
                    // @TODO: Move this to food management?
                    let corpse_food: Vec<&Vector> = headless_segments.iter()
                        .filter(|&s| self.game_state.grid.is_within_bounds(*s))
                        .collect();
                    next_turn.food.extend(corpse_food);
                }
            }
        }
    }

    fn manage_food(&mut self, next_turn: &mut TurnState) {
        for (_, food) in next_turn.eaten.iter() {
            next_turn.food.remove(&food);
        }

        if next_turn.food.len() < 1 {
            let new_food = self.game_state.grid.random_cell(&mut *self.rng);
            next_turn.food.insert(new_food);
        }
    }
}
