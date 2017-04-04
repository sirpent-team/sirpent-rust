extern crate log;
extern crate env_logger;
extern crate futures;
extern crate tokio_core;
extern crate sirpent;
extern crate serde_json;
extern crate rand;
extern crate tokio_timer;
extern crate tokio_io;
extern crate uuid;
extern crate comms;

use std::env;
use std::str;
use rand::OsRng;
use std::thread;
use std::convert::Into;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::collections::HashSet;
use futures::{future, Future, Sink, Stream};
use futures::sync::mpsc;
use tokio_core::net::TcpListener;
use tokio_core::reactor::{Core, Handle};
use tokio_timer::Timer;
use tokio_io::AsyncRead;
use comms::{Client, Room};

use sirpent::utils::*;
use sirpent::net::*;
use sirpent::engine::*;
use sirpent::state::*;

fn main() {
    drop(env_logger::init());

    // Take the first command line argument as an address to listen on, or fall
    // back to just some localhost default.
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());
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
    let timeout: Option<Milliseconds> = Some(Milliseconds::new(5000));

    let names: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let players: Arc<Mutex<Vec<MsgClient<String>>>> = Arc::new(Mutex::new(Vec::new()));

    let (spectator_tx, spectator_rx) = mpsc::channel(3);
    let (spectator_msg_tx, spectator_msg_rx) = mpsc::channel(3);
    let spectators = Spectators::new(spectator_rx, spectator_msg_rx);
    handle.spawn(spectators);

    // Run TCP server to welcome clients and register them as players.
    handle.spawn(server(listener,
                        handle.clone(),
                        names.clone(),
                        grid,
                        timeout.unwrap(),
                        players.clone(),
                        spectator_tx,
                        timer.clone()));

    // @TODO: Game requirements:
    // * Take existing player clients and play a game of sirpent with them until completion.
    // * Once game is concluded return player clients to the pool.
    // * After a short wait Milliseconds play a new game, as before with all pooled player clients.
    // * Continue indefinitely.
    thread::spawn(move || {
        thread::sleep(Milliseconds::new(10000).into());
        let mut lp = Core::new().unwrap();
        lp.run(play_games(names.clone(),
                            grid,
                            players.clone(),
                            spectator_msg_tx,
                            timer.clone(),
                            timeout))
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
          timeout_millis: Milliseconds,
          players_pool: Arc<Mutex<Vec<MsgClient<String>>>>,
          spectator_tx: mpsc::Sender<MsgClient<String>>,
          timer: tokio_timer::Timer)
          -> Box<Future<Item = (), Error = ()>> {
    let server = listener
        .incoming()
        .map_err(|_| ())
        .for_each(move |(socket, addr)| {
            let msg_transport = socket.framed(MsgCodec);
            let client = Client::new(addr, msg_transport);

            let players_pool = players_pool.clone();
            let spectator_tx = spectator_tx.clone();

            let handshake_future =
                handshake(client, timeout_millis, timer.clone(), names.clone(), grid)
                    .map_err(|_| ())
                    .and_then(move |(client, kind)| -> Box<Future<Item = (), Error = ()>> {
                        match kind {
                            ClientKind::Player => {
                                players_pool.lock().unwrap().push(client);
                                Box::new(future::ok(()))
                            }
                            ClientKind::Spectator => {
                                Box::new(spectator_tx.send(client).map(|_| ()).map_err(|_| ()))
                            }
                        }
                    });

            handle.spawn(handshake_future);
            Ok(())
        })
        .then(|_| Ok(()));
    Box::new(server)
}

fn handshake(client: MsgClient<SocketAddr>,
             timeout_millis: Milliseconds,
             timer: tokio_timer::Timer,
             mut names: Arc<Mutex<HashSet<String>>>,
             grid: Grid)
             -> Box<Future<Item = (MsgClient<String>, ClientKind), Error = ()>> {
    let version_msg = Msg::version();
    Box::new(client
                 .transmit(version_msg)
                 .map_err(|_| ())
                 .and_then(move |client| {
                               client
                                   .receive()
                                   .with_hard_timeout(*timeout_millis, &timer)
                                   .map_err(|_| ())
                           })
                 .and_then(move |(maybe_msg, client)| -> Box<Future<Item = _, Error = _>> {
        if let Msg::Register { desired_name, kind } = maybe_msg {
            let name = find_unique_name(&mut names, desired_name);
            let client = client.rename(name.clone());

            let welcome_future = client.transmit(Msg::Welcome {
                                                     name: name,
                                                     grid: grid.into(),
                                                     timeout_millis: Some(timeout_millis),
                                                 });

            Box::new(welcome_future
                         .map(move |client| (client, kind))
                         .map_err(|_| ()))
        } else {
            println!("Error welcoming client: unexpected {:?}", maybe_msg);
            Box::new(future::err(()))
        }
    }))
}

/// Find an unused name based upon the `desired_name`.
fn find_unique_name(names: &mut Arc<Mutex<HashSet<String>>>, desired_name: String) -> String {
    {
        // Use the desired name if it's unused.
        println!("prelock2");
        let mut names_lock = names.lock().unwrap();
        if !names_lock.contains(&desired_name) {
            // Reserve this name.
            names_lock.insert(desired_name.clone());
            return desired_name;
        }
        println!("postlock2");
    }

    // Find a unique name.
    let mut n = 1;
    loop {
        let name = format!("{}_{}", desired_name, roman_numerals(n));
        println!("{:?}", name);
        println!("prelock3");
        let mut names_lock = names.lock().unwrap();
        if !names_lock.contains(&name) {
            // Reserve this name.
            names_lock.insert(name.clone());
            return name;
        }
        println!("postlock3");
        n += 1;
    }
}

fn play_games(_: Arc<Mutex<HashSet<String>>>,
              grid: Grid,
              player_queue_ref: Arc<Mutex<Vec<MsgClient<String>>>>,
              spectators_ref: mpsc::Sender<Msg>,
              timer: Timer,
              timeout: Option<Milliseconds>)
              -> Box<Future<Item = (), Error = ()>> {
    Box::new(future::loop_fn((), move |_| -> Box<Future<Item = _, Error = _>> {
        let continue_ = future::Loop::Continue(());
        let timer2 = timer.clone();

        println!("prelock4");
        let mut players_queue = player_queue_ref.lock().unwrap();
        if players_queue.len() < 2 {
            println!("Not enough players yet. Waiting 10 seconds.");
            println!("postlock4/1");
            Box::new(sleep(&timer, milliseconds(10000)).map(|_| continue_))
        } else {
            // Acquire players and spectators from those available.
            // @TODO: Once drained, check we still have sufficient players. No lock between
            // the waiting loop above and now.
            let players = Room::new(players_queue.drain(..).collect());
            println!("postlock4/2");

            let game = Game::new(OsRng::new().unwrap(), grid);
            Box::new(game_future(game,
                                 players,
                                 spectators_ref.clone(),
                                 timeout,
                                 timer.clone())
                             .and_then(move |(game, _, _)| {
                println!("End of game! {:?} {:?}",
                         game.game_state(),
                         game.round_state());

                // @TODO: Return players and spectators to the waiting pool.
                // let mut players_queue = player_queue_ref2.lock().unwrap();
                // players_queue.extend(players.into_clients()
                //     .0
                //     .into_iter()
                //     .map(|(_, v)| v)
                //     .collect::<Vec<_>>());

                sleep(&timer2, milliseconds(2000)).map(|_| continue_)
            }))
        }
    }))
}

fn sleep(timer: &Timer, ms: Milliseconds) -> Box<Future<Item = (), Error = ()>> {
    Box::new(timer.sleep(ms.into()).map(|_| ()).map_err(|_| ()))
}
