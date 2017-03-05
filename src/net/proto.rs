use std::io;
use std::str;
use serde_json;
use tokio_core::io::Codec;
use tokio_core::net::TcpStream;
use tokio_core::io::{EasyBuf, Framed};

use net::*;
use utils::*;

pub type MsgTransport = Framed<TcpStream, MsgCodec>;

// https://github.com/tokio-rs/tokio-line/blob/master/src/framed_transport.rs
#[derive(Clone, Copy, Debug)]
pub struct MsgCodec;

impl Codec for MsgCodec {
    type In = Msg;
    type Out = Msg;

    fn decode(&mut self, buf: &mut EasyBuf) -> io::Result<Option<Msg>> {
        // If our buffer contains a newline...
        if let Some(n) = buf.as_ref().iter().position(|b| *b == b'\n') {
            // remove this line and the newline from the buffer.
            let line = buf.drain_to(n);
            buf.drain_to(1); // Also remove the '\n'.

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

    fn encode(&mut self, msg: Msg, buf: &mut Vec<u8>) -> io::Result<()> {
        // Attempt Msg encode into JSON.
        let msg_str = serde_json::to_string(&msg).map_err(io_error_from_error)?;
        // Write to output buffer followed by a newline.
        buf.extend_from_slice(msg_str.as_bytes());
        buf.push(b'\n');

        Ok(())
    }
}
