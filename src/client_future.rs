use std::io;
use std::net::SocketAddr;
use std::time::Duration;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

use futures::{BoxFuture, Future, Stream, Sink, Poll, Async};
use futures::sync::{mpsc, oneshot};
use tokio_timer::{Timer, Sleep};

use protocol::Msg;
use net::{other_labelled, other};

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum ClientKind {
    #[serde(rename = "player")]
    Player,
    #[serde(rename = "spectator")]
    Spectator,
}

// pub type BoxedFuture<I, E> = Box<Future<Item = I, Error = E>>;

// pub struct Clients {
//     sending: HashMap<ClientID, InFlight<Response<Msg, io::Error>>>,
//     tx_queue: HashMap<ClientID, VecDeque<Msg>>,
//     rx_queue: HashMap<ClientID, VecDeque<Msg>>
// }

// impl Clients {
//     pub fn send(ids: Vec<ClientID>, msg: Msg) {
//         for id in ids {
//             self.tx_queue[ids] = msg;
//         }
//     }

//     pub fn receive(ids: Vec<ClientID>) -> Vec<Option<Msg>> {
//         ids.map(|id| self.rx_queue.get(id)).collect()
//     }
// }

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

impl<I, S, T, C> ClientFuture<I, S, T, C>
    where I: Eq + Hash + Clone,
          S: Sink<SinkItem = Msg, SinkError = io::Error> + 'static,
          T: Stream<Item = Msg, Error = io::Error> + 'static,
          C: Stream<Item = ClientFutureCommand<I>, Error = ()> + 'static
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
                        id_relay_tx.complete(self.client_id());
                    }
                }
            }
            Ok(Async::Ready(None)) => return Err(broken_pipe()),
            Err(()) => unreachable!(),
            _ => {}
        };

        // Second continue sending messages to the client. If it is ready for a new message
        // then send one.
        match self.client_tx.poll_complete() {
            Ok(Async::Ready(_)) => {
                if let Some(msg_tx) = self.msg_tx_queue.pop_front() {
                    match self.client_tx.start_send(msg_tx) {
                        Err(e) => return Err(e.into()),
                        // @TODO: needs Ok(Async::Ready(None)) handling?
                        _ => {}
                    };
                }
            }
            Err(e) => return Err(e.into()),
            _ => {}
        };

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

fn broken_pipe() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "Broken channel.")
}

pub struct Receive<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientFutureCommand<I>, SinkError = ()> + Send + 'static,
          D: Future<Item = (Msg, C), Error = ()> + 'static
{
    reads: HashMap<I, D>,
    entries: Option<HashMap<I, (Msg, C)>>,
}

impl<I, C, D> Receive<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientFutureCommand<I>, SinkError = ()> + Send + 'static,
          D: Future<Item = (Msg, C), Error = ()> + 'static
{
    pub fn new(mut clients: HashMap<I, C>) -> Receive<I, C, BoxFuture<(Msg, C), ()>> {
        Receive {
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
            entries: Some(HashMap::new()),
        }
    }

    pub fn entries(mut self) -> HashMap<I, (Msg, C)> {
        self.entries.take().unwrap()
    }
}

impl<I, C, D> Future for Receive<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientFutureCommand<I>, SinkError = ()> + Send + 'static,
          D: Future<Item = (Msg, C), Error = ()> + 'static
{
    type Item = HashMap<I, (Msg, C)>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut remove = vec![];
        for (i, mut read) in self.reads.iter_mut() {
            match read.poll() {
                Ok(Async::Ready((msg, command_tx))) => {
                    self.entries.as_mut().unwrap().insert(i.clone(), (msg, command_tx));
                    remove.push(i.clone());
                }
                Ok(Async::NotReady) => {}
                Err(_) => return Err(other_labelled("Unspecified error.")),
            }
        }
        for i in remove {
            self.reads.remove(&i);
        }

        if self.reads.is_empty() {
            Ok(Async::Ready(self.entries.take().unwrap()))
        } else {
            Ok(Async::NotReady)
        }
    }
}

pub struct TimedReceive<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientFutureCommand<I>, SinkError = ()> + Send + 'static,
          D: Future<Item = (Msg, C), Error = ()> + 'static
{
    receive: Option<Receive<I, C, D>>,
    sleep: Sleep,
}

impl<I, C, D> TimedReceive<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientFutureCommand<I>, SinkError = ()> + Send + 'static,
          D: Future<Item = (Msg, C), Error = ()> + 'static
{
    pub fn new(clients: HashMap<I, C>,
               timeout: Duration,
               timer: &Timer)
               -> TimedReceive<I, C, BoxFuture<(Msg, C), ()>> {
        TimedReceive {
            receive: Some(Receive::<I, C, D>::new(clients)),
            sleep: timer.sleep(timeout),
        }
    }
}

