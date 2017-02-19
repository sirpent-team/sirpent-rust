#![feature(conservative_impl_trait, box_syntax)]

extern crate log;
extern crate env_logger;
extern crate futures;
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
use std::collections::{HashSet, HashMap};
use std::time::Duration;
use std::thread;

use futures::{future, Future, Stream, Sink};
use futures::stream::{SplitStream, SplitSink};
use futures::sync::{mpsc, oneshot};
use tokio_core::net::TcpListener;
use tokio_core::reactor::{Core, Handle};
use tokio_core::io::Io;
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
    let players: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<ClientFutureCommand<String>>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let spectators: Arc<Mutex<HashMap<String,
                                          mpsc::UnboundedSender<ClientFutureCommand<String>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

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
        thread::sleep(Duration::from_secs(10));
        let mut lp = Core::new().unwrap();
        lp.run(play_games(names.clone(),
                            grid.clone(),
                            players.clone(),
                            spectators.clone(),
                            timer.clone(),
                            timeout.expect("Must have a timeout for now.")))
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
          players_pool: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<ClientFutureCommand<String>>>>>,
          spectators_pool: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<ClientFutureCommand<String>>>>>)
          -> impl Future<Item = (), Error = ()> {
    let clients = listener.incoming()
        .map(move |(socket, addr)| {
            let msg_transport = socket.framed(MsgCodec);
            let (tx, rx) = msg_transport.split();
            (ClientFuture::unbounded(addr, tx, rx), addr)
        });

    let server = clients.for_each(move |((client_future, command_tx), addr)| {
            handle.clone().spawn(client_future.map_err(|_| ()));

            // @TODO: If and when I build a client object, keep addr handy in it.
            let mut names_ref = names.clone();
            let players_ref = players_pool.clone();
            let spectators_ref = spectators_pool.clone();

            let fut = ClientsTimedReceive::single(addr, command_tx, timeout.unwrap(), &timer)
                .map_err(|_| ())
                .and_then(|(msgs, command_txs)| {
                    if msgs.is_empty() {
                        future::err(()).boxed()
                    } else {
                        let (_, msg) = msgs.into_iter().next().unwrap();
                        let (_, command_tx) = command_txs.into_iter().next().unwrap();
                        if let Msg::Register { desired_name, kind } = msg {
                            let name = find_unique_name(&mut names_ref, desired_name);
                            let welcome_msg = Msg::Welcome {
                                name: name,
                                grid: grid,
                                timeout: timeout,
                            };
                            command_tx.send(ClientFutureCommand::Transmit(welcome_msg))
                                .map(|command_tx| (name, kind, command_tx))
                                .map_err(|_| ())
                                .boxed()
                        } else {
                            future::err(()).boxed()
                        }
                    }
                });
            let fut = fut.map(move |(name, client_kind, command_tx)| {
                match client_kind {
                    ClientKind::Spectator => {
                        spectators_ref.lock().unwrap().insert(name, command_tx);
                    }
                    ClientKind::Player => {
                        players_ref.lock().unwrap().insert(name, command_tx);
                    }
                }
                ()
            });

            // Close clients with unsuccessful handshakes.
            let fut = fut.map_err(|(e, _)| {
                println!("Error welcoming client: {:?}", e);
                ()
            });

            handle.clone().spawn(fut);
            Ok(())
        })
        .then(|_| Ok(()));
    box server
}

/// Find an unused name based upon the `desired_name`.
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

fn play_games(names: Arc<Mutex<HashSet<String>>>,
            grid: Grid,
            players_pool: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<ClientFutureCommand<String>>>>>,
            spectators_pool: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<ClientFutureCommand<String>>>>>,
            timer: Timer,
            timeout: Duration)
            -> BoxedFuture<(), ()>
{
    box future::loop_fn((),
                        move |_| -> BoxedFuture<future::Loop<(), ()>, future::Loop<(), ()>> {
        let game = Game::new(OsRng::new().unwrap(), grid);

        let players_ref = players_pool.clone();
        let spectators_ref = spectators_pool.clone();

        while players_pool.lock().unwrap().len() < 2 {
            println!("Not enough players yet. Waiting 10 seconds.");
            return box timer.sleep(Duration::from_secs(10))
                .map(|_| future::Loop::Continue(()))
                .map_err(|_| future::Loop::Break(()));
        }

        let mut players_lock = players_pool.lock().unwrap();
        let players = players_lock.drain().collect();

        let mut spectators_lock = spectators_pool.lock().unwrap();
        let spectators = spectators_lock.drain().collect();

        box GameFuture::new(game, players, spectators, timer.clone(), timeout)
            .map(move |(game, players, spectators)| {
                println!("End of game! {:?} {:?}", game.game_state, game.turn_state);

                let mut players_lock = players_ref.lock().unwrap();
                let mut players = players.into_iter();
                players_lock.extend(&mut players);

                let mut spectators_lock = spectators_ref.lock().unwrap();
                let mut spectators = spectators.into_iter();
                spectators_lock.extend(&mut spectators);

                future::Loop::Continue(())
            })
            .map_err(|_| future::Loop::Break(()))
    })
        .map(|_| ())
        .map_err(|_| ())
}
