//! An example [SOCKSv5] proxy server on top of futures
//!
//! [SOCKSv5]: https://www.ietf.org/rfc/rfc1928.txt
//!
//! This program is intended to showcase many aspects of the futures crate and
//! I/O integration, explaining how many of the features can interact with one
//! another and also provide a concrete example to see how easily pieces can
//! interoperate with one another.
//!
//! A SOCKS proxy is a relatively easy protocol to work with. Each TCP
//! connection made to a server does a quick handshake to determine where data
//! is going to be proxied to, another TCP socket is opened up to this
//! destination, and then bytes are shuffled back and forth between the two
//! sockets until EOF is reached.
//!
//! This server implementation is relatively straightforward, but
//! architecturally has a few interesting pieces:
//!
//! * The entire server only has one buffer to read/write data from. This global
//!   buffer is shared by all connections and each proxy pair simply reads
//!   through it. This is achieved by waiting for both ends of the proxy to be
//!   ready, and then the transfer is done.
//!
//! * Initiating a SOCKS proxy connection may involve a DNS lookup, which
//!   is done with the TRust-DNS futures-based resolver. This demonstrates the
//!   ease of integrating a third-party futures-based library into our futures
//!   chain.
//!
//! * The entire SOCKS handshake is implemented using the various combinators in
//!   the `futures` crate as well as the `tokio_core::io` module. The actual
//!   proxying of data, however, is implemented through a manual implementation
//!   of `Future`. This shows how it's easy to transition back and forth between
//!   the two, choosing whichever is the most appropriate for the situation at
//!   hand.
//!
//! You can try out this server with `cargo test` or just `cargo run` and
//! throwing connections at it yourself, and there should be plenty of comments
//! below to help walk you through the implementation as well!

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
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::str;
use std::time::Duration;
use std::collections::HashSet;
use tokio_core::io::Codec;

use futures::future;
use futures::{BoxFuture, Future, Stream, Sink};
use tokio_core::net::{TcpStream, TcpListener};
use tokio_core::reactor::Core;
use tokio_core::io::{Io, EasyBuf, Framed};

use sirpent::*;

// ---------------- ---------------- ---------------- ---------------- ----------------

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
    let buffer = Arc::new(RefCell::new(vec![0; 64 * 1024]));
    let handle = lp.handle();
    let listener = TcpListener::bind(&addr, &handle).unwrap();

    let game = Arc::new(RefCell::new("abc".to_string()));

    // Construct a future representing our server. This future processes all
    // incoming connections and spawns a new task for each client which will do
    // the proxy work.
    //
    // This essentially means that for all incoming connections, those received
    // from `listener`, we'll create an instance of `Client` and convert it to a
    // future representing the completion of handling that client. This future
    // itself is then *spawned* onto the event loop to ensure that it can
    // progress concurrently with all other connections.
    println!("Listening for socks5 proxy connections on {}", addr);
    let clients = listener.incoming().map(move |(socket, addr)| {
        let transport = socket.framed(MsgCodec);
        (Client.handshake(transport), addr)
    });
    let handle = lp.handle();

    let strings: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    let server = clients.for_each(|(client, addr)| {
        let strings_copy = strings.clone();
        handle.spawn(client.then(move |res| {
            match res {
                Ok((msg, transport)) => {
                    let identify_msg: ProtocolResult<IdentifyMsg> = msg.clone().to_typed();
                    if identify_msg.is_ok() {
                        let identify_msg = identify_msg.unwrap();
                        let mut name = identify_msg.desired_name.clone();
                        {
                            let mut strings_copy = strings_copy.lock().unwrap();
                            while strings_copy.contains(&name) {
                                name += "_";
                            }
                            strings_copy.insert(name.clone());
                        }
                        println!("deduped name {} to {} from {}",
                                 identify_msg.desired_name,
                                 name.clone(),
                                 addr);
                        // @TODO: Don't wait() - it blocks the thread!
                        match Client.welcome(transport, name, Grid::new(25), None).wait() {
                            Err(e) => panic!(e),
                            _ => {}
                        }
                    } else {
                        println!("could not decode identifymsg out of {:?} from {}",
                                 msg,
                                 addr);
                    }
                }
                Err(e) => println!("error for {}: {}", addr, e),
            }
            future::ok(())
        }));
        Ok(())
    });

    // Now that we've got our server as a future ready to go, let's run it!
    //
    // This `run` method will return the resolution of the future itself, but
    // our `server` futures will resolve to `io::Result<()>`, so we just want to
    // assert that it didn't hit an error.
    lp.run(server).unwrap();
}

