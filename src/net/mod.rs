mod msg;
mod proto;
pub mod comms;

pub use self::msg::*;
pub use self::proto::*;

#[derive(PartialEq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ClientKind {
    #[serde(rename = "player")]
    Player,
    #[serde(rename = "spectator")]
    Spectator,
}
