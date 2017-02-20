use std::hash::Hash;
use std::fmt::Debug;
use std::collections::HashMap;

use futures::{BoxFuture, Future, Sink};

use clients::*;

#[derive(Clone, Debug)]
pub enum MessageMode<Id>
    where Id: Eq + Hash + Clone + Debug + Send
{
    Constant(Msg),
    Lookup(HashMap<Id, Msg>),
}

pub fn group_transmit<Id, CmdSink>(clients: HashMap<Id, CmdSink>,
                                   msgs: MessageMode<Id>)
                                   -> BoxFuture<HashMap<Id, Result<CmdSink>>, Error>
    where Id: Eq + Hash + Clone + Debug + Send + 'static,
          CmdSink: Sink<SinkItem = Cmd, SinkError = Error> + Send + 'static
{
    match msgs {
        MessageMode::Constant(msg) => {
            let cmd = CommandMode::Constant(Cmd::Transmit(msg));
            group_command(clients, cmd).boxed()
        }
        MessageMode::Lookup(mut id_to_msg) => {
            let pairs = clients.keys()
                .map(|id| {
                    // @TODO: Instead of `panic!`ing if no message set, return an Err.
                    (id.clone(), Cmd::Transmit(id_to_msg.remove(&id).unwrap()))
                })
                .collect();
            group_command(clients, CommandMode::Lookup(pairs)).boxed()
        }
    }
}
