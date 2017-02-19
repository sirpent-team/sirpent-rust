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
    Lookup(HashMap<Id, CmdSink>)
}

/// Sends `Cmd` down a group of `Sink`s. Intended for sending an arbitrary command
/// to a group of clients. Can send the same message to every client or a different
/// message for each client - see `CommandMode`.
pub struct ClientsCommand<Id, CmdSink>
    where Id: Eq + Hash + Clone + Send,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static
{
    send_queue: VecDeque<(Id, CmdSink)>,
    flushing_queue: VecDeque<(Id, CmdSink)>,
    completed: Option<HashMap<Id, CmdSink>>,
}

impl<Id, CmdSink> ClientsCommand<Id, CmdSink>
    where Id: Eq + Hash + Clone + Send,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static
{
    pub fn new(mut clients: HashMap<Id, CmdSink>, cmds: CommandMode<Id, CmdSink>) -> Self {
        // Drain clients into a queue for each to be sent `cmd`.
        // N.B. Drain retains the memory allocated by `clients` but it is now empty, so
        // we can reuse it!
        let send_queue = clients.drain().map(Self::pair_client_with_cmd);

        ClientsCommand {
            send_queue: send_queue.collect(),
            flushing_queue: VecDeque::new(),
            // Reuse newly-emptied clients for completions, as sufficient memory already allocated.
            completed: Some(clients)
        }
    }

    /// Identifies which `Cmd` a client should be using and returns the `(client, Cmd)` pair.
    fn pair_client_with_cmd(mut client: (Id, CmdSink), cmds: CommandMode<Id, CmdSink>) -> ((Id, CmdSink), Cmd) {
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
                Err(e) => complete_client(client_id, Err(e))
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
                Err(e) => complete_client(client_id, Err(e))
            };
        }

        // Reenqueue clients whose flushing is yet to be complete.
        self.flushing_queue.append(reenqueue);
    }

    /// Mark a client as complete. Nicer way to wrap taking out of Option.
    fn complete_client(client_id: Id, result: Result<CmdSink, CmdSink::Error>) {
        self.completed.as_mut().unwrap().insert(client_id, result)
    }
}

impl<Id, CmdSink> Future for ClientsCommand<Id, CmdSink>
    where Id: Eq + Hash + Clone + Send,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static
{
    type Item = HashMap<Id, CmdSink>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        // @TODO: Get a better sense of how Futures commonly do these guards.
        assert!(self.completed.is_some());

        self.poll_the_send_queue();
        self.poll_the_flushing_queue();

        // Complete when queues are depleted.
        if self.send_queue.is_empty() && self.flushing_queue.is_empty() {
            Ok(Async::Ready(self.completed.take().unwrap()))
        } else {
            Ok(Async::NotReady)
        }
    }
}
