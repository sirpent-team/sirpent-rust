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
use std::thread;

use futures::{Future, BoxFuture, Stream};
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
    let players = Arc::new(Mutex::new(vec![]));

    // Run TCP server to welcome clients and register them as players.
    let handle = lp.handle();
    handle.spawn(server(listener,
                        lp.remote(),
                        names.clone(),
                        grid.clone(),
                        timeout.clone(),
                        players.clone()));

    // @TODO: As the server is only being spawned there is no *need* for a second thread.
    // The integration of games with the rest of the program is unclear but bear the above in mind.
    //
    // Game requirements:
    // * Take existing player clients and play a game of sirpent with them until completion.
    // * Once game is concluded return player clients to the pool.
    // * After a short wait duration play a new game, as before with all pooled player clients.
    // * Continue indefinitely.
    //let names_ref = names.clone();
    let players_ref = players.clone();
    //let timeout_ref = timeout.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(10));

        let engine = Engine::new(OsRng::new().unwrap(), grid.clone());
        let mut players_lock = players_ref.lock().unwrap();
        let games = play_game(engine, players_lock.drain(..).collect(), timeout);

        let mut lp = Core::new().unwrap();
        lp.run(games).unwrap();
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
          players: Arc<Mutex<Vec<Client>>>)
          -> BoxFuture<(), ()> {
    let clients = listener.incoming()
        .map_err(|e| ProtocolError::from(e))
        .map(move |(socket, addr)| {
            let transport = socket.framed(MsgCodec);
            // Say hello and get a desired_name from the client.
            (tell_handshake(transport, VersionMsg::new()), addr)
        });

    let server = clients.for_each(move |(transport, _)| {
            // @TODO: If and when I build a client object, keep addr handy in it.
            let mut names_ref = names.clone();
            let players_ref = players.clone();

            // Find a unique name for the Client and then send WelcomeMsg.
            let client_future = transport.and_then(move |(identify_msg, transport)| {
                let name = find_unique_name(&mut names_ref, identify_msg.desired_name);
                let welcome_msg = WelcomeMsg {
                    name: name.clone(),
                    grid: grid,
                    timeout: timeout,
                };
                tell_welcome(transport, welcome_msg).map(|transport| (name, transport))
            });
            // Queue the Client as a new player.
            let client_future = client_future.map(move |(name, transport)| {
                    players_ref.lock().unwrap().push((name, transport));
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
    let game = engine.lock().unwrap().game.game.clone();
    let new_game_msg = NewGameMsg { game: game };
    let game_future = tell_new_game(players, new_game_msg);

    let loop_future = game_future.and_then(move |players| play_loop(players, engine.clone()));

    return loop_future.boxed();
}

fn play_loop(players: Vec<Client>,
             engine: Arc<Mutex<Engine<OsRng>>>)
             -> BoxFuture<(State, Vec<Client>), ProtocolError> {
    let turn = engine.lock().unwrap().game.turn.clone();

    let turn_msg = TurnMsg { turn: turn };
    take_turn(players, turn_msg)
        .and_then(move |mut players_with_move_msgs| {
            // Separate players_with_move_msgs into players and moves.
            let mut moves: HashMap<String, Direction> = HashMap::new();
            let players: Vec<Client> = players_with_move_msgs.drain(..)
                .map(|(opt_move_msg, (name, transport))| {
                    if opt_move_msg.is_some() {
                        moves.insert(name.clone(), opt_move_msg.unwrap().direction);
                    }
                    (name, transport)
                })
                .collect();
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
                tell_dead(players, new_turn)
                    .and_then(move |players| play_loop(players, engine).boxed())
                    .boxed()
            }
        })
        .boxed()
}
