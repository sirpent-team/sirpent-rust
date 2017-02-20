use std::time::Duration;

use game::*;
use grids::*;
use clients::*;

pub static PROTOCOL_VERSION: &'static str = "0.3";

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
//#[serde(tag = "msg")]
pub enum Msg {
    #[serde(rename = "version")]
    Version { sirpent: String, protocol: String },
    #[serde(rename = "register")]
    Register {
        desired_name: String,
        kind: ClientKind,
    },
    #[serde(rename = "welcome")]
    Welcome {
        name: String,
        grid: Grid,
        timeout: Option<Duration>,
    },
    #[serde(rename = "new_game")]
    NewGame { game: GameState },
    #[serde(rename = "turn")]
    Turn { turn: TurnState },
    #[serde(rename = "move")]
    Move { direction: Direction },
    #[serde(rename = "game_over")]
    GameOver { turn: TurnState },
}

impl Msg {
    pub fn version() -> Msg {
        Msg::Version {
            sirpent: env!("CARGO_PKG_VERSION").to_string(),
            protocol: PROTOCOL_VERSION.to_string(),
        }
    }
}
