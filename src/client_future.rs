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

pub enum ClientFutureCommand<I>
    where I: Eq + Hash
{
    Transmit(Msg),
    Receive(oneshot::Sender<Msg>),
    GetId(oneshot::Sender<I>),
}

pub struct ClientFuture<I, S, T, C>
    where I: Eq + Hash + Clone,
          S: Sink<SinkItem = Msg, SinkError = io::Error> + 'static,
          T: Stream<Item = Msg, Error = io::Error> + 'static,
          C: Stream<Item = ClientFutureCommand<I>, Error = ()> + 'static
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

impl<I, S, T> ClientFuture<I, S, T, mpsc::Receiver<ClientFutureCommand<I>>>
    where I: Eq + Hash + Clone,
          S: Sink<SinkItem = Msg, SinkError = io::Error> + 'static,
          T: Stream<Item = Msg, Error = io::Error> + 'static
{
    pub fn bounded(client_id: I,
                   client_tx: S,
                   client_rx: T,
                   queue_limit: usize)
                   -> (ClientFuture<I, S, T, mpsc::Receiver<ClientFutureCommand<I>>>,
                       mpsc::Sender<ClientFutureCommand<I>>) {
        let (command_tx, command_rx) = mpsc::channel(queue_limit);
        (ClientFuture {
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

impl<I, S, T> ClientFuture<I, S, T, mpsc::UnboundedReceiver<ClientFutureCommand<I>>>
    where I: Eq + Hash + Clone,
          S: Sink<SinkItem = Msg, SinkError = io::Error> + 'static,
          T: Stream<Item = Msg, Error = io::Error> + 'static
{
    pub fn unbounded(client_id: I,
                     client_tx: S,
                     client_rx: T)
                     -> (ClientFuture<I, S, T, mpsc::UnboundedReceiver<ClientFutureCommand<I>>>,
                         mpsc::UnboundedSender<ClientFutureCommand<I>>) {
        let (command_tx, command_rx) = mpsc::unbounded();
        (ClientFuture {
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

impl<I, S, T, C> Future for ClientFuture<I, S, T, C>
    where I: Eq + Hash + Clone,
          S: Sink<SinkItem = Msg, SinkError = io::Error> + 'static,
          T: Stream<Item = Msg, Error = io::Error> + 'static,
          C: Stream<Item = ClientFutureCommand<I>, Error = ()> + 'static
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
                    ClientFutureCommand::Transmit(msg_tx) => {
                        if let Some(queue_limit) = self.queue_limit {
                            if self.msg_tx_queue.len() >= queue_limit {
                                return Err(other_labelled("Tried to exceed msg tx queue \
                                                           capacity."));
                            }
                        }
                        self.msg_tx_queue.push_back(msg_tx)
                    }
                    // Queue a oneshot to relay a message received from the client.
                    ClientFutureCommand::Receive(msg_relay_tx) => {
                        if let Some(queue_limit) = self.queue_limit {
                            if self.msg_relay_tx_queue.len() >= queue_limit {
                                return Err(other_labelled("Tried to exceed msg relay tx queue \
                                                           capacity."));
                            }
                        }
                        self.msg_relay_tx_queue.push_back(msg_relay_tx)
                    }
                    // Send the client id to the oneshot.
                    ClientFutureCommand::GetId(id_relay_tx) => {
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

impl<I, S, T, C> Drop for ClientFuture<I, S, T, C>
    where I: Eq + Hash + Clone,
          S: Sink<SinkItem = Msg, SinkError = io::Error> + 'static,
          T: Stream<Item = Msg, Error = io::Error> + 'static,
          C: Stream<Item = ClientFutureCommand<I>, Error = ()> + 'static
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

pub struct ClientsReceive<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientFutureCommand<I>> + Send + 'static,
          D: Future<Item = (Msg, C), Error = ()> + 'static
{
    reads: HashMap<I, D>,
    entries_msgs: Option<HashMap<I, Msg>>,
    entries_txs: Option<HashMap<I, C>>,
}

impl<I, C, D> ClientsReceive<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientFutureCommand<I>> + Send + 'static,
          D: Future<Item = (Msg, C), Error = ()> + 'static
{
    pub fn new(mut clients: HashMap<I, C>) -> ClientsReceive<I, C, BoxFuture<(Msg, C), ()>> {
        ClientsReceive {
            reads: clients.drain()
                .map(|(id, command_tx)| {
                    let (msg_relay_tx, msg_relay_rx) = oneshot::channel();
                    let msg_relay_cmd = ClientFutureCommand::Receive(msg_relay_tx);
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
          C: Sink<SinkItem = ClientFutureCommand<I>> + Send + 'static,
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

pub struct ClientsTimedReceive<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientFutureCommand<I>> + Send + 'static,
          D: Future<Item = (Msg, C), Error = ()> + 'static
{
    receive: Option<ClientsReceive<I, C, D>>,
    sleep: Sleep,
}

impl<I, C, D> ClientsTimedReceive<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientFutureCommand<I>> + Send + 'static,
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
          C: Sink<SinkItem = ClientFutureCommand<I>> + Send + 'static,
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

pub struct ClientsSend<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientFutureCommand<I>> + Send + 'static,
          D: Future<Item = C, Error = ()> + 'static
{
    sends: HashMap<I, D>,
    entries_txs: Option<HashMap<I, C>>,
}

impl<I, C, D> ClientsSend<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientFutureCommand<I>> + Send + 'static,
          D: Future<Item = C, Error = ()> + 'static
{
    pub fn new(mut clients: HashMap<I, C>, msg: Msg) -> ClientsSend<I, C, BoxFuture<C, ()>> {
        let sends = clients.drain()
            .map(|(id, command_tx)| {
                let msg_relay_cmd = ClientFutureCommand::Transmit(msg.clone());
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
          C: Sink<SinkItem = ClientFutureCommand<I>> + Send + 'static,
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
