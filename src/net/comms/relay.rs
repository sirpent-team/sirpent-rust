use futures::{Stream, Sink};
use std::collections::{HashMap, VecDeque};
use super::*;

/// Relays messages between server connections (e.g., `Codec`-wrapped TCP Sockets)
/// and implementations of `Communicator`. One instance of this acts as a relay for
/// many clients. As polling this could potentially do a lot of work it is suggested
/// to run this in a dedicated thread.
// @TODO: It would make some sense to use `ClientRelay` as a boxed trait object,
// where `Message` had to match that on Relay but other details could vary. Errors
// could be forced to `io::Error` or something for interoperability until a more
// solid implementation is refineable.
//
// The issue with doing this is that it's vtable overhead for no benefit in the
// general case. If someone really wants to do this they can coerce the channels
// to have the same types using some custom futures code, or suggest a change.
// Even then it would be best to have a separate homogenous relay rather than
// putting needless overhead onto the ordinary case.
pub struct Relay<Message, CommandStream, ClientSink, ClientStream>
    where CommandStream: Stream<Item = Command> + 'static,
          ClientSink: Sink<SinkItem = Message> + 'static,
          ClientStream: Stream<Item = Message> + 'static
{
    command_rx: CommandStream,
    clients: HashMap<ClientId, ClientRelay<Message, ClientSink, ClientStream>>,
    queue_limit: Option<usize>,
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
pub struct ClientRelay<Message, ClientSink, ClientStream>
    where ClientSink: Sink<SinkItem = Message> + 'static,
          ClientStream: Stream<Item = Message> + 'static
{
    tx: ClientSink,
    rx: ClientStream,
    tx_queue: VecDeque<Message>,
    rx_queue: VecDeque<Message>,
    forward_tx_queue: VecDeque<oneshot::Sender<Message>>,
}
