use game::*;
use grid::*;
use player::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Command<V: Vector> {
    HELLO { player: Player },
    SPEC { world: World },
    JOIN,
    GAME { player: Player, game: Game<V> },
    TURN { player: Player, game: Game<V> },
    MOVE { direction: V::Direction },
    DIED { player: Player, game: Game<V> },
    WON { player: Player, game: Game<V> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message<V: Vector> {
    pub sirpent_version: String,
    pub command: Command<V>,
}

impl<V: Vector> Message<V> {
    pub fn new(command: Command<V>) -> Message<V> {
        Message {
            sirpent_version: env!("CARGO_PKG_VERSION").to_string(),
            command: command,
        }
    }
}
