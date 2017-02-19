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
    pub fn new(mut clients: HashMap<I, C>) -> ClientsReceive<I, C, BoxFuture<(Msg, C), ()>> {
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
        // Poll activities related to the
        while !self.commanding_queue.is_empty() {
            //
            let (client_id, command_tx) = self.commanding_queue.pop_front();

            // Create a oneshot channel for the received message to be passed back to us on.
            let (msg_relay_tx, msg_relay_rx) = oneshot::channel();
            // We transmit this oneshot's tx along the channel to `Client` and then wait for
            // a reply from the oneshot's rx.
            // This (perhaps surprisingly) delivers nicer code.
            let client_cmd = ClientCommand::Receive(msg_relay_tx);

            // Try to send this command into the appropriate `Sink`.
            match command_tx.as_mut().unwrap().start_send(client_cmd) {
                // If the command was sent successfully, try sending another.
                Ok(AsyncSink::Ready) => continue,
                // If the command could not be sent, requeue it and don't try again right now.
                Ok(AsyncSink::NotReady(client_cmd)) => {
                    self.command_queue.push_front((client_id, client_cmd));
                    break
                },
                // If sending the command errored, we can assume it is forever unable to accept
                // further items. To let it `Drop` we take it off self.
                Err(e) => {
                    self.command_tx.take().unwrap();
                    return Err(e)
                }
            }
        }
        // Try to make progress on flushing the command channel.
        match self.client_tx.poll_complete() {
            // All requests processed. No need to call this further.
            // @TODO: Once the `command_queue` is empty and this happens, stop calling this.
            Ok(Async::Ready(())) => {},
            // There are requests left to process.
            Ok(Async::NotReady) => {},
            // If polling the `Sink` errored, we can assume it is forever unable to accept
            // further items. To let it `Drop` we take it off self.
            Err(e) => {
                self.command_tx.take().unwrap();
                return Err(e)
            }
        }



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
