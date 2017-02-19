use std::time::Duration;
use std::fmt::{self, Display, Formatter};
use std::io;
use serde_json;
use std::error::Error;

use futures::sync::mpsc;

use grids::*;
use game::*;
use client_future::ClientKind;

pub static PROTOCOL_VERSION: &'static str = "0.3";

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "msg")]
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
    #[serde(rename = "close")]
    Close { reason: String },
    #[serde(rename = "new_game")]
    NewGame { game: GameState },
    #[serde(rename = "turn")]
    Turn { turn: TurnState },
    #[serde(rename = "move")]
    Move { direction: Direction },
    #[serde(rename = "died")]
    Died,
    #[serde(rename = "won")]
    Won,
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
    Internal,
    Timeout,
    StreamFinishedUnexpectedly,
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
            ProtocolError::Internal => "Unspecified internal error.",
            ProtocolError::Timeout => "Client timed out.",
            ProtocolError::StreamFinishedUnexpectedly => "Client connection closed unexpectedly.",
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
            ProtocolError::Internal => "Unspecified internal error.".to_string(),
            ProtocolError::Timeout => "Client timed out.".to_string(),
            ProtocolError::StreamFinishedUnexpectedly => {
                "Client connection closed unexpectedly.".to_string()
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

impl From<mpsc::SendError<(String, Direction)>> for ProtocolError {
    fn from(_: mpsc::SendError<(String, Direction)>) -> ProtocolError {
        ProtocolError::Internal
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
        convert_typedmsg_to_msg(RegisterMsg {
            desired_name: "abc".to_string(),
            kind: ClientKind::Player,
        });
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
        convert_typedmsg_to_msg(GameOverMsg { turn: TurnState::new() });

        let register_msg = RegisterMsg {
            desired_name: "jkl".to_string(),
            kind: ClientKind::Player,
        };

        let mut map: serde_json::value::Map<String, serde_json::Value> =
            serde_json::value::Map::new();
        map.insert("desired_name".to_string(),
                   serde_json::Value::String("jkl".to_string()));
        map.insert("kind".to_string(),
                   serde_json::Value::String("player".to_string()));
        let plain_msg = Msg::new(MsgTypeName::Register, serde_json::Value::Object(map));

        assert_eq!(format!("{:?}", Msg::from_typed(register_msg)),
                   format!("{:?}", plain_msg));
    }

    fn convert_typedmsg_to_msg<T: TypedMsg>(msg: T) {
        println!("{:?} {:?}", msg.clone(), Msg::from_typed(msg));

        let desired_name = "jkl".to_string();

        let mut map: serde_json::value::Map<String, serde_json::Value> =
            serde_json::value::Map::new();
        map.insert("desired_name".to_string(),
                   serde_json::Value::String(desired_name.clone()));
        map.insert("kind".to_string(),
                   serde_json::Value::String("spectator".to_string()));
        let register_msg2: RegisterMsg = Msg::new(MsgTypeName::Register,
                                                  serde_json::Value::Object(map))
            .try_into_typed()
            .unwrap();

        assert_eq!(format!("{:?}", register_msg2),
                   format!("{:?}",
                           RegisterMsg {
                               desired_name: desired_name.clone(),
                               kind: ClientKind::Spectator,
                           }));
    }

    // #[test]
    // fn can_convert_msgs_to_msg() {
    //     convert_msg_to_msg(VersionMsg::new());
    //     convert_msg_to_msg(RegisterMsg { desired_player_name: "abc".to_string() });
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
