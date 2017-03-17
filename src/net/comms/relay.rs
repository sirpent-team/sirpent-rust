use std::fmt::Debug;
use std::collections::VecDeque;
use futures::{Stream, Sink, Async, AsyncSink};
use futures::sync::{mpsc, oneshot};
use super::*;

#[derive(Debug)]
enum RelayError<A, B> {
    IncorrectClientIdInCommand,
    BrokenPipe(String),
    QueueLimitExceeded(String),
    Tx(A),
    Rx(B),
}

struct ClientRelay<A, B, T, R>
    where A: Sink<SinkItem = T> + 'static,
          B: Stream<Item = R> + 'static,
          A::SinkError: Debug,
          B::Error: Debug,
          T: Clone + Debug + PartialEq + Send + 'static,
          R: Clone + Debug + PartialEq + Send + 'static
{
    id: ClientId,
    name: Option<String>,
    queue_limit: Option<usize>,
    tx_relay: ClientRelayTx<A, T>,
    rx_relay: ClientRelayRx<B, R>,
    cmd_rx: mpsc::Receiver<Command<T, R>>,
}

impl<A, B, T, R> ClientRelay<A, B, T, R>
    where A: Sink<SinkItem = T> + 'static,
          B: Stream<Item = R> + 'static,
          A::SinkError: Debug,
          B::Error: Debug,
          T: Clone + Debug + PartialEq + Send + 'static,
          R: Clone + Debug + PartialEq + Send + 'static
{
    pub fn new(id: ClientId,
               name: Option<String>,
               queue_limit: Option<usize>,
               tx: A,
               rx: B)
               -> (ClientRelay<A, B, T, R>, mpsc::Sender<Command<T, R>>) {
        let (cmd_tx, cmd_rx) = mpsc::channel(queue_limit.unwrap_or(3));
        let client_relay = ClientRelay {
            id: id,
            name: name,
            queue_limit: queue_limit,
            tx_relay: ClientRelayTx::new(tx),
            rx_relay: ClientRelayRx::new(rx),
            cmd_rx: cmd_rx,
        };
        (client_relay, cmd_tx)
    }

    // It would be nice to make all `Command` come with a oneshot reply. That way the
    // relay could manage much more logic than at present, state is more clearly synced,
    // and so forth. A new variant of ClientStatus would indicate when channels to the
    // relay are dropped, and ClientStatus::Closed would be purely for relay-controlled
    // closing.
    fn cmdpoll(&mut self) -> Poll<(), RelayError<A::SinkError, B::Error>> {
        loop {
            match self.cmd_rx.poll() {
                Ok(Async::NotReady) => break,
                Ok(Async::Ready(Some(cmd))) => {
                    match cmd {
                        Command::Transmit(id, msg) => {
                            if id != self.id {
                                return Err(RelayError::IncorrectClientIdInCommand);
                            }
                            if let Some(queue_limit) = self.queue_limit {
                                if self.tx_relay.buffer.len() >= queue_limit {
                                    return Err(RelayError::QueueLimitExceeded("tx_relay.buffer"
                                        .to_string()));
                                }
                            }
                            self.tx_relay.buffer.push_back(msg);
                        }
                        Command::ReceiveInto(id, timeout, reply) => {
                            if id != self.id {
                                return Err(RelayError::IncorrectClientIdInCommand);
                            }
                            // @TODO: Timeout.
                            if let Some(queue_limit) = self.queue_limit {
                                if self.rx_relay.forward_txs.len() >= queue_limit {
                                    return Err(RelayError::QueueLimitExceeded("forward_txs"
                                        .to_string()));
                                }
                            }
                            self.rx_relay.forward_txs.push_back(reply);
                        }
                        Command::DiscardReceiveBuffer(id) => {
                            if id != self.id {
                                return Err(RelayError::IncorrectClientIdInCommand);
                            }
                            self.rx_relay.buffer.clear();
                        }
                        Command::StatusInto(id, reply) => {
                            if id != self.id {
                                return Err(RelayError::IncorrectClientIdInCommand);
                            }
                            if let Some(queue_limit) = self.queue_limit {
                                if self.rx_relay.status_txs.len() >= queue_limit {
                                    return Err(RelayError::QueueLimitExceeded("status_txs"
                                        .to_string()));
                                }
                            }
                            self.rx_relay.status_txs.push_back(reply);
                        }
                        Command::Close(id) => {
                            if id != self.id {
                                return Err(RelayError::IncorrectClientIdInCommand);
                            }
                            // End the relay's entire future, dropping all attached channels.
                            return Ok(Async::Ready(()));
                        }
                    }
                }
                Ok(Async::Ready(None)) => return Err(RelayError::BrokenPipe("cmd_rx".to_string())),
                Err(()) => unreachable!(),
            }
        }

        Ok(Async::NotReady)
    }
}

