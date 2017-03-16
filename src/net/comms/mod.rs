mod client;
mod room;
//mod relay;

pub use self::client::*;
pub use self::room::*;
//pub use self::relay::*;

use futures::{Future, Poll, Async, BoxFuture};
use futures::stream::Stream;
use uuid::Uuid;
use std::time::Duration;
use futures::sync::oneshot;
use std::mem;

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

/// A future which collects all of the outputs of a stream into a vector of Result<Item, Error>.
///
/// This future is created by the `Stream::collect_results` method.
#[must_use = "streams do nothing unless polled"]
pub struct CollectResults<S>
    where S: Stream
{
    stream: S,
    items: Vec<Result<S::Item, S::Error>>,
}

pub fn new<S>(s: S) -> CollectResults<S>
    where S: Stream
{
    CollectResults {
        stream: s,
        items: Vec::new(),
    }
}

impl<S: Stream> CollectResults<S> {
    fn finish(&mut self) -> Vec<Result<S::Item, S::Error>> {
        mem::replace(&mut self.items, Vec::new())
    }
}

impl<S> Future for CollectResults<S>
    where S: Stream
{
    type Item = Vec<Result<S::Item, S::Error>>;
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, ()> {
        loop {
            match self.stream.poll() {
                Ok(Async::Ready(Some(e))) => self.items.push(Ok(e)),
                Ok(Async::Ready(None)) => return Ok(Async::Ready(self.finish())),
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(e) => self.items.push(Err(e)),
            }
        }
    }
}
