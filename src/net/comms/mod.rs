mod client;
mod room;
mod relay;

pub use self::client::*;
pub use self::room::*;
pub use self::relay::*;

use futures::{Future, Sink, StartSend, Poll};
use uuid::Uuid;
use net::Msg;
use std::collections::{HashSet, HashMap};
use std::time::Duration;
use futures::sync::oneshot;

#[derive(Hash, Copy, Clone, Debug, PartialEq, Eq)]
pub struct ClientId {
    client_id: Uuid,
    relay_id: Uuid,
}

impl ClientId {
    pub fn new_for_relay(relay_id: Uuid) -> ClientId {
        ClientId {
            client_id: Uuid::new_v4(),
            relay_id: relay_id,
        }
    }

    pub fn client_id(&self) -> Uuid {
        self.client_id
    }

    pub fn relay_id(&self) -> Uuid {
        self.relay_id
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClientTimeout {
    None,
    Optional(Duration),
    Disconnecting(Duration),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClientStatus {
    Ready,
    Waiting,
    Gone,
}

pub enum Command {
    // Send a message to a single client.
    Transmit(ClientId, Msg),
    // Send specific messages to specific clients.
    TransmitToGroup(HashMap<ClientId, Msg>),
    // Send a message to all clients on the other end.
    Broadcast(Msg),
    // Receive a message from a single client into a `oneshot::Receiver`.
    ReceiveInto(ClientId, oneshot::Sender<Msg>, ClientTimeout),
    // Receive one message from each specified clients into `oneshot::Receiver`s.
    ReceiveFromGroupInto(HashSet<ClientId>, oneshot::Sender<HashMap<ClientId, Msg>>, ClientTimeout),
    // Discard all messages already received from a client.
    DiscardReceiveBuffer(ClientId),
    // Discard all messages already received from specified clients.
    DiscardReceiveBufferForGroup(HashSet<ClientId>),
    // Receive a message from a single client into a `oneshot::Receiver`.
    StatusInto(ClientId, oneshot::Sender<ClientStatus>),
    // Receive one message from each specified clients into `oneshot::Receiver`s.
    StatusFromGroupInto(HashSet<ClientId>, oneshot::Sender<HashMap<ClientId, ClientStatus>>),
    // Disconnect a single client.
    Close(ClientId),
    // Disconnect specified clients.
    CloseGroup(HashSet<ClientId>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommandChannel<C>
    where C: Sink<SinkItem = Command> + Send + Clone + 'static
{
    relay_id: Uuid,
    cmd_tx: C,
}

impl<C> CommandChannel<C>
    where C: Sink<SinkItem = Command> + Send + Clone + 'static
{
    pub fn new_for_relay(relay_id: Uuid, cmd_tx: C) -> CommandChannel<C> {
        CommandChannel {
            relay_id: relay_id,
            cmd_tx: cmd_tx,
        }
    }

    pub fn relay_id(&self) -> Uuid {
        self.relay_id
    }

    pub fn can_command(&self, client_id: &ClientId) -> bool {
        self.relay_id == client_id.relay_id()
    }
}

impl<C> Sink for CommandChannel<C>
    where C: Sink<SinkItem = Command> + Send + Clone + 'static
{
    type SinkItem = C::SinkItem;
    type SinkError = C::SinkError;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        self.cmd_tx.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.cmd_tx.poll_complete()
    }
}

pub trait Communicator {
    type Transmit;
    type Receive;
    type Status;
    type Error;

    fn transmit(&mut self, msg: Self::Transmit) -> Box<Future<Item = (), Error = Self::Error>>;

    fn receive(&mut self,
               optionality: ClientTimeout)
               -> Box<Future<Item = Self::Receive, Error = Self::Error>>;

    fn status(&mut self) -> Box<Future<Item = Self::Status, Error = Self::Error>>;

    fn close(&mut self) -> Box<Future<Item = (), Error = Self::Error>>;
}
