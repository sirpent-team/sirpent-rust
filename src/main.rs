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

use std::env;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::str;
use rand::OsRng;
use std::collections::{HashSet, HashMap};
use std::time::Duration;

use futures::{future, Future, BoxFuture, Stream};
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;
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
    let players = Arc::new(Mutex::new(vec![]));

    let clients = listener.incoming()
        .map_err(|e| ProtocolError::from(e))
        .map(move |(socket, addr)| {
            let transport = socket.framed(MsgCodec);
            // Say hello and get a desired_name from the client.
            (Client.handshake(transport), addr)
        });

    let server = clients.for_each(|(client, addr)| {
        let names_ref = names.clone();
        let players_ref = players.clone();
        let client_future =
            client.and_then(move |(identify_msg, transport)| {
                    // Find an unused name based upon the desired_name.
                    // Subtly coded to ensure `names` is locked to ensure unique name still free.
                    let mut name = identify_msg.desired_name;
                    loop {
                        let mut names_ref = names_ref.lock().unwrap();
                        if names_ref.contains(&name) {
                            name += "_";
                        } else {
                            // Reserve the new name.
                            names_ref.insert(name.clone());
                            break;
                        }
                    }

                    Client.welcome(transport, name.clone(), grid.clone(), timeout)
                        .map(move |transport| (name, transport))
                })
                .then(move |result| {
                    match result {
                        Ok((name, transport)) => {
                            players_ref.lock().unwrap().push((name, transport));
                        }
                        Err(e) => println!("Error welcoming client: {:?}", e),
                    };
                    //                Ok(futures::done(Ok(5)))
                    //            })
                    //            .and_then(|_| {
                    // if players_ref.lock().unwrap().len() > 3 {
                    let mut players_lock = players_ref.lock().unwrap();
                    let game_players_future =
                        futurise_and_join(players_lock.drain(..), |(msg, transport)| {
                            futures::done(Ok((msg, transport))).boxed()
                        });

                    let grid_ref = grid.clone();
                    let timeout_ref = timeout.clone();
                    let play_game_future = game_players_future.and_then(move |game_player2s| {
                        let engine = Engine::new(OsRng::new().unwrap(), grid_ref.clone());
                        play_game(engine, game_player2s, timeout_ref)
                    });
                    return play_game_future.map_err(|_| ()).map(|_| ());
                    // }
                    // Ok(())
                })
                .boxed();

        handle.spawn(client_future);
        Ok(())
    });

    lp.run(server).unwrap();
}

fn play_game(mut engine: Engine<OsRng>,
             players: Vec<(String, MsgTransport)>,
             timeout: Option<Duration>)
             -> BoxFuture<(State, Vec<(String, MsgTransport)>), ProtocolError> {
    // Add players to the game.
    for &(ref name, _) in players.iter() {
        engine.add_player(name.clone());
    }

    // Wrap engine in sync primitives.
    let engine = Arc::new(Mutex::new(engine));

    // Issue GameMsg to all players.
    let game = engine.lock().unwrap().game.game.clone();
    let game_future = game_future(game, players);

    let loop_future = game_future.and_then(move |players| {
        let turn = engine.lock().unwrap().game.turn.clone();
        play_loop(engine.clone(), turn, players)
    });

    return loop_future.boxed();
}

fn play_loop(engine: Arc<Mutex<Engine<OsRng>>>,
             turn: TurnState,
             players: Vec<(String, MsgTransport)>)
             -> BoxFuture<(State, Vec<(String, MsgTransport)>), ProtocolError> {
    let engine_ref2 = engine.clone();
    let engine_ref3 = engine.clone();
    futures::done(Ok((turn, players)))
        .and_then(|(turn, players)| turn_future(turn, players))
        .map(move |mut players_with_move_msgs| {
            // Separate players_with_move_msgs into players and moves.
            // @TODO: Borrow issues are now absent - reimplement functionally.
            let mut moves: HashMap<String, Direction> = HashMap::new();
            let mut players: Vec<(String, MsgTransport)> = vec![];
            for (direction, name, transport) in players_with_move_msgs.drain(..) {
                moves.insert(name.clone(), direction);
                players.push((name, transport));
            }
            println!("{:?}", moves.clone());

            // Compute and save the next turn.
            let new_turn = engine_ref2.lock().unwrap().advance_turn(moves);

            (new_turn, players)
        })
        .and_then(move |(new_turn, players)| {
            let engine_lock = engine_ref3.lock().unwrap();
            if engine_lock.concluded() {
                let state = engine_lock.game.clone();
                game_over_future(state.turn.clone(), players)
                    .map(|players| (state, players))
                    .boxed()
            } else {
                play_loop(engine, new_turn, players).boxed()
            }
        })
        .boxed()
}

fn game_future(game: GameState,
               players: Vec<(String, MsgTransport)>)
               -> BoxFuture<Vec<(String, MsgTransport)>, ProtocolError> {
    futurise_and_join(players, |(name, transport)| {
            Client.game(transport, game.clone())
                .map(move |transport| (name, transport))
                .boxed()
        })
        .boxed()
}

fn turn_future(turn: TurnState,
               players: Vec<(String, MsgTransport)>)
               -> BoxFuture<Vec<(Direction, String, MsgTransport)>, ProtocolError> {
    futurise_and_join(players, |(name, transport)| {
            Client.turn(transport, turn.clone())
                .map(move |(move_msg, transport)| (move_msg.direction, name, transport))
                .boxed()
        })
        .boxed()
}

fn game_over_future(turn: TurnState,
                    players: Vec<(String, MsgTransport)>)
                    -> BoxFuture<Vec<(String, MsgTransport)>, ProtocolError> {
    futurise_and_join(players, |(name, transport)| {
            Client.game_over(transport, turn.clone())
                .map(move |transport| (name, transport))
                .boxed()
        })
        .boxed()
}

// @TODO: Remove Box requirement.
/// Map a collection to a vector of futures using a provided callback. Then run all those
/// futures in parallel using future::join_all.
fn futurise_and_join<I, F, O, E>(items: I, f: F) -> future::JoinAll<Vec<BoxFuture<O, E>>>
    where I: IntoIterator,
          F: FnMut(I::Item) -> BoxFuture<O, E>
{
    let futurised_items = items.into_iter()
        .map(f)
        .collect();
    future::join_all(futurised_items)
}
