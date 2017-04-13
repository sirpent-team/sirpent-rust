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

    let mut lp = Core::new().unwrap();
    let handle = lp.handle();
    let timer = Timer::default();

    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());
    let addr = addr.parse::<SocketAddr>().unwrap();

    let listener = TcpListener::bind(&addr, &handle).unwrap();
    println!("Listening on {}", addr);

    // ----------------------------------------------------------------

    let grid = Grid::new(25);
    let timeout = Milliseconds::new(5000);

    // ----------------------------------------------------------------

    let (player_tx, player_rx) = mpsc::channel(3);
    let (spectator_tx, spectator_rx) = mpsc::channel(3);
    let (spectate_msg_tx, spectate_msg_rx) = mpsc::channel(3);

    let handshaker_actor = kabuki::Builder::new().spawn(&handle, {
        Handshake::new(grid.clone(), timeout, timer.clone(), {
            kabuki::Builder::new().spawn(&handle, Nameserver::default())
        })
    });

    let (client_tx, client_rx) = mpsc::channel(3);
    let player_tx_clone = player_tx.clone();
    handle.spawn(client_rx.for_each(move |client| {
        let player_tx = player_tx_clone.clone();
        let spectator_tx = spectator_tx.clone();
        handshaker_actor
            .clone()
            .call(client)
            .and_then(move |(client, kind)| {
                match kind {
                        ClientKind::Player => player_tx,
                        ClientKind::Spectator => spectator_tx,
                    }
                    .send(client)
                    .map_err(|_| ())
                    .map(|_| ())
            })
    }));
    handle.spawn(listener
                     .incoming()
                     .map_err(|_| ())
                     .for_each(move |(socket, addr)| {
                                   let client = Client::new(addr, socket.framed(MsgCodec));
                                   client_tx
                                       .clone()
                                       .send(client)
                                       .map_err(|_| ())
                                       .map(|_| ())
                               }));

    // ----------------------------------------------------------------

    let spectators = Spectators::new(spectator_rx, spectate_msg_rx);
    handle.spawn(spectators);

    // ----------------------------------------------------------------

    let game_server_actor_ref = kabuki::Builder::new().spawn(&handle, {
        let game_actor_ref = kabuki::Builder::new().spawn(&handle, {
            GameActor::new(timer.clone(), spectate_msg_tx)
        });
        let rng_fn = || -> Box<Rng> { Box::new(OsRng::new().unwrap()) };
        GameServerActor::new(rng_fn, grid, timeout, game_actor_ref)
    });

    let gsw = player_rx
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
        .forward(player_tx.sink_map_err(|_| ()))
        .map(|_| ());
    handle.spawn(gsw);

    // ----------------------------------------------------------------

    // Poll event loop to keep program running.
    loop {
        lp.turn(None);
    }
}
