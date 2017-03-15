use std::fmt::Debug;
use std::collections::{HashMap, VecDeque};
use futures::{Stream, Sink};
use uuid::Uuid;
use super::*;

pub enum ClientRelayReplyItem<M> {
    Status(Status),
    Received(M, Status),
}

pub type StatusReply = HashMap<CommsId, Status>;
pub type ReceiveReply<M> = (HashMap<CommsId, M>, StatusReply);

/// Relays messages between server connections (e.g., `Codec`-wrapped TCP Sockets)
/// and implementations of `Communicator`. One instance of this acts as a relay for
/// many clients. As polling this could potentially do a lot of work it is suggested
/// to run this in a dedicated thread.
pub struct Relay<M, C, L, S, T, N>
    where L: Stream<Item = (S, T, N)>,
          C: Stream<Item = Command>,
          S: Sink<SinkItem = M> + 'static,
          T: Stream<Item = M> + 'static,
          N: Debug + 'static
{
    relay_id: Uuid,
    cmd_rx: C,
    listener_rx: L,
    queue_limit: Option<usize>,
    client_relays: HashMap<CommunicationId, ClientRelay<M, S, T, N>>,
    operations: VecDeque<Box<Future<Item = (), Error = ()>>>,
    receives: HashSet<oneshot::Sender<ReceiveReply<M>>>,
}

impl<M, C, L, S, T, N> Relay<M, C, L, S, T, N>
    where L: Stream<Item = (S, T, N)>,
          C: Stream<Item = Command>,
          S: Sink<SinkItem = M> + 'static,
          T: Stream<Item = M> + 'static,
          N: Debug + 'static
{
    pub fn bind_to_listener(listener_rx: L, cmd_rx: C, queue_limit: Option<usize>) -> Self {
        Relay {
            id: Uuid::new_v4(),
            listener_rx: listener_rx,
            cmd_rx: cmd_rx,
            clients: HashMap::new(),
            queue_limit: queue_limit,
        }
    }

    fn transmit(&mut self, msgs: HashMap<CommsId, M>, reply: oneshot::Sender<StatusReply>) {
        collect_results(msgs.into_iter().map(|(comms_id, msg)| {
            self.client_relay[comms_id].transmit(msg)
        })).map(|out| {
            let mut msgs = out.iter().map(|(comms_id, (status, ))|)
            out.into_iter().unzip()
        })
    }

    fn receive(&mut self, comms_ids: HashSet<CommsId>, reply: oneshot::Sender<ReceiveReply<M>>) {
        collect_results(comms_id.into_iter().map(|comms_id| {
            self.client_relay[comms_id].receive()
        })).map(|out| {
            let mut msgs = HashMap::new();
            let mut statuses = HashMap::new();
            for (comms_id, status, option_msg) in out {
                if let Some(msg) = option_msg {
                    msgs.insert(comms_id, msg);
                }
                statuses.insert(comms_id, status);
            }
            let mut msgs = out.inter().map(|(comms_id, status, msg)| )
        })
    }
}

impl<M, C, L, S, T, N> Future for Relay<M, C, L, S, T, N>
    where L: Stream<Item = (S, T, N)>,
          C: Stream<Item = Command>,
          S: Sink<SinkItem = M> + 'static,
          T: Stream<Item = M> + 'static,
          N: Debug + 'static
{
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        // First check for anything being instructed.
        // This is first because it provides possible messages to send and possible places
        // to send messages - both needed later.
        loop {
            match self.cmd_rx.poll() {
                Ok(Async::NotReady) => break,
                Ok(Async::Ready(Some(cmd))) => {
                    use Command::*;
                    match cmd {
                        Transmit(id, msg, reply) => {
                            match self.client_relay.get_mut(&id) {
                                Some(cr) => cr.transmit(msg, reply),
                                None => reply.complete(CommsError::ClientIdNotFound(id))
                            }
                        }
                        Receive(id, timeout, reply) => {
                            match self.client_relay.get_mut(&id) {
                                Some(cr) => cr.receive(timeout, reply),
                                None => reply.complete(CommsError::ClientIdNotFound(id))
                            }
                        }
                        DiscardReceived(id, reply) => {
                            match self.client_relay.get_mut(&id) {
                                Some(cr) => cr.discard_received(reply),
                                None => reply.complete(CommsError::ClientIdNotFound(id))
                            }
                        }
                        Status(id, reply) => {
                            match self.client_relay.get_mut(&id) {
                                Some(cr) => cr.status(reply),
                                None => reply.complete(CommsError::ClientIdNotFound(id))
                            }
                        }
                        Close(id, reply) => {
                            match self.client_relays.remove(&id) {
                                Some(cr) => cr.close(reply),
                                None => reply.complete(CommsError::ClientIdNotFound(id))
                            }
                        }
                    }
                }
                Ok(Async::Ready(None)) => return Err(CommsError::BrokenPipe),
                Err(()) => unreachable!(),
            }
        }

        for client_relay in &mut self.client_relays {
            client_relay.subpoll()
        }

        Ok(Async::NotReady)
    }
}



