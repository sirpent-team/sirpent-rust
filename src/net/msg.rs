use std::time::Duration;

use game::*;
use grids::*;
use clients::*;

pub static PROTOCOL_VERSION: &'static str = "0.4";

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
//#[serde(tag = "msg")]
pub enum Msg {
    #[serde(rename = "version")]
    Version {
        sirpent: String,
        protocol: String
    },
    #[serde(rename = "register")]
    Register {
        desired_name: String,
        kind: ClientKind,
    },
    #[serde(rename = "welcome")]
    Welcome {
        name: String,
        grid: Grid,
        timeout: Option<i64>,
    },
    #[serde(rename = "game")]
    Game { game: GameState }
    #[serde(rename = "round")]
    Round {
        round: RoundState,
        game_uuid: Uuid
    }
    #[serde(rename = "move")]
    Move { direction: Direction }
    #[serde(rename = "outcome")]
    Outcome {
        winners: HashSet<String>,
        conclusion: RoundState,
        game_uuid: Uuid
    }
}

impl Msg {
    pub fn version() -> Msg {
        Msg::Version {
            sirpent: env!("CARGO_PKG_VERSION").to_string(),
            protocol: PROTOCOL_VERSION.to_string(),
        }
    }

    pub fn welcome(name: String, grid: Grid, timeout: Option<Duration>) {
        Msg::Welcome {
            name: name,
            grid: grid,
            timeout: timeout.map(Duration::num_milliseconds)
        }
    }

    pub fn outcome(game_uuid: Uuid, round: RoundState) -> Msg {
        Msg::Outcome {
            winners: round.snakes.keys().cloned().collect(),
            conclusion: round,
            game_uuid: game_uuid,
        }
    }
}


