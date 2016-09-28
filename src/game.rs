use uuid::Uuid;
use std::collections::HashMap;

use grid::*;
use snake::*;
use player::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game<V: Vector> {
    pub uuid: Uuid,
    pub world: World,
    pub players: HashMap<String, Player>,
    pub state: GameState<V>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState<V: Vector> {
    pub food: V,
    pub snakes: HashMap<Uuid, Snake<V>>,
}
