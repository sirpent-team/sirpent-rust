use std::io;
use std::time::Duration;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

use futures::{BoxFuture, Future, Stream, Sink, Poll, Async, AsyncSink};
use futures::sync::{mpsc, oneshot};
use tokio_timer::{Timer, Sleep};

use protocol::Msg;
use net::{other, other_labelled};

pub struct ClientsSend<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientCommand<I>> + Send + 'static,
          D: Future<Item = C, Error = ()> + 'static
{
    sends: HashMap<I, D>,
    entries_txs: Option<HashMap<I, C>>,
}

impl<I, C, D> ClientsSend<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientCommand<I>> + Send + 'static,
          D: Future<Item = C, Error = ()> + 'static
{
    pub fn new(mut clients: HashMap<I, C>, msg: Msg) -> ClientsSend<I, C, BoxFuture<C, ()>> {
        let sends = clients.drain()
            .map(|(id, command_tx)| {
                let msg_relay_cmd = ClientCommand::Transmit(msg.clone());
                (id, command_tx.send(msg_relay_cmd).map_err(|_| ()).boxed())
            })
            .collect();

        ClientsSend {
            sends: sends,
            entries_txs: Some(HashMap::new()),
        }
    }

    pub fn entries(&mut self) -> HashMap<I, C> {
        self.entries_txs.take().unwrap()
    }
}

impl<I, C, D> Future for ClientsSend<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientCommand<I>> + Send + 'static,
          D: Future<Item = C, Error = ()> + 'static
{
    type Item = HashMap<I, C>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut remove = vec![];
        for (i, mut send) in self.sends.iter_mut() {
            match send.poll() {
                Ok(Async::Ready(command_tx)) => {
                    self.entries_txs.as_mut().unwrap().insert(i.clone(), command_tx);
                    remove.push(i.clone());
                }
                Ok(Async::NotReady) => {}
                Err(_) => remove.push(i.clone()),
            };
        }
        for i in remove {
            self.sends.remove(&i);
        }

        if self.sends.is_empty() {
            Ok(Async::Ready(self.entries()))
        } else {
            Ok(Async::NotReady)
        }
    }
}
