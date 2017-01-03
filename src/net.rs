//! An example [SOCKSv5] proxy server on top of futures
//!
//! [SOCKSv5]: https://www.ietf.org/rfc/rfc1928.txt
//!
//! This program is intended to showcase many aspects of the futures crate and
//! I/O integration, explaining how many of the features can interact with one
//! another and also provide a concrete example to see how easily pieces can
//! interoperate with one another.
//!
//! A SOCKS proxy is a relatively easy protocol to work with. Each TCP
//! connection made to a server does a quick handshake to determine where data
//! is going to be proxied to, another TCP socket is opened up to this
//! destination, and then bytes are shuffled back and forth between the two
//! sockets until EOF is reached.
//!
//! This server implementation is relatively straightforward, but
//! architecturally has a few interesting pieces:
//!
//! * The entire server only has one buffer to read/write data from. This global
//!   buffer is shared by all connections and each proxy pair simply reads
//!   through it. This is achieved by waiting for both ends of the proxy to be
//!   ready, and then the transfer is done.
//!
//! * Initiating a SOCKS proxy connection may involve a DNS lookup, which
//!   is done with the TRust-DNS futures-based resolver. This demonstrates the
//!   ease of integrating a third-party futures-based library into our futures
//!   chain.
//!
//! * The entire SOCKS handshake is implemented using the various combinators in
//!   the `futures` crate as well as the `tokio_core::io` module. The actual
//!   proxying of data, however, is implemented through a manual implementation
//!   of `Future`. This shows how it's easy to transition back and forth between
//!   the two, choosing whichever is the most appropriate for the situation at
//!   hand.
//!
//! You can try out this server with `cargo test` or just `cargo run` and
//! throwing connections at it yourself, and there should be plenty of comments
//! below to help walk you through the implementation as well!

use std::io;
use std::str;
use std::time::Duration;
use tokio_core::io::Codec;
use std::error::Error;
use std::marker::Send;

use futures::{BoxFuture, Future, Stream, Sink};
use tokio_core::net::TcpStream;
use tokio_core::io::{EasyBuf, Framed};
use serde_json;

use grid::*;
use state::*;
use protocol::*;

pub type MsgTransport = Framed<TcpStream, MsgCodec>;
pub type SendFuture = BoxFuture<MsgTransport, ProtocolError>;
pub type RecvFuture<M: TypedMsg> = BoxFuture<(M, MsgTransport), ProtocolError>;

// Data used to when processing a client to perform various operations over its
// lifetime.
pub struct Client;

// http://aturon.github.io/blog/2016/08/11/futures/
// https://raw.githubusercontent.com/tokio-rs/tokio-socks5/master/src/main.rs
impl Client {
    fn send_msg<M: TypedMsg>(transport: MsgTransport, typed_msg: M) -> SendFuture
        where M: Send + 'static
    {
        let msg = Msg::from_typed(typed_msg);
        transport.send(msg).map_err(|e| ProtocolError::from(e)).boxed()
    }

    fn recv_msg<M: TypedMsg>(transport: MsgTransport) -> RecvFuture<M>
        where M: Send + 'static
    {
        transport.into_future()
            .map_err(|(e, _)| ProtocolError::from(e))
            .and_then(|(option_msg, transport)| {
                option_msg.ok_or(ProtocolError::NoMsgReceived)
                    .and_then(|msg| msg.to_typed().map_err(|e| ProtocolError::from(e)))
                    .and_then(|typed_msg| Ok((typed_msg, transport)))
            })
            .boxed()
    }

    pub fn handshake(self, transport: MsgTransport) -> RecvFuture<IdentifyMsg> {
        let version_msg = VersionMsg::new();
        Self::send_msg(transport, version_msg).and_then(Self::recv_msg).boxed()
    }

    pub fn welcome(self,
                   transport: MsgTransport,
                   name: String,
                   grid: Grid,
                   timeout: Option<Duration>)
                   -> SendFuture {
        let welcome_msg = WelcomeMsg {
            name: name,
            grid: grid,
            timeout: timeout,
        };
        Self::send_msg(transport, welcome_msg)
    }

    pub fn game(self, transport: MsgTransport, game: GameState) -> SendFuture {
        let new_game_msg = NewGameMsg { game: game };
        Self::send_msg(transport, new_game_msg).boxed()
    }

    pub fn turn(self, transport: MsgTransport, turn: TurnState) -> RecvFuture<MoveMsg> {
        let turn_msg = TurnMsg { turn: turn };
        Self::send_msg(transport, turn_msg).and_then(Self::recv_msg).boxed()
    }

    pub fn game_over(self, transport: MsgTransport, turn: TurnState) -> SendFuture {
        let game_over_msg = GameOverMsg { turn: turn };
        Self::send_msg(transport, game_over_msg).boxed()
    }
}

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