// ---------------- ---------------- ---------------- ---------------- ----------------

type MsgTransport = Framed<TcpStream, MsgCodec>;
type MsgSendFuture = BoxFuture<MsgTransport, io::Error>;
type MsgOptReceiveFuture = BoxFuture<(Option<Msg>, MsgTransport), (io::Error, MsgTransport)>;
type MsgOptReceiveResult = Result<(Option<Msg>, MsgTransport), (io::Error, MsgTransport)>;
type MsgReceiveFuture = BoxFuture<(Msg, MsgTransport), io::Error>;
type MsgReceiveResult = Result<(Msg, MsgTransport), io::Error>;

// Data used to when processing a client to perform various operations over its
// lifetime.
struct Client;

// http://aturon.github.io/blog/2016/08/11/futures/
// https://raw.githubusercontent.com/tokio-rs/tokio-socks5/master/src/main.rs
impl Client {
    fn handshake(self, transport: MsgTransport) -> MsgReceiveFuture {
        let version_msg = Msg::from_typed(VersionMsg::new());
        let versioned: MsgSendFuture = transport.send(version_msg)
            .boxed();

        let registered: MsgReceiveFuture = versioned.and_then(|transport| {
                transport.into_future()
                    .then(recv_postprocess)
            })
            .boxed();

        registered
    }

    fn welcome(self,
               transport: MsgTransport,
               name: String,
               grid: Grid,
               timeout: Option<Duration>)
               -> MsgSendFuture {
        let welcome_msg = Msg::from_typed(WelcomeMsg {
            name: name,
            grid: grid,
            timeout: timeout,
        });
        let welcomed: MsgSendFuture = transport.send(welcome_msg)
            .boxed();
        welcomed
    }
}

fn other(desc: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, desc)
}

/// Postprocess result of MsgTransport::into_future() into something with these requirements:
/// * Err does not contain the MsgTransport. This is for type compatibility with
///   MsgTransport::send() errors.
/// * Ok with a None message is mapped to an Err. This is because for my purposes a missing Msg
///   is an error condition.
fn recv_postprocess(result: MsgOptReceiveResult) -> MsgReceiveResult {
    match result {
        Ok((Some(msg), transport)) => Ok((msg, transport)),
        Ok((None, _)) => Err(other("No Msg received.")),
        Err((e, _)) => Err(e),
    }
}

// ---------------- ---------------- ---------------- ---------------- ----------------

// https://github.com/tokio-rs/tokio-line/blob/master/src/framed_transport.rs
pub struct MsgCodec;

impl Codec for MsgCodec {
    type In = Msg;
    type Out = Msg;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Msg>, io::Error> {
        // If our buffer contains a newline...
        if let Some(n) = buf.as_ref().iter().position(|b| *b == b'\n') {
            // remove this line and the newline from the buffer.
            let line = buf.drain_to(n);
            buf.drain_to(1); // Also remove the '\n'.

            // Turn this data into a UTF string and return it in a Frame.
            let line = match str::from_utf8(line.as_ref()) {
                Ok(s) => s,
                Err(_) => return Err(io::Error::new(io::ErrorKind::Other, "invalid string")),
            };

            let msg: Result<Msg, serde_json::Error> = serde_json::from_str(line);
            return match msg {
                Ok(msg) => Ok(Some(msg)),
                Err(e) => Err(other(&format!("Msg decode error: {}", e))),
            };
        }

        Ok(None)
    }

    fn encode(&mut self, msg: Msg, buf: &mut Vec<u8>) -> io::Result<()> {
        let msg_str = match serde_json::to_string(&msg) {
            Ok(s) => s,
            Err(e) => return Err(other(&format!("Msg encode error: {}", e))),
        };

        for byte in msg_str.as_bytes() {
            buf.push(*byte);
        }

        buf.push(b'\n');
        Ok(())
    }
}
