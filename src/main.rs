/*
extern crate log;
extern crate env_logger;
extern crate futures;
extern crate tokio_core;
extern crate sirpent;
extern crate serde_json;
extern crate rand;
extern crate tokio_timer;

use std::env;
use std::str;
use rand::OsRng;
use std::thread;
use std::convert::Into;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::collections::{HashSet, HashMap};
use futures::{future, BoxFuture, Future, Stream, Sink};
use futures::sync::mpsc;
use tokio_core::net::TcpListener;
use tokio_core::reactor::{Core, Handle};
use tokio_core::io::Io;
use tokio_timer::Timer;

use sirpent::utils::*;
use sirpent::net::*;
use sirpent::engine::*;
use sirpent::state::*;
use sirpent::net::clients::*;

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
    let timeout: Option<Milliseconds> = Some(Milliseconds::new(5000));

    let names: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let players: Arc<Mutex<HashMap<String, MapToError<mpsc::UnboundedSender<Cmd>>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let spectators: Arc<Mutex<HashMap<String, MapToError<mpsc::UnboundedSender<Cmd>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // Run TCP server to welcome clients and register them as players.
    handle.spawn(server(listener,
                        handle.clone(),
                        names.clone(),
                        grid.clone(),
                        timeout,
                        players.clone(),
                        spectators.clone()));

    // @TODO: Game requirements:
    // * Take existing player clients and play a game of sirpent with them until completion.
    // * Once game is concluded return player clients to the pool.
    // * After a short wait Milliseconds play a new game, as before with all pooled player clients.
    // * Continue indefinitely.
    thread::spawn(move || {
        thread::sleep(Milliseconds::new(10000).into());
        let mut lp = Core::new().unwrap();
        lp.run(play_games(names.clone(),
                            grid.clone(),
                            players.clone(),
                            spectators.clone(),
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
          timeout: Option<Milliseconds>,
          players_pool: Arc<Mutex<HashMap<String, MapToError<mpsc::UnboundedSender<Cmd>>>>>,
          spectators_pool: Arc<Mutex<HashMap<String, MapToError<mpsc::UnboundedSender<Cmd>>>>>)
          -> Box<Future<Item = (), Error = ()>> {
    let clients = listener.incoming()
        .map(move |(socket, addr)| {
            let msg_transport = map2error(socket.framed(MsgCodec));
            let (tx, rx) = msg_transport.split();
            (tx, rx, addr)
        });

    let server = clients.for_each(move |(msg_tx, msg_rx, addr)| {
            // @TODO: If and when I build a client object, keep addr handy in it.
            let mut names_ref = names.clone();
            let players_ref = players_pool.clone();
            let spectators_ref = spectators_pool.clone();

            let fut = msg_tx.send(Msg::version())
                .and_then(move |msg_tx| {
                    msg_rx.into_future()
                        .map_err(|(e, _)| e)
                        .and_then(move |(msg, msg_rx)| {
                            if let Some(Msg::Register { desired_name, kind }) = msg {
                                let name = find_unique_name(&mut names_ref, desired_name);
                                let welcome_msg = Msg::Welcome {
                                    name: name.clone(),
                                    grid: grid.clone().into(),
                                    timeout_millis: timeout,
                                };
                                msg_tx.send(welcome_msg)
                                    .map(move |msg_tx| (msg_tx, msg_rx, addr, name, kind))
                                    .boxed()
                            } else {
                                println!("{:?}", msg);
                                future::err(format!("message was not a Msg::Register :: {:?}", msg)
                                        .into())
                                    .boxed()
                            }
                        })
                });

            let handle2 = handle.clone();
            let fut = fut.map(move |(msg_tx, msg_rx, _, name, kind)| {
                let (client_future, command_tx) = Client::unbounded(name.clone(), msg_tx, msg_rx);
                handle2.spawn(client_future.map_err(|e| {
                    println!("{:?}", e);
                    ()
                }));

                match kind {
                    ClientKind::Spectator => {
                        spectators_ref.lock().unwrap().insert(name, map2error(command_tx));
                    }
                    ClientKind::Player => {
                        players_ref.lock().unwrap().insert(name, map2error(command_tx));
                    }
                }
                ()
            });

            // Close clients with unsuccessful handshakes.
            let fut = fut.map_err(|e| {
                println!("Error welcoming client: {:?}", e);
                ()
            });

            handle.clone().spawn(fut);
            Ok(())
        })
        .then(|_| Ok(()));
    Box::new(server)
}

/// Find an unused name based upon the `desired_name`.
fn find_unique_name(names: &mut Arc<Mutex<HashSet<String>>>, desired_name: String) -> String {
    // Use the desired name if it's unused.
    {
        let mut names_lock = names.lock().unwrap();
        if !names_lock.contains(&desired_name) {
            // Reserve this name.
            names_lock.insert(desired_name.clone());
            return desired_name;
        }
    }

    // Find a unique name.
    let mut n = 1;
    loop {
        let name = format!("{}_{}", desired_name, roman_numerals(n));
        println!("{:?}", name);
        let mut names_lock = names.lock().unwrap();
        if !names_lock.contains(&name) {
            // Reserve this name.
            names_lock.insert(name.clone());
            return name;
        }
        n += 1;
    }
}

fn play_games(_: Arc<Mutex<HashSet<String>>>,
              grid: Grid,
              players_pool: Arc<Mutex<HashMap<String, MapToError<mpsc::UnboundedSender<Cmd>>>>>,
              spectators_pool: Arc<Mutex<HashMap<String,
                                                 MapToError<mpsc::UnboundedSender<Cmd>>>>>,
              timer: Timer,
              timeout: Option<Milliseconds>)
              -> BoxFuture<(), ()> {
    Box::new(future::loop_fn((), move |_| {
        let players_ref = players_pool.clone();
        let spectators_ref = spectators_pool.clone();

        while players_pool.lock().unwrap().len() < 2 {
            println!("Not enough players yet. Waiting 10 seconds.");
            return timer.sleep(Milliseconds::new(10000).into())
                .map(|_| future::Loop::Continue(()))
                .map_err(|_| ())
                .boxed();
        }

        // Acquire players and spectators from those available.
        // @TODO: Once drained, check we still have sufficient players. No lock between
        // the waiting loop above and now.
        let mut players = players_pool.lock().unwrap();
        let mut spectators = spectators_pool.lock().unwrap();
        let players = players.drain().collect();
        let spectators = spectators.drain().collect();

        let game = Game::new(OsRng::new().unwrap(), grid);
        GameFuture::new(game, players, spectators, timeout)
            .map(move |(game, players, spectators)| {
                println!("End of game! {:?} {:?}", game.game_state, game.round_state);

                // Return players and spectators to the waiting pool.
                let mut players_pool = players_ref.lock().unwrap();
                let mut spectators_pool = spectators_ref.lock().unwrap();
                players_pool.extend(&mut players.into_iter());
                spectators_pool.extend(&mut spectators.into_iter());

                future::Loop::Continue(())
            })
            .map_err(|_| ())
            .boxed()
    }))
}
*/
