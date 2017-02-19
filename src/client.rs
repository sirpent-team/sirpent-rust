use std::io;
use std::time::Duration;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

use futures::{BoxFuture, Future, Stream, Sink, Poll, Async, AsyncSink};
use futures::sync::{mpsc, oneshot};
use tokio_timer::{Timer, Sleep};

use protocol::Msg;
use net::{other, other_labelled};

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum ClientKind {
    #[serde(rename = "player")]
    Player,
    #[serde(rename = "spectator")]
    Spectator,
}

// Notably missing:
// - CancelRx can be safely performed by dropping the `oneshot::Receiver<Msg>`.
// Notably included:
// - Close could be safely performed by dropping the `CmdSink` but this problematically
//   relies on access to all the producers.
pub enum Cmd
{
    Transmit(Msg),
    ReceiveInto(oneshot::Sender<Msg>),
    Close,
}

pub struct Client<Id, ClientMsgSink, ClientMsgStream, ServerCmdStream>
    where Id: Eq + Hash + Clone,
          ClientMsgSink: Sink<SinkItem = Msg, SinkError = io::Error> + 'static,
          ClientMsgStream: Stream<Item = Msg, Error = io::Error> + 'static,
          ServerCmdStream: Stream<Item = Cmd, Error = ()> + 'static
{
    pub client_id: I,
    client_tx: S,
    client_rx: T,
    msg_tx_queue: VecDeque<Msg>,
    msg_rx_queue: VecDeque<Msg>,
    command_rx: C,
    msg_relay_tx_queue: VecDeque<oneshot::Sender<Msg>>,
    queue_limit: Option<usize>,
}

impl<I, S, T> Client<I, S, T, mpsc::Receiver<Cmd>>
    where I: Eq + Hash + Clone,
          S: Sink<SinkItem = Msg, SinkError = io::Error> + 'static,
          T: Stream<Item = Msg, Error = io::Error> + 'static
{
    pub fn bounded(client_id: I,
                   client_tx: S,
                   client_rx: T,
                   queue_limit: usize)
                   -> (Client<I, S, T, mpsc::Receiver<Cmd>>,
                       mpsc::Sender<Cmd>) {
        let (command_tx, command_rx) = mpsc::channel(queue_limit);
        (Client {
             client_id: client_id,
             client_tx: client_tx,
             client_rx: client_rx,
             msg_tx_queue: VecDeque::with_capacity(queue_limit),
             msg_rx_queue: VecDeque::with_capacity(queue_limit),
             command_rx: command_rx,
             msg_relay_tx_queue: VecDeque::new(),
             queue_limit: Some(queue_limit),
         },
         command_tx)
    }
}

impl<I, S, T> Client<I, S, T, mpsc::UnboundedReceiver<Cmd>>
    where I: Eq + Hash + Clone,
          S: Sink<SinkItem = Msg, SinkError = io::Error> + 'static,
          T: Stream<Item = Msg, Error = io::Error> + 'static
{
    pub fn unbounded(client_id: I,
                     client_tx: S,
                     client_rx: T)
                     -> (Client<I, S, T, mpsc::UnboundedReceiver<Cmd>>,
                         mpsc::UnboundedSender<Cmd>) {
        let (command_tx, command_rx) = mpsc::unbounded();
        (Client {
             client_id: client_id,
             client_tx: client_tx,
             client_rx: client_rx,
             msg_tx_queue: VecDeque::new(),
             msg_rx_queue: VecDeque::new(),
             command_rx: command_rx,
             msg_relay_tx_queue: VecDeque::new(),
             queue_limit: None,
         },
         command_tx)
    }

    pub fn client_id(&self) -> I {
        self.client_id.clone()
    }
}

