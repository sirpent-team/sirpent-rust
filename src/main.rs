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

use futures::{Future, BoxFuture, Stream};
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
            (tell_handshake(transport, VersionMsg::new()), addr)
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

                    let welcome_msg = WelcomeMsg {
                        name: name.clone(),
                        grid: grid.clone(),
                        timeout: timeout,
                    };
                    tell_welcome(transport, welcome_msg).map(move |transport| (name, transport))
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
             players: Vec<Client>,
             timeout: Option<Duration>)
             -> BoxFuture<(State, Vec<Client>), ProtocolError> {
    // Add players to the game.
    for &(ref name, _) in players.iter() {
        engine.add_player(name.clone());
    }

    // Wrap engine in sync primitives.
    let engine = Arc::new(Mutex::new(engine));

    // Issue GameMsg to all players.
    let new_game_msg = NewGameMsg { game: engine.lock().unwrap().game.game.clone() };
    let game_future = tell_new_game(players, new_game_msg);

    let loop_future = game_future.and_then(move |players| {
        let turn = engine.lock().unwrap().game.turn.clone();
        play_loop(engine.clone(), turn, players)
    });

    return loop_future.boxed();
}

fn play_loop(engine: Arc<Mutex<Engine<OsRng>>>,
             turn: TurnState,
             players: Vec<Client>)
             -> BoxFuture<(State, Vec<Client>), ProtocolError> {
    let turn_msg = TurnMsg { turn: turn };
    take_turn(players, turn_msg)
        .and_then(move |mut players_with_move_msgs| {
            // Separate players_with_move_msgs into players and moves.
            // @TODO: Borrow issues are now absent - reimplement functionally.
            let mut moves: HashMap<String, Direction> = HashMap::new();
            let mut players: Vec<Client> = vec![];
            for (opt_move_msg, (name, transport)) in players_with_move_msgs.drain(..) {
                match opt_move_msg {
                    Some(move_msg) => {
                        moves.insert(name.clone(), move_msg.direction);
                    }
                    _ => {}
                };
                players.push((name, transport));
            }
            println!("{:?}", moves.clone());

            let engine_ref = engine.clone();
            let mut engine_lock = engine_ref.lock().unwrap();

            // Compute and save the next turn.
            let new_turn = engine_lock.advance_turn(moves);

            let state = engine_lock.game.clone();
            if engine_lock.concluded() {
                let game_over_msg = GameOverMsg { turn: new_turn.clone() };
                tell_game_over(players, game_over_msg)
                    .and_then(move |players| tell_winners(players, new_turn))
                    .map(move |players| (state.clone(), players))
                    .boxed()
            } else {
                tell_dead(players, new_turn.clone())
                    .and_then(move |players| play_loop(engine, new_turn, players).boxed())
                    .boxed()
            }
        })
        .boxed()
}
