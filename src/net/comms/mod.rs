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
pub struct CommunicationId {
    client_id: Uuid,
    relay_id: Uuid,
}

impl CommunicationId {
    pub fn new_for_relay(relay_id: Uuid) -> CommunicationId {
        CommunicationId {
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
    Transmit(CommunicationId, Msg),
    // Send specific messages to specific clients.
    TransmitToGroup(HashMap<CommunicationId, Msg>),
    // Send a message to all clients on the other end.
    Broadcast(Msg),
    // Receive a message from a single client into a `oneshot::Receiver`.
    ReceiveInto(CommunicationId, oneshot::Sender<Msg>, ClientTimeout),
    // Receive one message from each specified clients into `oneshot::Receiver`s.
    ReceiveFromGroupInto(HashSet<CommunicationId>,
                         oneshot::Sender<HashMap<CommunicationId, Msg>>,
                         ClientTimeout),
    // Discard all messages already received from a client.
    DiscardReceiveBuffer(CommunicationId),
    // Discard all messages already received from specified clients.
    DiscardReceiveBufferForGroup(HashSet<CommunicationId>),
    // Receive a message from a single client into a `oneshot::Receiver`.
    StatusInto(CommunicationId, oneshot::Sender<ClientStatus>),
    // Receive one message from each specified clients into `oneshot::Receiver`s.
    StatusFromGroupInto(HashSet<CommunicationId>,
                        oneshot::Sender<HashMap<CommunicationId, ClientStatus>>),
    // Disconnect a single client.
    Close(CommunicationId),
    // Disconnect specified clients.
    CloseGroup(HashSet<CommunicationId>),
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

    pub fn can_command(&self, comm_id: &CommunicationId) -> bool {
        self.relay_id == comm_id.relay_id()
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
