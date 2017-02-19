use std::io;
use std::time::Duration;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

use futures::{BoxFuture, Future, Stream, Sink, Poll, Async, AsyncSink};
use futures::sync::{mpsc, oneshot};
use tokio_timer::{Timer, Sleep};

use net::*;
use clients::*;

#[derive(Clone)]
pub enum MessageMode<Id>
    where Id: Eq + Hash + Clone + Send
{
    Constant(Msg),
    Lookup(HashMap<Id, Msg>),
}

pub fn group_transmit<Id, CmdSink>
    (clients: HashMap<Id, CmdSink>,
     msgs: MessageMode<Id>)
     -> BoxFuture<HashMap<Id, Result<CmdSink, CmdSink::SinkError>>, io::Error>
    where Id: Eq + Hash + Clone + Send + 'static,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static,
          CmdSink::SinkError: Send + 'static
{
    match msgs {
        MessageMode::Constant(msg) => {
            let cmd = CommandMode::Constant(Cmd::Transmit(msg));
            group_command(clients, cmd).boxed()
        }
        MessageMode::Lookup(id_to_msg) => {
            let pairs = clients.keys().cloned().map(|id| {
                // @TODO: Instead of `panic!`ing if no message set, return an Err.
                (id, Cmd::Transmit(id_to_msg.remove(&id).unwrap()))
            });
            group_command(clients, CommandMode::Lookup(pairs.collect())).boxed()
        }
    }
}
