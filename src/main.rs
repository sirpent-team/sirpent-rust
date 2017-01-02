#[macro_use]
extern crate log;
extern crate env_logger;
#[macro_use]
extern crate futures;
#[macro_use]
extern crate tokio_core;
extern crate sirpent;
extern crate serde_json;

use std::cell::RefCell;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::str;
use std::collections::HashSet;

use futures::{Future, Stream};
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

    let names: Arc<RefCell<HashSet<String>>> = Arc::new(RefCell::new(HashSet::new()));

    let clients = listener.incoming()
        .map_err(|e| ProtocolError::from(e))
        .map(move |(socket, addr)| {
            let transport = socket.framed(MsgCodec);
            // Say hello and get a desired_name from the client.
            (Client.handshake(transport), addr)
        });

    let handle = lp.handle();
    let server = clients.for_each(|(client, addr)| {
        let names_ref = names.clone();

        handle.spawn(client.map_err(|_| ()).and_then(move |(identify_msg, transport)| {
            // Find an unused name based upon the desired_name.
            // Subtly coded to ensure `names` is locked to ensure unique name still free.
            let mut name = identify_msg.desired_name;
            loop {
                let mut names_ref = names_ref.borrow_mut();
                if names_ref.contains(&name) {
                    name += "_";
                } else {
                    // Reserve the new name.
                    names_ref.insert(name.clone());
                    break;
                }
            }
            // @DEBUG
            println!("addr={:?} name={:?}", addr, name.clone());

            // future::ok(())
            Client.welcome(transport, name, Grid::new(25), None).then(|_| Ok(()))
        }));
        Ok(())
    });

    lp.run(server).unwrap();
}
