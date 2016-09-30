use uuid::Uuid;
use std::collections::HashMap;

use grid::*;
use snake::*;
use player::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub uuid: Uuid,
    pub grid: Grid,
    pub players: HashMap<String, Player>,
    pub state: GameState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub food: Vector,
    pub snakes: HashMap<Uuid, Snake>,
}
