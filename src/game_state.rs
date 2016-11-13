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
    pub snakes_to_remove: HashSet<PlayerName>,

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
            snakes_to_remove: HashSet::new(),
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

        // Apply movement and remove snakes that did not move.
        for (player_name, snake) in self.context.snakes.iter_mut() {
            if self.snake_plans.contains_key(player_name) {
                let plan = self.snake_plans.get(player_name).unwrap();
                if self.debug {
                    println!("Snake {:?} moved {:?}.", player_name, plan);
                }
                snake.step_in_direction(*plan);
            } else {
                if self.debug {
                    println!("Snake {:?} was not moved.", player_name);
                }
                // Snakes which weren't moved turn into food and die.
                self.context.food.extend(snake.segments.iter());
                self.snakes_to_remove.insert(player_name.clone());
            }
        }
        for player_name in self.snakes_to_remove.drain() {
            if self.debug {
                println!("Snake {:?} was removed (remove stage 1).", player_name);
            }
            self.context.snakes.remove(&player_name);
        }

        // Grow snakes whose heads collided with a food.
        for (player_name, snake) in self.context.snakes.iter_mut() {
            if self.context.food.contains(&snake.segments[0]) {
                if self.debug {
                    println!("Snake {:?} ate a food {:?}.",
                             player_name,
                             snake.segments[0]);
                }
                // @TODO: Remove food at snake.segments[0].
                snake.grow();
            }
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
                    self.context.food.extend(snake.segments.iter());
                    self.snakes_to_remove.insert(player_name.clone());
                    break;
                }
            }
        }
        for player_name in self.snakes_to_remove.drain() {
            if self.debug {
                println!("Snake {:?} was removed (remove stage 2).", player_name);
            }
            self.context.snakes.remove(&player_name);
        }

        self.turn_number += 1;
    }
}
