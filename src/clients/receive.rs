use std::io;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

use futures::{BoxFuture, Future, Stream, Sink, Poll, Async};
use futures::sync::oneshot;

use clients::*;

pub fn group_command<Id, CmdSink>
    (clients: HashMap<Id, CmdSink>)
     -> BoxFuture<HashMap<Id, Result<(Msg, CmdSink), CmdSink::SinkError>>, io::Error>
    where Id: Eq + Hash + Clone + Send + 'static,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static,
          CmdSink::SinkError: Send + 'static
{
    // @TODO: Try to squash the `Vec<Vec<_>>` somehow.
    GroupReceive::new(clients)
        .collect()
        .map(|nested_vec_results| {
            nested_vec_results.into_iter().flat_map(|v| v.into_iter()).collect::<HashMap<_, _>>()
        })
        .boxed()
}

pub struct GroupReceive<Id, CmdSink>
    where Id: Eq + Hash + Clone + Send,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static
{
    command_stream: Option<GroupCommand<Id, CmdSink>>,
    ready_queue: HashMap<Id, oneshot::Receiver<Msg>>,
    receive_queue: VecDeque<(Id, CmdSink, oneshot::Receiver<Msg>)>,
    completed: Vec<(Id, Result<(Msg, CmdSink), CmdSink::SinkError>)>,
}

impl<Id, CmdSink> GroupReceive<Id, CmdSink>
    where Id: Eq + Hash + Clone + Send,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static
{
    pub fn new(clients: HashMap<Id, CmdSink>) -> Self {
        let mut cmds = HashMap::new();
        let mut ready_queue = HashMap::new();
        for client_id in clients.keys() {
            let (cmd, oneshot_rx) = Self::new_client_oneshot();
            cmds.insert(client_id.clone(), cmd);
            ready_queue.insert(client_id.clone(), oneshot_rx);
        }

        GroupReceive {
            command_stream: Some(GroupCommand::new(clients, CommandMode::Lookup(cmds))),
            ready_queue: ready_queue,
            receive_queue: VecDeque::new(),
            completed: Vec::new(),
        }
    }

    fn new_client_oneshot() -> (Cmd, oneshot::Receiver<Msg>) {
        // Create a oneshot channel for the received message to be passed back to us on.
        let (oneshot_tx, oneshot_rx) = oneshot::channel();
        // We transmit this oneshot's tx along the channel to `Client` and then wait for
        // a reply from the oneshot's rx. This (perhaps surprisingly) delivers nicer code.
        let cmd = Cmd::ReceiveInto(RaceableOneshotSender::new(oneshot_tx));

        (cmd, oneshot_rx)
    }

    /// Make progress receiving messages from clients.
    ///
    /// Applies `oneshot::Receiver::poll` for each client until a message arrives.
    fn poll_the_receive_queue(&mut self) {
        // Maintain a separate list of clients to be requeued, so that the loop terminates.
        let mut reenqueue = VecDeque::new();

        while let Some((client_id, cmd_tx, mut oneshot_rx)) = self.receive_queue.pop_front() {
            match oneshot_rx.poll() {
                Ok(Async::Ready(msg)) => self.complete_client(client_id, Ok((msg, cmd_tx))),
                Ok(Async::NotReady) => reenqueue.push_back((client_id, cmd_tx, oneshot_rx)),
                Err(_) => unimplemented!(), //self.complete_client(client_id, Err(e)),
            }
        }

        // Reenqueue clients who weren't ready yet.
        self.receive_queue.append(&mut reenqueue);
    }

    fn enqueue_for_receive(&mut self,
                           commanded_clients: Vec<(Id, Result<CmdSink, CmdSink::SinkError>)>) {
        for (client_id, result) in commanded_clients.into_iter() {
            match result {
                Ok(cmd_tx) => {
                    let oneshot_rx = self.ready_queue.remove(&client_id).unwrap();
                    self.receive_queue.push_back((client_id, cmd_tx, oneshot_rx));
                }
                Err(e) => self.complete_client(client_id, Err(e)),
            }
        }
    }

    /// Mark a client as complete. Nicer way to wrap taking out of Option.
    fn complete_client(&mut self,
                       client_id: Id,
                       result: Result<(Msg, CmdSink), CmdSink::SinkError>) {
        self.completed.push((client_id, result))
    }
}

impl<Id, CmdSink> Stream for GroupReceive<Id, CmdSink>
    where Id: Eq + Hash + Clone + Send,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static
{
    type Item = Vec<(Id, Result<(Msg, CmdSink), CmdSink::SinkError>)>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if self.command_stream.is_some() {
            match self.command_stream.as_mut().unwrap().poll() {
                // When the CommandStream sends a group of results, receive-queue them.
                Ok(Async::Ready(Some(commanded_clients))) => {
                    self.enqueue_for_receive(commanded_clients);
                }
                // Once the CommandStream has finished sending results, destroy it.
                Ok(Async::Ready(None)) => {
                    self.command_stream.take().unwrap();
                }
                Ok(Async::NotReady) => {}
                // If the CommandStream errors, the logic of what to do is unclear.
                // At the time of writing CommandStream can't error.
                // @TODO: Decide good rules here.
                Err(_) => unimplemented!(),
            }
        }

        self.poll_the_receive_queue();

        // Complete when queues are depleted.
        if !self.completed.is_empty() {
            let completed = self.completed.drain(..).collect();
            Ok(Async::Ready(Some(completed)))
        } else if self.command_stream.is_none() && self.ready_queue.is_empty() &&
                  self.receive_queue.is_empty() {
            Ok(Async::Ready(None))
        } else {
            Ok(Async::NotReady)
        }
    }
}
