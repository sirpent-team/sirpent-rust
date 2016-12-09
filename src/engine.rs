use rand::Rng;
use std::error::Error;
use std::collections::HashMap;

use grid::*;
use state::*;
use snake::*;
use player::*;
use protocol::*;

pub struct Engine<R: Rng> {
    pub rng: Box<R>,
    pub state: State,
    pub new_turn: TurnState,
}

impl<R: Rng> Engine<R> {
    pub fn new(rng: R, grid: Grid) -> Engine<R> {
        let state = State::new(grid);
        Engine {
            rng: Box::new(rng),
            new_turn: state.turn.clone(),
            state: state,
        }
    }

    pub fn add_player(&mut self,
                      player: Player,
                      connection: PlayerConnection)
                      -> Result<PlayerName, ProtocolError> {
        let new_snake = Snake::new(vec![self.state.game.grid.random_cell(&mut *self.rng)]);
        self.state.add_player(player, connection, new_snake)
    }

    pub fn new_game(&mut self) -> HashMap<PlayerName, Result<(), ProtocolError>> {
        self.manage_food();
        self.state.new_game()
    }

    pub fn concluded(&mut self) -> Option<HashMap<PlayerName, (Player, Snake)>> {
        let living_players = self.state.living_players();
        match living_players.len() {
            0 => {
                let ref previous_casualties = self.state.turn.casualties;
                Some(previous_casualties.iter()
                    .map(|(player_name, &(_, ref snake))| {
                        (player_name.clone(),
                         (self.state.game.players[player_name].clone(), snake.clone()))
                    })
                    .collect())
            }
            1 => Some(living_players),
            _ => None,
        }
    }

    pub fn turn(&mut self) -> TurnState {
        let moves = self.state.request_moves();

        self.new_turn = self.state.turn.clone();
        // N.B. does not free memory.
        self.new_turn.eaten.clear();
        self.new_turn.directions.clear();
        self.new_turn.casualties.clear();

        // Apply movement and remove snakes that did not move.
        self.snake_movement(moves);
        self.remove_snakes();

        // Grow snakes whose heads collided with a food.
        self.snake_eating();
        self.manage_food();

        // Detect collisions with snakes and remove colliding snakes.
        self.snake_collisions();
        self.remove_snakes();

        // Detect snakes outside grid and remove them.
        // @TODO: I think it is sound to move this to being straight after applying movement,
        // so long as snakes are not removed before collision detection.
        self.snake_grid_bounds();
        self.remove_snakes();

        self.new_turn.turn_number += 1;

        self.state.turn = self.new_turn.clone();
        self.new_turn.clone()
    }

    fn snake_movement(&mut self, moves: HashMap<PlayerName, Move>) {
        // Apply movement and remove snakes that did not move.
        // Snake plans are Result<Direction, MoveError>. MoveError = String.
        // So we can specify an underlying error rather than just omitting any move.
        // Then below if no snake plan is set, we use a default error message.
        // While intricate this very neatly leads to CauseOfDeath.

        for (player_name, snake) in self.new_turn.snakes.iter_mut() {
            // Retrieve snake plan if one exists.
            let ref move_ = moves[player_name];

            // Move if a direction provided else use MoveError for CauseOfDeath.
            match *move_ {
                Ok(direction) => {
                    snake.step_in_direction(direction);
                    self.new_turn.directions.insert(player_name.clone(), direction);
                }
                Err(ref move_error) => {
                    let cause_of_death =
                        CauseOfDeath::NoMoveMade((*move_error).description().to_string());
                    self.new_turn
                        .casualties
                        .insert(player_name.clone(), (cause_of_death, snake.clone()));
                }
            }
        }
    }

    fn snake_eating(&mut self) {
        for (player_name, snake) in self.new_turn.snakes.iter_mut() {
            if self.new_turn.food.contains(&snake.segments[0]) {
                // Remove this food only after the full loop, such that N snakes colliding on top of a
                // food all grow. They immediately die but this way collision with growth of both snakes
                // is possible.
                snake.grow();
                self.new_turn.eaten.insert(player_name.clone(), snake.segments[0]);
            }
        }
    }

    fn snake_collisions(&mut self) {
        for (player_name, snake) in self.new_turn.snakes.iter() {
            for (coll_player_name, coll_snake) in self.new_turn.snakes.iter() {
                if snake != coll_snake && snake.has_collided_into(coll_snake) {
                    self.new_turn
                        .casualties
                        .insert(player_name.clone(),
                                (CauseOfDeath::CollidedWithSnake(coll_player_name.clone()),
                                 snake.clone()));
                    break;
                }
            }
        }
    }

    fn snake_grid_bounds(&mut self) {
        for (player_name, snake) in self.new_turn.snakes.iter() {
            for &segment in snake.segments.iter() {
                if !self.state.game.grid.is_within_bounds(segment) {
                    self.new_turn.casualties.insert(player_name.clone(),
                                                    (CauseOfDeath::CollidedWithBounds(segment),
                                                     snake.clone()));
                }
            }
        }
    }

    fn remove_snakes(&mut self) {
        // N.B. At one point we .drain()ed the dead_snakes Set. This was removed so it
        // can be used to track which players were killed.
        for (player_name, _) in self.new_turn.casualties.iter() {
            // Kill snake if not already killed, and drop food at non-head segments within the grid.
            // @TODO: This code is much cleaner than the last draft but still lots goes on here.
            if let Some(dead_snake) = self.new_turn.snakes.remove(player_name) {
                // Get segments[1..] safely. Directly slicing panics if the Vec had <2 elements.
                if let Some((_, headless_segments)) = dead_snake.segments.split_first() {
                    // Only retain segments if within grid.
                    // @TODO: Move this to food management?
                    let corpse_food: Vec<&Vector> = headless_segments.iter()
                        .filter(|&s| self.state.game.grid.is_within_bounds(*s))
                        .collect();
                    self.new_turn.food.extend(corpse_food);
                }
            }
        }
    }

    fn manage_food(&mut self) {
        for (_, food) in self.new_turn.eaten.iter() {
            self.new_turn.food.remove(&food);
        }

        if self.new_turn.food.len() < 1 {
            let new_food = self.state.game.grid.random_cell(&mut *self.rng);
            self.new_turn.food.insert(new_food);
        }
    }
}