impl<A, B, T, R> Future for ClientRelay<A, B, T, R>
    where A: Sink<SinkItem = T> + 'static,
          B: Stream<Item = R> + 'static,
          A::SinkError: Debug,
          B::Error: Debug,
          T: Clone + Debug + PartialEq + Send + 'static,
          R: Clone + Debug + PartialEq + Send + 'static
{
    type Item = ();
    type Error = RelayError<A::SinkError, B::Error>;

    fn poll(&mut self) -> Poll<(), RelayError<A::SinkError, B::Error>> {
        // To ensure commands are performed, they must be read in before polling the
        // other 'subfutures.'
        match self.cmdpoll() {
            Ok(Async::NotReady) => {}
            Ok(Async::Ready(())) => return Ok(Async::Ready(())),
            Err(e) => return Err(e),
        };

        match self.tx_relay.subpoll(self.queue_limit) {
            Ok(Async::NotReady) => {}
            Ok(Async::Ready(())) => return Ok(Async::Ready(())),
            Err(e) => return Err(e),
        };

        match self.rx_relay.subpoll(self.queue_limit) {
            Ok(Async::NotReady) => {}
            Ok(Async::Ready(())) => return Ok(Async::Ready(())),
            Err(e) => return Err(e),
        };

        Ok(Async::NotReady)
    }
}

struct ClientRelayTx<A, T>
    where A: Sink<SinkItem = T> + 'static,
          A::SinkError: Debug,
          T: Clone + Debug + PartialEq + Send + 'static
{
    tx: A,
    buffer: VecDeque<T>,
}

impl<A, T> ClientRelayTx<A, T>
    where A: Sink<SinkItem = T> + 'static,
          A::SinkError: Debug,
          T: Clone + Debug + PartialEq + Send + 'static
{
    fn new(tx: A) -> ClientRelayTx<A, T> {
        ClientRelayTx {
            tx: tx,
            buffer: VecDeque::new(),
        }
    }

    fn subpoll<B>(&mut self, _: Option<usize>) -> Poll<(), RelayError<A::SinkError, B>> {
        // Performs rounds of, "fill Sink then flush," until the queue is empty or no
        // further progress can be made with flushing.
        while !self.buffer.is_empty() {
            // Queue items until the `Sink` is full.
            while let Some(head) = self.buffer.pop_front() {
                match self.tx.start_send(head) {
                    Ok(AsyncSink::Ready) => {}
                    // Indicates the `Sink` is full.
                    Ok(AsyncSink::NotReady(head)) => {
                        self.buffer.push_front(head);
                        break;
                    }
                    Err(e) => return Err(RelayError::Tx(e)),
                };
            }
            // Make progress flushing the `Sink`.
            match self.tx.poll_complete() {
                Ok(Async::Ready(())) | Ok(Async::NotReady) => {}
                Err(e) => return Err(RelayError::Tx(e)),
            };
        }

        Ok(Async::NotReady)
    }
}

struct ClientRelayRx<B, R>
    where B: Stream<Item = R> + 'static,
          B::Error: Debug,
          R: Clone + Debug + PartialEq + Send + 'static
{
    rx: B,
    buffer: VecDeque<R>,
    forward_txs: VecDeque<oneshot::Sender<R>>,
    status_txs: VecDeque<oneshot::Sender<ClientStatus>>,
}

impl<B, R> ClientRelayRx<B, R>
    where B: Stream<Item = R> + 'static,
          B::Error: Debug,
          R: Clone + Debug + PartialEq + Send + 'static
{
    fn new(rx: B) -> ClientRelayRx<B, R> {
        ClientRelayRx {
            rx: rx,
            buffer: VecDeque::new(),
            forward_txs: VecDeque::new(),
            status_txs: VecDeque::new(),
        }
    }

    fn subpoll<A>(&mut self, queue_limit: Option<usize>) -> Poll<(), RelayError<A, B::Error>> {
        // Try to read new messages from the client.
        loop {
            match self.rx.poll() {
                Ok(Async::Ready(Some(msg))) => {
                    if let Some(queue_limit) = queue_limit {
                        if self.buffer.len() >= queue_limit {
                            return Err(RelayError::QueueLimitExceeded("Tried to exceed msg rx \
                                                                       queue capacity."
                                .to_string()));
                        }
                    }
                    self.buffer.push_back(msg);
                }
                Ok(Async::Ready(None)) => {
                    return Err(RelayError::BrokenPipe("rx_relay.rx".to_string()))
                }
                Ok(Async::NotReady) => break,
                Err(e) => return Err(RelayError::Rx(e)),
            };
        }

        // Try to forward received messages from the client.
        while !self.buffer.is_empty() && !self.forward_txs.is_empty() {
            let mut forward_tx = self.forward_txs.pop_front().unwrap();
            // This is how `oneshot::Sender` indicates the `Receiver` has not been dropped.
            if forward_tx.poll_cancel() == Ok(Async::NotReady) {
                let msg = self.buffer.pop_front().unwrap();
                forward_tx.complete(msg);
            }
        }

        while !self.status_txs.is_empty() {
            let mut status_tx = self.status_txs.pop_front().unwrap();
            // This is how `oneshot::Sender` indicates the `Receiver` has not been dropped.
            if status_tx.poll_cancel() == Ok(Async::NotReady) {
                status_tx.complete(ClientStatus::Ready);
            }
        }

        Ok(Async::NotReady)
    }
}
