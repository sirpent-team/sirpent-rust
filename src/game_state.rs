use uuid::Uuid;
use rand::OsRng;
use std::collections::{HashSet, HashMap};

use grid::*;
use snake::*;
use player::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameContext {
    pub food: HashSet<Vector>,
    pub snakes: HashMap<PlayerName, Snake>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub uuid: Uuid,
    pub grid: Grid,
    pub players: HashMap<PlayerName, PlayerBox>,

    pub context: GameContext,

    pub snake_plans: HashMap<PlayerName, Result<Direction, MoveError>>,
    pub dead_snakes: HashMap<PlayerName, CauseOfDeath>,
    pub eaten_food: HashSet<Vector>,
    pub turn_number: u32,

    pub debug: bool,
}

impl GameState {
    pub fn new(grid: Grid, debug: bool) -> GameState {
        GameState {
            uuid: Uuid::new_v4(),
            grid: grid,
            players: HashMap::new(),
            context: GameContext {
                food: HashSet::new(),
                snakes: HashMap::new(),
            },
            snake_plans: HashMap::new(),
            dead_snakes: HashMap::new(),
            eaten_food: HashSet::new(),
            turn_number: 0,
            debug: debug,
        }
    }

    pub fn add_player(&mut self, mut player: Player) -> PlayerName {
        // Find an unused name.
        let mut player_name = player.clone().name;
        while self.players.contains_key(&player_name) {
            player_name.push('_');
        }
        player.name = player_name.clone();

        let player_box = Box::new(player);
        self.players.insert(player_name.clone(), player_box);

        let snake = Snake::new(vec![self.grid.random_cell(OsRng::new().unwrap())]);
        self.context.snakes.insert(player_name.clone(), snake);

        player_name
    }

    pub fn simulate_next_turn(&mut self) {
        if self.debug {
            println!("Simulating next turn");
        }

        // N.B. does not free memory.
        self.dead_snakes.clear();
        self.eaten_food.clear();

        // Apply movement and remove snakes that did not move.
        self.simulate_snake_movement();
        self.remove_snakes();

        // Grow snakes whose heads collided with a food.
        self.simulate_snake_eating();
        self.remove_food();

        // Detect collisions with snakes and remove colliding snakes.
        self.simulate_snake_collisions();
        self.remove_snakes();

        // Detect snakes outside grid and remove them.
        // @TODO: I think it is sound to move this to being straight after applying movement.
        self.simulate_grid_bounds();
        self.remove_snakes();

        self.turn_number += 1;
    }

    fn simulate_snake_movement(&mut self) {
        // Apply movement and remove snakes that did not move.
        // Snake plans are Result<Direction, MoveError>. MoveError = String.
        // So we can specify an underlying error rather than just omitting any move.
        // Then below if no snake plan is set, we use a default error message.
        // While intricate this very neatly leads to CauseOfDeath.

        let default_planless_error = Err("No move information.".to_string());

        for (player_name, snake) in self.context.snakes.iter_mut() {
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
        for (_, snake) in self.context.snakes.iter_mut() {
            if self.context.food.contains(&snake.segments[0]) {
                // Remove this food only after the full loop, such that N snakes colliding on top of a
                // food all grow. They immediately die but this way collision with growth of both snakes
                // is possible.
                self.eaten_food.insert(snake.segments[0]);
                snake.grow();
            }
        }
    }

    fn simulate_snake_collisions(&mut self) {
        for (player_name, snake) in self.context.snakes.iter() {
            for (coll_player_name, coll_snake) in self.context.snakes.iter() {
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
        for (player_name, snake) in self.context.snakes.iter() {
            for &segment in snake.segments.iter() {
                if !self.grid.is_within_bounds(segment) {
                    self.dead_snakes.insert(player_name.clone(),
                                            CauseOfDeath::CollidedWithBounds(segment));
                }
            }
        }
    }

    fn remove_snakes(&mut self) {
        // N.B. At one point we .drain()ed the dead_snakes Set. This was removed so it
        // can be used to track which players were killed.
        for (player_name, _) in self.dead_snakes.iter() {
            // Kill snake if not already killed, and drop food at non-head segments within the grid.
            // @TODO: This code is much cleaner than the last draft but still lots goes on here.
            if let Some(dead_snake) = self.context.snakes.remove(player_name) {
                // Get segments[1..] safely. Directly slicing panics if the Vec had <2 elements.
                if let Some((_, headless_segments)) = dead_snake.segments.split_first() {
                    // Only retain segments if within grid.
                    let corpse_food: Vec<&Vector> = headless_segments.iter()
                        .filter(|&s| self.grid.is_within_bounds(*s))
                        .collect();
                    self.context.food.extend(corpse_food);
                }
            }
        }
    }

    fn remove_food(&mut self) {
        for food in self.eaten_food.iter() {
            self.context.food.remove(&food);
        }
    }
}
