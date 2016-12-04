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
    pub snakes_to_remove: HashMap<PlayerName, CauseOfDeath>,

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
            snakes_to_remove: HashMap::new(),
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

    pub fn remove_snakes(&mut self) {
        // N.B. At one point we .drain()ed the snakes_to_remove Set. This was removed so it
        // can be used to track which players were killed.
        for (player_name, _) in self.snakes_to_remove.iter() {
            // Kill snake if not already killed, and drop food at all its segments that are within the grid.
            match self.context.snakes.remove(player_name) {
                Some(mut dead_snake) => {
                    dead_snake.segments.retain(|&segment| self.grid.is_within_bounds(segment));
                    self.context.food.extend(dead_snake.segments.iter());
                }
                _ => {}
            }
        }
    }

    pub fn simulate_next_turn(&mut self) {
        if self.debug {
            println!("Simulating next turn");
        }

        // N.B. does not free memory.
        self.snakes_to_remove.clear();
        let mut foods_to_remove = HashSet::new();

        // Apply movement and remove snakes that did not move.
        for (player_name, snake) in self.context.snakes.iter_mut() {
            let mut cause_of_death: Option<CauseOfDeath> =
                Some(CauseOfDeath::NoMoveMade("No move information.".to_string()));
            if self.snake_plans.contains_key(player_name) {
                match *self.snake_plans.get(player_name).unwrap() {
                    Ok(plan) => {
                        cause_of_death = None;
                        snake.step_in_direction(plan);
                    }
                    Err(ref move_error) => {
                        cause_of_death = Some(CauseOfDeath::NoMoveMade(move_error.clone()));
                    }
                };
            }
            if cause_of_death.is_some() {
                self.snakes_to_remove.insert(player_name.clone(), cause_of_death.unwrap());
            }
        }
        self.remove_snakes();

        // Grow snakes whose heads collided with a food.
        for (player_name, snake) in self.context.snakes.iter_mut() {
            if self.context.food.contains(&snake.segments[0]) {
                if self.debug {
                    println!("Snake {:?} ate a food {:?}.",
                             player_name,
                             snake.segments[0]);
                }
                // Remove this food afterwards such that N snakes colliding on top of a food all grow.
                // They immediately die but this way collision with growth of both snakes is possible.
                foods_to_remove.insert(snake.segments[0]);
                snake.grow();
            }
        }
        for food in foods_to_remove.drain() {
            self.context.food.remove(&food);
        }

        // Detect collisions with snakes and remove colliding snakes.
        for (player_name, snake) in self.context.snakes.iter() {
            for (player_name2, snake2) in self.context.snakes.iter() {
                if snake != snake2 && snake.has_collided_into(snake2) {
                    if self.debug {
                        println!("Snake {:?} collided into Snake {:?}.",
                                 player_name,
                                 player_name2);
                    }
                    self.snakes_to_remove
                        .insert(player_name.clone(),
                                CauseOfDeath::CollidedWithSnake(player_name2.clone()));
                    break;
                }
            }
        }
        self.remove_snakes();

        // Detect snakes outside grid and remove them.
        for (player_name, snake) in self.context.snakes.iter() {
            for &segment in snake.segments.iter() {
                if !self.grid.is_within_bounds(segment) {
                    self.snakes_to_remove.insert(player_name.clone(),
                                                 CauseOfDeath::CollidedWithBounds(segment));
                }
            }
        }
        self.remove_snakes();

        self.turn_number += 1;
    }
}
