use std::io;
use std::time::Duration;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

use futures::{BoxFuture, Future, Stream, Sink, Poll, Async, AsyncSink};
use futures::sync::{mpsc, oneshot};
use tokio_timer::{Timer, Sleep};

use protocol::Msg;
use net::{other, other_labelled};

pub fn transmit(clients: HashMap<Id, CmdSink>, msgs: CommandMode<Id, Msg>) {
    let cmds = match msg {
        CommandMode::Constant(msg) => clients.keys().cloned().map(|id| (id, msg.clone())),
        CommandMode::Lookup(id_to_cmd) => clients.keys().cloned().map(|id| {
            // @TODO: Instead of `panic!`ing if no message set, return an Err.
            (id, id_to_cmd.remove(id).unwrap())
        })
    };
    command(clients, cmds)
}
