use rand::Rng;
use std::collections::HashMap;

use grid::*;
use snake::*;
use state::*;

pub struct Engine<R: Rng> {
    pub rng: Box<R>,
    pub game: State,
}

impl<R: Rng> Engine<R> {
    pub fn new(rng: R, grid: Grid) -> Engine<R> {
        let mut engine = Engine {
            rng: Box::new(rng),
            game: State::new(grid),
        };
        let mut replacement_turn = engine.game.turn.clone();
        engine.manage_food(&mut replacement_turn);
        engine.game.turn = replacement_turn;
        return engine;
    }

    pub fn add_player(&mut self, desired_name: String) -> String {
        let head = self.game.game.grid.random_cell(&mut *self.rng);
        let snake = Snake::new(vec![head]);
        self.game.add_player(desired_name, snake)
    }

    // pub fn concluded(&mut self) -> Option<HashMap<String, (Player, Snake)>> {
    // let living_players = self.state.living_players();
    // match living_players.len() {
    // 0 => {
    // let ref previous_casualties = self.state.turn.casualties;
    // Some(previous_casualties.iter()
    // .map(|(player_name, &(_, ref snake))| {
    // (player_name.clone(),
    // (self.state.game.players[player_name].clone(), snake.clone()))
    // })
    // .collect())
    // }
    // 1 => Some(living_players),
    // _ => None,
    // }
    // }
    //

    pub fn turn(&mut self, moves: HashMap<String, Direction>) -> TurnState {
        let mut next_turn: TurnState = self.game.turn.clone();

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

        return next_turn;
    }

    fn snake_movement(&mut self, next_turn: &mut TurnState, moves: HashMap<String, Direction>) {
        // Apply movement and remove snakes that did not move.
        // Snake plans are Result<Direction, MoveError>. MoveError = String.
        // So we can specify an underlying error rather than just omitting any move.
        // Then below if no snake plan is set, we use a default error message.
        // While intricate this very neatly leads to CauseOfDeath.

        for (name, snake) in next_turn.snakes.iter_mut() {
            // Move if a direction provided else kill the snake.
            if moves.contains_key(name) {
                let move_ = moves[name];
                snake.step_in_direction(move_);
                next_turn.directions.insert(name.clone(), move_);
            } else {
                let cause_of_death = CauseOfDeath::NoMoveMade("".to_string());
                next_turn.casualties
                    .insert(name.clone(), (cause_of_death, snake.clone()));
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
                                (CauseOfDeath::CollidedWithSnake(coll_player_name.clone()),
                                 snake.clone()));
                    break;
                }
            }
        }
    }

    fn snake_grid_bounds(&mut self, next_turn: &mut TurnState) {
        for (name, snake) in next_turn.snakes.iter() {
            for &segment in snake.segments.iter() {
                if !self.game.game.grid.is_within_bounds(segment) {
                    next_turn.casualties.insert(name.clone(),
                                                (CauseOfDeath::CollidedWithBounds(segment),
                                                 snake.clone()));
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
                        .filter(|&s| self.game.game.grid.is_within_bounds(*s))
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
            let new_food = self.game.game.grid.random_cell(&mut *self.rng);
            next_turn.food.insert(new_food);
        }
    }
}
