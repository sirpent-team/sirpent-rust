use std::time::Duration;

use game::*;
use grid::*;
use player::*;

pub static PROTOCOL_VERSION: &'static str = "0.2";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Command {
    // Upon connect, the server must send a VERSION message.
    #[serde(rename = "version")]
    Version { sirpent: String, protocol: String },
    // The server must then send a SERVER message.
    #[serde(rename = "server")]
    Server {
        grid: Grid,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout: Option<Duration>,
    },
    // The client should decide whether it is compatible with this protocol and server setup.
    // If the client wishes to continue it must send a HELLO message.
    #[serde(rename = "hello")]
    Hello {
        player: Player,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        secret: Option<String>,
    },
    // Otherwise or at any time, the client can send a QUIT message or just close the socket.
    #[serde(rename = "quit")]
    Quit,
    // In case of a problem on either side, an ERROR message can be sent.
    #[serde(rename = "error")]
    Error,
    // To begin a new game, the server must send a NEW_GAME message to indicate this.
    #[serde(rename = "new_game")]
    NewGame,
    // The server must send a TURN message with the initial state of the Game.
    #[serde(rename = "turn")]
    Turn { game: Game },
    // The server must send a MAKE_A_MOVE message to request the player's next move.
    #[serde(rename = "make_a_move")]
    MakeAMove,
    // The client must reply with a MOVE message to indicate their next action.
    // The server must then send a new TURN message to start the next TURN.
    #[serde(rename = "move")]
    Move { direction: Direction },
    // The server may kill players who do not reply within a certain time. The server must send a
    // TIMEDOUT message to such players.
    #[serde(rename = "timed_out")]
    TimedOut,
    // If a player died during this turn, the server must send a DIED message.
    #[serde(rename = "died")]
    Died,
    // If a player was the only survivor of this turn, the server must send a WON message.
    #[serde(rename = "won")]
    Won,
    // A new round now starts. The server may only send further messages to surviving players.
    // The server must send a new TURN message with the result of the previous round.
    // At the conclusion of the game, the server should send a GAME_OVER message to all players.
    #[serde(rename = "game_over")]
    GameOver,
}

impl Command {
    pub fn version() -> Command {
        Command::Version {
            sirpent: env!("CARGO_PKG_VERSION").to_string(),
            protocol: PROTOCOL_VERSION.to_string(),
        }
    }
}
