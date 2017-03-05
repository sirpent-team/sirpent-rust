use futures::{Sink, Future};
use errors::Error;
use super::*;
use net::Msg;
use futures::sync::oneshot;

#[derive(Clone, Debug, PartialEq)]
pub struct Client<S, E>
    where S: Sink<SinkItem = Command, SinkError = E> + Send + Clone + 'static,
          E: Into<Error> + 'static
{
    // Clients are identified by UUID.
    id: ClientId,
    // Clients can also have an ID specific to their communication form, e.g., SocketAddr.
    name: Option<String>,
    // Channel to command communications with.
    cmd_tx: CommandChannel<S>,
}

impl<S, E> Client<S, E>
    where S: Sink<SinkItem = Command, SinkError = E> + Send + Clone + 'static,
          E: Into<Error> + 'static
{
    pub fn new(id: ClientId,
               name: Option<String>,
               cmd_tx: CommandChannel<S>)
               -> Result<Client<S, E>, Error> {
        if !cmd_tx.can_command(&id) {
            return Err(format!("Attempted to add a client to a room using a different listener")
                .into());
        }
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

    fn command(&mut self, cmd: Command) -> Box<Future<Item = (), Error = Error>> {
        Box::new(self.cmd_tx.clone().send(cmd).map(|_| ()).map_err(|e| e.into()))
    }
}

impl<S, E> Communicator for Client<S, E>
    where S: Sink<SinkItem = Command, SinkError = E> + Send + Clone + 'static,
          E: Into<Error> + 'static
{
    type Transmit = Msg;
    type Receive = Msg;
    type Status = ClientStatus;
    type Error = Error;

    fn transmit(&mut self, msg: Self::Transmit) -> Box<Future<Item = (), Error = Error>> {
        let cmd = Command::Transmit(self.id(), msg);
        Box::new(self.command(cmd))
    }

    fn receive(&mut self,
               timeout: ClientTimeout)
               -> Box<Future<Item = Self::Receive, Error = Error>> {
        let (msg_forward_tx, msg_forward_rx) = oneshot::channel();
        let cmd = Command::ReceiveInto(self.id(), msg_forward_tx, timeout);
        Box::new(self.command(cmd).and_then(|_| msg_forward_rx.map_err(|e| e.into())))
    }

    fn status(&mut self) -> Box<Future<Item = Self::Status, Error = Error>> {
        let (status_tx, status_rx) = oneshot::channel();
        let cmd = Command::StatusInto(self.id(), status_tx);
        Box::new(self.command(cmd).and_then(|_| status_rx.map_err(|e| e.into())))
    }

    fn close(&mut self) -> Box<Future<Item = (), Error = Error>> {
        let cmd = Command::Close(self.id());
        Box::new(self.command(cmd))
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
        let client_id = ClientId {
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
