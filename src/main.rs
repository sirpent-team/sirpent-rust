extern crate log;
extern crate env_logger;
extern crate futures;
extern crate tokio_core;
extern crate sirpent;
extern crate serde_json;
extern crate rand;
extern crate tokio_timer;
extern crate tokio_io;

use std::env;
use std::str;
use rand::OsRng;
use std::thread;
use std::convert::Into;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::collections::HashSet;
use futures::{future, Future, Stream};
use tokio_core::net::TcpListener;
use tokio_core::reactor::{Core, Handle};
use tokio_timer::Timer;
use tokio_io::AsyncRead;

use sirpent::utils::*;
use sirpent::net::*;
use sirpent::engine::*;
use sirpent::state::*;

fn main() {
    drop(env_logger::init());

    // Take the first command line argument as an address to listen on, or fall
    // back to just some localhost default.
    let addr = env::args().nth(1).unwrap_or_else(|| "127.0.0.1:8080".to_string());
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
    let players: Arc<Mutex<Vec<Client>>> = Arc::new(Mutex::new(Vec::new()));
    let spectators: Arc<Mutex<Room>> = Arc::new(Mutex::new(Room::default()));

    // Run TCP server to welcome clients and register them as players.
    handle.spawn(server(listener,
                        handle.clone(),
                        names.clone(),
                        grid,
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
                            grid,
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
          players_pool: Arc<Mutex<Vec<Client>>>,
          spectators_pool: Arc<Mutex<Room>>)
          -> Box<Future<Item = (), Error = ()>> {
    let clients = listener.incoming()
        .map(move |(socket, addr)| {
            let msg_transport = map2error(socket.framed(MsgCodec));
            let (tx, rx) = msg_transport.split();
            (tx, rx, addr)
        });

    let server = clients.for_each(move |(msg_tx, msg_rx, addr)| {
            let (mut client, client_relay) =
                client(Some(format!("{}", addr)), Some(16), msg_tx, msg_rx);
            handle.spawn(client_relay.map_err(|e| {
                println!("CLIENTRELAY ERROR: {:?}", e);
                ()
            }));

            // @TODO: If and when I build a client object, keep addr handy in it.
            let mut names_ref = names.clone();
            let players_ref = players_pool.clone();
            let spectators_ref = spectators_pool.clone();

            let version_tx = client.transmit(Msg::version());
            let register_rx = client.receive(ClientTimeout::disconnect_after(timeout.map(|m| *m)));
            handle.spawn(version_tx.join(register_rx)
                .and_then(move |(_, (_, _, maybe_msg))| {
                    if let Some(Msg::Register { desired_name, kind }) = maybe_msg {
                        let name = find_unique_name(&mut names_ref, desired_name);
                        client.rename(Some(name.clone()));

                        let welcome_tx = client.transmit(Msg::Welcome {
                                name: name,
                                grid: grid.into(),
                                timeout_millis: timeout,
                            })
                            .map(|_| ());

                        match kind {
                            ClientKind::Spectator => {
                                spectators_ref.lock().unwrap().insert(client);
                            }
                            ClientKind::Player => {
                                players_ref.lock().unwrap().push(client);
                            }
                        }

                        welcome_tx.boxed()
                    } else {
                        println!("Error welcoming client: unexpected {:?}", maybe_msg);
                        future::err(()).boxed()
                    }
                }));

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
              player_queue_ref: Arc<Mutex<Vec<Client>>>,
              spectators_ref: Arc<Mutex<Room>>,
              timer: Timer,
              timeout: Option<Milliseconds>)
              -> Box<Future<Item = (), Error = ()>> {
    Box::new(future::loop_fn((), move |_| -> Box<Future<Item = _, Error = _>> {
        let continue_ = future::Loop::Continue(());
        let timer2 = timer.clone();

        let player_queue_ref2 = player_queue_ref.clone();
        let mut players_queue = player_queue_ref.lock().unwrap();
        if players_queue.len() < 2 {
            println!("Not enough players yet. Waiting 10 seconds.");
            Box::new(sleep(&timer, milliseconds(10000)).map(|_| continue_))
        } else {
            // Acquire players and spectators from those available.
            // @TODO: Once drained, check we still have sufficient players. No lock between
            // the waiting loop above and now.
            let players = Room::new(players_queue.drain(..).collect());

            let game = Game::new(OsRng::new().unwrap(), grid);
            Box::new(game_future(game, players, spectators_ref.clone(), timeout)
                .and_then(move |(game, players, _)| {
                    println!("End of game! {:?} {:?}",
                             game.game_state(),
                             game.round_state());

                    // Return players and spectators to the waiting pool.
                    let mut players_queue = player_queue_ref2.lock().unwrap();
                    players_queue.extend(players.into_clients());

                    sleep(&timer2, milliseconds(2000)).map(|_| continue_)
                }))
        }
    }))
}

fn sleep(timer: &Timer, ms: Milliseconds) -> Box<Future<Item = (), Error = ()>> {
    Box::new(timer.sleep(ms.into())
        .map(|_| ())
        .map_err(|_| ()))
}
