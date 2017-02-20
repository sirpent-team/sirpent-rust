use std::hash::Hash;
use std::fmt::Debug;
use std::time::Duration;
use std::collections::{HashMap, VecDeque};
use futures::{BoxFuture, Future, Stream, Sink, Poll, Async};
use futures::sync::oneshot;

use clients::*;

pub fn group_receive<Id, CmdSink>(clients: HashMap<Id, CmdSink>,
                                  timeout: Option<Duration>)
                                  -> BoxFuture<HashMap<Id, Result<(Msg, CmdSink)>>, Error>
    where Id: Eq + Hash + Clone + Debug + Send + 'static,
          CmdSink: Sink<SinkItem = Cmd, SinkError = Error> + Send + 'static
{
    let vec_vec_results = match timeout {
        Some(timeout) => GroupReceiveTimeout::new(clients, timeout).boxed(),
        None => GroupReceive::new(clients).collect().boxed(),
    };
    // @TODO: Try to squash the `Vec<Vec<_>>` somehow.
    vec_vec_results.map(|nested_vec_results| {
            nested_vec_results.into_iter().flat_map(|v| v.into_iter()).collect::<HashMap<_, _>>()
        })
        .boxed()
}

pub struct GroupReceive<Id, CmdSink>
    where Id: Eq + Hash + Clone + Debug + Send,
          CmdSink: Sink<SinkItem = Cmd, SinkError = Error> + Send + 'static
{
    group_command: Option<GroupCommand<Id, CmdSink>>,
    ready_queue: HashMap<Id, oneshot::Receiver<Msg>>,
    receive_queue: VecDeque<(Id, CmdSink, oneshot::Receiver<Msg>)>,
    completed: Vec<(Id, Result<(Msg, CmdSink)>)>,
}

impl<Id, CmdSink> GroupReceive<Id, CmdSink>
    where Id: Eq + Hash + Clone + Debug + Send,
          CmdSink: Sink<SinkItem = Cmd, SinkError = Error> + Send + 'static
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
            group_command: Some(GroupCommand::new(clients, CommandMode::Lookup(cmds))),
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
                Err(e) => {
                    self.complete_client(client_id,
                                         Err(e).chain_err(|| {
                                             "oneshot had been cancelled in GroupReceive receive \
                                              queue"
                                         }))
                }
            }
        }

        // Reenqueue clients who weren't ready yet.
        self.receive_queue.append(&mut reenqueue);
    }

    fn enqueue_for_receive(&mut self, commanded_clients: Vec<(Id, Result<CmdSink>)>) {
        for (client_id, result) in commanded_clients {
            match result {
                Ok(cmd_tx) => {
                    let oneshot_rx = self.ready_queue.remove(&client_id).unwrap();
                    self.receive_queue.push_back((client_id, cmd_tx, oneshot_rx));
                }
                Err(e) => {
                    self.complete_client(client_id,
                                         Err(e).chain_err(|| {
                                             "commanding receive errored in GroupReceive"
                                         }))
                }
            }
        }
    }

    /// Mark a client as complete. Nicer way to wrap taking out of Option.
    fn complete_client(&mut self, client_id: Id, result: Result<(Msg, CmdSink)>) {
        self.completed.push((client_id, result))
    }
}

impl<Id, CmdSink> Stream for GroupReceive<Id, CmdSink>
    where Id: Eq + Hash + Clone + Debug + Send,
          CmdSink: Sink<SinkItem = Cmd, SinkError = Error> + Send + 'static
{
    type Item = Vec<(Id, Result<(Msg, CmdSink)>)>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        while let Some(mut group_command_) = self.group_command.take() {
            match group_command_.poll() {
                // When the CommandStream sends a group of results, receive-queue them and
                // see if there's more to come.
                Ok(Async::Ready(Some(commanded_clients))) => {
                    self.enqueue_for_receive(commanded_clients);
                }
                // Once the CommandStream has finished sending results, destroy it.
                Ok(Async::Ready(None)) => break,
                // If the stream isn't ready yet then save it but move on.
                Ok(Async::NotReady) => {
                    self.group_command = Some(group_command_);
                    break;
                }
                // If the GroupCommand errors, the logic of what to do is unclear.
                // At the time of writing it can't error.
                // @TODO: Decide good rules here.
                Err(e) => return Err(e).chain_err(|| "GroupCommand errored inside GroupReceive"),
            }
            self.group_command = Some(group_command_);
        }

        self.poll_the_receive_queue();

        // Complete when queues are depleted.
        if !self.completed.is_empty() {
            let completed = self.completed.drain(..).collect();
            Ok(Async::Ready(Some(completed)))
        } else if self.group_command.is_none() && self.ready_queue.is_empty() &&
                  self.receive_queue.is_empty() {
            Ok(Async::Ready(None))
        } else {
            Ok(Async::NotReady)
        }
    }
}
