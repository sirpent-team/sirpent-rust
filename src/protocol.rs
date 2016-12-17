use std::time::Duration;
use std::fmt::{self, Display, Formatter};
use std::io;
use serde_json;
use serde::{Serialize, Deserialize};
use std::error::Error;
use std::fmt::Debug;

use grid::*;
use snake::*;
use player::*;
use state::*;

pub static PROTOCOL_VERSION: &'static str = "0.2";

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum MessageTypeName {
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

pub trait TypedMessage: Debug + Clone + Sized + Serialize + Deserialize {
    const MESSAGE_TYPE_NAME: MessageTypeName;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlainMessage {
    #[serde(rename = "msg")]
    pub msg_type_name: MessageTypeName,
    pub data: serde_json::Value,
}

impl PlainMessage {
    pub fn new(msg_type_name: MessageTypeName, data: serde_json::Value) -> PlainMessage {
        PlainMessage {
            msg_type_name: msg_type_name,
            data: data,
        }
    }

    pub fn from_typed<T: TypedMessage>(message_typed: T) -> PlainMessage {
        PlainMessage {
            msg_type_name: T::MESSAGE_TYPE_NAME,
            data: serde_json::to_value(message_typed),
        }
    }

    pub fn to_typed<T: TypedMessage>(self) -> ProtocolResult<T> {
        if self.msg_type_name != T::MESSAGE_TYPE_NAME {
            return Err(ProtocolError::WrongCommand);
        }
        match serde_json::from_value(self.data) {
            Ok(v) => Ok(v),
            Err(e) => Err(From::from(e)),
        }
    }
}


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VersionMessage {
    pub sirpent: String,
    pub protocol: String,
}

impl VersionMessage {
    pub fn new() -> VersionMessage {
        VersionMessage {
            sirpent: env!("CARGO_PKG_VERSION").to_string(),
            protocol: PROTOCOL_VERSION.to_string(),
        }
    }
}

impl TypedMessage for VersionMessage {
    const MESSAGE_TYPE_NAME: MessageTypeName = MessageTypeName::Version;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IdentifyMessage {
    pub desired_player_name: PlayerName,
}

impl TypedMessage for IdentifyMessage {
    const MESSAGE_TYPE_NAME: MessageTypeName = MessageTypeName::Identify;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WelcomeMessage {
    pub player_name: PlayerName,
    pub grid: Grid,
    pub timeout: Option<Duration>,
}

impl TypedMessage for WelcomeMessage {
    const MESSAGE_TYPE_NAME: MessageTypeName = MessageTypeName::Welcome;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewGameMessage {
    pub game: GameState,
}

impl TypedMessage for NewGameMessage {
    const MESSAGE_TYPE_NAME: MessageTypeName = MessageTypeName::NewGame;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TurnMessage {
    pub turn: TurnState,
}

impl TypedMessage for TurnMessage {
    const MESSAGE_TYPE_NAME: MessageTypeName = MessageTypeName::Turn;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MoveMessage {
    pub direction: Direction,
}

impl TypedMessage for MoveMessage {
    const MESSAGE_TYPE_NAME: MessageTypeName = MessageTypeName::Move;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiedMessage {
    pub cause_of_death: CauseOfDeath,
}

impl TypedMessage for DiedMessage {
    const MESSAGE_TYPE_NAME: MessageTypeName = MessageTypeName::Died;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WonMessage {}

impl TypedMessage for WonMessage {
    const MESSAGE_TYPE_NAME: MessageTypeName = MessageTypeName::Won;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameOverMessage {}

impl TypedMessage for GameOverMessage {
    const MESSAGE_TYPE_NAME: MessageTypeName = MessageTypeName::GameOver;
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_convert_io_errors_to_protocol_errors() {
        let io_err = io::Error::new(io::ErrorKind::Other, "oh no!");
        let protocol_err: ProtocolError = From::from(io_err);
        println!("{:?}", protocol_err);
    }

    #[test]
    fn can_convert_serde_json_errors_to_protocol_errors() {
        let serde_json_err =
            serde_json::Error::Syntax(serde_json::error::ErrorCode::ExpectedColon, 0, 0);
        let protocol_err: ProtocolError = From::from(serde_json_err);
        println!("{:?}", protocol_err);
    }

    #[test]
    fn can_convert_msgs_to_plainmessage() {
        convert_msg_to_plainmessage(VersionMessage::new());
        convert_msg_to_plainmessage(IdentifyMessage { desired_player_name: "abc".to_string() });
        convert_msg_to_plainmessage(WelcomeMessage {
            player_name: "def".to_string(),
            grid: Grid { radius: 15 },
            timeout: None,
        });
        convert_msg_to_plainmessage(NewGameMessage { game: GameState::new(Grid { radius: 15 }) });
        convert_msg_to_plainmessage(TurnMessage { turn: TurnState::new() });
        convert_msg_to_plainmessage(MoveMessage { direction: Direction::variants()[0] });
        convert_msg_to_plainmessage(DiedMessage {
            cause_of_death: CauseOfDeath::NoMoveMade("ghi".to_string()),
        });
        convert_msg_to_plainmessage(WonMessage {});
        convert_msg_to_plainmessage(GameOverMessage {});

        let identify_msg = IdentifyMessage { desired_player_name: "jkl".to_string() };

        let mut map: serde_json::value::Map<String, serde_json::Value> =
            serde_json::value::Map::new();
        map.insert("desired_player_name".to_string(),
                   serde_json::Value::String("jkl".to_string()));
        let plain_msg = PlainMessage::new(MessageTypeName::Identify,
                                          serde_json::Value::Object(map));

        assert_eq!(format!("{:?}", PlainMessage::from_typed(identify_msg)),
                   format!("{:?}", plain_msg));
    }

    fn convert_msg_to_plainmessage<T: TypedMessage>(msg: T) {
        println!("{:?} {:?}", msg.clone(), PlainMessage::from_typed(msg));

        let desired_player_name = "jkl".to_string();

        let mut map: serde_json::value::Map<String, serde_json::Value> =
            serde_json::value::Map::new();
        map.insert("desired_player_name".to_string(),
                   serde_json::Value::String(desired_player_name.clone()));
        let identify_msg2: IdentifyMessage = PlainMessage::new(MessageTypeName::Identify,
                                                               serde_json::Value::Object(map))
            .to_typed()
            .unwrap();

        assert_eq!(format!("{:?}", identify_msg2),
                   format!("{:?}",
                           IdentifyMessage { desired_player_name: desired_player_name.clone() }));
    }

    // #[test]
    // fn can_convert_plainmessages_to_msg() {
    //     convert_plainmessage_to_msg(VersionMessage::new());
    //     convert_plainmessage_to_msg(IdentifyMessage { desired_player_name: "abc".to_string() });
    //     convert_plainmessage_to_msg(WelcomeMessage {
    //         player_name: "def".to_string(),
    //         grid: Grid { radius: 15 },
    //         timeout: None,
    //     });
    //     convert_plainmessage_to_msg(NewGameMessage { game: GameState::new(Grid { radius: 15 }) });
    //     convert_plainmessage_to_msg(TurnMessage { turn: TurnState::new() });
    //     convert_plainmessage_to_msg(MoveMessage { direction: Direction::variants()[0] });
    //     convert_plainmessage_to_msg(DiedMessage {
    //         cause_of_death: CauseOfDeath::NoMoveMade("ghi".to_string()),
    //     });
    //     convert_plainmessage_to_msg(WonMessage {});
    //     convert_plainmessage_to_msg(GameOverMessage {});
    // }

    // fn convert_plainmessage_to_msg<T: TypedMessage>(plain_msg: PlainMessage, msg: &mut T) {
    //     *msg = plain_msg.clone().to_typed().unwrap();
    //     println!("{:?} {:?}", plain_msg, msg.clone());
    // }
}
