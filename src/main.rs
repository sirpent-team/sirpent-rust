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
use std::rc::Rc;
use std::cell::RefCell;

use futures::{future, Future, Stream, Sink};
use futures::stream::{SplitStream, SplitSink};
use tokio_core::net::TcpListener;
use tokio_core::reactor::{Core, Remote};

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

            // Find a unique name for the Client and then send WelcomeMsg.
            let client_future = client_future.and_then(move |(identify_msg, client)| {
                let name = find_unique_name(&mut names_ref, identify_msg.desired_name);
                client.welcome(name, grid, timeout)
            });
            // Queue the Client as a new player.
            let client_future = client_future.map(move |client| {
                players_ref.lock().unwrap().push(client);
                ()
            });

            // Close clients with unsuccessful handshakes.
            let client_future = client_future.map_err(|(e, _)| {
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
                    -> BoxFutureNotSend<(), ()>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send,
          T: Stream<Item = Msg, Error = io::Error> + Send
{
    Box::new(future::loop_fn((), move |_| {
            let engine = Engine::new(OsRng::new().unwrap(), grid);

            let players_ref = players_pool.clone();
            while players_pool.lock().unwrap().len() < 2 {
                //println!("Not enough players yet.");
                //return future::ok(future::Loop::Continue(()));
            }

            let mut players_lock = players_pool.lock().unwrap();
            let players = players_lock.drain(..).collect();
            play_game(Rc::new(RefCell::new(engine)), timeout, players)
                .map(move |(engine, players)| {
                    let state = engine.borrow().state.clone();
                    println!("End of game! {:?}", state);

                    let mut players_lock = players_ref.lock().unwrap();
                    let mut players = players.into_iter().collect::<Vec<_>>();
                    players_lock.append(&mut players);
                    future::Loop::Continue(())
                })
        })
        //.map(|_| ())
        .map_err(|_| ()))
}

fn play_game<S, T>(engine: Rc<RefCell<Engine<OsRng>>>,
                   timeout: Option<Duration>,
                   players: Clients<S, T>)
                   -> BoxFutureNotSend<(Rc<RefCell<Engine<OsRng>>>, Clients<S, T>), ()>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send,
          T: Stream<Item = Msg, Error = io::Error> + Send
{
    // Add players to the game.
    for name in players.names() {
        engine.borrow_mut().add_player(name.clone());
    }
    let engine4 = engine.clone();

    // Tell players about the new GameState.
    // @TODO: Determine if Collect will stop accumulating if a client errored.
    let game = engine.borrow().state.game.clone();
    Box::new(players.new_game(game)
        .and_then(move |players| {
            let engine = engine4.clone();
            // Take turns in a loop until game has finished.
            future::loop_fn(players, move |players| {
                let engine1 = engine.clone();
                let engine2 = engine.clone();
                let engine3 = engine.clone();
                // Tell players the current turn and ask for their next move.
                let turn = engine1.borrow().state.turn.clone();
                println!("{:?}", turn.clone());
                players.new_turn(turn.clone())
                    .and_then(move |players| {
                        let movers =
                            turn.snakes.keys().map(|name| name.clone()).collect::<HashSet<_>>();
                        println!("ask for moves {:?}", movers.clone());
                        players.ask_moves(&movers)
                    })
                    .and_then(move |(mut move_msgs, players)| {
                        let moves = move_msgs.drain()
                            .map(|(name, move_msg)| (name, move_msg.direction))
                            .collect();
                        // Transition to the next turn and tell the players whom died.
                        let new_turn = engine2.borrow_mut().advance_turn(moves);
                        println!("{:?}", new_turn.clone());
                        let casualty_names = new_turn.casualties
                            .iter()
                            .map(|(name, &(ref cause_of_death, _))| {
                                (name.clone(), cause_of_death.clone())
                            });
                        players.die(&casualty_names.collect())
                    })
                    .map(move |players| {
                        // @TODO: This works around a nasty `if and else have incompatible types'
                        // error I was having but is nasty.
                        // Decide whether game is complete or further turns will be made.
                        if engine3.borrow().concluded() {
                            future::Loop::Break(players)
                        } else {
                            future::Loop::Continue(players)
                        }
                    })
            })
        })
        .and_then(|players| {
            // Notify winners and inform all players the game is over.
            let new_turn = engine.borrow().state.turn.clone();
            let winner_names = new_turn.snakes.clone().keys().map(|name| name.clone()).collect();
            players.win(&winner_names)
                .and_then(|players| players.end_game(new_turn))
                .map(|players| (engine, players))
        }))
}
