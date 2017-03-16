use futures::{Sink, Future, BoxFuture, future};
use errors::Error;
use super::*;
use futures::sync::{mpsc, oneshot};

#[derive(Clone)]
pub struct Client<T, R>
    where T: Send + 'static,
          R: Send + 'static
{
    // Clients are identified by UUID.
    id: ClientId,
    // Clients can also have an ID specific to their communication form, e.g., SocketAddr.
    name: Option<String>,
    // Channel to command communications with.
    cmd_tx: mpsc::Sender<Command<T, R>>,
}

impl<T, R> Client<T, R>
    where T: Send + 'static,
          R: Send + 'static
{
    pub fn new(id: ClientId,
               name: Option<String>,
               cmd_tx: mpsc::Sender<Command<T, R>>)
               -> Result<Client<T, R>, Error> {
        Ok(Client {
            id: id,
            name: name,
            cmd_tx: cmd_tx,
        })
    }

    pub fn id(&self) -> ClientId {
        self.id
    }

    pub fn name(&self) -> Option<String> {
        self.name.clone()
    }

    fn command(&mut self, cmd: Command<T, R>) -> BoxFuture<ClientStatus, ClientStatus> {
        // Clone the cmd_tx. This allows composing a simple Future rather than wrapping
        // extended use of `start_send` and `poll_complete`.
        //
        // N.B., Cloning the channel potentially prevents backpressure:
        // "The channel capacity is equal to buffer + num-senders. In other words, each
        //  sender gets a guaranteed slot in the channel capacity, and on top of that
        //  there are buffer "first come, first serve" slots available to all senders."
        // https://docs.rs/futures/0.1/futures/sync/mpsc/fn.channel.html
        let tmp_cmd_tx = self.cmd_tx.clone();
        // Send the command into the sink. On success discard the cloned sink.
        let send_future = tmp_cmd_tx.send(cmd)
            .map(|_| ClientStatus::Ready)
            .map_err(|_| ClientStatus::Closed);
        Box::new(send_future)
    }

    fn command_noerr(&mut self, cmd: Command<T, R>) -> BoxFuture<(ClientId, ClientStatus), ()> {
        let id = self.id;
        self.command(cmd)
            .then(move |v| match v {
                Ok(s) => future::ok((id, s)),
                Err(s) => future::ok((id, s)),
            })
            .boxed()
    }
}

