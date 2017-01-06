use std::io;
use std::iter::FromIterator;
use std::net::SocketAddr;
use std::time::Duration;
use std::marker::Send;
use std::collections::{HashSet, HashMap};

use futures::{future, Future, BoxFuture, Stream, Sink};
use futures::stream::{SplitStream, SplitSink, futures_unordered};
use tokio_core::net::TcpStream;
use tokio_core::io::Io;

use net::*;
use grid::*;
use snake::*;
use state::*;
use protocol::*;

pub type BoxFutureNotSend<I, E> = Box<Future<Item = I, Error = E>>;
pub type ClientErr<S, T> = (ProtocolError, Option<Client<S, T>>);

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

    fn send<M: TypedMsg>(mut self, typed_msg: M) -> BoxFuture<Self, (ProtocolError, Option<Self>)>
        where M: 'static
    {
        let msg = Msg::from_typed(typed_msg);
        self.msg_tx
            .take()
            .unwrap()
            .send(msg)
            .map_err(|e| (ProtocolError::from(e), None))
            .map(|msg_tx| self.with_new_msg_tx(msg_tx))
            .boxed()
    }

    fn receive<M: TypedMsg>(mut self) -> BoxFuture<(M, Self), (ProtocolError, Option<Self>)>
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
                    Err((e, msg_rx)) => Err((e, Some(self.with_new_msg_rx(msg_rx)))),
                }
            })
            .boxed()
    }

    /// Tell the client our protocol version and expect them to send back a name to use.
    /// A Client will be included with the ProtocolError unless sending the VersionMsg failed.
    pub fn handshake(self) -> BoxFuture<(IdentifyMsg, Self), (ProtocolError, Option<Self>)> {
        self.send(VersionMsg::new())
            .and_then(|client| client.receive())
            .boxed()
    }

    pub fn welcome(mut self,
                   name: String,
                   grid: Grid,
                   timeout: Option<Duration>)
                   -> BoxFuture<Self, (ProtocolError, Option<Self>)> {
        self.name = Some(name.clone());
        self.send(WelcomeMsg {
                name: name,
                grid: grid,
                timeout: timeout,
            })
            .boxed()
    }

    pub fn new_game(self, game: GameState) -> BoxFuture<Self, (ProtocolError, Option<Self>)> {
        self.send(NewGameMsg { game: game }).boxed()
    }

    pub fn new_turn(self, turn: TurnState) -> BoxFuture<Self, (ProtocolError, Option<Self>)> {
        self.send(TurnMsg { turn: turn }).boxed()
    }

    pub fn ask_move(self) -> BoxFuture<(MoveMsg, Self), (ProtocolError, Option<Self>)> {
        self.receive().boxed()
    }

    pub fn die(self,
               cause_of_death: CauseOfDeath)
               -> BoxFuture<Self, (ProtocolError, Option<Self>)> {
        self.send(DiedMsg { cause_of_death: cause_of_death }).boxed()
    }

    pub fn win(self) -> BoxFuture<Self, (ProtocolError, Option<Self>)> {
        self.send(WonMsg {}).boxed()
    }

    pub fn end_game(self, turn: TurnState) -> BoxFuture<Self, (ProtocolError, Option<Self>)> {
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
    ok_clients: Vec<Client<S, T>>,
    err_clients: Vec<ClientErr<S, T>>,
}

