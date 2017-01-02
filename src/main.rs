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

use std::cell::RefCell;
use std::env;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::rc::Rc;
use std::str;
use rand::OsRng;
use std::collections::{HashSet, HashMap};
use std::time::Duration;

use futures::{future, Async, Poll, Future, BoxFuture, Stream, Sink, IntoFuture};
use futures::sync::{oneshot, mpsc};
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

    // @TODO steps. Try committing as each one approaches done.
    // (1) Fix any type errors in play_game.
    // (2) Bring back the old name-deduplication code that went here.
    //     Without clever code we can't easily track clients as they Err and remove their
    //     names from the used list. Leave for later.
    // (3) Use that to drive Client.welcome.
    // (4) Find how to start play_game going once we have a few players.
    //     Run it in a separate thread to keep things a little separated?
    //     Build a vec of player futures (dumb ones - use futures::done() or whatever the
    //     tiny precompleted one is called) then use join_all to get it ready for play_game?
    // (5) Bring back an end condition to the game.
    // (6) Figure out how to retrieve players from an ended game.
    // (7) (Implement telling players when they die, win, etc.)
    // (8) Consider sensible refactoring, error types, whether Client methods should be passed
    //     TypedMsgs rather than parameters - given they *return* TypedMsgs it seems silly to
    //     pass lots of parameters in. Removing return of TypedMsgs sounds a recipe for pain.
    //     Try to remove client names from used name list when the client connections drop.
    //     Client type with fields to wrap around (name:String, TypedMessage)?
    // (9) Consider what tests are possible. Could we test the futures individually? Engine and
    //     such are totally free to be tested.

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
        let handle_ref = handle.clone();
        let client_future = client.and_then(move |(identify_msg, transport)| {
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
                    },
                    Err(e) => println!("Error welcoming client: {:?}", e),
                };
//                Ok(futures::done(Ok(5)))
//            })
//            .and_then(|_| {
                //if players_ref.lock().unwrap().len() > 3 {
                    let mut players_lock = players_ref.lock().unwrap();
                    let mut game_players = vec![];
                    for (msg, transport) in players_lock.drain(..) {
                        game_players.push(futures::done(Ok((msg, transport))));
                    }
                    let game_players_future: BoxFuture<Vec<(String, MsgTransport)>, ProtocolError> =
                        future::join_all(game_players).boxed();
                    let grid_ref = grid.clone();
                    let timeout_ref = timeout.clone();
                    let play_game_future = game_players_future.and_then(move |game_player2s| {
                        play_game(grid_ref.clone(), timeout_ref, game_player2s)
                    });
                    return play_game_future.map_err(|_| ());
                //}
                //Ok(())
            })
            .boxed();

        handle.spawn(client_future);
        Ok(())
    });

    /*
    // println!("{:?}", players.drain(..).map(|player| futures::done(player)));
    let zz = players.clone();
    let mut players_drained = zz.lock().unwrap();
    let mut game_players = vec![];
    for (msg, transport) in players_drained.drain(..) {
        game_players.push(futures::done(Ok((msg, transport))));
    }
    let game_players_future: BoxFuture<Vec<(String, MsgTransport)>, ProtocolError> =
        future::join_all(game_players).boxed();
    let play_game_future = game_players_future.and_then(|game_player2s| {
        play_game(grid.clone(), timeout, game_player2s)
    });
    //handle.spawn(play_game_future);
    */

    // move_future --> turn_future --> move_future

    lp.run(server).unwrap();
}

fn play_game(grid: Grid,
             timeout: Option<Duration>,
             players_vec: Vec<(String, MsgTransport)>) -> BoxFuture<(), ProtocolError> {
    let game_engine = Arc::new(Mutex::new(Engine::new(OsRng::new().unwrap(), grid)));
    for &(ref name, _) in players_vec.iter() {
        game_engine.lock().unwrap().add_player(name.clone());
    }
    let game_engine_lock = game_engine.lock().unwrap();
    play_turn(game_engine.clone(), game_engine_lock.game.turn.clone(), players_vec)
}

fn play_turn(game_engine: Arc<Mutex<Engine<OsRng>>>, turn: TurnState, players: Vec<(String, MsgTransport)>) -> BoxFuture<(), ProtocolError> {
    println!("{:?}", turn.clone());
    let game_engine_ref1 = game_engine.clone();
    let game_engine_ref2 = game_engine.clone();
    futures::done(Ok(turn)).and_then(|turn| {
        future::join_all(
            players.into_iter().map(|(name, transport)| {
                Client.turn(transport, turn.clone())
                    .map(move |(move_msg, transport)| {
                        (move_msg, name, transport)
                    })
            }).collect::<Vec<_>>()
        )
    }).and_then(move |mut players_with_move_msgs| {
        let mut moves: HashMap<String, Direction> = HashMap::new();
        let mut players: Vec<(String, MsgTransport)> = vec![];
        for (move_msg, name, transport) in players_with_move_msgs.drain(..) {
            moves.insert(name.clone(), move_msg.direction);
            players.push((name, transport));
        }
        println!("{:?}", moves.clone());

        let mut locked_game_engine = game_engine_ref1.lock().unwrap();

        // Compute and save the next turn.
        let new_turn = locked_game_engine.turn(moves);
        locked_game_engine.game.turn = new_turn.clone();

        future::ok((new_turn, players))
    }).and_then(move |(new_turn, players)| {
        // Get new moves from players for this new turn.
        play_turn(game_engine_ref2, new_turn, players)
    }).boxed()
    /*

    // When a message is received we're ready to begin getting new moves from players.
    // The fold cleverly preserves players and their updated MsgTransports between each iteration.
    // @TODO: This does not allow for inserting new players once it has begun!
    let turn_future = turns_rx.fold(players_vec, |players_vec, turn: TurnState| {
        future::join_all(
            players_vec.into_iter().map(|(name, transport)| {
                Client.turn(transport, turn.clone())
                    .map(move |(move_msg, transport)| {
                        moves.lock().unwrap().insert(name.clone(), move_msg.direction);
                        (name, transport)
                    })
            }).collect()
        )
    }).and_then(|_| {
        moved_tx.send(true).map(|_| ()).map_err(|_| ())
    }).boxed();

    // When a message is received we've completed receiving moves from players and are
    // ready to perform those moves and advance to the next turn.
    let transition_future = moved_rx.map_err(|_| ()).and_then(|_| {
        let locked_game_engine = game_engine.lock().unwrap();
        let locked_moves = moves.lock().unwrap();

        // Compute and save the next turn.
        let new_turn = locked_game_engine.turn(locked_moves.clone());
        locked_game_engine.game.turn = new_turn.clone();

        // Remove this turn's moves before beginning the next.
        locked_moves.drain();

        // Get new moves from players for this new turn.
        turns_tx.send(new_turn).map_err(|_| ())
    }).boxed();

    turn_future.join(transition_future).into_future().boxed()*/
}
