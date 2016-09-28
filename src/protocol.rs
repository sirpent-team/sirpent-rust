use std::time::Duration;

use game::*;
use grid::*;
use player::*;

pub static PROTOCOL_VERSION: &'static str = "0.2";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Command<V: Vector> {
    // Upon connect, the server must send a VERSION message.
    #[serde(rename = "VERSION")]
    Version { sirpent: String, protocol: String },
    // Then the client must send a HELLO message.
    #[serde(rename = "HELLO")]
    Hello { player: Player },
    // The server must reply with a SERVER message.
    #[serde(rename = "SERVER")]
    Server {
        world: Option<World>,
        timeout: Option<Duration>,
    },
    // If the client supports that STATUS and wants to play, they must send a JOIN message.
    #[serde(rename = "JOIN")]
    Join,
    // Otherwise or at any time, the client can send a QUIT message before closing the socket.
    #[serde(rename = "QUIT")]
    Quit,
    // In case of a problem on either side, an ERROR message can be sent.
    #[serde(rename = "ERROR")]
    Error,
    // To begin a new game, the server must send a NEW_GAME message to indicate this.
    #[serde(rename = "NEW_GAME")]
    NewGame,
    // The server must send a TURN message with the initial state of the Game.
    #[serde(rename = "TURN")]
    Turn { game: Game<V> },
    // The server must send a MAKE_A_MOVE message to request the player's next move.
    #[serde(rename = "MAKE_A_MOVE")]
    MakeAMove,
    // The client must reply with a MOVE message to indicate their next action.
    // The server must then send a new TURN message to start the next TURN.
    #[serde(rename = "MOVE")]
    Move { direction: V::Direction },
    // The server may kill players who do not reply within a certain time. The server must send a
    // TIMEDOUT message to such players.
    #[serde(rename = "TIMED_OUT")]
    TimedOut,
    // If a player died during this turn, the server must send a DIED message.
    #[serde(rename = "DIED")]
    Died,
    // If a player was the only survivor of this turn, the server must send a WON message.
    #[serde(rename = "WON")]
    Won,
    // A new round now starts. The server may only send further messages to surviving players.
    // The server must send a new TURN message with the result of the previous round.
    // At the conclusion of the game, the server should send a GAME_OVER message to all players.
    #[serde(rename = "GAME_OVER")]
    GameOver,
}

impl<V: Vector> Command<V> {
    pub fn version() -> Command<V> {
        Command::Version {
            sirpent: env!("CARGO_PKG_VERSION").to_string(),
            protocol: PROTOCOL_VERSION.to_string(),
        }
    }
}
