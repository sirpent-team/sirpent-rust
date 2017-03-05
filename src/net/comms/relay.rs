use std::fmt::Debug;
use std::collections::{HashMap, VecDeque};
use futures::{Stream, Sink};
use uuid::Uuid;
use super::*;

/// Relays messages between server connections (e.g., `Codec`-wrapped TCP Sockets)
/// and implementations of `Communicator`. One instance of this acts as a relay for
/// many clients. As polling this could potentially do a lot of work it is suggested
/// to run this in a dedicated thread.
pub struct Relay<Message, NewClientStream, CommandStream, ClientSink, ClientStream, ClientName>
    where NewClientStream: Stream<Item = (ClientSink, ClientStream, ClientName)>,
          CommandStream: Stream<Item = Command> + 'static,
          ClientSink: Sink<SinkItem = Message> + 'static,
          ClientStream: Stream<Item = Message> + 'static,
          ClientName: Debug + 'static
{
    relay_id: Uuid,
    new_clients_rx: NewClientStream,
    command_rx: CommandStream,
    clients: HashMap<ClientId, ClientRelay<Message, ClientSink, ClientStream, ClientName>>,
    queue_limit: Option<usize>,
}

impl<Message, NewClientStream, CommandStream, ClientSink, ClientStream, ClientName>
    Relay<Message, NewClientStream, CommandStream, ClientSink, ClientStream, ClientName>
    where NewClientStream: Stream<Item = (ClientSink, ClientStream, ClientName)>,
          CommandStream: Stream<Item = Command> + 'static,
          ClientSink: Sink<SinkItem = Message> + 'static,
          ClientStream: Stream<Item = Message> + 'static,
          ClientName: Debug + 'static
{
}

impl<Message, NewClientStream, CommandStream, ClientSink, ClientStream, ClientName>
    Relay<Message, NewClientStream, CommandStream, ClientSink, ClientStream, ClientName>
    where NewClientStream: Stream<Item = (ClientSink, ClientStream, ClientName)>,
          CommandStream: Stream<Item = Command> + 'static,
          ClientSink: Sink<SinkItem = Message> + 'static,
          ClientStream: Stream<Item = Message> + 'static,
          ClientName: Debug + 'static
{
    pub fn bind_to_listener(new_clients_rx: NewClientStream,
                            command_rx: CommandStream,
                            queue_limit: Option<usize>)
                            -> Self {
        Relay {
            relay_id: Uuid::new_v4(),
            new_clients_rx: new_clients_rx,
            command_rx: command_rx,
            clients: HashMap::new(),
            queue_limit: queue_limit,
        }
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
pub struct ClientRelay<Message, ClientSink, ClientStream, ClientName>
    where ClientSink: Sink<SinkItem = Message> + 'static,
          ClientStream: Stream<Item = Message> + 'static,
          ClientName: Debug + 'static
{
    client_id: ClientId,
    client_name: Option<ClientName>,
    tx: ClientSink,
    rx: ClientStream,
    tx_queue: VecDeque<Message>,
    rx_queue: VecDeque<Message>,
    forward_tx_queue: VecDeque<oneshot::Sender<Message>>,
}

impl<Message, ClientSink, ClientStream, ClientName> ClientRelay<Message,
                                                                ClientSink,
                                                                ClientStream,
                                                                ClientName>
    where ClientSink: Sink<SinkItem = Message> + 'static,
          ClientStream: Stream<Item = Message> + 'static,
          ClientName: Debug + 'static
{
    pub fn new(tx: ClientSink,
               rx: ClientStream,
               client_name: Option<ClientName>,
               relay_id: Uuid)
               -> Self {
        ClientRelay {
            client_id: ClientId::new_for_relay(relay_id),
            client_name: client_name,
            tx: tx,
            rx: rx,
            tx_queue: VecDeque::new(),
            rx_queue: VecDeque::new(),
            forward_tx_queue: VecDeque::new(),
        }
    }
}