impl<I, S, T, C> Future for Client<I, S, T, C>
    where I: Eq + Hash + Clone,
          S: Sink<SinkItem = Msg, SinkError = io::Error> + 'static,
          T: Stream<Item = Msg, Error = io::Error> + 'static,
          C: Stream<Item = Cmd, Error = ()> + 'static
{
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {
        // First check for anything being instructed.
        // This is first because it provides possible messages to send and possible places
        // to send messages - both needed later.
        match self.command_rx.poll() {
            Ok(Async::Ready(Some(command))) => {
                match command {
                    // Queue a message for transmission.
                    ClientCommand::Transmit(msg_tx) => {
                        if let Some(queue_limit) = self.queue_limit {
                            if self.msg_tx_queue.len() >= queue_limit {
                                return Err(other_labelled("Tried to exceed msg tx queue \
                                                           capacity."));
                            }
                        }
                        self.msg_tx_queue.push_back(msg_tx)
                    }
                    // Queue a oneshot to relay a message received from the client.
                    ClientCommand::Receive(msg_relay_tx) => {
                        if let Some(queue_limit) = self.queue_limit {
                            if self.msg_relay_tx_queue.len() >= queue_limit {
                                return Err(other_labelled("Tried to exceed msg relay tx queue \
                                                           capacity."));
                            }
                        }
                        self.msg_relay_tx_queue.push_back(msg_relay_tx)
                    }
                    // Send the client id to the oneshot.
                    ClientCommand::GetId(id_relay_tx) => {
                        id_relay_tx.complete(self.client_id.clone());
                    }
                }
            }
            Ok(Async::Ready(None)) => return Err(broken_pipe()),
            Err(()) => unreachable!(),
            _ => {}
        };

        // Second send messages to the client until the sender has to pause.
        while !self.msg_tx_queue.is_empty() {
            // Keep queueing items until the buffer gets full.
            while !self.msg_tx_queue.is_empty() {
                let msg_tx = self.msg_tx_queue[0].clone();
                match self.client_tx.start_send(msg_tx) {
                    // Only deque the item if it was started sending successfully.
                    Ok(AsyncSink::Ready) => {
                        self.msg_tx_queue.pop_front();
                    }
                    // Go flush the loop if the sender's internal buffer is full.
                    Ok(AsyncSink::NotReady(_)) => break,
                    Err(e) => return Err(e.into()),
                };
            }
            // Start flushing the sender's internal buffer.
            match self.client_tx.poll_complete() {
                Ok(Async::Ready(())) => {}
                Ok(Async::NotReady) => {}
                Err(e) => return Err(e.into()),
            };
        }

        // Third see if there's anything to read from the client.
        match self.client_rx.poll() {
            Ok(Async::Ready(Some(msg_rx))) => {
                if let Some(queue_limit) = self.queue_limit {
                    if self.msg_rx_queue.len() >= queue_limit {
                        return Err(other_labelled("Tried to exceed msg rx queue capacity."));
                    }
                }
                self.msg_rx_queue.push_back(msg_rx)
            }
            Ok(Async::Ready(None)) => return Err(broken_pipe()),
            Err(e) => return Err(e.into()),
            _ => {}
        };

        // Fourth see if we can forward any messages. We need a queued received message
        // *and* a queued oneshot to send it to.
        // N.B. Oneshot completes immediately with no need to keep polling.
        if !self.msg_rx_queue.is_empty() && !self.msg_relay_tx_queue.is_empty() {
            let msg_rx = self.msg_rx_queue.pop_front().unwrap();
            let relay_tx = self.msg_relay_tx_queue.pop_front().unwrap();
            relay_tx.complete(msg_rx);
        };

        Ok(Async::NotReady)
    }
}

impl<I, S, T, C> Drop for Client<I, S, T, C>
    where I: Eq + Hash + Clone,
          S: Sink<SinkItem = Msg, SinkError = io::Error> + 'static,
          T: Stream<Item = Msg, Error = io::Error> + 'static,
          C: Stream<Item = Cmd, Error = ()> + 'static
{
    fn drop(&mut self) {
        // Generally `Drop` will only occur when the non-client channels are dropped, so
        // this just ensures all messages reach the client.
        // @TODO: There *has* to be a better way to do this! Does `Wait` work here?
        while !self.msg_tx_queue.is_empty() {
            // Keep queueing items until the buffer gets full.
            while !self.msg_tx_queue.is_empty() {
                let msg_tx = self.msg_tx_queue[0].clone();
                match self.client_tx.start_send(msg_tx) {
                    // Only deque the item if it was started sending successfully.
                    Ok(AsyncSink::Ready) => {
                        self.msg_tx_queue.pop_front();
                    }
                    // Go flush the loop if the sender's internal buffer is full.
                    Ok(AsyncSink::NotReady(_)) => break,
                    Err(_) => return,
                };
            }
            // Start flushing the sender's internal buffer.
            match self.client_tx.poll_complete() {
                Ok(Async::Ready(())) => {}
                Ok(Async::NotReady) => {}
                Err(_) => return,
            };
        }
    }
}

fn broken_pipe() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "Broken channel.")
}