impl<S, T> Clients<S, T>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send + 'static,
          T: Stream<Item = Msg, Error = io::Error> + Send + 'static
{
    pub fn ok_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for ok_client in self.ok_clients.iter() {
            names.push(ok_client.name.clone().unwrap());
        }
        return names;
    }

    fn new_from_map<F>(mut self, client_to_future_fn: F) -> BoxFutureNotSend<Clients<S, T>, ()>
        where F: FnMut(Client<S, T>) -> BoxFuture<Client<S, T>, ClientErr<S, T>>
    {
        let mut err_clients = self.err_clients;
        let futures = self.ok_clients.drain(..).map(client_to_future_fn);
        Box::new(futures_unordered(futures)
            .collect_results()
            .map(move |items| {
                let mut ok_clients = Vec::new();
                for item in items {
                    match item {
                        Ok(o) => ok_clients.push(o),
                        Err(e) => err_clients.push(e),
                    };
                }
                Clients {
                    ok_clients: ok_clients,
                    err_clients: err_clients,
                }
            }))
    }

    fn new_from_map_receive<F, M>
        (mut self,
         client_to_future_fn: F)
         -> BoxFutureNotSend<(HashMap<String, Option<M>>, Clients<S, T>), ()>
        where F: FnMut(Client<S, T>) -> BoxFuture<(Option<M>, Client<S, T>), ClientErr<S, T>>,
              M: 'static
    {
        let mut err_clients = self.err_clients;
        let futures = self.ok_clients.drain(..).map(client_to_future_fn);
        Box::new(futures_unordered(futures)
            .collect_results()
            .map(move |items| {
                let mut ok_clients = Vec::new();
                let mut msgs = HashMap::new();
                for item in items {
                    match item {
                        Ok((msg, client)) => {
                            let name = client.name.clone().unwrap();
                            msgs.insert(name, msg);
                            ok_clients.push(client);
                        }
                        Err(e) => err_clients.push(e),
                    };
                }
                (msgs,
                 Clients {
                     ok_clients: ok_clients,
                     err_clients: err_clients,
                 })
            }))
    }

    pub fn new_game(self, game: GameState) -> BoxFutureNotSend<Self, ()> {
        Box::new(self.new_from_map(|client| client.new_game(game.clone())))
    }

    pub fn new_turn(self, turn: TurnState) -> BoxFutureNotSend<Self, ()> {
        Box::new(self.new_from_map(|client| client.new_turn(turn.clone())))
    }

    pub fn ask_moves(self,
                     moving_player_names: HashSet<String>)
                     -> BoxFutureNotSend<(HashMap<String, Option<MoveMsg>>, Self), ()> {
        Box::new(self.new_from_map_receive(|client| {
            let name = client.name.clone().unwrap();
            if moving_player_names.contains(&name) {
                Box::new(client.ask_move().map(|(move_msg, client)| (Some(move_msg), client)))
            } else {
                Box::new(future::done(Ok((None, client))))
            }
        }))
    }

    pub fn notify_dead(self,
                       casualties: &HashMap<String, (CauseOfDeath, Snake)>)
                       -> BoxFutureNotSend<Self, ()> {
        Box::new(self.new_from_map(|client| {
            let name = client.name.clone().unwrap();
            if casualties.contains_key(&name) {
                client.die(casualties[&name].0.clone())
            } else {
                Box::new(future::done(Ok(client)))
            }
        }))
    }

    pub fn notify_winners(self,
                          winning_player_names: HashSet<String>)
                          -> BoxFutureNotSend<Self, ()> {
        Box::new(self.new_from_map(|client| {
            let name = client.name.clone().unwrap();
            if winning_player_names.contains(&name) {
                client.win()
            } else {
                Box::new(future::done(Ok(client)))
            }
        }))
    }

    pub fn end_game(self, turn: TurnState) -> BoxFutureNotSend<Self, ()> {
        Box::new(self.new_from_map(|client| client.end_game(turn.clone())))
    }
}

impl<S, T> FromIterator<Client<S, T>> for Clients<S, T>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send + 'static,
          T: Stream<Item = Msg, Error = io::Error> + Send + 'static
{
    fn from_iter<I: IntoIterator<Item = Client<S, T>>>(iter: I) -> Self {
        let mut items = Vec::new();
        for client in iter {
            items.push(client);
        }
        Clients {
            ok_clients: items,
            err_clients: Vec::new(),
        }
    }
}