impl<T, R> Communicator for Client<T, R>
    where T: Send + 'static,
          R: Send + 'static
{
    type Transmit = T;
    type Receive = (ClientId, (ClientStatus, Option<R>));
    type Status = (ClientId, ClientStatus);
    type Error = ();

    fn transmit(&mut self, msg: Self::Transmit) -> BoxFuture<Self::Status, ()> {
        let cmd = Command::Transmit(self.id, msg);
        self.command_noerr(cmd)
    }

    fn receive(&mut self, timeout: ClientTimeout) -> BoxFuture<Self::Receive, ()> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let cmd = Command::ReceiveInto(self.id, timeout, reply_tx);
        let id = self.id;
        self.command(cmd)
            .then(|v| match v {
                Ok(_) => reply_rx.map_err(|_| ClientStatus::Closed).boxed(),
                Err(e) => future::err(e).boxed(),
            })
            .then(move |v| match v {
                Ok(m) => future::ok((id, (ClientStatus::Ready, Some(m)))).boxed(),
                Err(e) => future::ok((id, (e, None))).boxed(),
            })
            .boxed()
    }

    fn status(&mut self) -> BoxFuture<Self::Status, ()> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let cmd = Command::StatusInto(self.id, reply_tx);
        let id = self.id;
        self.command(cmd)
            .then(move |v| match v {
                Ok(_) => {
                    reply_rx.then(move |v| match v {
                            Ok(s) => future::ok((id, s)),
                            Err(_) => future::ok((id, ClientStatus::Closed)),
                        })
                        .boxed()
                }
                Err(s) => future::ok((id, s)).boxed(),
            })
            .boxed()
    }

    fn close(&mut self) -> BoxFuture<Self::Status, ()> {
        let cmd = Command::Close(self.id());
        self.command_noerr(cmd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use futures::sync::mpsc;
    use futures::{Stream, executor};
    use std::sync::Arc;
    use net::Msg;

    fn unpark_noop() -> Arc<executor::Unpark> {
        struct Foo;

        impl executor::Unpark for Foo {
            fn unpark(&self) {}
        }

        Arc::new(Foo)
    }

    fn mock_command_channel(tx: mpsc::Sender<Command>) -> CommandChannel<mpsc::Sender<Command>> {
        CommandChannel::new_for_relay(Uuid::new_v4(), tx)
    }

    fn mock_client(command_channel: &CommandChannel<mpsc::Sender<Command>>)
                   -> Client<mpsc::Sender<Command>, mpsc::SendError<Command>> {
        let client_id = CommunicationId {
            client_id: Uuid::new_v4(),
            relay_id: command_channel.relay_id(),
        };
        Client::new(client_id, None, command_channel.clone()).unwrap()
    }

    #[test]
    fn can_transmit() {
        let (tx, rx) = mpsc::channel(1);
        let mut rx_stream = rx.wait().peekable();
        let mut client = mock_client(&mock_command_channel(tx));

        for _ in 0..10 {
            let msg = Msg::version();
            client.transmit(msg.clone()).wait().unwrap();

            match rx_stream.next() {
                Some(Ok(Command::Transmit(client_id, msg2))) => {
                    assert!(client_id == client.id());
                    assert!(msg == msg2);
                }
                _ => assert!(false),
            }
        }
    }

    #[test]
    fn can_receive() {
        let (tx, rx) = mpsc::channel(1);
        let mut rx_stream = rx.wait().peekable();
        let mut client = mock_client(&mock_command_channel(tx));

        for _ in 0..10 {
            let msg = Msg::version();
            let receive = client.receive(ClientTimeout::None);

            let mut future = executor::spawn(receive.fuse());

            assert!(future.poll_future(unpark_noop()).unwrap().is_not_ready());

            match rx_stream.next().unwrap().unwrap() {
                Command::ReceiveInto(client_id, msg_forward_tx, ClientTimeout::None) => {
                    assert!(client_id == client.id());
                    msg_forward_tx.complete(msg.clone());
                }
                _ => assert!(false),
            }

            match future.wait_future() {
                Ok(msg2) => assert!(msg == msg2),
                _ => assert!(false),
            }
        }
    }

    #[test]
    fn can_receive_queued() {
        let (tx, rx) = mpsc::channel(1);
        let mut rx_stream = rx.wait().peekable();
        let mut client = mock_client(&mock_command_channel(tx));

        let mut futures = vec![];

        for _ in 0..10 {
            let msg = Msg::version();
            let mut future = executor::spawn(client.receive(ClientTimeout::None));
            assert!(future.poll_future(unpark_noop()).unwrap().is_not_ready());
            futures.push((msg, future));
        }

        for &mut (ref mut msg, _) in &mut futures {
            match rx_stream.next().unwrap().unwrap() {
                Command::ReceiveInto(client_id, msg_forward_tx, ClientTimeout::None) => {
                    assert!(client_id == client.id());
                    msg_forward_tx.complete(msg.clone());
                }
                _ => assert!(false),
            }
        }

        for &mut (ref mut msg, ref mut future) in &mut futures {
            match future.wait_future() {
                Ok(msg2) => assert!(msg.clone() == msg2),
                _ => assert!(false),
            }
        }
    }

    #[test]
    fn can_status() {
        let (tx, rx) = mpsc::channel(1);
        let mut rx_stream = rx.wait().peekable();
        let mut client = mock_client(&mock_command_channel(tx));

        for _ in 0..10 {
            let mut future = executor::spawn(client.status());

            assert!(future.poll_future(unpark_noop()).unwrap().is_not_ready());

            match rx_stream.next().unwrap().unwrap() {
                Command::StatusInto(client_id, status_reply_tx) => {
                    assert!(client_id == client.id());
                    status_reply_tx.complete(ClientStatus::Ready);
                }
                _ => assert!(false),
            }

            match future.wait_future() {
                Ok(status) => assert!(status == ClientStatus::Ready),
                _ => assert!(false),
            }
        }
    }

    #[test]
    fn can_close() {
        let (tx, rx) = mpsc::channel(1);
        let mut rx_stream = rx.wait().peekable();
        let mut client = mock_client(&mock_command_channel(tx));

        for _ in 0..10 {
            let msg = Msg::version();
            client.transmit(msg.clone()).wait().unwrap();
            match rx_stream.next() {
                Some(Ok(Command::Transmit(client_id, msg2))) => {
                    assert!(client_id == client.id());
                    assert!(msg == msg2);
                }
                _ => assert!(false),
            }
        }
    }
}
