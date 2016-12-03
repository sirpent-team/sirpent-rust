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
    pub snake_plans: HashMap<PlayerName, Direction>,

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

        let mut snakes_to_remove = HashSet::new();
        let mut foods_to_remove = HashSet::new();

        // Apply movement and remove snakes that did not move.
        for (player_name, snake) in self.context.snakes.iter_mut() {
            if self.snake_plans.contains_key(player_name) {
                let plan = self.snake_plans.get(player_name).unwrap();
                if self.debug {
                    println!("Snake {:?} moved {:?}.", player_name, plan);
                }
                snake.step_in_direction(*plan);
            } else {
                snakes_to_remove.insert(player_name.clone());
            }
        }
        for player_name in snakes_to_remove.drain() {
            if self.debug {
                println!("Snake {:?} was not moved and has been removed.",
                         player_name);
            }
            // Kill snake and drop food at all its segments that are within the grid.
            let mut dead_snake = self.context.snakes.remove(&player_name).unwrap();
            dead_snake.segments.retain(|&segment| self.grid.is_within_bounds(segment));
            self.context.food.extend(dead_snake.segments.iter());
        }

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
                    snakes_to_remove.insert(player_name.clone());
                    break;
                }
            }
        }
        for player_name in snakes_to_remove.drain() {
            if self.debug {
                println!("Snake {:?} collided with another snake and has been removed.",
                         player_name);
            }
            // Kill snake and drop food at all its segments that are within the grid.
            let mut dead_snake = self.context.snakes.remove(&player_name).unwrap();
            dead_snake.segments.retain(|&segment| self.grid.is_within_bounds(segment));
            self.context.food.extend(dead_snake.segments.iter());
        }

        // Detect snakes outside grid and remove them.
        for (player_name, snake) in self.context.snakes.iter() {
            for &segment in snake.segments.iter() {
                if !self.grid.is_within_bounds(segment) {
                    snakes_to_remove.insert(player_name.clone());
                }
            }
        }
        for player_name in snakes_to_remove.drain() {
            if self.debug {
                println!("Snake {:?} extended beyond Grid boundaries and has been removed.",
                         player_name);
            }
            // Kill snake and drop food at all its segments that are within the grid.
            let mut dead_snake = self.context.snakes.remove(&player_name).unwrap();
            dead_snake.segments.retain(|&segment| self.grid.is_within_bounds(segment));
            self.context.food.extend(dead_snake.segments.iter());
        }

        self.turn_number += 1;
    }
}
