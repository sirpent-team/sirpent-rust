use std::io;
use std::str;
use tokio_core::io::Codec;
use std::error::Error;

use tokio_core::net::TcpStream;
use tokio_core::io::{EasyBuf, Framed};
use serde_json;

use protocol::*;

pub type MsgTransport = Framed<TcpStream, MsgCodec>;

// https://github.com/tokio-rs/tokio-line/blob/master/src/framed_transport.rs
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
                Err(_) => return Err(other_labelled("invalid string")),
            };

            let msg: Result<Msg, serde_json::Error> = serde_json::from_str(line);
            return match msg {
                Ok(msg) => Ok(Some(msg)),
                Err(e) => Err(other(e)),
            };
        }

        Ok(None)
    }

    fn encode(&mut self, msg: Msg, buf: &mut Vec<u8>) -> io::Result<()> {
        let msg_str = match serde_json::to_string(&msg) {
            Ok(s) => s,
            Err(e) => return Err(other(e)),
        };

        for byte in msg_str.as_bytes() {
            buf.push(*byte);
        }

        buf.push(b'\n');
        Ok(())
    }
}

fn other_labelled(desc: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, desc)
}

fn other<E: Error>(e: E) -> io::Error {
    other_labelled(&*format!("{:?}", e))
}
