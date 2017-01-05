#![feature(conservative_impl_trait)]

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

use std::io;
use std::env;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::str;
use rand::OsRng;
use std::collections::{HashSet, HashMap};
use std::time::Duration;
use std::thread;

use futures::{future, stream, Future, BoxFuture, Stream, IntoFuture, Sink, Map};
use futures::stream::{SplitStream, SplitSink};
use tokio_core::net::TcpListener;
use tokio_core::reactor::{Core, Remote};
use tokio_core::io::Io;

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
    let listener = TcpListener::bind(&addr, &handle).unwrap();

    println!("Listening on {}", addr);

    let grid = Grid::new(25);
    let timeout: Option<Duration> = None;

    let names: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let players: Arc<Mutex<Vec<Client<SplitSink<MsgTransport>, SplitStream<MsgTransport>>>>> =
        Arc::new(Mutex::new(vec![]));

    // Run TCP server to welcome clients and register them as players.
    handle.spawn(server(listener,
                        lp.remote(),
                        names.clone(),
                        grid.clone(),
                        timeout.clone(),
                        players.clone()));

    // @TODO: Game requirements:
    // * Take existing player clients and play a game of sirpent with them until completion.
    // * Once game is concluded return player clients to the pool.
    // * After a short wait duration play a new game, as before with all pooled player clients.
    // * Continue indefinitely.
    thread::spawn(move || {
        let mut lp = Core::new().unwrap();
        lp.run(play_games(names.clone(),
                            grid.clone(),
                            timeout.clone(),
                            players.clone()))
            .unwrap();
    });

    // Poll event loop to keep program running.
    loop {
        lp.turn(None);
    }
}

fn server(listener: TcpListener,
          remote_handle: Remote,
          names: Arc<Mutex<HashSet<String>>>,
          grid: Grid,
          timeout: Option<Duration>,
          players: Arc<Mutex<Vec<Client<SplitSink<MsgTransport>, SplitStream<MsgTransport>>>>>)
          -> impl Future<Item = (), Error = ()> {
    let clients = listener.incoming()
        .map_err(|e| ProtocolError::from(e))
        .map(|(socket, addr)| Client::from_incoming(socket, addr).handshake());

    let server = clients.for_each(move |client_future| {
            // @TODO: If and when I build a client object, keep addr handy in it.
            let mut names_ref = names.clone();
            let players_ref = players.clone();

            // Close clients with unsuccessful handshakes.
            let client_future = client_future.map_err(|(e, _)| e);

            // Find a unique name for the Client and then send WelcomeMsg.
            let client_future = client_future.and_then(move |(identify_msg, client)| {
                let name = find_unique_name(&mut names_ref, identify_msg.desired_name);
                client.welcome(name, grid, timeout)
            });
            // Queue the Client as a new player.
            let client_future = client_future.map(move |client| {
                    players_ref.lock().unwrap().push(client);
                    ()
                })
                .map_err(|e| {
                    println!("Error welcoming client: {:?}", e);
                    ()
                });

            remote_handle.spawn(|_| client_future.boxed());
            Ok(())
        })
        .then(|_| Ok(()));

    return server.boxed();
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
                    timeout: Option<Duration>,
                    players_pool: Arc<Mutex<Vec<Client<S, T>>>>)
                    -> impl Future<Item = (), Error = ()>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send,
          T: Stream<Item = Msg, Error = io::Error> + Send
{
    future::loop_fn((), move |_| {
            let engine = Engine::new(OsRng::new().unwrap(), grid);

            let players_ref = players_pool.clone();
            let mut players_lock = players_pool.lock().unwrap();
            if players_lock.len() < 2 {
                println!("Not enough players yet.");
                return Ok(future::Loop::Continue(())).into_future();
            }

            let players = players_lock.drain(..).collect();
            play_game(engine, timeout, players)
                .and_then(move |(engine, mut players)| {
                    let state = engine.game.clone();
                    println!("End of game! {:?}", state);

                    let mut players_lock = players_ref.lock().unwrap();
                    players_lock.append(&mut players);
                    Ok(future::Loop::Continue(()))
                })
                .map_err(|e| {
                    println!("error {:?}", e);
                    e
                })
        })
        //.map(|_| ())
        .map_err(|_| ())
}

fn play_game<S, T>
    (mut engine: Engine<OsRng>,
     timeout: Option<Duration>,
     players: Vec<Client<S, T>>)
     -> impl Future<Item = (Engine<OsRng>, Vec<Client<S, T>>), Error = ProtocolError>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send,
          T: Stream<Item = Msg, Error = io::Error> + Send
{
    // Add players to the game.
    for player in players.iter() {
        engine.add_player(player.name.unwrap().clone());
    }

    // Issue GameMsg to all players.
    let game = engine.game.game.clone();
    // @TODO: Determine if Collect will stop accumulating if a client errored.
    stream::futures_unordered(players.iter().map(|client| client.new_game(game.clone())))
        .collect()
        .and_then(move |players| {
            future::loop_fn((engine, players), move |(mut engine, players)| {
                let turn = engine.game.turn.clone();
                stream::futures_unordered(players.iter().map(|client| client.turn(turn.clone())))
                    .collect()
                    .map(move |mut players_with_move_msgs| {
                        // Separate players_with_move_msgs into players and moves.
                        // @TODO: This should be handled by `take_turn`.
                        let mut moves: HashMap<String, Direction> = HashMap::new();
                        let players: Vec<Client<S, T>> = players_with_move_msgs.drain(..)
                            .map(|(move_msg, client)| {
                                moves.insert(client.name.unwrap().clone(), move_msg.direction);
                                client
                            })
                            .collect();
                        println!("{:?}", moves.clone());

                        // Compute and save the next turn.
                        engine.advance_turn(moves);
                        (engine, players)
                    })
                    .and_then(|(engine, players)| {
                        let turn = engine.game.turn.clone();
                        stream::futures_unordered(players.iter().map(|client| {
                                if turn.casualties.contains_key(&client.name.unwrap()) {
                                    client.die(turn.casualties[&client.name.unwrap()].0)
                                } else {
                                    future::done(Ok(client))
                                }
                            }))
                            .collect()
                            .map(|players| (engine, players))
                    })
                    .and_then(|(engine, players)| {
                        if engine.concluded() {
                            let turn = engine.game.turn.clone();
                            stream::futures_unordered(players.iter()
                                    .map(|client| client.end_game(turn)))
                                .collect()
                                .and_then(move |players| {
                                    stream::futures_unordered(players.iter()
                                            .map(|client| {
                                                if turn.snakes.contains_key(&client.name.unwrap()) {
                                                    client.win()
                                                } else {
                                                    future::done(Ok(client))
                                                }
                                            }))
                                        .collect()
                                })
                                .map(move |players| future::Loop::Break((engine, players)))
                        } else {
                            future::ok(future::Loop::Continue((engine, players)))
                        }
                    })
            })
        })
        .map_err(|e| {
            println!("error {:?}", e);
            e
        })
}
