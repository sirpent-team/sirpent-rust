use std::io;
use std::time::Duration;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

use futures::{BoxFuture, Future, Stream, Sink, Poll, Async, AsyncSink};
use futures::sync::{mpsc, oneshot};
use tokio_timer::{Timer, Sleep};

use protocol::Msg;
use net::{other, other_labelled};

pub fn group_command
    (clients: HashMap<Id, CmdSink>)
     -> BoxFuture<Item = HashMap<Id, Result<(Msg, CmdSink), CmdSink::Error>>, Error = io::Error> {
    // @TODO: Try to squash the `Vec<Vec<_>>` somehow.
    GroupReceive::new(clients)
        .collect()
        .and_then(|nested_vec_results| nested_vec_results.flat_map().collect())
}

pub struct GroupReceive<Id, CmdSink>
    where Id: Eq + Hash + Clone + Send,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static
{
    command_stream: Option<GroupCommand<Id, CmdSink>>,
    ready_queue: HashMap<Id, oneshot::Receiver<Msg>>,
    receive_queue: VecDeque<(Id, CmdSink, oneshot::Receiver<Msg>)>,
    completed: Vec<(Id, (Msg, CmdSink))>,
}

impl<Id, CmdSink> GroupReceive<Id, CmdSink>
    where Id: Eq + Hash + Clone + Send,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static
{
    pub fn new(clients: HashMap<Id, CmdSink>) -> Self {
        let mut cmds = HashMap::new();
        let mut receive_queue = VecDeque::new();
        for client_id in clients.keys() {
            let (cmd, oneshot_rx) = self.new_client_oneshot();
            cmds.insert(client_id.clone(), cmd);
            ready_queue.insert(client_id.clone(), oneshot_rx);
        }

        GroupReceive {
            command_stream: Some(GroupCommand(clients, cmds)),
            receive_queue: receive_queue,
            completed: Vec::new(),
        }
    }

    fn new_client_oneshot() -> (Cmd, oneshot::Receiver<Msg>) {
        // Create a oneshot channel for the received message to be passed back to us on.
        let (oneshot_tx, oneshot_rx) = oneshot::channel();
        // We transmit this oneshot's tx along the channel to `Client` and then wait for
        // a reply from the oneshot's rx. This (perhaps surprisingly) delivers nicer code.
        let cmd = Cmd::ReceiveInto(oneshot_tx);

        (cmd, oneshot_rx)
    }

    /// Make progress receiving messages from clients.
    ///
    /// Applies `oneshot::Receiver::poll` for each client until a message arrives.
    fn poll_the_receive_queue(&mut self) {
        // Maintain a separate list of clients to be requeued, so that the loop terminates.
        let mut reenqueue = VecDeque::new();

        while let Some((client_id, cmd_tx, oneshot_rx)) = self.receive_queue.pop_front() {
            match oneshot_rx.poll() {
                Ok(Async::Ready(msg)) => complete_client(client_id, Ok((msg, cmd_tx))),
                Ok(Async::NotReady) => reenqueue.push_back((client_id, cmd_tx, oneshot_rx)),
                Err(e) => complete_client(client_id, Err(e)),
            }
        }

        // Reenqueue clients who weren't ready yet.
        self.receive_queue.append(reenqueue);
    }

    fn enqueue_for_receive(commanded_clients: Vec<(Id, CmdSink)>) {
        for (client_id, result) in commanded_clients.into_iter() {
            match result {
                Ok(cmd_tx) => {
                    let (_, oneshot_rx) = self.ready_queue.remove(client_id).unwrap();
                    self.receive_queue.insert((client_id, cmd_tx, oneshot_rx));
                }
                Err(e) => self.complete_client(client_id, Err(e)),
            }
        }
    }

    /// Mark a client as complete. Nicer way to wrap taking out of Option.
    fn complete_client(client_id: Id, result: Result<(Msg, CmdSink), CmdSink::Error>) {
        self.completed.push((client_id, result))
    }
}

impl<Id, CmdSink> Stream for GroupReceive<Id, CmdSink>
    where Id: Eq + Hash + Clone + Send,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static
{
    type Item = Vec<(Id, (Msg, CmdSink))>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if self.command_stream.is_some() {
            match self.complete_stream.as_mut().unwrap().poll() {
                // When the CommandStream sends a group of results, receive-queue them.
                Ok(Async::Ready(Some(commanded_clients))) => {
                    self.enqueue_for_receive(commanded_clients)
                }
                // Once the CommandStream has finished sending results, destroy it.
                Ok(Async::Ready(None)) => self.command_stream.take().unwrap(),
                Ok(Async::NotReady) => {}
                // If the CommandStream errors, the logic of what to do is unclear.
                // At the time of writing CommandStream can't error.
                // @TODO: Decide good rules here.
                Err(e) => unimplemented!(),
            }
        }

        self.poll_the_receive_queue();

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
