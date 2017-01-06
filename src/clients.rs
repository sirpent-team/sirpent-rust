use std::io;
use std::vec;
use std::iter::FromIterator;
use std::net::SocketAddr;
use std::time::Duration;
use std::marker::Send;
use std::collections::{HashSet, HashMap};
use std::collections::hash_map::{Keys, Drain};

use futures::{Future, BoxFuture, Stream, Sink};
use futures::stream::{SplitStream, SplitSink, futures_unordered};
use tokio_core::net::TcpStream;
use tokio_core::io::Io;

use net::*;
use grid::*;
use snake::*;
use state::*;
use protocol::*;

pub type BoxFutureNotSend<I, E> = Box<Future<Item = I, Error = E>>;

pub struct Client<S, T>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send + 'static,
          T: Stream<Item = Msg, Error = io::Error> + Send + 'static
{
    pub name: Option<String>,
    pub addr: Option<SocketAddr>,
    msg_tx: Option<S>,
    msg_rx: Option<T>,
}

impl<S, T> Client<S, T>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send + 'static,
          T: Stream<Item = Msg, Error = io::Error> + Send + 'static
{
    pub fn new(name: Option<String>,
               addr: Option<SocketAddr>,
               msg_tx: S,
               msg_rx: T)
               -> Client<S, T> {
        Client {
            name: name,
            addr: addr,
            msg_tx: Some(msg_tx),
            msg_rx: Some(msg_rx),
        }
    }

    pub fn from_incoming(stream: TcpStream,
                         addr: SocketAddr)
                         -> Client<SplitSink<MsgTransport>, SplitStream<MsgTransport>> {
        let msg_transport = stream.framed(MsgCodec);
        let (msg_tx, msg_rx) = msg_transport.split();
        Client::new(None, Some(addr), msg_tx, msg_rx)
    }

    fn with_new_msg_tx(mut self, msg_tx: S) -> Self {
        self.msg_tx = Some(msg_tx);
        return self;
    }

    fn with_new_msg_rx(mut self, msg_rx: T) -> Self {
        self.msg_rx = Some(msg_rx);
        return self;
    }

    fn send<M: TypedMsg>(mut self, typed_msg: M) -> BoxFuture<Self, (ProtocolError, Self)>
        where M: 'static
    {
        let msg = Msg::from_typed(typed_msg);
        self.msg_tx
            .take()
            .unwrap()
            .send(msg)
            .then(|result| {
                match result {
                    Ok(msg_tx) => Ok(self.with_new_msg_tx(msg_tx)),
                    Err(e) => Err((ProtocolError::from(e), self)),
                }
            })
            .boxed()
    }

    fn receive<M: TypedMsg>(mut self) -> BoxFuture<(M, Self), (ProtocolError, Self)>
        where M: 'static
    {
        self.msg_rx
            .take()
            .unwrap()
            .into_future()
            .map_err(|(e, msg_rx)| (ProtocolError::from(e), msg_rx))
            .and_then(|(maybe_msg, msg_rx)| {
                let msg = maybe_msg.ok_or(ProtocolError::NoMsgReceived);
                match msg.and_then(|msg| Msg::to_typed(msg)) {
                    Ok(typed_msg) => Ok((typed_msg, msg_rx)),
                    Err(e) => Err((e, msg_rx)),
                }
            })
            .then(|result| {
                match result {
                    Ok((typed_msg, msg_rx)) => Ok((typed_msg, self.with_new_msg_rx(msg_rx))),
                    Err((e, msg_rx)) => Err((e, self.with_new_msg_rx(msg_rx))),
                }
            })
            .boxed()
    }

    /// Tell the client our protocol version and expect them to send back a name to use.
    /// A Client will be included with the ProtocolError unless sending the VersionMsg failed.
    pub fn handshake(self) -> BoxFuture<(IdentifyMsg, Self), (ProtocolError, Self)> {
        self.send(VersionMsg::new())
            .and_then(|client| client.receive())
            .boxed()
    }

    pub fn welcome(mut self,
                   name: String,
                   grid: Grid,
                   timeout: Option<Duration>)
                   -> BoxFuture<Self, (ProtocolError, Self)> {
        self.name = Some(name.clone());
        self.send(WelcomeMsg {
                name: name,
                grid: grid,
                timeout: timeout,
            })
            .boxed()
    }

    pub fn new_game(self, game: GameState) -> BoxFuture<Self, (ProtocolError, Self)> {
        self.send(NewGameMsg { game: game }).boxed()
    }

    pub fn new_turn(self, turn: TurnState) -> BoxFuture<Self, (ProtocolError, Self)> {
        self.send(TurnMsg { turn: turn }).boxed()
    }

    pub fn ask_move(self) -> BoxFuture<(MoveMsg, Self), (ProtocolError, Self)> {
        self.receive().boxed()
    }

    pub fn die(self, cause_of_death: CauseOfDeath) -> BoxFuture<Self, (ProtocolError, Self)> {
        self.send(DiedMsg { cause_of_death: cause_of_death }).boxed()
    }

    pub fn win(self) -> BoxFuture<Self, (ProtocolError, Self)> {
        self.send(WonMsg {}).boxed()
    }

    pub fn end_game(self, turn: TurnState) -> BoxFuture<Self, (ProtocolError, Self)> {
        self.send(GameOverMsg { turn: turn }).boxed()
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

pub struct Clients<S, T>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send + 'static,
          T: Stream<Item = Msg, Error = io::Error> + Send + 'static
{
    clients: HashMap<String, Client<S, T>>,
    failures: HashMap<String, ProtocolError>,
}

impl<S, T> Clients<S, T>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send + 'static,
          T: Stream<Item = Msg, Error = io::Error> + Send + 'static
{
    /// Typically one creates Clients from an iterator rather than with new.
    pub fn new(clients: HashMap<String, Client<S, T>>,
               failures: HashMap<String, ProtocolError>)
               -> Self {
        Clients {
            clients: clients,
            failures: failures,
        }
    }

    pub fn names(&self) -> Keys<String, Client<S, T>> {
        self.clients.keys()
    }

    pub fn drain_failures(&mut self) -> Drain<String, ProtocolError> {
        self.failures.drain()
    }

    pub fn new_game(mut self, game: GameState) -> BoxFutureNotSend<Self, ()> {
        let futures =
            self.clients.drain().map(|(_, client)| client.new_game(game.clone())).collect();
        self.clients_dataless_future(futures)
    }

    pub fn new_turn(mut self, turn: TurnState) -> BoxFutureNotSend<Self, ()> {
        let futures =
            self.clients.drain().map(|(_, client)| client.new_turn(turn.clone())).collect();
        self.clients_dataless_future(futures)
    }

    pub fn ask_moves(mut self,
                     movers: &HashSet<String>)
                     -> BoxFutureNotSend<(HashMap<String, MoveMsg>, Self), ()> {
        let mut futures = Vec::new();
        for name in movers {
            match self.clients.remove(name) {
                Some(client) => {
                    futures.push(client.ask_move());
                }
                None => continue,
            }
        }
        self.clients_dataful_future(futures)
    }

    pub fn die(mut self, casualties: &HashMap<String, CauseOfDeath>) -> BoxFutureNotSend<Self, ()> {
        let mut futures = Vec::new();
        for (name, cause_of_death) in casualties {
            match self.clients.remove(name) {
                Some(client) => {
                    futures.push(client.die(cause_of_death.clone()));
                }
                None => continue,
            }
        }
        self.clients_dataless_future(futures)
    }

    pub fn win(mut self, winners: &HashSet<String>) -> BoxFutureNotSend<Self, ()> {
        let mut futures = Vec::new();
        for name in winners {
            match self.clients.remove(name) {
                Some(client) => {
                    futures.push(client.win());
                }
                None => continue,
            }
        }
        self.clients_dataless_future(futures)
    }

    pub fn end_game(mut self, turn: TurnState) -> BoxFutureNotSend<Self, ()> {
        let futures =
            self.clients.drain().map(|(_, client)| client.end_game(turn.clone())).collect();
        self.clients_dataless_future(futures)
    }

    // @TODO: Had a surprising Send requirement when trying to make futures IntoIterator.
    fn clients_dataless_future(self,
                               futures: Vec<BoxFuture<Client<S, T>,
                                                      (ProtocolError, Client<S, T>)>>)
                               -> BoxFutureNotSend<Self, ()> {
        // Run futures concurrently.
        // Collect Result<Client, (ProtocolError, Client)> iterator.
        let joined_future = futures_unordered(futures).collect_results();
        // Process each future's returned Result<Client, (ProtocolError, Client)>.
        let reconstruct_future = joined_future.map(move |client_results| {
            let mut clients = HashMap::new();
            let mut failures = HashMap::new();
            for client_result in client_results {
                // Retain successful clients.
                // Drop failed clients and retain their ProtocolError.
                // @TODO: Determine good approach to dropping clients.
                match client_result {
                    Ok(client) => {
                        clients.insert(client.name.clone().unwrap(), client);
                    }
                    Err((e, client)) => {
                        failures.insert(client.name.clone().unwrap(), e);
                    }
                }
            }
            // Return the updated Clients.
            Self::new(clients, failures)
        });
        return Box::new(reconstruct_future);
    }

    // @TODO: Had a surprising Send requirement when trying to make futures IntoIterator.
    fn clients_dataful_future<R>(self,
                                 futures: Vec<BoxFuture<(R, Client<S, T>),
                                                        (ProtocolError, Client<S, T>)>>)
                                 -> BoxFutureNotSend<(HashMap<String, R>, Self), ()>
        where R: 'static
    {
        // Run futures concurrently.
        // Collect Result<(M, Client), (ProtocolError, Client)> iterator.
        let joined_future = futures_unordered(futures).collect_results();
        // Process each future's returned Result<(M, Client), (ProtocolError, Client)>.
        let reconstruct_future = joined_future.map(move |client_results| {
            let mut clients = HashMap::new();
            let mut failures = HashMap::new();
            let mut returned = HashMap::new();
            for client_result in client_results {
                // Retain successful clients and record the message read.
                // Drop failed clients and retain their ProtocolError.
                // @TODO: Determine good approach to dropping clients.
                match client_result {
                    Ok((return_, client)) => {
                        returned.insert(client.name.clone().unwrap(), return_);
                        clients.insert(client.name.clone().unwrap(), client);
                    }
                    Err((e, client)) => {
                        failures.insert(client.name.clone().unwrap(), e);
                    }
                }
            }
            // Return the received messages and updated Clients.
            (returned, Self::new(clients, failures))
        });
        return Box::new(reconstruct_future);
    }

    // fn send_to_some<M: TypedMsg, F>(self, typed_msg: M, filter_fn: F) -> BoxFutureNotSend<Self, ()>
    // where F: FnMut(&Client<S, T>) -> bool,
    // M: 'static + Send
    // {
    // let (subset, rest): (Clients<S, T>, Clients<S, T>) = self.into_iter().partition(filter_fn);
    // let subset_send_future = subset.send_to_all(typed_msg);
    // let rejoin_future = subset_send_future.map(|subset| {
    // subset.into_iter().chain(rest.into_iter()).collect::<Clients<S, T>>()
    // });
    // return Box::new(rejoin_future);
    // }
    //
    // fn receive_from_some<M: TypedMsg, F>(self,
    // filter_fn: F)
    // -> BoxFutureNotSend<(HashMap<String, M>, Self), ()>
    // where F: FnMut(&Client<S, T>) -> bool,
    // M: 'static
    // {
    // let (subset, rest): (Clients<S, T>, Clients<S, T>) = self.into_iter().partition(filter_fn);
    // let subset_receive_future = subset.receive_from_all();
    // let rejoin_future = subset_receive_future.map(|(subset_typed_msgs, subset)| {
    // let rejoined = subset.into_iter().chain(rest.into_iter()).collect::<Clients<S, T>>();
    // (subset_typed_msgs, rejoined)
    // });
    // return Box::new(rejoin_future);
    // }
    //
}

impl<S, T> IntoIterator for Clients<S, T>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send + 'static,
          T: Stream<Item = Msg, Error = io::Error> + Send + 'static
{
    type Item = Client<S, T>;
    type IntoIter = vec::IntoIter<Client<S, T>>;

    fn into_iter(self) -> Self::IntoIter {
        // Consume into an iterator and drop the name key.
        // Dropping the name key loses no information (it's in client.name) and means that
        // we serialise and deserialise perfectly.
        // @TODO: Experiment with type signatures of implementing IntoIter/FromIter with and
        // without name.
        // @TODO: HashMap::values does not take ownership. Try rewrite this using IntoIter::map.
        let mut values = Vec::new();
        for (_, value) in self.clients {
            values.push(value);
        }
        return values.into_iter();
    }
}

impl<S, T> FromIterator<Client<S, T>> for Clients<S, T>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send + 'static,
          T: Stream<Item = Msg, Error = io::Error> + Send + 'static
{
    fn from_iter<I: IntoIterator<Item = Client<S, T>>>(iter: I) -> Self {
        Clients {
            clients: iter.into_iter()
                .map(|client| (client.name.clone().unwrap(), client))
                .collect(),
            failures: HashMap::new(),
        }
    }
}

impl<S, T> Default for Clients<S, T>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send + 'static,
          T: Stream<Item = Msg, Error = io::Error> + Send + 'static
{
    fn default() -> Clients<S, T> {
        Clients::new(HashMap::new(), HashMap::new())
    }
}

impl<S, T> Extend<Client<S, T>> for Clients<S, T>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send + 'static,
          T: Stream<Item = Msg, Error = io::Error> + Send + 'static
{
    fn extend<I: IntoIterator<Item = Client<S, T>>>(&mut self, iter: I) {
        for client in iter {
            self.clients.insert(client.name.clone().unwrap(), client);
        }
    }
}
