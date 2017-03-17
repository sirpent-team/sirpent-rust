mod msg;

pub use self::msg::*;

use std::io;
use std::str;
use serde_json;
use bytes::{BufMut, BytesMut};
use tokio_io::codec::{Encoder, Decoder, Framed};
use tokio_core::net::TcpStream;
use comms;

use utils::*;

// Use `comms`. Define some local type aliases and reexport some plain comms one.
pub use comms::{client, ClientId, ClientStatus, ClientTimeout, Communicator};
pub type Client = comms::Client<Msg, Msg>;
pub type Room = comms::Room<Msg, Msg>;

#[derive(PartialEq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ClientKind {
    #[serde(rename = "player")]
    Player,
    #[serde(rename = "spectator")]
    Spectator,
}

pub type MsgTransport = Framed<TcpStream, MsgCodec>;

// https://github.com/tokio-rs/tokio-line/blob/master/src/framed_transport.rs
#[derive(Clone, Copy, Debug)]
pub struct MsgCodec;

impl Decoder for MsgCodec {
    type Item = Msg;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Msg>, io::Error> {
        // If our buffer contains a newline...
        if let Some(n) = buf.as_ref().iter().position(|b| *b == b'\n') {
            // Remove this line and the newline from the buffer.
            // @TODO @DEBUG: Unsure if this porting to tokio-io/bytes is correct.
            let line = buf.split_to(n);
            buf.split_to(1); // Also remove the '\n'.

            // Turn this data into a UTF string and return it in a Frame.
            let line = match str::from_utf8(line.as_ref()) {
                Ok(s) => s,
                Err(_) => return Err(io_error_from_str("invalid string")),
            };

            // Attempt JSON decode into Msg.
            return match serde_json::from_str(line) {
                Ok(msg) => Ok(Some(msg)),
                Err(e) => Err(io_error_from_error(e)),
            };
        }

        Ok(None)
    }
}

impl Encoder for MsgCodec {
    type Item = Msg;
    type Error = io::Error;

    fn encode(&mut self, msg: Msg, buf: &mut BytesMut) -> io::Result<()> {
        // Attempt Msg encode into JSON.
        let msg_str = serde_json::to_string(&msg).map_err(io_error_from_error)?;
        // Write to output buffer followed by a newline.
        buf.extend(msg_str.as_bytes());
        buf.put(b'\n');

        Ok(())
    }
}
