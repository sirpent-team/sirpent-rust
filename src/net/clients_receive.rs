use std::io;
use std::time::Duration;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

use futures::{BoxFuture, Future, Stream, Sink, Poll, Async, AsyncSink};
use futures::sync::{mpsc, oneshot};
use tokio_timer::{Timer, Sleep};

use protocol::Msg;
use net::{other, other_labelled};

pub struct ClientsReceive<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientCommand<I>> + Send + 'static,
          D: Future<Item = (Msg, C), Error = ()> + 'static
{
    reads: HashMap<I, D>,
    entries_msgs: Option<HashMap<I, Msg>>,
    entries_txs: Option<HashMap<I, C>>,
}

impl<I, C, D> ClientsReceive<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientCommand<I>> + Send + 'static,
          D: Future<Item = (Msg, C), Error = ()> + 'static
{
    pub fn new(mut clients: HashMap<I, C>) -> ClientsReceive<I, C, Box<(Msg, C), ()>> {
        ClientsReceive {
            reads: clients.drain()
                .map(|(id, command_tx)| {
                    let (msg_relay_tx, msg_relay_rx) = oneshot::channel();
                    let msg_relay_cmd = ClientCommand::Receive(msg_relay_tx);
                    (id,
                     command_tx.send(msg_relay_cmd)
                         .map_err(|_| ())
                         .and_then(|command_tx| {
                             msg_relay_rx.map(|msg| (msg, command_tx)).map_err(|_| ())
                         })
                         .boxed())
                })
                .collect(),
            entries_msgs: Some(HashMap::new()),
            entries_txs: Some(HashMap::new()),
        }
    }

    pub fn entries(&mut self) -> (HashMap<I, Msg>, HashMap<I, C>) {
        (self.entries_msgs.take().unwrap(), self.entries_txs.take().unwrap())
    }
}

impl<I, C, D> Future for ClientsReceive<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientCommand<I>> + Send + 'static,
          D: Future<Item = (Msg, C), Error = ()> + 'static
{
    type Item = (HashMap<I, Msg>, HashMap<I, C>);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut remove = vec![];
        for (i, mut read) in self.reads.iter_mut() {
            match read.poll() {
                Ok(Async::Ready((msg, command_tx))) => {
                    self.entries_msgs.as_mut().unwrap().insert(i.clone(), msg);
                    self.entries_txs.as_mut().unwrap().insert(i.clone(), command_tx);
                    remove.push(i.clone());
                }
                Ok(Async::NotReady) => {}
                Err(_) => remove.push(i.clone()),
            };
        }
        for i in remove {
            self.reads.remove(&i);
        }

        if self.reads.is_empty() {
            Ok(Async::Ready(self.entries()))
        } else {
            Ok(Async::NotReady)
        }
    }
}
