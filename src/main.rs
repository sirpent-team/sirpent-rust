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
use std::cmp::min;
use std::time::Duration;
use std::convert::Into;
use std::net::SocketAddr;
use std::collections::HashSet;
use futures::{future, stream, Future, Sink, Stream};
use futures::sync::{mpsc, oneshot};
use tokio_core::net::TcpListener;
use tokio_core::reactor::{Core, Handle};
use tokio_timer::Timer;
use tokio_io::AsyncRead;
use comms::{Client, Room};
use std::cell::RefCell;
use std::rc::Rc;

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

    let (queue_player_tx, queue_player_rx) = mpsc::channel(3);
    let (dequeue_player_tx, dequeue_player_rx) = mpsc::channel(3);
    let clientqueue = clientqueue(queue_player_rx, dequeue_player_rx);
    handle.spawn(clientqueue.0);
    handle.spawn(clientqueue.1);

    let (spectator_tx, spectator_rx) = mpsc::channel(3);
    let (spectator_msg_tx, spectator_msg_rx) = mpsc::channel(3);
    let spectators = Spectators::new(spectator_rx, spectator_msg_rx);
    handle.spawn(spectators);

    // Run a nameserver to decide unique names for clients.
    let (nameserver_tx, nameserver_rx) = mpsc::channel(5);
    handle.spawn(nameserver(nameserver_rx));

    // Run TCP server to welcome clients and register them as players.
    handle.spawn(server(listener,
                        handle.clone(),
                        nameserver_tx,
                        grid,
                        timeout.unwrap(),
                        queue_player_tx.clone(),
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
        lp.run(play_games(grid,
                            dequeue_player_tx,
                            queue_player_tx,
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
          nameserver_tx: mpsc::Sender<(String, oneshot::Sender<String>)>,
          grid: Grid,
          timeout_millis: Milliseconds,
          player_tx: mpsc::Sender<MsgClient<String>>,
          spectator_tx: mpsc::Sender<MsgClient<String>>,
          timer: tokio_timer::Timer)
          -> Box<Future<Item = (), Error = ()>> {
    let server = listener
        .incoming()
        .map_err(|_| ())
        .for_each(move |(socket, addr)| {
            let msg_transport = socket.framed(MsgCodec);
            let client = Client::new(addr, msg_transport);

            let spectator_tx = spectator_tx.clone();
            let player_tx = player_tx.clone();

            let handshake_future = handshake(client,
                                             timeout_millis,
                                             timer.clone(),
                                             nameserver_tx.clone(),
                                             grid)
                    .map_err(|_| ())
                    .and_then(move |(client, kind)| -> Box<Future<Item = (), Error = ()>> {
                        match kind {
                            ClientKind::Player => {
                                Box::new(player_tx.send(client).map(|_| ()).map_err(|_| ()))
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
             nameserver_tx: mpsc::Sender<(String, oneshot::Sender<String>)>,
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
            let (name_tx, name_rx) = oneshot::channel();
            Box::new(nameserver_tx
                         .send((desired_name, name_tx))
                         .map_err(|_| ())
                         .and_then(move |_| {
                name_rx
                    .map_err(|_| ())
                    .and_then(move |name| {
                        let client = client.rename(name.clone());
                        let welcome_msg = Msg::Welcome {
                            name: name,
                            grid: grid.into(),
                            timeout_millis: Some(timeout_millis),
                        };
                        client
                            .transmit(welcome_msg)
                            .map(move |client| (client, kind))
                            .map_err(|_| ())
                    })
            }))
        } else {
            println!("Error welcoming client: unexpected {:?}", maybe_msg);
            Box::new(future::err(()))
        }
    }))
}

fn nameserver(desires: mpsc::Receiver<(String, oneshot::Sender<String>)>)
              -> Box<Future<Item = (), Error = ()>> {
    let mut names: HashSet<String> = HashSet::new();
    Box::new(desires.for_each(move |(desired_name, name_tx)| {
        let mut name = desired_name.clone();
        let mut n = 1;
        while names.contains(&name) {
            name = format!("{}_{}", desired_name, roman_numerals(n));
            n += 1;
        }
        names.insert(name.clone());
        let _ = name_tx.send(name);
        future::ok(())
    }))
}

fn clientqueue(add_rx: mpsc::Receiver<MsgClient<String>>,
               dequeue_rx: mpsc::Receiver<(usize,
                                           usize,
                                           oneshot::Sender<Vec<MsgClient<String>>>)>)
               -> (Box<Future<Item = (), Error = ()>>, Box<Future<Item = (), Error = ()>>) {
    let queue = Rc::new(RefCell::new(Vec::new()));

    let queue_ref = queue.clone();
    let add_future = add_rx
        .for_each(move |client| {
                      queue_ref.borrow_mut().push(client);
                      future::ok(())
                  })
        .map_err(|_| ());

    let queue_ref = queue.clone();
    let dequeue_future = dequeue_rx
        .for_each(move |(min_n, max_n, reply_tx)| {
            let mut queue_lock = queue_ref.borrow_mut();
            let mut n = min(max_n, queue_lock.len());
            if n < min_n {
                n = 0;
            }
            let reply = if n > 0 {
                queue_lock.drain(..n - 1).collect()
            } else {
                vec![]
            };
            reply_tx.send(reply).map_err(|_| ())
        })
        .map_err(|_| ());

    (Box::new(add_future), Box::new(dequeue_future))
}

fn play_games(grid: Grid,
              dequeue_players_tx: mpsc::Sender<(usize,
                                                usize,
                                                oneshot::Sender<Vec<MsgClient<String>>>)>,
              queue_players_tx: mpsc::Sender<MsgClient<String>>,
              spectators_ref: mpsc::Sender<Msg>,
              timer: Timer,
              timeout: Option<Milliseconds>)
              -> Box<Future<Item = (), Error = ()>> {
    Box::new(timer
                 .clone()
                 .interval(Duration::from_secs(10))
                 .map_err(|_| ())
                 .for_each(move |_| {
        let (tx, rx) = oneshot::channel();
        let timer = timer.clone();
        let grid = grid.clone();
        let spectators_ref = spectators_ref.clone();
        let queue_players_tx = queue_players_tx.clone();
        dequeue_players_tx
            .clone()
            .send((2, 10, tx))
            .map_err(|_| ())
            .and_then(move |_| {
                rx.map_err(|_| ())
                    .and_then(move |players_vec| -> Box<Future<Item = _, Error = _>> {
                        if players_vec.is_empty() {
                            println!("Not enough players yet.");
                            return Box::new(future::ok(()));
                        }

                        let players = Room::new(players_vec.into_iter().collect());
                        play_game(grid.clone(),
                                  players,
                                  queue_players_tx,
                                  spectators_ref,
                                  timer,
                                  timeout)
                    })
            })
    }))
}

fn play_game(grid: Grid,
             players: MsgRoom<String>,
             queue_players_tx: mpsc::Sender<MsgClient<String>>,
             spectators_ref: mpsc::Sender<Msg>,
             timer: Timer,
             timeout: Option<Milliseconds>)
             -> Box<Future<Item = (), Error = ()>> {
    let game = Game::new(OsRng::new().unwrap(), grid);
    Box::new(game_future(game, players, spectators_ref, timeout, timer)
                 .and_then(move |(game, players, _)| {
        println!("End of game! {:?} {:?}",
                 game.game_state(),
                 game.round_state());

        let players_ok = players.into_iter().map(Ok);
        queue_players_tx
            .send_all(stream::iter(players_ok))
            .map_err(|_| ())
            .map(|_| ())
    }))
}