// @TODO: Research as part of implementation.
//
// All these datastructures are a bit much per client. It may benchmark better if coded
// up slightly less neatly.
//
// There's real need for something like `rx_queue`. We want to limit how many messages a
// client can send before they get disconnected, in order to limit Denial Of Service potential.
// This can't be done by adding a `futures::stream::Buffer` because I don't think we could tell
// when the buffer was filling up. The details of this seem rather server-specific so a general
// guarantee is rather good to have here until some sort of conclusion is reached.
// We can't pass things through and detect a `Sink` queueing up because we're putting `Message`
// into sometimes-provided `oneshot::Sender`s.
//
// The need for `tx_queue` is unclear. For a socket server, if these messages start queueing
// up then we need to worry the client is broken - but even then I don't know for sure. Again,
// it provides a potentially-slightly-costly guarantee of something I am otherwise unsure of.
struct ClientRelay<M, S, T, N>
    where S: Sink<SinkItem = M> + 'static,
          T: Stream<Item = M> + 'static,
          N: Debug + 'static
{
    id: CommunicationId,
    name: Option<N>,
    tx_relay: ClientRelayTx<S, M>,
    rx_relay: ClientRelayRx<T, M>,
}

impl<M, S, T, N> ClientRelay<M, S, T, N>
    where S: Sink<SinkItem = M> + 'static,
          T: Stream<Item = M> + 'static,
          N: Debug + 'static
{
    fn new(relay_id: Uuid, name: Option<N>, tx: S, rx: T) -> Self {
        ClientRelay {
            id: CommunicationId::new_for_relay(relay_id),
            name: name,
            tx_relay: ClientRelayTx::new(tx),
            rx_relay: ClientRelayRx::new(rx),
        }
    }

    // ... oops, I forgot the need to reply when implementing subpoll on tx_relay and rx_relay
    // ... worse oops, I didn't consider that replies want grouping. passing oneshots from Relay
    //     down to these would be about as expensive as the existing clients setup. erm. but that
    //     could be okay as it keeps Room and Client quite simple.
    // ??? Relay handles the coordination. I sort of forgot that is its reason for being while
    //     coding for a bit. so Relay has to track what work is/isn't done.  subpoll could return a collection of work complete.
    fn transmit(&mut self, msg: M, reply: oneshot::Sender<>)

    fn subpoll(&mut self) {
        self.tx_relay.subpoll()
        self.rx_relay.subpoll()
    }
}

impl<M, S, T, N> Stream for ClientRelay<M, S, T, N>
    where S: Sink<SinkItem = M> + 'static,
          T: Stream<Item = M> + 'static,
          N: Debug + 'static
{
    type Item = Vec<ClientRelayReplyItem>;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {

    }
}


