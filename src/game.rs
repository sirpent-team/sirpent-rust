use uuid::Uuid;
use std::collections::HashMap;

use grid::*;
use player::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub uuid: Uuid,
    pub grid: Grid,
    pub players: HashMap<PlayerName, Player>,
    pub food: Vector,
}
