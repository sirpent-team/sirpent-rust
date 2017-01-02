use std::time::Duration;
use std::fmt::{self, Display, Formatter};
use std::io;
use serde_json;
use serde::{Serialize, Deserialize};
use std::error::Error;
use std::fmt::Debug;

use grid::*;
use snake::*;
use state::*;

pub static PROTOCOL_VERSION: &'static str = "0.2";

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum MsgTypeName {
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

pub trait TypedMsg: Debug + Clone + Sized + Serialize + Deserialize {
    const MSG_TYPE_NAME: MsgTypeName;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Msg {
    #[serde(rename = "msg")]
    pub msg_type_name: MsgTypeName,
    pub data: serde_json::Value,
}

impl Msg {
    pub fn new(msg_type_name: MsgTypeName, data: serde_json::Value) -> Msg {
        Msg {
            msg_type_name: msg_type_name,
            data: data,
        }
    }

    pub fn from_typed<T: TypedMsg>(msg_typed: T) -> Msg {
        Msg {
            msg_type_name: T::MSG_TYPE_NAME,
            data: serde_json::to_value(msg_typed),
        }
    }

    pub fn to_typed<T: TypedMsg>(self) -> ProtocolResult<T> {
        if self.msg_type_name != T::MSG_TYPE_NAME {
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

impl TypedMsg for VersionMsg {
    const MSG_TYPE_NAME: MsgTypeName = MsgTypeName::Version;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IdentifyMsg {
    pub desired_name: String,
}

impl TypedMsg for IdentifyMsg {
    const MSG_TYPE_NAME: MsgTypeName = MsgTypeName::Identify;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WelcomeMsg {
    pub name: String,
    pub grid: Grid,
    pub timeout: Option<Duration>,
}

impl TypedMsg for WelcomeMsg {
    const MSG_TYPE_NAME: MsgTypeName = MsgTypeName::Welcome;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewGameMsg {
    pub game: GameState,
}

impl TypedMsg for NewGameMsg {
    const MSG_TYPE_NAME: MsgTypeName = MsgTypeName::NewGame;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TurnMsg {
    pub turn: TurnState,
}

impl TypedMsg for TurnMsg {
    const MSG_TYPE_NAME: MsgTypeName = MsgTypeName::Turn;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MoveMsg {
    pub direction: Direction,
}

impl TypedMsg for MoveMsg {
    const MSG_TYPE_NAME: MsgTypeName = MsgTypeName::Move;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiedMsg {
    pub cause_of_death: CauseOfDeath,
}

impl TypedMsg for DiedMsg {
    const MSG_TYPE_NAME: MsgTypeName = MsgTypeName::Died;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WonMsg {}

impl TypedMsg for WonMsg {
    const MSG_TYPE_NAME: MsgTypeName = MsgTypeName::Won;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameOverMsg {}

impl TypedMsg for GameOverMsg {
    const MSG_TYPE_NAME: MsgTypeName = MsgTypeName::GameOver;
}

#[derive(Debug)]
pub enum ProtocolError {
    Io(io::Error),
    Serde(serde_json::Error),
    NoMsgReceived,
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
    InvalidStateTransition { from_state: String, event: String },
}

impl Error for ProtocolError {
    fn description(&self) -> &str {
        match *self {
            ProtocolError::Io(ref io_err) => io_err.description(),
            ProtocolError::Serde(ref serde_json_err) => serde_json_err.description(),
            ProtocolError::NoMsgReceived => "No message received.",
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
            // Means: put format strings in fmt::Display and use that for CauseOfDeath.
            ProtocolError::WrongCommand => "Wrong command was read.",
            ProtocolError::InvalidStateTransition { .. } => "Invalid state transition requested.",
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

impl Display for ProtocolError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let disp = match *self {
            ProtocolError::Io(ref io_err) => {
                format!("IO Error in protocol handling: {:?}", io_err.description())
            }
            ProtocolError::Serde(ref serde_json_err) => {
                format!("Serde JSON Error in protocol handling: {:?}",
                        serde_json_err.description())
            }
            ProtocolError::NoMsgReceived => "No message received.".to_string(),
            ProtocolError::NothingReadFromStream => "Nothing read from stream.".to_string(),
            ProtocolError::MessageReadNotADictionary => {
                "Message from stream was not a dictionary.".to_string()
            }
            ProtocolError::MessageReadMissingMsgField => {
                "No msg field in message from stream.".to_string()
            }
            ProtocolError::MessageReadNonStringMsgField => {
                "msg field was not a string in message from stream.".to_string()
            }
            ProtocolError::MessageReadMissingDataField => {
                "No data field provided from stream.".to_string()
            }
            ProtocolError::CommandWasEmpty => "The outer Command object was empty.".to_string(),
            ProtocolError::CommandDataWasNotObject => "Command data was not an object.".to_string(),
            ProtocolError::CommandSerialiseNotObjectNotString => {
                "Serialised Command was not an object or string.".to_string()
            }
            ProtocolError::SendToUnknownPlayer => "Sending to unknown player_name.".to_string(),
            ProtocolError::RecieveFromUnknownPlayer => {
                "Receiving from unknown player_name.".to_string()
            }
            ProtocolError::UnexpectedCommand => "Unexpected command read.".to_string(),
            // @TODO: Really want to include the wrong command in the message usable by clients.
            // Means: put format strings in fmt::Display and use that for CauseOfDeath.
            ProtocolError::WrongCommand => "Wrong command was read.".to_string(),
            ProtocolError::InvalidStateTransition { ref from_state, ref event } => {
                format!("Invalid state transition requested: {:?} --{:?}--> ???.",
                        from_state,
                        event)
            }
        };
        f.write_str(&*disp)
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
    fn can_convert_typedmsgs_to_msg() {
        convert_typedmsg_to_msg(VersionMsg::new());
        convert_typedmsg_to_msg(IdentifyMsg { desired_name: "abc".to_string() });
        convert_typedmsg_to_msg(WelcomeMsg {
            name: "def".to_string(),
            grid: Grid { radius: 15 },
            timeout: None,
        });
        convert_typedmsg_to_msg(NewGameMsg { game: GameState::new(Grid { radius: 15 }) });
        convert_typedmsg_to_msg(TurnMsg { turn: TurnState::new() });
        convert_typedmsg_to_msg(MoveMsg { direction: Direction::variants()[0] });
        convert_typedmsg_to_msg(DiedMsg {
            cause_of_death: CauseOfDeath::NoMoveMade("ghi".to_string()),
        });
        convert_typedmsg_to_msg(WonMsg {});
        convert_typedmsg_to_msg(GameOverMsg {});

        let identify_msg = IdentifyMsg { desired_name: "jkl".to_string() };

        let mut map: serde_json::value::Map<String, serde_json::Value> =
            serde_json::value::Map::new();
        map.insert("desired_name".to_string(),
                   serde_json::Value::String("jkl".to_string()));
        let plain_msg = Msg::new(MsgTypeName::Identify, serde_json::Value::Object(map));

        assert_eq!(format!("{:?}", Msg::from_typed(identify_msg)),
                   format!("{:?}", plain_msg));
    }

    fn convert_typedmsg_to_msg<T: TypedMsg>(msg: T) {
        println!("{:?} {:?}", msg.clone(), Msg::from_typed(msg));

        let desired_name = "jkl".to_string();

        let mut map: serde_json::value::Map<String, serde_json::Value> =
            serde_json::value::Map::new();
        map.insert("desired_name".to_string(),
                   serde_json::Value::String(desired_name.clone()));
        let identify_msg2: IdentifyMsg = Msg::new(MsgTypeName::Identify,
                                                  serde_json::Value::Object(map))
            .to_typed()
            .unwrap();

        assert_eq!(format!("{:?}", identify_msg2),
                   format!("{:?}", IdentifyMsg { desired_name: desired_name.clone() }));
    }

    // #[test]
    // fn can_convert_msgs_to_msg() {
    //     convert_msg_to_msg(VersionMsg::new());
    //     convert_msg_to_msg(IdentifyMsg { desired_player_name: "abc".to_string() });
    //     convert_msg_to_msg(WelcomeMsg {
    //         player_name: "def".to_string(),
    //         grid: Grid { radius: 15 },
    //         timeout: None,
    //     });
    //     convert_msg_to_msg(NewGameMsg { game: GameState::new(Grid { radius: 15 }) });
    //     convert_msg_to_msg(TurnMsg { turn: TurnState::new() });
    //     convert_msg_to_msg(MoveMsg { direction: Direction::variants()[0] });
    //     convert_msg_to_msg(DiedMsg {
    //         cause_of_death: CauseOfDeath::NoMoveMade("ghi".to_string()),
    //     });
    //     convert_msg_to_msg(WonMsg {});
    //     convert_msg_to_msg(GameOverMsg {});
    // }

    // fn convert_msg_to_msg<T: TypedMsg>(plain_msg: Msg, msg: &mut T) {
    //     *msg = plain_msg.clone().to_typed().unwrap();
    //     println!("{:?} {:?}", plain_msg, msg.clone());
    // }
}
