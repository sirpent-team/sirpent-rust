use uuid::Uuid;
use std::collections::{HashSet, HashMap};

use grid::*;
use snake::*;
use player::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub uuid: Uuid,
    pub grid: Grid,
    pub food: HashSet<Vector>,
    pub snakes: HashMap<PlayerName, Snake>,
    pub turn_number: u32,
}

impl GameState {
    pub fn new(grid: Grid) -> GameState {
        GameState {
            uuid: Uuid::new_v4(),
            grid: grid,
            food: HashSet::new(),
            snakes: HashMap::new(),
            turn_number: 0,
        }
    }
}
