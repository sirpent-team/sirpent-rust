use rand::Rng;
use std::sync::{Arc, RwLock, RwLockWriteGuard};
use std::collections::{HashSet, HashMap};

use grid::*;
use snake::*;
use player::*;
use game_state::*;

pub struct GameEngine<R: Rng> {
    pub rng: Box<R>,
    pub state: Arc<RwLock<GameState>>,
    pub players: HashMap<PlayerName, PlayerBox>,
    pub snake_plans: HashMap<PlayerName, Result<Direction, MoveError>>,
    pub dead_snakes: HashMap<PlayerName, CauseOfDeath>,
    pub eaten_food: HashSet<Vector>,
}

impl<R: Rng> GameEngine<R> {
    pub fn new(rng: R, game_state: GameState) -> GameEngine<R> {
        GameEngine {
            rng: Box::new(rng),
            state: Arc::new(RwLock::new(game_state)),
            players: HashMap::new(),
            snake_plans: HashMap::new(),
            dead_snakes: HashMap::new(),
            eaten_food: HashSet::new()
        }
    }

    pub fn add_player(&mut self, mut player: Player) -> PlayerName {
        let mut state = self.state.write().unwrap();

        // Find an unused name.
        let mut player_name = player.clone().name;
        while self.players.contains_key(&player_name) {
            player_name.push('_');
        }
        player.name = player_name.clone();

        let player_box = Box::new(player);
        self.players.insert(player_name.clone(), player_box);

        let snake = Snake::new(vec![state.grid.random_cell(&mut *self.rng)]);
        state.snakes.insert(player_name.clone(), snake);

        player_name
    }

    pub fn add_snake_plan(&mut self, player_name: PlayerName, snake_plan: Result<Direction, MoveError>) {
        self.snake_plans.insert(player_name.clone(), snake_plan);
    }

    pub fn simulate_next_turn(&mut self) {
        // N.B. does not free memory.
        self.dead_snakes.clear();
        self.eaten_food.clear();

        // Apply movement and remove snakes that did not move.
        self.simulate_snake_movement();
        self.remove_snakes();

        // Grow snakes whose heads collided with a food.
        self.simulate_snake_eating();
        self.manage_food();

        // Detect collisions with snakes and remove colliding snakes.
        self.simulate_snake_collisions();
        self.remove_snakes();

        // Detect snakes outside grid and remove them.
        // @TODO: I think it is sound to move this to being straight after applying movement.
        self.simulate_grid_bounds();
        self.remove_snakes();

        let mut state = self.state.write().unwrap();
        state.turn_number += 1;
    }

    fn simulate_snake_movement(&mut self) {
        let mut state = self.state.write().unwrap();

        // Apply movement and remove snakes that did not move.
        // Snake plans are Result<Direction, MoveError>. MoveError = String.
        // So we can specify an underlying error rather than just omitting any move.
        // Then below if no snake plan is set, we use a default error message.
        // While intricate this very neatly leads to CauseOfDeath.

        let default_planless_error = Err("No move information.".to_string());

        for (player_name, snake) in state.snakes.iter_mut() {
            // Retrieve snake plan if one exists.
            let snake_plan: &Result<Direction, MoveError> = self.snake_plans
                .get(player_name)
                .unwrap_or(&default_planless_error);

            // Move if a direction provided else use MoveError for CauseOfDeath.
            match *snake_plan {
                Ok(direction) => snake.step_in_direction(direction),
                Err(ref move_error) => {
                    let cause_of_death = CauseOfDeath::NoMoveMade(move_error.clone());
                    self.dead_snakes.insert(player_name.clone(), cause_of_death);
                }
            }
        }
    }

    fn simulate_snake_eating(&mut self) {
        let mut state = self.state.write().unwrap();

        let mut snakes_to_grow = HashSet::new();
        for (player_name, snake) in state.snakes.iter() {
            if state.food.contains(&snake.segments[0]) {
                // Remove this food only after the full loop, such that N snakes colliding on top of a
                // food all grow. They immediately die but this way collision with growth of both snakes
                // is possible.
                self.eaten_food.insert(snake.segments[0]);
                snakes_to_grow.insert(player_name.clone());
            }
        }
        for player_name in snakes_to_grow.iter() {
            state.snakes.get_mut(player_name).unwrap().grow();
        }
    }

    fn simulate_snake_collisions(&mut self) {
        let mut state = self.state.write().unwrap();

        for (player_name, snake) in state.snakes.iter() {
            for (coll_player_name, coll_snake) in state.snakes.iter() {
                if snake != coll_snake && snake.has_collided_into(coll_snake) {
                    self.dead_snakes
                        .insert(player_name.clone(),
                                CauseOfDeath::CollidedWithSnake(coll_player_name.clone()));
                    break;
                }
            }
        }
    }

    fn simulate_grid_bounds(&mut self) {
        let mut state = self.state.write().unwrap();

        for (player_name, snake) in state.snakes.iter() {
            for &segment in snake.segments.iter() {
                if !state.grid.is_within_bounds(segment) {
                    self.dead_snakes.insert(player_name.clone(),
                                            CauseOfDeath::CollidedWithBounds(segment));
                }
            }
        }
    }

    fn remove_snakes(&mut self) {
        let mut state = self.state.write().unwrap();

        // N.B. At one point we .drain()ed the dead_snakes Set. This was removed so it
        // can be used to track which players were killed.
        for (player_name, _) in self.dead_snakes.iter() {
            // Kill snake if not already killed, and drop food at non-head segments within the grid.
            // @TODO: This code is much cleaner than the last draft but still lots goes on here.
            if let Some(dead_snake) = state.snakes.remove(player_name) {
                // Get segments[1..] safely. Directly slicing panics if the Vec had <2 elements.
                if let Some((_, headless_segments)) = dead_snake.segments.split_first() {
                    // Only retain segments if within grid.
                    let corpse_food: Vec<&Vector> = headless_segments.iter()
                        .filter(|&s| state.grid.is_within_bounds(*s))
                        .collect();
                    state.food.extend(corpse_food);
                }
            }
        }
    }

    fn manage_food(&mut self) {
        let mut state = self.state.write().unwrap();

        for food in self.eaten_food.iter() {
            state.food.remove(&food);
        }

        if state.food.len() < 1 {
            let new_food = state.grid.random_cell(&mut *self.rng);
            state.food.insert(new_food);
        }
    }
}
