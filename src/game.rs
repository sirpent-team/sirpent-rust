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

impl Game {
    pub fn add_player(&mut self, player: Player) {
        self.players.insert(player.clone().name, player);
    }
}
