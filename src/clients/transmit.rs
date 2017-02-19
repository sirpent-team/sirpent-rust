use std::io;
use std::time::Duration;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

use futures::{BoxFuture, Future, Stream, Sink, Poll, Async, AsyncSink};
use futures::sync::{mpsc, oneshot};
use tokio_timer::{Timer, Sleep};

use net::*;
use clients::*;

pub fn group_transmit<Id, CmdSink>(clients: HashMap<Id, CmdSink>, msgs: CommandMode<Id, Msg>)
    where Id: Eq + Hash + Clone + Send,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static
{
    let cmds = match msgs {
        CommandMode::Constant(msg) => clients.keys().cloned().map(|id| (id, msg.clone())),
        CommandMode::Lookup(id_to_cmd) => {
            clients.keys().cloned().map(|id| {
                // @TODO: Instead of `panic!`ing if no message set, return an Err.
                (id, id_to_cmd.remove(id).unwrap())
            })
        }
    };
    group_command(clients, cmds)
}
