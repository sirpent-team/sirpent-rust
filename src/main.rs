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
extern crate kabuki;

use std::env;
use std::str;
use rand::OsRng;
use std::thread;
use std::cmp::min;
use std::time::Duration;
use std::net::SocketAddr;
use futures::{future, stream, Future, Sink, Stream};
use futures::sync::{mpsc, oneshot};
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;
use tokio_timer::Timer;
use tokio_io::AsyncRead;
use comms::{Client, Room};
use std::cell::RefCell;
use std::rc::Rc;

use sirpent::utils::*;
use sirpent::net::*;
use sirpent::engine::*;
use sirpent::state::*;
use sirpent::actors::*;

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

    let nameserver = Nameserver::default();
    let nameserver_actor = kabuki::Builder::new().spawn(&handle, nameserver);
    let handshaker = Handshake::new(grid.clone(),
                                    timeout.unwrap(),
                                    timer.clone(),
                                    nameserver_actor);
    let handshaker_actor = kabuki::Builder::new().spawn(&handle, handshaker);
    handle.spawn(server(listener,
                        handshaker_actor,
                        queue_player_tx.clone(),
                        spectator_tx));

    // @TODO: Game requirements:
    // * Take existing player clients and play a game of sirpent with them until completion.
    // * Once game is concluded return player clients to the pool.
    // * After a short wait Milliseconds play a new game, as before with all pooled player clients.
    // * Continue indefinitely.
    thread::spawn(move || {
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
          handshaker_actor: kabuki::ActorRef<MsgClient<SocketAddr>,
                                             (MsgClient<String>, ClientKind),
                                             ()>,
          player_tx: mpsc::Sender<MsgClient<String>>,
          spectator_tx: mpsc::Sender<MsgClient<String>>)
          -> Box<Future<Item = (), Error = ()>> {
    let server = listener
        .incoming()
        .map_err(|_| ())
        .for_each(move |(socket, addr)| {
            let msg_transport = socket.framed(MsgCodec);
            let unnamed_client = Client::new(addr, msg_transport);

            let player_tx = player_tx.clone();
            let spectator_tx = spectator_tx.clone();
            handshaker_actor
                .clone()
                .call(unnamed_client)
                .map_err(|_| ())
                .map(move |(client, kind)| match kind {
                         ClientKind::Player => (client, player_tx),
                         ClientKind::Spectator => (client, spectator_tx),
                     })
                .and_then(|(client, tx)| tx.send(client).map_err(|_| ()))
                .then(|_| Ok(()))
        })
        .then(|_| Ok(()));
    Box::new(server)
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
                queue_lock.drain(..n).collect()
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

        let dequeue_players_tx_future = dequeue_players_tx
            .clone()
            .send((2, 10, tx))
            .map_err(|_| ());
        let dequeue_players_rx_future = rx.map_err(|_| ());
        dequeue_players_tx_future
            .join(dequeue_players_rx_future)
            .and_then(move |(_, players_vec)| -> Box<Future<Item = _, Error = _>> {
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
    let game_future = game_future(game, players, spectators_ref, timeout, timer)
        .and_then(move |(game, players, _)| {
            println!("End of game! {:?} {:?}",
                     game.game_state(),
                     game.round_state());

            let players_ok = players.into_iter().map(Ok);
            queue_players_tx
                .send_all(stream::iter(players_ok))
                .map_err(|_| ())
                .map(|_| ())
        });
    Box::new(game_future)
}
