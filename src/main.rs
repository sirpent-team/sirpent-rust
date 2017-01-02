#[macro_use]
extern crate log;
extern crate env_logger;
#[macro_use]
extern crate futures;
#[macro_use]
extern crate tokio_core;
extern crate sirpent;
extern crate serde_json;
extern crate rand;

use std::cell::RefCell;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::str;
use rand::OsRng;

use futures::{future, Future, Stream};
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;
use tokio_core::io::Io;

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
    let listener = TcpListener::bind(&addr, &handle).unwrap();

    println!("Listening on {}", addr);

    let grid = Grid::new(25);
    let timeout = None;

    let engine: Arc<RefCell<Engine<OsRng>>> =
        Arc::new(RefCell::new(Engine::new(OsRng::new().unwrap(), grid)));

    let clients = listener.incoming()
        .map_err(|e| ProtocolError::from(e))
        .map(move |(socket, addr)| {
            let transport = socket.framed(MsgCodec);
            // Say hello and get a desired_name from the client.
            (Client.handshake(transport), addr)
        });

    let handle = lp.handle();
    let server = clients.for_each(|(client, addr)| {
        let engine_ref1 = engine.clone();
        let engine_ref2 = engine.clone();
        let engine_ref3 = engine.clone();

        let client_future = client.and_then(move |(identify_msg, transport)| {
                let name = engine_ref1.borrow_mut().add_player(identify_msg.desired_name);
                // @DEBUG
                println!("addr={:?} name={:?}", addr, name.clone());

                Client.welcome(transport, name.clone(), grid, timeout)
            })
            .and_then(move |transport| {
                let game: GameState = engine_ref2.borrow().game.game.clone();
                Client.game(transport, game)
            })
            .and_then(move |transport| {
                let turn: TurnState = engine_ref3.borrow().game.turn.clone();
                Client.turn(transport, turn)
            })
            .and_then(move |(move_msg, transport)| {
                println!("{:?}", move_msg);
                Ok(())
            })
            .then(|result| {
                match result {
                    Ok(_) => Ok(()),
                    Err(_) => Err(()),
                }
            });

        handle.spawn(client_future);
        Ok(())
    });

    // move_future --> turn_future --> move_future

    lp.run(server).unwrap();
}
