mod client;
mod group;

use self::client::*;
use self::group::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Timeout {
    None,
    Optional(Duration),
    Disconnecting(Duration),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    // Send a message to a single client.
    Transmit((Uuid, Msg)),
    // Send specific messages to specific clients.
    TransmitToGroup(HashMap<Uuid, Msg>),
    // Send a message to all clients on the other end.
    Broadcast(Msg),
    // Receive a message from a single client into a `oneshot::Receiver`.
    ReceiveInto((Uuid, oneshot::Sender<Msg>), Timeout),
    // Receive one message from each specified clients into `oneshot::Receiver`s.
    ReceiveFromGroupInto(Vec<Uuid>, oneshot::Sender<HashMap<Uuid, Msg>>, Timeout),
    // Discard all messages already received from a client.
    DiscardReceiveBuffer(Uuid),
    // Discard all messages already received from specified clients.
    DiscardReceiveBufferForGroup(Vec<Uuid>),
    // Receive a message from a single client into a `oneshot::Receiver`.
    StatusInto((Uuid, oneshot::Sender<Status>)),
    // Receive one message from each specified clients into `oneshot::Receiver`s.
    StatusFromGroupInto(Vec<Uuid>, oneshot::Sender<HashMap<Uuid, Status>>),
    // Disconnect a single client.
    Close(Uuid),
    // Disconnect specified clients.
    CloseGroup(Vec<Uuid>),
}

pub trait Commander {
    type Transmit = Msg;
    type Receive = Msg;
    type Status = Status;

    fn transmit(&mut self, msg: Self::Transmit) -> Future<Item = (), Error = Error>;

    fn receive(&mut self, optionality: Timeout) -> Future<Item = Self::Receive, Error = Error>;

    fn status(&mut self) -> Future<Item = Self::Status, Error = Error>;

    fn close(&mut self) -> Future<Item = (), Error = Error>;
}
