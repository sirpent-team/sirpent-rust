use uuid::Uuid;
use std::collections::HashSet;

use super::*;
use utils::*;
use state::*;
use state::grids::*;

pub static PROTOCOL_VERSION: &'static str = "0.4";

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Msg {
    Version { sirpent: String, protocol: String },
    Register {
        desired_name: String,
        kind: ClientKind,
    },
    Welcome {
        name: String,
        grid: GridEnum,
        timeout_millis: Option<Milliseconds>,
    },
    Game { game: Box<GameState> },
    Round {
        round: Box<RoundState>,
        game_uuid: Uuid,
    },
    Move { direction: Direction },
    Outcome {
        winners: HashSet<String>,
        conclusion: Box<RoundState>,
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
            conclusion: Box::new(final_round_state),
            game_uuid: game_uuid,
        }
    }
}
