use uuid::Uuid;
use std::collections::{HashSet, HashMap};

use grid::*;
use snake::*;

#[derive(Debug)]
pub struct State {
    pub game: GameState,
    pub turn: TurnState,
}

impl State {
    pub fn new(grid: Grid) -> State {
        State {
            game: GameState::new(grid),
            turn: TurnState::new(),
        }
    }

    pub fn add_player(&mut self, desired_name: String, snake: Snake) -> String {
        // Find an unused name based upon the desired_name.
        let mut final_name = desired_name;
        while self.game.players.contains(&final_name) {
            final_name += "_";
        }
        // Reserve the new name.
        self.game.players.insert(final_name.clone());

        // Insert the snake.
        self.turn.snakes.insert(final_name.clone(), snake);

        return final_name;
    }
}

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
    pub casualties: HashMap<String, (CauseOfDeath, Snake)>,
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
