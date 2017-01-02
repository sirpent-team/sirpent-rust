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
use std::collections::HashMap;
use std::time::Duration;

use futures::{future, Async, Poll, Future, Stream, Sink};
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

    let clients = listener.incoming()
        .map_err(|e| ProtocolError::from(e))
        .map(move |(socket, addr)| {
            let transport = socket.framed(MsgCodec);
            // Say hello and get a desired_name from the client.
            (Client.handshake(transport), addr)
        });

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

    let handle = lp.handle();
    let server = clients.for_each(|(client, addr)| {
        let engine_ref = engine.clone();

        let client_future = client.and_then(move |(identify_msg, transport)| {
                let (name, game, timeout, turn_rx, move_tx) =
                    engine_future_ref.lock().unwrap().add_player(identify_msg);
                player_tx
                Client.welcome(transport, name.clone(), game.grid.clone(), timeout)
                    .and_then(|transport| Client.game(transport, game))
                    .map(|transport| (transport, name, turn_rx, move_tx))
            })
            .and_then(move |(transport, name, turn_rx, move_tx)| {
                turn_rx
                    .map_err(|e| ProtocolError::Internal)
                    .fold((transport, move_tx, name), |(transport, mut move_tx, name), turn| {
                        Client.turn(transport, turn).and_then(move |(move_msg, transport)| {
                            println!("{:?}", move_msg);
                            move_tx.send((name.clone(), move_msg.direction))
                                .map(|_| (transport, move_tx, name))
                                .map_err(|e| ProtocolError::from(e))
                        })
                    })
            })
            .then(|result| {
                match result {
                    Ok(_) => Ok(()),
                    Err(_) => Err(()),
                }
            })
            .boxed();

        handle.spawn(client_future);
        Ok(())
    });

    handle.spawn(engine_future);

    // move_future --> turn_future --> move_future

    lp.run(server).unwrap();
}

fn play_game(grid: Grid, timeout: Option<Duration>, players: StreamFuture<(String, MsgTransport)>) {
    let game_engine = Rc::new(Mutex::new(Engine::new(OsRng::new().unwrap(), grid)));

    // These swap between "get moves from players" futures and "perform move" futures.
    let (turns_tx, turns_rx) = mpsc::channel(1);
    let (moved_tx, moved_rx) = mpsc::channel(1);

    // Stores the moves players choose to make.
    // @TODO: Needs to accommodate errors so CauseOfDeath can be properly computed.
    let moves: Arc<Mutex<HashMap<String, Direction>>> = Arc::new(Mutex::new(HashMap::new()));

    // When a message is received we're ready to begin getting new moves from players.
    // The fold cleverly preserves players and their updated MsgTransports between each iteration.
    // @TODO: But as yet this does not allow for inserting new players once it has begun!
    let turn_future = turns_rx.fold(players, |players, turn| {
        players.map(|(name, transport)| {
            // Send TurnMsg to clients and receive a MoveMsg.
            // This maps each (name:String, MsgTransport) to (MoveMsg, MsgTransport).
            // Record the chosen move direction then return the new (name:String, transport) pair.
            Client.turn(transport, turn.clone())
                .map(move |(move_msg, transport)| {
                    moves.lock().unwrap().insert(name.clone(), move_msg.direction);
                    (name, transport)
                })
        }).and_then(|players| {
            moved_tx.send(true).map(|_| players)
        })
    });

    // When a message is received we've completed receiving moves from players and are
    // ready to perform those moves and advance to the next turn.
    let transition_future = moved_rx.and_then(|_| {
        let locked_game_engine = game_engine.lock().unwrap();
        let locked_moves = moves.lock().unwrap();

        // Compute and save the next turn.
        let new_turn = locked_game_engine.turn(locked_moves.clone());
        locked_game_engine.game.turn = new_turn.clone();

        // Remove this turn's moves before beginning the next.
        locked_moves.drain();

        // Get new moves from players for this new turn.
        turns_tx.send(new_turn)
    });

    turn_future.join(transition_future)
}
