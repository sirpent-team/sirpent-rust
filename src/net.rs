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
use tokio_core::io::Codec;
use std::error::Error;
use std::marker::Send;

use futures::{future, BoxFuture, Future, Stream, Sink};
use tokio_core::net::TcpStream;
use tokio_core::io::{EasyBuf, Framed};
use serde_json;

use protocol::*;

pub type MsgTransport = Framed<TcpStream, MsgCodec>;
pub type SendFuture = BoxFuture<MsgTransport, ProtocolError>;
pub type RecvFuture<M: TypedMsg> = BoxFuture<(M, MsgTransport), ProtocolError>;
pub type Client = (String, MsgTransport);

pub fn send_msg<M: TypedMsg>(transport: MsgTransport, typed_msg: M) -> SendFuture
    where M: Send + 'static
{
    let msg = Msg::from_typed(typed_msg);
    transport.send(msg).map_err(|e| ProtocolError::from(e)).boxed()
}

pub fn recv_msg<M: TypedMsg>(transport: MsgTransport) -> RecvFuture<M>
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

pub fn tell_handshake(transport: MsgTransport, version_msg: VersionMsg) -> RecvFuture<IdentifyMsg> {
    send_msg(transport, version_msg).and_then(recv_msg).boxed()
}

pub fn tell_welcome(transport: MsgTransport, welcome_msg: WelcomeMsg) -> SendFuture {
    send_msg(transport, welcome_msg)
}

pub fn tell_new_game(players: Vec<Client>,
                     new_game_msg: NewGameMsg)
                     -> BoxFuture<Vec<Client>, ProtocolError> {
    futurise_and_join(players, |(name, transport)| {
            send_msg(transport, new_game_msg.clone())
                .map(move |transport| (name, transport))
                .boxed()
        })
        .boxed()
}

// @TODO: This only takes MoveMsgs from living players, but sends TurnMsg to all.
//        Implementing that restriction at this level is unpleasant. It makes a lot
//        of sense to do in the wrapper vs composing one future for living players
//        and one future for dead ones. But it's too high-level to keep here.
// @TODO: In any case for God's sake test this, and equivalent restriction in Engine.
pub fn take_turn(players: Vec<Client>,
                 turn_msg: TurnMsg)
                 -> BoxFuture<Vec<(Option<MoveMsg>, Client)>, ProtocolError> {
    futurise_and_join(players, |(name, transport)| {
            if turn_msg.turn.snakes.contains_key(&name) {
                send_msg(transport, turn_msg.clone())
                    .and_then(recv_msg)
                    .map(move |(move_msg, transport)| (Some(move_msg), (name, transport)))
                    .boxed()
            } else {
                send_msg(transport, turn_msg.clone())
                    .map(move |transport| (None, (name, transport)))
                    .boxed()
            }
        })
        .boxed()
}

pub fn tell_game_over(players: Vec<Client>,
                      game_over_msg: GameOverMsg)
                      -> BoxFuture<Vec<Client>, ProtocolError> {
    futurise_and_join(players, |(name, transport)| {
            send_msg(transport, game_over_msg.clone())
                .map(move |transport| (name, transport))
                .boxed()
        })
        .boxed()
}

// @TODO: Remove Box requirement.
/// Map a collection to a vector of futures using a provided callback. Then run all those
/// futures in parallel using future::join_all.
pub fn futurise_and_join<I, F, O, E>(items: I, f: F) -> future::JoinAll<Vec<BoxFuture<O, E>>>
    where I: IntoIterator,
          F: FnMut(I::Item) -> BoxFuture<O, E>
{
    let futurised_items = items.into_iter()
        .map(f)
        .collect();
    future::join_all(futurised_items)
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
