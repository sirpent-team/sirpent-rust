mod client;
mod group;

pub use self::client::*;
pub use self::group::*;

use futures::Future;
use uuid::Uuid;
use net::Msg;
use std::collections::HashMap;
use std::time::Duration;
use futures::sync::oneshot;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Timeout {
    None,
    Optional(Duration),
    Disconnecting(Duration),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    Ready,
    Waiting,
    Gone,
}

pub enum Command {
    // Send a message to a single client.
    Transmit(Uuid, Msg),
    // Send specific messages to specific clients.
    TransmitToGroup(HashMap<Uuid, Msg>),
    // Send a message to all clients on the other end.
    Broadcast(Msg),
    // Receive a message from a single client into a `oneshot::Receiver`.
    ReceiveInto(Uuid, oneshot::Sender<Msg>, Timeout),
    // Receive one message from each specified clients into `oneshot::Receiver`s.
    ReceiveFromGroupInto(Vec<Uuid>, oneshot::Sender<HashMap<Uuid, Msg>>, Timeout),
    // Discard all messages already received from a client.
    DiscardReceiveBuffer(Uuid),
    // Discard all messages already received from specified clients.
    DiscardReceiveBufferForGroup(Vec<Uuid>),
    // Receive a message from a single client into a `oneshot::Receiver`.
    StatusInto(Uuid, oneshot::Sender<Status>),
    // Receive one message from each specified clients into `oneshot::Receiver`s.
    StatusFromGroupInto(Vec<Uuid>, oneshot::Sender<HashMap<Uuid, Status>>),
    // Disconnect a single client.
    Close(Uuid),
    // Disconnect specified clients.
    CloseGroup(Vec<Uuid>),
}

pub trait Commander {
    type Transmit;
    type Receive;
    type Status;
    type Error;

    fn transmit(&mut self, msg: Self::Transmit) -> Box<Future<Item = (), Error = Self::Error>>;

    fn receive(&mut self,
               optionality: Timeout)
               -> Box<Future<Item = Self::Receive, Error = Self::Error>>;

    fn status(&mut self) -> Box<Future<Item = Self::Status, Error = Self::Error>>;

    fn close(&mut self) -> Box<Future<Item = (), Error = Self::Error>>;
}
