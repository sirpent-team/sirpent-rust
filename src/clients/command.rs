use std::io;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::fmt::Debug;

use futures::{BoxFuture, Future, Stream, Sink, Poll, Async, AsyncSink};

use clients::*;

/// Determines which message should each client send.
pub enum CommandMode<Id>
    where Id: Eq + Hash + Clone + Debug + Send
{
    /// Send an identical command to all clients.
    Constant(Cmd),
    /// Send a different message to each client.
    Lookup(HashMap<Id, Cmd>),
}

/// Sends `Cmd` down a group of `Sink`s. Intended for sending an arbitrary command
/// to a group of clients. Can send the same message to every client or a different
/// message for each client - see `CommandMode`.
pub fn group_command<Id, CmdSink>
    (clients: HashMap<Id, CmdSink>,
     cmds: CommandMode<Id>)
     -> BoxFuture<HashMap<Id, Result<CmdSink, CmdSink::SinkError>>, io::Error>
    where Id: Eq + Hash + Clone + Debug + Send + 'static,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static,
          CmdSink::SinkError: Send + 'static
{
    // @TODO: Try to squash the `Vec<Vec<_>>` somehow.
    GroupCommand::new(clients, cmds)
        .collect()
        .map(|nested_vec_results| {
            nested_vec_results.into_iter().flat_map(|v| v.into_iter()).collect::<HashMap<_, _>>()
        })
        .boxed()
}

/// Sends `Cmd` down a group of `Sink`s. See `command` for details.
///
/// This is implemented as a `Stream` so completed clients can be used before the entire
/// group has completed.
pub struct GroupCommand<Id, CmdSink>
    where Id: Eq + Hash + Clone + Debug + Send,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static
{
    send_queue: VecDeque<((Id, CmdSink), Cmd)>,
    flushing_queue: VecDeque<(Id, CmdSink)>,
    completed: Vec<(Id, Result<CmdSink, CmdSink::SinkError>)>,
}

impl<Id, CmdSink> GroupCommand<Id, CmdSink>
    where Id: Eq + Hash + Clone + Debug + Send,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static
{
    pub fn new(mut clients: HashMap<Id, CmdSink>, mut cmds: CommandMode<Id>) -> Self {
        // Drain clients into a queue for each to be sent `cmd`.
        let send_queue = clients.drain()
            .map(|client| Self::pair_client_with_cmd(client, &mut cmds));

        GroupCommand {
            send_queue: send_queue.collect(),
            flushing_queue: VecDeque::new(),
            completed: Vec::new(),
        }
    }

    /// Identifies which `Cmd` a client should be using and returns the `(client, Cmd)` pair.
    fn pair_client_with_cmd(client: (Id, CmdSink),
                            cmds: &mut CommandMode<Id>)
                            -> ((Id, CmdSink), Cmd) {
        let cmd = match cmds {
            &mut CommandMode::Constant(ref cmd) => cmd.clone(),
            &mut CommandMode::Lookup(ref mut id_to_cmd) => {
                // @TODO: Instead of `panic!`ing if no message set, return an Err.
                // We can't silently pass them into `completed` because it will give this type
                // nasty semantics.
                id_to_cmd.remove(&client.0).unwrap()
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

        while let Some(((client_id, mut cmd_tx), cmd)) = self.send_queue.pop_front() {
            match cmd_tx.start_send(cmd) {
                // If the command was sent successfully, queue it for polling.
                Ok(AsyncSink::Ready) => self.flushing_queue.push_back((client_id, cmd_tx)),
                // If the command could not be sent, requeue it for trying later.
                Ok(AsyncSink::NotReady(cmd)) => reenqueue.push_back(((client_id, cmd_tx), cmd)),
                // If sending the command errored, we can assume the Sink is forever unable to accept
                // further items. We let the Sink drop and record this failure.
                Err(e) => self.complete_client(client_id, Err(e)),
            };
        }

        // Reenqueue clients whose channel temporarily wasn't ready to be sent data.
        self.send_queue.append(&mut reenqueue);
    }

    /// Make progress flushing commands down their CmdSinks.
    ///
    /// Applies `Sink::poll_complete` for each client until they are flushed.
    fn poll_the_flushing_queue(&mut self) {
        // Maintain a separate list of clients to be requeued, so that the loop terminates.
        let mut reenqueue = VecDeque::new();

        while let Some((client_id, mut cmd_tx)) = self.flushing_queue.pop_front() {
            match cmd_tx.poll_complete() {
                // If the command was flushed successfully, record the success and the Sink.
                Ok(Async::Ready(())) => {
                    println!("flushing complete for {:?}", client_id);
                    self.complete_client(client_id, Ok(cmd_tx))
                }
                // If the command could not be sent, requeue it for trying later.
                Ok(Async::NotReady) => reenqueue.push_back((client_id, cmd_tx)),
                // If polling the Sink errored, we can assume the Sink is forever unable to make progress.
                // We let the Sink drop and record this failure.
                Err(e) => self.complete_client(client_id, Err(e)),
            };
        }

        // Reenqueue clients whose flushing is yet to be complete.
        self.flushing_queue.append(&mut reenqueue);
    }

    /// Mark a client as complete. Nicer way to wrap taking out of Option.
    fn complete_client(&mut self, client_id: Id, result: Result<CmdSink, CmdSink::SinkError>) {
        self.completed.push((client_id, result))
    }
}

impl<Id, CmdSink> Stream for GroupCommand<Id, CmdSink>
    where Id: Eq + Hash + Clone + Debug + Send,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static
{
    type Item = Vec<(Id, Result<CmdSink, CmdSink::SinkError>)>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.poll_the_send_queue();
        self.poll_the_flushing_queue();

        // Complete when queues are depleted.
        if !self.completed.is_empty() {
            let completed = self.completed.drain(..).collect();
            Ok(Async::Ready(Some(completed)))
        } else if self.send_queue.is_empty() && self.flushing_queue.is_empty() {
            Ok(Async::Ready(None))
        } else {
            Ok(Async::NotReady)
        }
    }
}
