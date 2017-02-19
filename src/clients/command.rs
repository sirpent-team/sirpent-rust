use std::io;
use std::time::Duration;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

use futures::{BoxFuture, Future, Stream, Sink, Poll, Async, AsyncSink};
use futures::sync::{mpsc, oneshot};
use tokio_timer::{Timer, Sleep};

use protocol::Msg;
use net::{other, other_labelled};

/// Determines which message should each client send.
pub enum CommandMode<Id, CmdSink> {
    /// Send an identical command to all clients.
    Constant(Cmd),
    /// Send a different message to each client.
    Lookup(HashMap<Id, CmdSink>),
}

/// Sends `Cmd` down a group of `Sink`s. Intended for sending an arbitrary command
/// to a group of clients. Can send the same message to every client or a different
/// message for each client - see `CommandMode`.
pub fn group_command
    (clients: HashMap<Id, CmdSink>,
     cmds: CommandMode<Id, CmdSink>)
     -> BoxFuture<Item = HashMap<Id, Result<CmdSink, CmdSink::Error>>, Error = io::Error> {
    // @TODO: Try to squash the `Vec<Vec<_>>` somehow.
    GroupCommand::new(clients, cmds)
        .collect()
        .and_then(|nested_vec_results| nested_vec_results.flat_map().collect())
}

/// Sends `Cmd` down a group of `Sink`s. See `command` for details.
///
/// This is implemented as a `Stream` so completed clients can be used before the entire
/// group has completed.
pub struct GroupCommand<Id, CmdSink>
    where Id: Eq + Hash + Clone + Send,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static
{
    send_queue: VecDeque<(Id, CmdSink)>,
    flushing_queue: VecDeque<(Id, CmdSink)>,
    completed: Vec<(Id, CmdSink)>,
}

impl<Id, CmdSink> GroupCommand<Id, CmdSink>
    where Id: Eq + Hash + Clone + Send,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static
{
    pub fn new(mut clients: HashMap<Id, CmdSink>, mut cmds: CommandMode<Id, CmdSink>) -> Self {
        // Drain clients into a queue for each to be sent `cmd`.
        let send_queue = clients.drain().map(|client| self.pair_client_with_cmd(client, &mut cmds));

        GroupCommand {
            send_queue: send_queue.collect(),
            flushing_queue: VecDeque::new(),
            completed: Vec::new(),
        }
    }

    /// Identifies which `Cmd` a client should be using and returns the `(client, Cmd)` pair.
    fn pair_client_with_cmd(mut client: (Id, CmdSink),
                            &mut cmds: CommandMode<Id, CmdSink>)
                            -> ((Id, CmdSink), Cmd) {
        let cmd = match cmds {
            CommandMode::Constant(cmd) => cmd.clone(),
            CommandMode::Lookup(id_to_cmd) => {
                // @TODO: Instead of `panic!`ing if no message set, return an Err.
                // We can't silently pass them into `completed` because it will give this type
                // nasty semantics.
                id_to_cmd.remove(client.0).unwrap()
            }
        };
        (client, cmd)
    }

    /// Make progress starting to send the commands down their CmdSinks.
    ///
    /// Applies `Sink::start_send` for each client until they are finished.
    fn poll_the_send_queue(&mut self) {
        // Maintain a separate list of clients to be requeued, so that the loop terminates.
        let mut reenqueue = VecDeque::new();

        while let Some((client, cmd)) = self.send_queue.pop_front() {
            let (client_id, cmd_tx) = client;

            match cmd_tx.start_send(cmd) {
                // If the command was sent successfully, queue it for polling.
                Ok(AsyncSink::Ready) => self.flushing_queue.push_back(client),
                // If the command could not be sent, requeue it for trying later.
                Ok(AsyncSink::NotReady(client_cmd)) => reenqueue.push_back((client, cmd)),
                // If sending the command errored, we can assume the Sink is forever unable to accept
                // further items. We let the Sink drop and record this failure.
                Err(e) => complete_client(client_id, Err(e)),
            };
        }

        // Reenqueue clients whose channel temporarily wasn't ready to be sent data.
        self.send_queue.append(reenqueue);
    }

    /// Make progress flushing commands down their CmdSinks.
    ///
    /// Applies `Sink::poll_complete` for each client until they are flushed.
    fn poll_the_flushing_queue(&mut self) {
        // Maintain a separate list of clients to be requeued, so that the loop terminates.
        let mut reenqueue = VecDeque::new();

        while let Some((client, cmd)) = self.flushing_queue.pop_front() {
            let (client_id, cmd_tx) = client;

            match cmd_tx.poll_complete(cmd) {
                // If the command was flushed successfully, record the success and the Sink.
                Ok(AsyncSink::Ready) => complete_client(client_id, Ok(cmd_tx)),
                // If the command could not be sent, requeue it for trying later.
                Ok(AsyncSink::NotReady(client_cmd)) => reenqueue.push_back((client, cmd)),
                // If polling the Sink errored, we can assume the Sink is forever unable to make progress.
                // We let the Sink drop and record this failure.
                Err(e) => complete_client(client_id, Err(e)),
            };
        }

        // Reenqueue clients whose flushing is yet to be complete.
        self.flushing_queue.append(reenqueue);
    }

    /// Mark a client as complete. Nicer way to wrap taking out of Option.
    fn complete_client(client_id: Id, result: Result<CmdSink, CmdSink::Error>) {
        self.completed.push((client_id, result))
    }
}

impl<Id, CmdSink> Stream for GroupCommand<Id, CmdSink>
    where Id: Eq + Hash + Clone + Send,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static
{
    type Item = Vec<(Id, CmdSink)>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.poll_the_send_queue();
        self.poll_the_flushing_queue();

        // Complete when queues are depleted.
        if !self.completed.is_empty() {
            let completed = self.completed;
            self.completed = Vec::new();
            Ok(Async::Ready(Some(completed)))
        } else if self.send_queue.is_empty() && self.flushing_queue.is_empty() {
            Ok(Async::Ready(None))
        } else {
            Ok(Async::NotReady)
        }
    }
}
