use uuid::Uuid;
use std::collections::{HashSet, HashMap};

use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameState {
    pub uuid: Uuid,
    pub grid: GridEnum,
    pub players: HashSet<String>,
}

impl GameState {
    pub fn new(grid: Grid) -> GameState {
        GameState {
            uuid: Uuid::new_v4(),
            grid: grid.into(),
            players: HashSet::new(),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoundState {
    pub round_number: usize,
    pub food: HashSet<Vector>,
    pub eaten: HashMap<String, Vector>,
    pub snakes: HashMap<String, Snake>,
    pub directions: HashMap<String, Direction>,
    pub casualties: HashMap<String, CauseOfDeath>,
}
