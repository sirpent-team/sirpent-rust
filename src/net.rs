use std::io;
use std::str;
use tokio_core::io::Codec;
use std::error::Error;
use std::net::SocketAddr;
use std::time::Duration;
use std::marker::Send;

use futures::{Future, Stream, Sink};
use futures::stream::{SplitStream, SplitSink};
use tokio_core::net::TcpStream;
use tokio_core::io::{Io, EasyBuf, Framed};
use serde_json;

use grid::*;
use snake::*;
use state::*;
use protocol::*;

pub struct Client<S, T>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send,
          T: Stream<Item = Msg, Error = io::Error> + Send
{
    pub name: Option<String>,
    pub addr: Option<SocketAddr>,
    msg_tx: S,
    msg_rx: T,
}

impl<S, T> Client<S, T>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send,
          T: Stream<Item = Msg, Error = io::Error> + Send
{
    pub fn new(name: Option<String>,
               addr: Option<SocketAddr>,
               msg_tx: S,
               msg_rx: T)
               -> Client<S, T> {
        Client {
            name: name,
            addr: addr,
            msg_tx: msg_tx,
            msg_rx: msg_rx,
        }
    }

    pub fn from_incoming(stream: TcpStream,
                         addr: SocketAddr)
                         -> Client<SplitSink<MsgTransport>, SplitStream<MsgTransport>> {
        let msg_transport = stream.framed(MsgCodec);
        let (msg_tx, msg_rx) = msg_transport.split();
        Client::new(None, Some(addr), msg_tx, msg_rx)
    }

    fn send<M: TypedMsg>(self, typed_msg: M) -> impl Future<Item = Self, Error = ProtocolError> {
        let name = self.name;
        let addr = self.addr;
        let msg_rx = self.msg_rx;
        let msg = Msg::from_typed(typed_msg);
        self.msg_tx
            .send(msg)
            .map_err(|e| ProtocolError::from(e))
            .map(move |msg_tx| Client::new(name, addr, msg_tx, msg_rx))
    }

    fn receive<M: TypedMsg>(self) -> impl Future<Item = (M, Self), Error = (ProtocolError, Self)> {
        let name = self.name;
        let addr = self.addr;
        let msg_tx = self.msg_tx;
        self.msg_rx
            .into_future()
            .map_err(|(e, msg_rx)| (ProtocolError::from(e), msg_rx))
            .and_then(|(maybe_msg, msg_rx)| {
                let msg = maybe_msg.ok_or(ProtocolError::NoMsgReceived);
                match msg.and_then(|msg| Msg::to_typed(msg)) {
                    Ok(typed_msg) => Ok((typed_msg, msg_rx)),
                    Err(e) => Err((e, msg_rx)),
                }
            })
            .then(move |result| {
                match result {
                    Ok((typed_msg, msg_rx)) => {
                        Ok((typed_msg, Client::new(name, addr, msg_tx, msg_rx)))
                    }
                    Err((e, msg_rx)) => Err((e, Client::new(name, addr, msg_tx, msg_rx))),
                }
            })
    }

    /// Tell the client our protocol version and expect them to send back a name to use.
    /// A Client will be included with the ProtocolError unless sending the VersionMsg failed.
    pub fn handshake
        (self)
         -> impl Future<Item = (IdentifyMsg, Self), Error = (ProtocolError, Option<Self>)> {
        self.send(VersionMsg::new())
            .map_err(|e| (e, None))
            .and_then(|client| client.receive().map_err(|(e, client)| (e, Some(client))))
    }

    pub fn welcome(mut self,
                   name: String,
                   grid: Grid,
                   timeout: Option<Duration>)
                   -> impl Future<Item = Self, Error = ProtocolError> {
        self.name = Some(name.clone());
        self.send(WelcomeMsg {
            name: name,
            grid: grid,
            timeout: timeout,
        })
    }

    pub fn new_game(self, game: GameState) -> impl Future<Item = Self, Error = ProtocolError> {
        self.send(NewGameMsg { game: game })
    }

    pub fn turn(self,
                turn: TurnState)
                -> impl Future<Item = (MoveMsg, Self), Error = (ProtocolError, Option<Self>)> {
        self.send(TurnMsg { turn: turn })
            .map_err(|e| (e, None))
            .and_then(|client| client.receive().map_err(|(e, client)| (e, Some(client))))
    }

    pub fn die(self,
               cause_of_death: CauseOfDeath)
               -> impl Future<Item = Self, Error = ProtocolError> {
        self.send(DiedMsg { cause_of_death: cause_of_death })
    }

    pub fn end_game(self, turn: TurnState) -> impl Future<Item = Self, Error = ProtocolError> {
        self.send(GameOverMsg { turn: turn })
    }

    pub fn win(self) -> impl Future<Item = Self, Error = ProtocolError> {
        self.send(WonMsg {})
    }
}

// @TODO: Would it help my code to implement by own MsgTransport rather than using
// the Request-Response Service-focused one in tokio?
pub type MsgTransport = Framed<TcpStream, MsgCodec>;

// @TODO: Remove Box requirement.
// Map a collection to a vector of futures using a provided callback. Then run all those
// futures in parallel using future::join_all.
// pub fn futurise_and_join<I, F, O, E>(items: I, f: F) -> impl Future<Item = Vec<O>, Error = ()>
//     where I: IntoIterator,
//           F: FnMut(I::Item) -> BoxFuture<O, E>
// {
//     let futurised_items = items.into_iter()
//         .map(f)
//         .collect();
//     future::join_all(futurised_items)
// }

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
