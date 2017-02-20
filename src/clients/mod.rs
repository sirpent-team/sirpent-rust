use std::io;
use std::sync::Arc;
use std::hash::Hash;
use std::collections::VecDeque;
use futures::{Future, Stream, Sink, Poll, Async, AsyncSink};
use futures::sync::{mpsc, oneshot};

use net::*;
use errors::*;

pub mod command;
pub mod receive;
pub mod receive_timeout;
pub mod transmit;

pub use self::command::*;
pub use self::receive::*;
pub use self::receive_timeout::*;
pub use self::transmit::*;

#[derive(PartialEq, Clone, Copy, Debug, Serialize, Deserialize)]
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
#[derive(Clone)]
pub enum Cmd {
    Transmit(Msg),
    ReceiveInto(RaceableOneshotSender),
    DiscardReceiveBuffer,
    Close,
}

/// @TODO: This isn't actually raceable. Rather it only works if there's one
/// reference.
#[derive(Clone)]
pub struct RaceableOneshotSender {
    inner: Option<Arc<oneshot::Sender<Msg>>>,
}

impl RaceableOneshotSender {
    pub fn new(oneshot_tx: oneshot::Sender<Msg>) -> RaceableOneshotSender {
        RaceableOneshotSender { inner: Some(Arc::new(oneshot_tx)) }
    }

    pub fn complete(&mut self, msg: Msg) -> bool {
        if let Some(arc) = self.inner.take() {
            if let Ok(sender) = Arc::try_unwrap(arc) {
                sender.complete(msg);
                return true;
            }
        }
        false
    }

    pub fn poll_cancel(&mut self) -> Poll<(), ()> {
        // @TODO:
        if let Some(arc) = self.inner.take() {
            if let Ok(mut sender) = Arc::try_unwrap(arc) {
                let ret = sender.poll_cancel();
                self.inner = Some(Arc::new(sender));
                return ret;
            }
        }
        // If the oneshot has been used then act as if the Receiver was dropped.
        Ok(Async::Ready(()))
    }
}

pub struct Client<Id, ClientMsgSink, ClientMsgStream, ServerCmdStream>
    where Id: Eq + Hash + Clone,
          ClientMsgSink: Sink<SinkItem = Msg, SinkError = Error> + 'static,
          ClientMsgStream: Stream<Item = Msg, Error = Error> + 'static,
          ServerCmdStream: Stream<Item = Cmd, Error = ()> + 'static
{
    pub client_id: Id,
    client_tx: ClientMsgSink,
    client_rx: ClientMsgStream,
    msg_tx_queue: VecDeque<Msg>,
    msg_rx_queue: VecDeque<Msg>,
    command_rx: ServerCmdStream,
    msg_relay_tx_queue: VecDeque<RaceableOneshotSender>,
    queue_limit: Option<usize>,
}

