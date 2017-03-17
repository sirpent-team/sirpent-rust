mod client;
mod room;
mod relay;

pub use self::client::*;
pub use self::room::*;
pub use self::relay::*;

use futures::{Future, Poll, BoxFuture};
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
    //Missing,
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

// Utilities for testing `Communicator` implementations.
#[cfg(test)]
mod test {
    use super::*;
    use futures::sync::mpsc;
    use futures::executor;
    use std::sync::Arc;

    #[derive(Clone, PartialEq, Debug)]
    pub enum TinyMsg {
        A,
        B(String),
    }

    pub type TinyCommand = Command<TinyMsg, TinyMsg>;

    pub fn unpark_noop() -> Arc<executor::Unpark> {
        struct Foo;

        impl executor::Unpark for Foo {
            fn unpark(&self) {}
        }

        Arc::new(Foo)
    }

    pub fn mock_client_channelled() -> (mpsc::Receiver<TinyCommand>, Client<TinyMsg, TinyMsg>) {
        let (tx, rx) = mpsc::channel(1);
        (rx, mock_client(tx))
    }

    pub fn mock_client(tx: mpsc::Sender<TinyCommand>) -> Client<TinyMsg, TinyMsg> {
        Client::new(None, tx)
    }
}
