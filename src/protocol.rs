use std::time::Duration;
use std::fmt::{self, Display, Formatter};
use std::io;
use serde_json;
use serde::{Serialize, Deserialize};
use std::error::Error;

use grid::*;
use snake::*;
use player::*;
use state::*;

pub static PROTOCOL_VERSION: &'static str = "0.2";

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum MessageType {
    #[serde(rename = "version")]
    Version,
    #[serde(rename = "identify")]
    Identify,
    #[serde(rename = "welcome")]
    Welcome,
    #[serde(rename = "new_game")]
    NewGame,
    #[serde(rename = "turn")]
    Turn,
    #[serde(rename = "move")]
    Move,
    #[serde(rename = "died")]
    Died,
    #[serde(rename = "won")]
    Won,
    #[serde(rename = "game_over")]
    GameOver,
}

pub trait MessageTyped {
    const MessageType: MessageType;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlainMessage {
    #[serde(rename = "msg")]
    pub msg_type: MessageType,
    pub data: serde_json::Value,
}

impl PlainMessage {
    // pub fn from_string(line: String) -> ProtocolResult<Message> {
    // let line_value: serde_json::Value = serde_json::from_str(&line)?;
    // let obj = line_value.as_object_mut()
    // .ok_or(ProtocolError::MessageReadNotADictionary)?;
    // let msg = obj.remove("msg")
    // .ok_or(ProtocolError::MessageReadMissingMsgField)?
    // .as_str()
    // .ok_or(ProtocolError::MessageReadNonStringMsgField)?
    // .to_string();
    // let msg_type: MessageType = serde_json::from_str(msg)?;
    // let data = obj.remove("data")
    // .ok_or(ProtocolError::MessageReadMissingDataField)?;
    //
    // Ok(Message {
    // msg_type: msg_type,
    // data: data
    // })
    // }

    pub fn from_typed<T: Serialize + MessageTyped>(message_typed: T) -> PlainMessage {
        PlainMessage {
            msg_type: T::MessageType,
            data: serde_json::to_value(message_typed),
        }
    }

    pub fn to_typed<T: Deserialize + MessageTyped>(self) -> ProtocolResult<T>
        where T: Sized
    {
        if self.msg_type != T::MessageType {
            return Err(ProtocolError::WrongCommand);
        }
        match serde_json::from_value(self.data) {
            Ok(v) => Ok(v),
            Err(e) => Err(From::from(e)),
        }
    }
}


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VersionMsg {
    pub sirpent: String,
    pub protocol: String,
}

impl VersionMsg {
    pub fn new() -> VersionMsg {
        VersionMsg {
            sirpent: env!("CARGO_PKG_VERSION").to_string(),
            protocol: PROTOCOL_VERSION.to_string(),
        }
    }
}

impl MessageTyped for VersionMsg {
    const MessageType: MessageType = MessageType::Version;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IdentifyMsg {
    pub desired_player_name: PlayerName,
}

impl MessageTyped for IdentifyMsg {
    const MessageType: MessageType = MessageType::Identify;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WelcomeMsg {
    pub player_name: PlayerName,
    pub grid: Grid,
    pub timeout: Option<Duration>,
}

impl MessageTyped for WelcomeMsg {
    const MessageType: MessageType = MessageType::Welcome;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewGameMsg {
    pub game: GameState,
}

impl MessageTyped for NewGameMsg {
    const MessageType: MessageType = MessageType::NewGame;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TurnMsg {
    pub turn: TurnState,
}

impl MessageTyped for TurnMsg {
    const MessageType: MessageType = MessageType::Turn;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MoveMsg {
    pub direction: Direction,
}

impl MessageTyped for MoveMsg {
    const MessageType: MessageType = MessageType::Move;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiedMsg {
    pub cause_of_death: CauseOfDeath,
}

impl MessageTyped for DiedMsg {
    const MessageType: MessageType = MessageType::Died;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WonMsg {}

impl MessageTyped for WonMsg {
    const MessageType: MessageType = MessageType::Won;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameOverMsg {}

impl MessageTyped for GameOverMsg {
    const MessageType: MessageType = MessageType::GameOver;
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
    UnexpectedCommand,
    WrongCommand,
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
            ProtocolError::MessageReadNonStringMsgField => {
                "msg field was not a string in message from stream."
            }
            ProtocolError::MessageReadMissingDataField => "No data field provided from stream.",
            ProtocolError::CommandWasEmpty => "The outer Command object was empty.",
            ProtocolError::CommandDataWasNotObject => "Command data was not an object.",
            ProtocolError::CommandSerialiseNotObjectNotString => {
                "Serialised Command was not an object or string."
            }
            ProtocolError::SendToUnknownPlayer => "Sending to unknown player_name.",
            ProtocolError::RecieveFromUnknownPlayer => "Receiving from unknown player_name.",
            ProtocolError::UnexpectedCommand => "Unexpected command read.",
            // @TODO: Really want to include the wrong command in the message usable by clients.
            ProtocolError::WrongCommand => "Wrong command was read.",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            ProtocolError::Io(ref io_err) => Some(io_err),
            ProtocolError::Serde(ref serde_json_err) => Some(serde_json_err),
            _ => None,
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

pub type ProtocolResult<T> = Result<T, ProtocolError>;
