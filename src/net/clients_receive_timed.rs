use std::io;
use std::time::Duration;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

use futures::{BoxFuture, Future, Stream, Sink, Poll, Async, AsyncSink};
use futures::sync::{mpsc, oneshot};
use tokio_timer::{Timer, Sleep};

use protocol::Msg;
use net::{other, other_labelled};

pub struct ClientsTimedReceive<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientCommand<I>> + Send + 'static,
          D: Future<Item = (Msg, C), Error = ()> + 'static
{
    receive: Option<ClientsReceive<I, C, D>>,
    sleep: Sleep,
}

impl<I, C, D> ClientsTimedReceive<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientCommand<I>> + Send + 'static,
          D: Future<Item = (Msg, C), Error = ()> + 'static
{
    pub fn new(clients: HashMap<I, C>,
               timeout: Duration,
               timer: &Timer)
               -> ClientsTimedReceive<I, C, BoxFuture<(Msg, C), ()>> {
        ClientsTimedReceive {
            receive: Some(ClientsReceive::<I, C, D>::new(clients)),
            sleep: timer.sleep(timeout),
        }
    }

    pub fn single(id: I,
                  client: C,
                  timeout: Duration,
                  timer: &Timer)
                  -> ClientsTimedReceive<I, C, BoxFuture<(Msg, C), ()>> {
        let mut clients = HashMap::new();
        clients.insert(id, client);
        ClientsTimedReceive {
            receive: Some(ClientsReceive::<I, C, D>::new(clients)),
            sleep: timer.sleep(timeout),
        }
    }
}

impl<I, C, D> Future for ClientsTimedReceive<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientCommand<I>> + Send + 'static,
          D: Future<Item = (Msg, C), Error = ()> + 'static
{
    type Item = (HashMap<I, Msg>, HashMap<I, C>);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.sleep.poll() {
            // If the timeout has yet to be reached then poll receive.
            Ok(Async::NotReady) => self.receive.as_mut().unwrap().poll(),
            // If the timeout has been reached then return what entries we have.
            Ok(Async::Ready(_)) => Ok(Async::Ready(self.receive.take().unwrap().entries())),
            // If the timeout errored then return it as an `io::Error`.
            // @TODO: Also return what entries we have?
            Err(e) => Err(other(e)),
        }
    }
}
