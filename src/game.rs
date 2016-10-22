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
    pub fn add_player(&mut self, mut player: Player) -> PlayerName {
        let mut player_name = player.clone().name;
        while self.players.contains_key(&player_name) {
            player_name.push('_');
        }
        player.name = player_name.clone();
        self.players.insert(player_name.clone(), player);
        player_name
    }
}
