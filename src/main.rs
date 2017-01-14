#![feature(conservative_impl_trait, box_syntax)]

#[macro_use]
extern crate log;
extern crate env_logger;
#[macro_use]
extern crate futures;
#[macro_use]
extern crate tokio_core;
extern crate sirpent;
extern crate serde_json;
extern crate rand;
extern crate tokio_timer;

use std::io;
use std::env;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::str;
use rand::OsRng;
use std::collections::HashSet;
use std::time::Duration;
use std::thread;

use futures::{future, Future, Stream, Sink};
use futures::stream::{SplitStream, SplitSink};
use tokio_core::net::TcpListener;
use tokio_core::reactor::{Core, Handle};
use tokio_timer::Timer;

use sirpent::*;

fn main() {
    drop(env_logger::init());

    // Take the first command line argument as an address to listen on, or fall
    // back to just some localhost default.
    let addr = env::args().nth(1).unwrap_or("127.0.0.1:8080".to_string());
    let addr = addr.parse::<SocketAddr>().unwrap();

    // Initialize the various data structures we're going to use in our server.
    // Here we create the event loop, the global buffer that all threads will
    // read/write into, and the bound TCP listener itself.
    let mut lp = Core::new().unwrap();
    let handle = lp.handle();

    let timer = Timer::default();

    let listener = TcpListener::bind(&addr, &handle).unwrap();
    println!("Listening on {}", addr);

    let grid = Grid::new(25);
    let timeout: Option<Duration> = Some(Duration::from_secs(5));

    let names: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let players: Arc<Mutex<Vec<Client<SplitSink<MsgTransport>, SplitStream<MsgTransport>>>>> =
        Arc::new(Mutex::new(vec![]));
    let spectators: Arc<Mutex<Vec<Client<SplitSink<MsgTransport>, SplitStream<MsgTransport>>>>> =
        Arc::new(Mutex::new(vec![]));

    // Run TCP server to welcome clients and register them as players.
    handle.spawn(server(listener,
                        handle.clone(),
                        names.clone(),
                        grid.clone(),
                        timer.clone(),
                        timeout,
                        players.clone(),
                        spectators.clone()));

    // @TODO: Game requirements:
    // * Take existing player clients and play a game of sirpent with them until completion.
    // * Once game is concluded return player clients to the pool.
    // * After a short wait duration play a new game, as before with all pooled player clients.
    // * Continue indefinitely.
    thread::spawn(move || {
        // thread::sleep(Duration::from_secs(10));
        let mut lp = Core::new().unwrap();
        lp.run(play_games(names.clone(),
                            grid.clone(),
                            players.clone(),
                            spectators.clone(),
                            timer.clone()))
            .unwrap();
    });

    // Poll event loop to keep program running.
    loop {
        lp.turn(None);
    }
}

fn server(listener: TcpListener,
          handle: Handle,
          names: Arc<Mutex<HashSet<String>>>,
          grid: Grid,
          timer: Timer,
          timeout: Option<Duration>,
          players: Arc<Mutex<Vec<Client<SplitSink<MsgTransport>, SplitStream<MsgTransport>>>>>,
          spectators: Arc<Mutex<Vec<Client<SplitSink<MsgTransport>, SplitStream<MsgTransport>>>>>)
          -> impl Future<Item = (), Error = ()> {
    let clients = listener.incoming()
        .map_err(|e| ProtocolError::from(e))
        .map(move |(socket, addr)| {
            Client::from_incoming(socket, addr, timer.clone(), timeout).handshake()
        });

    let server = clients.for_each(move |client_future| {
            // @TODO: If and when I build a client object, keep addr handy in it.
            let mut names_ref = names.clone();
            let players_ref = players.clone();
            let spectators_ref = spectators.clone();

            // Find a unique name for the Client and then send WelcomeMsg.
            let client_future =
                client_future.and_then(move |(register_msg, client)| -> BoxedFuture<_, _> {
                    let name = find_unique_name(&mut names_ref, register_msg.desired_name.clone());
                    box client.welcome(name, grid, timeout)
                        .map(move |client| (client, register_msg.kind))
                });
            // Queue the Client as a new player or spectator.
            let client_future = client_future.map(move |(client, client_kind)| {
                match client_kind {
                    ClientKind::Spectator => {
                        spectators_ref.lock().unwrap().push(client);
                    }
                    ClientKind::Player => {
                        players_ref.lock().unwrap().push(client);
                    }
                }
                ()
            });

            // Close clients with unsuccessful handshakes.
            let client_future = client_future.map_err(|(e, _)| {
                println!("Error welcoming client: {:?}", e);
                ()
            });

            handle.clone().spawn(client_future);
            Ok(())
        })
        .then(|_| Ok(()));

    return box server;
}

/// Find an unused name based upon the desired_name.
fn find_unique_name(names: &mut Arc<Mutex<HashSet<String>>>, desired_name: String) -> String {
    let mut name = desired_name;
    loop {
        // Locked once each iteration to ensure nothing can be added between the uniqueness
        // check and the insertion.
        let mut names_lock = names.lock().unwrap();
        if names_lock.contains(&name) {
            name += "_";
        } else {
            // Reserve the new name.
            names_lock.insert(name.clone());
            return name;
        }
    }
}

fn play_games<S, T>(names: Arc<Mutex<HashSet<String>>>,
                    grid: Grid,
                    players_pool: Arc<Mutex<Vec<Client<S, T>>>>,
                    spectators_pool: Arc<Mutex<Vec<Client<S, T>>>>,
                    timer: Timer)
                    -> BoxedFuture<(), ()>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send,
          T: Stream<Item = Msg, Error = io::Error> + Send
{
    box future::loop_fn((), move |_| -> BoxedFuture<_, _> {
            let game = Game::new(OsRng::new().unwrap(), grid);

            let players_ref = players_pool.clone();
            let spectators_ref = spectators_pool.clone();

            while players_pool.lock().unwrap().len() < 2 {
                println!("Not enough players yet. Waiting 10 seconds.");
                return box timer.sleep(Duration::from_secs(10))
                    .map(|_| future::Loop::Continue(()))
                    .map_err(|_| ());
            }

            let mut players_lock = players_pool.lock().unwrap();
            let players = players_lock.drain(..).collect();

            let mut spectators_lock = spectators_pool.lock().unwrap();
            let spectators = spectators_lock.drain(..).collect();

            box GameFuture::new(game, players, spectators)
                .map(move |(game, players, spectators)| {
                    println!("End of game! {:?} {:?}", game.game_state, game.turn_state);

                    let mut players_lock = players_ref.lock().unwrap();
                    let mut players = players.into_iter().collect::<Vec<_>>();
                    players_lock.append(&mut players);

                    let mut spectators_lock = spectators_ref.lock().unwrap();
                    let mut spectators = spectators.into_iter().collect::<Vec<_>>();
                    spectators_lock.append(&mut spectators);

                    future::Loop::Continue(())
                })
        })
        //.map(|_| ())
        .map_err(|_| ())
}