impl<Id, ClientMsgSink, ClientMsgStream> Client<Id,
                                                ClientMsgSink,
                                                ClientMsgStream,
                                                mpsc::Receiver<Cmd>>
    where Id: Eq + Hash + Clone,
          ClientMsgSink: Sink<SinkItem = Msg, SinkError = Error> + 'static,
          ClientMsgStream: Stream<Item = Msg, Error = Error> + 'static
{
    pub fn bounded
        (client_id: Id,
         client_tx: ClientMsgSink,
         client_rx: ClientMsgStream,
         queue_limit: usize)
         -> (Client<Id, ClientMsgSink, ClientMsgStream, mpsc::Receiver<Cmd>>, mpsc::Sender<Cmd>) {
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

impl<Id, ClientMsgSink, ClientMsgStream> Client<Id,
                                                ClientMsgSink,
                                                ClientMsgStream,
                                                mpsc::UnboundedReceiver<Cmd>>
    where Id: Eq + Hash + Clone,
          ClientMsgSink: Sink<SinkItem = Msg, SinkError = Error> + 'static,
          ClientMsgStream: Stream<Item = Msg, Error = Error> + 'static
{
    pub fn unbounded(client_id: Id,
                     client_tx: ClientMsgSink,
                     client_rx: ClientMsgStream)
                     -> (Client<Id, ClientMsgSink, ClientMsgStream, mpsc::UnboundedReceiver<Cmd>>,
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

    pub fn client_id(&self) -> Id {
        self.client_id.clone()
    }
}

impl<Id, ClientMsgSink, ClientMsgStream, ServerCmdStream> Future
    for Client<Id, ClientMsgSink, ClientMsgStream, ServerCmdStream>
    where Id: Eq + Hash + Clone,
          ClientMsgSink: Sink<SinkItem = Msg, SinkError = Error> + 'static,
          ClientMsgStream: Stream<Item = Msg, Error = Error> + 'static,
          ServerCmdStream: Stream<Item = Cmd, Error = ()> + 'static
{
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        // First check for anything being instructed.
        // This is first because it provides possible messages to send and possible places
        // to send messages - both needed later.
        loop {
            match self.command_rx.poll() {
                Ok(Async::Ready(Some(command))) => {
                    match command {
                        // Queue a message for transmission.
                        Cmd::Transmit(msg_tx) => {
                            if let Some(queue_limit) = self.queue_limit {
                                if self.msg_tx_queue.len() >= queue_limit {
                                    bail!("Tried to exceed msg tx queue \
                                                               capacity.");
                                }
                            }
                            self.msg_tx_queue.push_back(msg_tx)
                        }
                        // Queue a oneshot to relay a message received from the client.
                        Cmd::ReceiveInto(msg_relay_tx) => {
                            if let Some(queue_limit) = self.queue_limit {
                                if self.msg_relay_tx_queue.len() >= queue_limit {
                                    bail!("Tried to exceed msg relay tx \
                                                               queue capacity.");
                                }
                            }
                            self.msg_relay_tx_queue.push_back(msg_relay_tx)
                        }
                        Cmd::DiscardReceiveBuffer => {
                            self.msg_rx_queue.drain(..);
                        }
                        Cmd::Close => {
                            // @TODO: Implement.
                            unimplemented!();
                        }
                    };
                    continue;
                }
                Ok(Async::Ready(None)) => bail!(broken_pipe()),
                Err(()) => unreachable!(),
                Ok(Async::NotReady) => break,
            }
        }

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
                    Err(e) => bail!(e),
                };
            }
            // Start flushing the sender's internal buffer.
            match self.client_tx.poll_complete() {
                Ok(Async::Ready(())) | Ok(Async::NotReady) => {}
                Err(e) => bail!(e),
            };
        }
        match self.client_tx.poll_complete() {
            Ok(Async::Ready(())) | Ok(Async::NotReady) => {}
            Err(e) => bail!(e),
        };

        // Third see if there's anything to read from the client.
        match self.client_rx.poll() {
            Ok(Async::Ready(Some(msg_rx))) => {
                if let Some(queue_limit) = self.queue_limit {
                    if self.msg_rx_queue.len() >= queue_limit {
                        bail!("Tried to exceed msg rx queue capacity.");
                    }
                }
                self.msg_rx_queue.push_back(msg_rx)
            }
            Ok(Async::Ready(None)) => bail!(broken_pipe()),
            Err(e) => bail!(e),
            _ => {}
        };

        // Fourth see if we can forward any messages. We need a queued received message
        // *and* a queued oneshot to send it to.
        // N.B. Oneshot completes immediately with no need to keep polling.
        // N.B. The while loop is to ensure we can keep ahead of incoming messages.
        while !self.msg_rx_queue.is_empty() && !self.msg_relay_tx_queue.is_empty() {
            let mut relay_tx = self.msg_relay_tx_queue.pop_front().unwrap();
            // Check if the oneshot::Sender still has an associated non-dropped Receiver.
            match relay_tx.poll_cancel() {
                // Drop this Sender if the Receiver is dropped.
                Ok(Async::Ready(_)) | Err(_) => continue,
                // If there is a corresponding Receiver still, then send the received message
                // at the head of the queue.
                Ok(Async::NotReady) => {
                  let msg_rx = self.msg_rx_queue.pop_front().unwrap();
                  relay_tx.complete(msg_rx);
                }
            }
        }

        Ok(Async::NotReady)
    }
}

impl<I, S, T, C> Drop for Client<I, S, T, C>
    where I: Eq + Hash + Clone,
          S: Sink<SinkItem = Msg, SinkError = Error> + 'static,
          T: Stream<Item = Msg, Error = Error> + 'static,
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
                Ok(Async::Ready(())) | Ok(Async::NotReady) => {}
                Err(_) => return,
            };
        }
    }
}

fn broken_pipe() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "Broken channel.")
}
