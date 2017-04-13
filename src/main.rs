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
use rand::{Rng, OsRng};
use std::net::SocketAddr;
use futures::{stream, Future, Sink, Stream};
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

    let handshaker_actor = kabuki::Builder::new().spawn(&handle, {
        let nameserver_actor = kabuki::Builder::new().spawn(&handle, {
            Nameserver::default()
        });
        Handshake::new(grid.clone(), timeout, timer.clone(), nameserver_actor)
    });
    handle.spawn(server(listener,
                        handshaker_actor,
                        queue_player_tx.clone(),
                        spectator_tx));

    let game_server_actor_ref = kabuki::Builder::new().spawn(&handle, {
        let game_actor_ref = kabuki::Builder::new().spawn(&handle, {
            GameActor::new(timer.clone(), spectator_msg_tx)
        });
        let rng_fn = || -> Box<Rng> { Box::new(OsRng::new().unwrap()) };
        GameServerActor::new(rng_fn, grid, timeout, game_actor_ref)
    });

    let gsw = queue_player_rx
        .chunks(10)
        .map(|players_vec| Room::new(players_vec.into_iter().collect()))
        .and_then(move |players| game_server_actor_ref.clone().call(players))
        .map(|(game, players)| {
                 println!("End of game! {:?} {:?}",
                          game.game_state(),
                          game.round_state());
                 stream::iter(players.into_iter().map(Ok))
             })
        .flatten()
        .filter(Client::is_connected)
        .forward(queue_player_tx.sink_map_err(|_| ()))
        .map(|_| ());
    handle.spawn(gsw);

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
