use std::time::Duration;
use std::fmt::{self, Display, Formatter};
use std::io;
use serde_json;
use std::error::Error;

use grid::*;
use player::*;
use game_state::*;

pub static PROTOCOL_VERSION: &'static str = "0.2";

// @TODO: Remove empty struct enums. Temporary workaround as custom deserialisation logic in
// PlayerConnection.read couldn't handle things correctly.
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
    Quit {},
    // In case of a problem on either side, an ERROR message can be sent.
    #[serde(rename = "error")]
    Error {},
    // To begin a new game, the server must send a NEW_GAME message to indicate this.
    #[serde(rename = "new_game")]
    NewGame {},
    // The server must send a TURN message with the initial state of the Game.
    #[serde(rename = "turn")]
    Turn { game: GameState },
    // The server must send a MAKE_A_MOVE message to request the player's next move.
    #[serde(rename = "make_a_move")]
    MakeAMove {},
    // The client must reply with a MOVE message to indicate their next action.
    // The server must then send a new TURN message to start the next TURN.
    #[serde(rename = "move")]
    Move { direction: Direction },
    // The server may kill players who do not reply within a certain time. The server must send a
    // TIMEDOUT message to such players.
    #[serde(rename = "timed_out")]
    TimedOut {},
    // If a player died during this turn, the server must send a DIED message.
    #[serde(rename = "died")]
    Died {},
    // If a player was the only survivor of this turn, the server must send a WON message.
    #[serde(rename = "won")]
    Won {},
    // A new round now starts. The server may only send further messages to surviving players.
    // The server must send a new TURN message with the result of the previous round.
    // At the conclusion of the game, the server should send a GAME_OVER message to all players.
    #[serde(rename = "game_over")]
    GameOver {},
}

impl Command {
    pub fn version() -> Command {
        Command::Version {
            sirpent: env!("CARGO_PKG_VERSION").to_string(),
            protocol: PROTOCOL_VERSION.to_string(),
        }
    }
}

#[derive(Debug)]
pub enum ProtocolError {
    Io(io::Error),
    Serde(serde_json::Error),
    NothingReadFromStream,
    MessageReadNotADictionary,
    MessageReadMissingMsgField,
    MessageReadNonStringMsgField,
    MessageReadMissingDataField,
    CommandWasEmpty,
    CommandDataWasNotObject,
    CommandSerialiseNotObjectNotString,
    SendToUnknownPlayer,
    RecieveFromUnknownPlayer,
    UnexpectedCommand
}

// @TODO: Consider if this is best.
impl Display for ProtocolError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}

impl Error for ProtocolError {
    fn description(&self) -> &str {
        match *self {
            ProtocolError::Io(ref io_err) => io_err.description(),
            ProtocolError::Serde(ref serde_json_err) => serde_json_err.description(),
            ProtocolError::NothingReadFromStream => "Nothing read from stream.",
            ProtocolError::MessageReadNotADictionary => "Message from stream was not a dictionary.",
            ProtocolError::MessageReadMissingMsgField => "No msg field in message from stream.",
            ProtocolError::MessageReadNonStringMsgField => "msg field was not a string in message from stream.",
            ProtocolError::MessageReadMissingDataField => "No data field provided from stream.",
            ProtocolError::CommandWasEmpty => "The outer Command object was empty.",
            ProtocolError::CommandDataWasNotObject => "Command data was not an object.",
            ProtocolError::CommandSerialiseNotObjectNotString => "Serialised Command was not an object or string.",
            ProtocolError::SendToUnknownPlayer => "Sending to unknown player_name.",
            ProtocolError::RecieveFromUnknownPlayer => "Receiving from unknown player_name.",
            ProtocolError::UnexpectedCommand => "Unexpected command read."
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            ProtocolError::Io(ref io_err) => Some(io_err),
            ProtocolError::Serde(ref serde_json_err) => Some(serde_json_err),
            _ => None
        }
    }
}

impl From<io::Error> for ProtocolError {
    fn from(err: io::Error) -> ProtocolError {
        ProtocolError::Io(err)
    }
}

impl From<serde_json::Error> for ProtocolError {
    fn from(err: serde_json::Error) -> ProtocolError {
        ProtocolError::Serde(err)
    }
}