cr.transmit(msg, reply)
y.complete(CommsError::
 reply) => {
t_relay.get_mut(&id) {
cr.receive(timeout, rep
y.complete(CommsError::
reply) => {
t_relay.get_mut(&id) {
cr.discard_received(rep
y.complete(CommsError::
 {
t_relay.get_mut(&id) {
cr.status(reply),
y.complete(CommsError::
{
t_relays.remove(&id) {
cr.close(reply),

struct ClientRelayTx<S, M>
    where S: Sink<SinkItem = M> + 'static
{
    tx: S,
    tx_queue: VecDeque<M>,
}

impl<S, M> ClientRelayTx<S, M>
    where S: Sink<SinkItem = M> + 'static
{
    fn new(tx: S) -> Self {
        ClientRelayTx {
            tx: tx,
            tx_queue: VecDeque::new(),
        }
    }

    fn subpoll(&mut self) {
        if self.tx_queue.is_empty() {
            match self.tx.poll_complete() {
                Ok(Async::Ready(())) | Ok(Async::NotReady) => {}
                Err(e) => bail!(e),
            };
            return;
        }

        // Performs rounds of, "fill Sink then flush," until the queue is empty or no
        // further progress can be made with flushing.
        while !self.tx_queue.is_empty() {
            // Queue items until the `Sink` is full.
            while !self.tx_queue.is_empty() {
                let head = self.tx_queue[0].clone();
                match self.tx.start_send(head) {
                    Ok(AsyncSink::Ready) => self.tx_queue.pop_front(),
                    // Indicates the `Sink` is full.
                    Ok(AsyncSink::NotReady(_)) => break,
                    Err(e) => bail!(e),
                };
            }
            // Make progress flushing the `Sink`.
            match self.tx.poll_complete() {
                Ok(Async::Ready(())) | Ok(Async::NotReady) => {}
                Err(e) => bail!(e),
            };
        }
    }
}

struct ClientRelayRx<T, M>
    where T: Stream<Item = M> + 'static
{
    rx: T,
    rx_buffer: VecDeque<M>,
    forwarding_queue: VecDeque<oneshot::Sender<M>>,
}

impl<T, M> ClientRelayRx<T, M>
    where T: Stream<Item = M> + 'static
{
    fn new(rx: T) -> Self {
        ClientRelayRx {
            rx: rx,
            rx_buffer: VecDeque::new(),
            forwarding_queue: VecDeque::new(),
        }
    }

    fn subpoll(&mut self) {
        // Try to read new messages from the client.
        match self.rx.poll() {
            Ok(Async::Ready(Some(msg))) => {
                if let Some(queue_limit) = self.queue_limit {
                    if self.rx_queue.len() >= queue_limit {
                        bail!("Tried to exceed msg rx queue capacity.");
                    }
                }
                self.rx_queue.push_back(msg_rx)
            }
            Ok(Async::Ready(None)) => bail!(broken_pipe()),
            Ok(Async::NotReady) => {},
            Err(e) => bail!(e),
        };

        // Try to forward received messages from the client.
        while !self.rx_queue.is_empty() && !self.forwarding_queue.is_empty() {
            let mut forwarder = self.forwarding_queue.pop_front().unwrap();
            // This is how `oneshot::Sender` indicates the `Receiver` has not been dropped.
            if forwarder.poll_cancel() == Ok(Async::NotReady) {
                let msg = self.rx_queue.pop_front().unwrap();
                forwarder.complete(msg);
            }
        }
    }
}










pub enum Command {
    // Send specific messages to specific clients.
    Transmit(HashMap<CommunicationId, Msg>),
    // Send a message to all clients on the other end.
    Broadcast(Msg),
    // Receive a message from a single client into a `oneshot::Receiver`.
    ReceiveInto(CommunicationId, oneshot::Sender<Msg>, ClientTimeout),
    // Receive one message from each specified clients into `oneshot::Receiver`s.
    ReceiveFromGroupInto(HashSet<CommunicationId>,
                         oneshot::Sender<HashMap<CommunicationId, Msg>>,
                         ClientTimeout),
    // Discard all messages already received from a client.
    DiscardReceiveBuffer(CommunicationId),
    // Discard all messages already received from specified clients.
    DiscardReceiveBufferForGroup(HashSet<CommunicationId>),
    // Receive a message from a single client into a `oneshot::Receiver`.
    StatusInto(CommunicationId, oneshot::Sender<ClientStatus>),
    // Receive one message from each specified clients into `oneshot::Receiver`s.
    StatusFromGroupInto(HashSet<CommunicationId>,
                        oneshot::Sender<HashMap<CommunicationId, ClientStatus>>),
    // Disconnect a single client.
    Close(CommunicationId),
    // Disconnect specified clients.
    CloseGroup(HashSet<CommunicationId>),
}
