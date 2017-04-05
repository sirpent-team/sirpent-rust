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

    let mut lp = Core::new().unwrap();
    let handle = lp.handle();

    let listener = TcpListener::bind(&addr, &handle).unwrap();
    println!("Listening on {}", addr);

    let grid = Grid::new(25);
    let timer = Timer::default();
    let timeout: Option<Milliseconds> = Some(Milliseconds::new(5000));

    let names: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let players: Arc<Mutex<Vec<MsgClient<String>>>> = Arc::new(Mutex::new(Vec::new()));

    let (spectator_tx, spectator_rx) = mpsc::channel(3);
    let (spectator_msg_tx, spectator_msg_rx) = mpsc::channel(3);
    let spectators = Spectators::new(spectator_rx, spectator_msg_rx);
    handle.spawn(spectators);

    let handshake_fn = handshake(grid.clone(), names.clone(), timeout.unwrap(), timer.clone());
    let handshake_actor = Builder::new().spawn_fn(handshake_fn);

    let accept_fn = |(socket, addr)| {
        let msg_transport = socket.framed(MsgCodec);
        let client = Client::new(addr, msg_transport);

        handshake_actor.call(client).map(|(client, kind)| {
            match kind {
                ClientKind::Player => {
                    player_queue.lock().unwrap().push(client);
                }
                ClientKind::Spectator => {
                    spectators.lock().unwrap().push(client);
                }
            }
        }
    };
    let accept_actor = Builder::new().spawn_fn(accept_fn);

    let server = listener.incoming().for_each(|pair| accept_actor.call(pair));
    handle.spawn(server.map_err(|_| ()));

    let game_fn = game(grid.clone(), spectator_tx, timeout.unwrap(), timer.clone());
    let game_actor = Builder::new().spawn_fn(game_fn);
    let games_at_interval = timer.interval(Duration::from_secs(10)).for_each(|_| {
        let players = player_queue.drain(..).collect();
        game_actor.call(players).and_then(|(players, game)| {
            player_queue.extend(players);
            println!("game completed: {:?}", game);
        });
    });
    handle.spawn(games_at_interval.map_err(|_| ()));

    // Poll event loop to keep program running.
    loop {
        lp.turn(None);
    }
}

fn handshake(grid: Grid,
             mut names: Arc<Mutex<HashSet<String>>>,
             timeout_millis: Milliseconds,
             timer: tokio_timer::Timer)
             -> ActorRef<MsgClient<SocketAddr>, (MsgClient<String>, ClientKind), ()> {

    let version_msg = Msg::version();
    client.transmit(version_msg).and_then(move |client| {
        client.receive().with_hard_timeout(*timeout_millis, &timer)
    }).and_then(move |(msg, client)| {
        if let Msg::Register { desired_name, kind } = maybe_msg {
            let name = find_unique_name(&mut names, desired_name);
            let client = client.rename(name.clone());
            let welcome_msg = Msg::Welcome {
                name: name,
                grid: grid.into(),
                timeout_millis: Some(timeout_millis),
            };
            client.transmit(welcome_msg).map(move |client| (client, kind))
        } else {
            future::err(client)
        }
    }).map_err(|_| ())

}

fn register_msg_fields((msg, client): ) -> Result<MsgClient<>, MsgClient<>>


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