impl<I, C, D> Future for TimedReceive<I, C, D>
    where I: Eq + Hash + Clone + Send,
          C: Sink<SinkItem = ClientFutureCommand<I>, SinkError = ()> + Send + 'static,
          D: Future<Item = (Msg, C), Error = ()> + 'static
{
    type Item = HashMap<I, (Msg, C)>;
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

// Clients
// * Acting on multiple clients:
//   * `apply`: Execute a function on all contained clients at the same time, collecting the results.
//   * `apply_filtered`: Wrap executing a function on a subset of clients without discard the other clients.
// * Acting on single clients:
//   * These could be `~O(1)` but if implemented using `apply_filtered` would take `O(n)` time.
//   * For now I'll use `apply_filtered`. If the number of clients supported climbs into the dozens then the cost might be worth it.
//   * `apply_named(name: Option<String>)`: Execute a function on a particularly-named client?
//   * `apply_addressed(addr: Option<SocketAddr>)`: Execute a function on a particular client?
//
// pub struct Clients<S, T>
// where S: Sink<SinkItem = Msg, SinkError = io::Error> + 'static,
// T: Stream<Item = Msg, Error = io::Error> + 'static
// {
// clients: HashMap<String, Client<S, T>>,
// failures: HashMap<String, ProtocolError>,
// }
//
// impl<S, T> Clients<S, T>
// where S: Sink<SinkItem = Msg, SinkError = io::Error> + 'static,
// T: Stream<Item = Msg, Error = io::Error> + 'static
// {
// Typically one creates Clients from an iterator rather than with new.
// pub fn new(clients: HashMap<String, Client<S, T>>,
// failures: HashMap<String, ProtocolError>)
// -> Self {
// Clients {
// clients: clients,
// failures: failures,
// }
// }
//
// pub fn add_client(&mut self, client: Client<S, T>) {
// @TODO: Convert to returning ProtocolResult<()>;
// assert!(client.name.is_some());
// self.clients.insert(client.name.clone().unwrap(), client);
// }
//
// pub fn names(&self) -> Keys<String, Client<S, T>> {
// self.clients.keys()
// }
//
// pub fn drain_failures(&mut self) -> Drain<String, ProtocolError> {
// self.failures.drain()
// }
//
// pub fn new_game(mut self, game: GameState) -> BoxedFuture<Self, ()> {
// let futures = self.all_to_futures(|_, client| client.new_game(game.clone()));
// self.dataless_future(futures)
// }
//
// pub fn new_turn(mut self, turn: TurnState) -> BoxedFuture<Self, ()> {
// let futures = self.all_to_futures(|_, client| client.new_turn(turn.clone()));
// self.dataless_future(futures)
// }
//
// pub fn ask_moves(mut self,
// movers: &HashSet<String>)
// -> BoxedFuture<(HashMap<String, ProtocolResult<MoveMsg>>, Self), ()> {
// let futures = self.named_to_futures(movers, |_, client| client.ask_move());
// self.dataful_future(futures)
// }
//
// pub fn die(mut self, casualties: &HashMap<String, CauseOfDeath>) -> BoxedFuture<Self, ()> {
// let mut futures = Vec::new();
// for (name, cause_of_death) in casualties {
// match self.clients.remove(name) {
// Some(client) => {
// futures.push(client.die(cause_of_death.clone()));
// }
// None => continue,
// }
// }
// self.dataless_future(futures)
// }
//
// pub fn win(mut self, winners: &HashSet<String>) -> BoxedFuture<Self, ()> {
// let futures = self.named_to_futures(winners, |_, client| client.win());
// self.dataless_future(futures)
// }
//
// pub fn end_game(mut self, turn: TurnState) -> BoxedFuture<Self, ()> {
// let futures = self.all_to_futures(|_, client| client.end_game(turn.clone()));
// self.dataless_future(futures)
// }
//
// fn all_to_futures<F, A, B>(&mut self, client_to_future_fn: F) -> Vec<BoxedFuture<A, B>>
// where F: Fn(String, Client<S, T>) -> BoxedFuture<A, B>
// {
// self.clients.drain().map(|(name, client)| client_to_future_fn(name, client)).collect()
// }
//
// fn named_to_futures<F, A, B>(&mut self,
// names: &HashSet<String>,
// client_to_future_fn: F)
// -> Vec<BoxedFuture<A, B>>
// where F: Fn(String, Client<S, T>) -> BoxedFuture<A, B>
// {
// let mut futures = Vec::new();
// for name in names {
// match self.clients.remove(name) {
// Some(client) => {
// futures.push(client_to_future_fn(name.clone(), client));
// }
// None => continue,
// }
// }
// return futures;
// }
//
// @TODO: Had a surprising Send requirement when trying to make futures IntoIterator.
// fn dataless_future(mut self,
// futures: Vec<BoxedFuture<Client<S, T>, (ProtocolError, Client<S, T>)>>)
// -> BoxedFuture<Self, ()> {
// Run futures concurrently.
// Collect Result<Client, (ProtocolError, Client)> iterator.
// let joined_future = collect_results(futures_unordered(futures));
// Process each future's returned Result<Client, (ProtocolError, Client)>.
// let reconstruct_future = joined_future.map(move |client_results| {
// for client_result in client_results {
// Retain successful clients.
// Drop failed clients and retain their ProtocolError.
// @TODO: Determine good approach to dropping clients.
// match client_result {
// Ok(client) => {
// self.clients.insert(client.name.clone().unwrap(), client);
// }
// Err((e, client)) => {
// self.failures.insert(client.name.clone().unwrap(), e);
// }
// }
// }
// Return the updated Clients.
// Self::new(self.clients, self.failures)
// });
// return box reconstruct_future;
// }
//
// @TODO: Had a surprising Send requirement when trying to make futures IntoIterator.
// fn dataful_future<R>(mut self,
// futures: Vec<BoxedFuture<(R, Client<S, T>),
// (ProtocolError, Client<S, T>)>>)
// -> BoxedFuture<(HashMap<String, ProtocolResult<R>>, Self), ()>
// where R: Clone + Debug + 'static
// {
// Run futures concurrently.
// Collect Result<(M, Client), (ProtocolError, Client)> iterator.
// let joined_future = collect_results(futures_unordered(futures));
// Process each future's returned Result<(M, Client), (ProtocolError, Client)>.
// let reconstruct_future = joined_future.map(move |client_results| {
// let mut returned = HashMap::new();
// for client_result in client_results {
// Retain successful clients and record the message read.
// Drop failed clients and retain their ProtocolError.
// @TODO: Determine good approach to dropping clients.
// match client_result {
// Ok((return_, client)) => {
// returned.insert(client.name.clone().unwrap(), Ok(return_));
// self.clients.insert(client.name.clone().unwrap(), client);
// }
// Err((e, client)) => {
// returned.insert(client.name.clone().unwrap(), Err(e));
// self.failures.insert(client.name.clone().unwrap(), e.clone());
// }
// }
// }
// Return the received messages and updated Clients.
// (returned, Self::new(self.clients, self.failures))
// });
// return box reconstruct_future;
// }
// }
//
// impl<S, T> FromIterator<Client<S, T>> for Clients<S, T>
// where S: Sink<SinkItem = Msg, SinkError = io::Error> + 'static,
// T: Stream<Item = Msg, Error = io::Error> + 'static
// {
// fn from_iter<I: IntoIterator<Item = Client<S, T>>>(iter: I) -> Self {
// Clients {
// clients: iter.into_iter()
// .map(|client| (client.name.clone().unwrap(), client))
// .collect(),
// failures: HashMap::new(),
// }
// }
// }
//
// impl<S, T> IntoIterator for Clients<S, T>
// where S: Sink<SinkItem = Msg, SinkError = io::Error> + 'static,
// T: Stream<Item = Msg, Error = io::Error> + 'static
// {
// type Item = Client<S, T>;
// type IntoIter = vec::IntoIter<Client<S, T>>;
//
// fn into_iter(self) -> Self::IntoIter {
// Consume into an iterator and drop the name key.
// Dropping the name key loses no information (it's in client.name) and means that
// we serialise and deserialise perfectly.
// @TODO: Experiment with type signatures of implementing IntoIter/FromIter with and
// without name.
// @TODO: HashMap::values does not take ownership. Try rewrite this using IntoIter::map.
// let mut values = Vec::new();
// for (_, value) in self.clients {
// values.push(value);
// }
// return values.into_iter();
// }
// }
//
// These are disabled because the Iterators do not preserve failures.
// These make it easy to do certain ways to deconstructing and reconstructing Clients that
// would discard failures silently.
// @TODO: Have the option of preserving failures on Iterators. Use Iterator<Item=Result<...>>.
//
// impl<S, T> Default for Clients<S, T>
// where S: Sink<SinkItem = Msg, SinkError = io::Error> + 'static,
// T: Stream<Item = Msg, Error = io::Error> + 'static
// {
// fn default() -> Clients<S, T> {
// Clients::new(HashMap::new(), HashMap::new())
// }
// }
//
// impl<S, T> Extend<Client<S, T>> for Clients<S, T>
// where S: Sink<SinkItem = Msg, SinkError = io::Error> + 'static,
// T: Stream<Item = Msg, Error = io::Error> + 'static
// {
// fn extend<I: IntoIterator<Item = Client<S, T>>>(&mut self, iter: I) {
// for client in iter {
// self.clients.insert(client.name.clone().unwrap(), client);
// }
// }
// }
//
//
