use uuid::Uuid;
use std::collections::HashSet;

use utils::*;
use state::*;
use state::grids::*;
use net::clients::*;

pub static PROTOCOL_VERSION: &'static str = "0.4";

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
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
        grid: GridEnum,
        timeout_millis: Option<Milliseconds>,
    },
    #[serde(rename = "game")]
    Game { game: GameState },
    #[serde(rename = "round")]
    Round { round: RoundState, game_uuid: Uuid },
    #[serde(rename = "move")]
    Move { direction: Direction },
    #[serde(rename = "outcome")]
    Outcome {
        winners: HashSet<String>,
        conclusion: RoundState,
        game_uuid: Uuid,
    },
}

impl Msg {
    pub fn version() -> Msg {
        Msg::Version {
            sirpent: env!("CARGO_PKG_VERSION").to_string(),
            protocol: PROTOCOL_VERSION.to_string(),
        }
    }

    pub fn outcome(final_round_state: RoundState, game_uuid: Uuid) -> Msg {
        Msg::Outcome {
            winners: final_round_state.snakes.keys().cloned().collect(),
            conclusion: final_round_state,
            game_uuid: game_uuid,
        }
    }
}
