use futures::{Sink, Future, BoxFuture, future};
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
    pub fn new(name: Option<String>, cmd_tx: mpsc::Sender<Command<T, R>>) -> Client<T, R> {
        Client {
            id: Uuid::new_v4(),
            name: name,
            cmd_tx: cmd_tx,
        }
    }

    pub fn id(&self) -> ClientId {
        self.id
    }

    pub fn name(&self) -> Option<String> {
        self.name.clone()
    }

    pub fn join(self, room: &mut Room<T, R>) -> bool {
        room.insert(self)
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
    type Receive = (ClientId, ClientStatus, Option<R>);
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
                Ok(m) => future::ok((id, ClientStatus::Ready, Some(m))).boxed(),
                Err(e) => future::ok((id, e, None)).boxed(),
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
    use super::test::*;
    use futures::sync::mpsc;
    use futures::{Stream, executor};

    #[test]
    fn can_join_room() {
        let (_, client0) = mock_client_channelled();
        let (_, client1) = mock_client_channelled();

        // Adding of a `Client` to a `Room` returns `true`.
        let mut room = Room::default();
        assert_eq!(room.client_ids().len(), 0);
        assert!(client0.clone().join(&mut room));
        assert_eq!(room.client_ids(), vec![client0.id]);

        // Adding a `Client` whose ID was already present returns `false` and doesn't
        // add a duplicate.
        let client0_id = client0.id;
        assert!(!client0.join(&mut room));
        assert_eq!(room.client_ids(), vec![client0_id]);

        // Adding a different-IDed `Client` to a `Room` works.
        let client1_id = client1.id;
        assert!(client1.join(&mut room));
        // Extended comparison necessary because ordering not preserved.
        let client_ids = room.client_ids();
        assert!(client_ids.len() == 2);
        assert!(client_ids.contains(&client0_id));
        assert!(client_ids.contains(&client1_id));
    }

    #[test]
    fn can_transmit() {
        let (tx, rx) = mpsc::channel(1);
        let mut rx_stream = rx.wait().peekable();
        let mut client = mock_client(tx);

        for _ in 0..10 {
            let msg = TinyMsg::A;
            client.transmit(msg.clone()).wait().unwrap();

            match rx_stream.next() {
                Some(Ok(Command::Transmit(client_id, msg2))) => {
                    assert_eq!(client_id, client.id());
                    assert_eq!(msg, msg2);
                }
                _ => assert!(false),
            }
        }
    }

    #[test]
    fn can_receive() {
        let (tx, rx) = mpsc::channel(1);
        let mut rx_stream = rx.wait().peekable();
        let mut client = mock_client(tx);

        for _ in 0..10 {
            let msg = TinyMsg::B("ABC".to_string());
            let receive = client.receive(ClientTimeout::None);

            let mut future = executor::spawn(receive.fuse());

            assert!(future.poll_future(unpark_noop()).unwrap().is_not_ready());

            match rx_stream.next().unwrap().unwrap() {
                Command::ReceiveInto(client_id, ClientTimeout::None, msg_forward_tx) => {
                    assert_eq!(client_id, client.id());
                    msg_forward_tx.complete(msg.clone());
                }
                _ => assert!(false),
            }

            match future.wait_future() {
                Ok((id, status, maybe_msg)) => {
                    assert_eq!(id, client.id);
                    assert_eq!(status, ClientStatus::Ready);
                    assert_eq!(msg, maybe_msg.unwrap());
                }
                _ => assert!(false),
            }
        }
    }

    #[test]
    fn can_receive_queued() {
        let (tx, rx) = mpsc::channel(1);
        let mut rx_stream = rx.wait().peekable();
        let mut client = mock_client(tx);

        let mut futures = vec![];

        for _ in 0..10 {
            let msg = TinyMsg::A;
            let mut future = executor::spawn(client.receive(ClientTimeout::None));
            assert!(future.poll_future(unpark_noop()).unwrap().is_not_ready());
            futures.push((msg, future));
        }

        for &mut (ref mut msg, _) in &mut futures {
            match rx_stream.next().unwrap().unwrap() {
                Command::ReceiveInto(client_id, ClientTimeout::None, msg_forward_tx) => {
                    assert_eq!(client_id, client.id);
                    msg_forward_tx.complete(msg.clone());
                }
                _ => assert!(false),
            }
        }

        for &mut (ref mut msg, ref mut future) in &mut futures {
            match future.wait_future() {
                Ok((id, status, maybe_msg)) => {
                    assert_eq!(id, client.id);
                    assert_eq!(status, ClientStatus::Ready);
                    assert_eq!(msg.clone(), maybe_msg.unwrap());
                }
                _ => assert!(false),
            }
        }
    }

    #[test]
    fn can_status() {
        let (tx, rx) = mpsc::channel(1);
        let mut rx_stream = rx.wait().peekable();
        let mut client = mock_client(tx);

        for _ in 0..10 {
            let mut future = executor::spawn(client.status());

            assert!(future.poll_future(unpark_noop()).unwrap().is_not_ready());

            match rx_stream.next().unwrap().unwrap() {
                Command::StatusInto(client_id, status_reply_tx) => {
                    assert_eq!(client_id, client.id());
                    status_reply_tx.complete(ClientStatus::Ready);
                }
                _ => assert!(false),
            }

            match future.wait_future() {
                Ok((id, status)) => {
                    assert_eq!(id, client.id);
                    assert_eq!(status, ClientStatus::Ready);
                }
                _ => assert!(false),
            }
        }
    }

    #[test]
    fn can_close() {
        let (tx, rx) = mpsc::channel(1);
        let mut rx_stream = rx.wait().peekable();
        let mut client = mock_client(tx);

        for _ in 0..10 {
            let msg = TinyMsg::B("test".to_string());
            client.transmit(msg.clone()).wait().unwrap();
            match rx_stream.next() {
                Some(Ok(Command::Transmit(client_id, msg2))) => {
                    assert_eq!(client_id, client.id());
                    assert_eq!(msg, msg2);
                }
                _ => assert!(false),
            }
        }
    }
}
