extern crate log;
extern crate env_logger;
extern crate futures;
extern crate tokio_core;
extern crate sirpent;
extern crate serde_json;
extern crate rand;
extern crate tokio_timer;

use std::env;
use std::str;
use rand::OsRng;
use std::thread;
use std::time::Duration;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::collections::{HashSet, HashMap};
use futures::{future, BoxFuture, Future, Stream, Sink};
use futures::sync::mpsc;
use tokio_core::net::TcpListener;
use tokio_core::reactor::{Core, Handle};
use tokio_core::io::Io;
use tokio_timer::Timer;

use sirpent::*;

fn main() {
    if let Err(ref e) = run() {
        println!("error: {}", e);

        for e in e.iter().skip(1) {
            println!("caused by: {}", e);
        }

        // The backtrace is not always generated. Try to run this example
        // with `RUST_BACKTRACE=1`.
        if let Some(backtrace) = e.backtrace() {
            println!("backtrace: {:?}", backtrace);
        }

        ::std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let n: u64 = serde_json::from_str("abc").chain_err(|| "unable to decode nonsense as a u64")?;

    Ok(())
}

// fn run() -> Result<()> {
// let n: u64 = serde_json::from_str("abc").chain_err(|| "unable to decode nonsense as a u64")?;
//
// Ok(())
// }
//
