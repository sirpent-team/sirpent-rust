mod client;
//mod room;
//mod relay;

pub use self::client::*;
//pub use self::room::*;
//pub use self::relay::*;

use futures::BoxFuture;
use uuid::Uuid;
use std::time::Duration;
use futures::sync::oneshot;

pub type ClientId = Uuid;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClientTimeout {
    None,
    KeepAliveAfter(Duration),
    DisconnectAfter(Duration),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClientStatus {
    Ready,
    Closed,
}

pub enum Command<T, R>
    where T: Send,
          R: Send
{
    // Send a message to a single client.
    Transmit(ClientId, T),
    // Receive a message from a single client into a `oneshot::Receiver`.
    ReceiveInto(ClientId, ClientTimeout, oneshot::Sender<R>),
    // Discard all messages already received from a client.
    DiscardReceiveBuffer(ClientId),
    // Receive a message from a single client into a `oneshot::Receiver`.
    StatusInto(ClientId, oneshot::Sender<ClientStatus>),
    // Disconnect a single client.
    Close(ClientId),
}

pub trait Communicator {
    type Transmit;
    type Receive;
    type Status;
    type Error;

    fn transmit(&mut self, msg: Self::Transmit) -> BoxFuture<Self::Status, Self::Error>;

    fn receive(&mut self, optionality: ClientTimeout) -> BoxFuture<Self::Receive, Self::Error>;

    fn status(&mut self) -> BoxFuture<Self::Status, Self::Error>;

    fn close(&mut self) -> BoxFuture<Self::Status, Self::Error>;
}
