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
use std::net::SocketAddr;
use futures::{future, stream, Future, Sink, Stream};
use futures::future::Either;
use futures::sync::mpsc;
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;
use tokio_timer::Timer;
use tokio_io::AsyncRead;
use comms::{Client, Room};

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
    let timeout = Milliseconds::new(5000);

    let (queue_player_tx, queue_player_rx) = mpsc::channel(3);

    let (spectator_tx, spectator_rx) = mpsc::channel(3);
    let (spectator_msg_tx, spectator_msg_rx) = mpsc::channel(3);
    let spectators = Spectators::new(spectator_rx, spectator_msg_rx);
    handle.spawn(spectators);

    let nameserver = Nameserver::default();
    let nameserver_actor = kabuki::Builder::new().spawn(&handle, nameserver);
    let handshaker = Handshake::new(grid.clone(), timeout, timer.clone(), nameserver_actor);
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
        let handle = lp.handle();

        let game_actor = GameActor::new(timer.clone(), spectator_msg_tx);
        let game_actor = kabuki::Builder::new().spawn(&handle, game_actor);

        lp.run(play_games(grid,
                            queue_player_tx,
                            Box::new(queue_player_rx.chunks(10)),
                            game_actor,
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

fn play_games(grid: Grid,
              add_tx: mpsc::Sender<MsgClient<String>>,
              add_rx: Box<Stream<Item = Vec<MsgClient<String>>, Error = ()>>,
              mut game_actor: kabuki::ActorRef<(Game, MsgRoom<String>, Milliseconds),
                                               (Game, MsgRoom<String>),
                                               ()>,
              timeout: Milliseconds)
              -> Box<Future<Item = (), Error = ()>> {
    let future = add_rx
        .then(|res| match res {
                  Ok(players_vec) => {
                      if players_vec.is_empty() {
                          Ok(None)
                      } else {
                          Ok(Some(players_vec))
                      }
                  }
                  Err(()) => Ok(None),
              })
        .for_each(move |res| if let Some(players_vec) = res {
                      let players = Room::new(players_vec.into_iter().collect());
                      let game = Game::new(Box::new(OsRng::new().unwrap()), grid.clone());
                      let add_tx = add_tx.clone();
                      Either::A(game_actor
                                    .call((game, players, timeout))
                                    .and_then(move |(game, players)| {
                println!("End of game! {:?} {:?}",
                         game.game_state(),
                         game.round_state());

                let players_ok = players.into_iter().filter(Client::is_connected).map(Ok);
                add_tx
                    .send_all(stream::iter(players_ok))
                    .map_err(|_| ())
                    .map(|_| ())
            }))
                  } else {
                      Either::B(future::ok(()))
                  });
    Box::new(future)
}
