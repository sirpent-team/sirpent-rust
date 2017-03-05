use futures::{Sink, Future};
use net::command::Command;
use errors::Error;
use uuid::Uuid;
use net::command::{Timeout, Status, Commander};
use net::Msg;
use futures::sync::oneshot;

#[derive(Clone, Debug, PartialEq)]
pub struct Client<S, E>
    where S: Sink<SinkItem = Command, SinkError = E> + Send + Clone + 'static,
          E: Into<Error>
{
    // Clients are identified by UUID.
    uuid: Uuid,
    // Clients can also have an ID specific to their communication form, e.g., SocketAddr.
    kind_id: Option<String>,
    // Channel to command communications with.
    cmd_tx: S,
}

impl<S, E> Client<S, E>
    where S: Sink<SinkItem = Command, SinkError = E> + Send + Clone + 'static,
          E: Into<Error>
{
    pub fn new(uuid: Uuid, kind_id: Option<String>, cmd_tx: S) -> Client<S, E> {
        Client {
            uuid: uuid,
            kind_id: kind_id,
            cmd_tx: cmd_tx,
        }
    }

    pub fn uuid(&self) -> Uuid {
        self.uuid
    }

    pub fn kind_id(&self) -> Option<String> {
        self.kind_id.clone()
    }

    pub fn join(self, room: &mut Room<S, E>) -> Result<(), Error> {
        room.add(self.id)
    }

    fn command(&mut self, cmd: Command) -> Box<Future<Item = (), Error = Error>> {
        Box::new(self.cmd_tx.clone().send(cmd).map(|_| ()).map_err(|e| e.into()))
    }
}

impl<S, E> Commander for Client<S, E>
    where S: Sink<SinkItem = Command, SinkError = E> + Send + Clone + 'static,
          E: Into<Error>
{
    type Transmit = Msg;
    type Receive = Msg;
    type Status = Status;
    type Error = Error;

    fn transmit(&mut self, msg: Self::Transmit) -> Box<Future<Item = (), Error = Error>> {
        let cmd = Command::Transmit(self.uuid(), msg);
        Box::new(self.command(cmd))
    }

    fn receive(&mut self, timeout: Timeout) -> Box<Future<Item = Self::Receive, Error = Error>> {
        let (msg_forward_tx, msg_forward_rx) = oneshot::channel();
        let cmd = Command::ReceiveInto(self.uuid(), msg_forward_tx, timeout);
        Box::new(self.command(cmd).and_then(|_| msg_forward_rx.map_err(|e| e.into())))
    }

    fn status(&mut self) -> Box<Future<Item = Self::Status, Error = Error>> {
        let (status_tx, status_rx) = oneshot::channel();
        let cmd = Command::StatusInto(self.uuid(), status_tx);
        Box::new(self.command(cmd).and_then(|_| status_rx.map_err(|e| e.into())))
    }

    fn close(&mut self) -> Box<Future<Item = (), Error = Error>> {
        let cmd = Command::Close(self.uuid());
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

    #[test]
    fn can_transmit() {
        let (tx, rx) = mpsc::channel(1);
        let uuid = Uuid::new_v4();
        let mut client = Client::new(uuid, None, tx);
        let mut rx_stream = rx.wait().peekable();
        for _ in 0..10 {
            let msg = Msg::version();
            client.transmit(msg.clone()).wait().unwrap();
            match rx_stream.next() {
                Some(Ok(Command::Transmit(uuid2, msg2))) => {
                    assert!(uuid == uuid2);
                    assert!(msg == msg2);
                }
                _ => assert!(false),
            }
        }
    }

    #[test]
    fn can_receive() {
        let (tx, rx) = mpsc::channel(1);
        let uuid = Uuid::new_v4();
        let mut client = Client::new(uuid, None, tx);
        let mut rx_stream = rx.wait().peekable();
        for _ in 0..10 {
            let msg = Msg::version();
            let receive = client.receive(Timeout::None);

            let mut future = executor::spawn(receive.fuse());
            assert!(future.poll_future(unpark_noop()).unwrap().is_not_ready());
            match rx_stream.next().unwrap().unwrap() {
                Command::ReceiveInto(uuid2, msg_forward_tx, Timeout::None) => {
                    assert!(uuid == uuid2);
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
        let uuid = Uuid::new_v4();
        let mut client = Client::new(uuid, None, tx);
        let mut rx_stream = rx.wait().peekable();
        let mut futures = vec![];

        for _ in 0..10 {
            let msg = Msg::version();
            let mut future = executor::spawn(client.receive(Timeout::None));
            assert!(future.poll_future(unpark_noop()).unwrap().is_not_ready());
            futures.push((msg, future));
        }

        for &mut (ref mut msg, _) in &mut futures {
            match rx_stream.next().unwrap().unwrap() {
                Command::ReceiveInto(uuid2, msg_forward_tx, Timeout::None) => {
                    assert!(uuid == uuid2);
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
        let uuid = Uuid::new_v4();
        let mut client = Client::new(uuid, None, tx);
        let mut rx_stream = rx.wait().peekable();
        for _ in 0..10 {
            let mut future = executor::spawn(client.status());
            assert!(future.poll_future(unpark_noop()).unwrap().is_not_ready());
            match rx_stream.next().unwrap().unwrap() {
                Command::StatusInto(uuid2, status_reply_tx) => {
                    assert!(uuid == uuid2);
                    status_reply_tx.complete(Status::Ready);
                }
                _ => assert!(false),
            }
            match future.wait_future() {
                Ok(status) => assert!(status == Status::Ready),
                _ => assert!(false),
            }
        }
    }

    #[test]
    fn can_close() {
        let (tx, rx) = mpsc::channel(1);
        let uuid = Uuid::new_v4();
        let mut client = Client::new(uuid, None, tx);
        let mut rx_stream = rx.wait().peekable();
        for _ in 0..10 {
            let msg = Msg::version();
            client.transmit(msg.clone()).wait().unwrap();
            match rx_stream.next() {
                Some(Ok(Command::Transmit(uuid2, msg2))) => {
                    assert!(uuid == uuid2);
                    assert!(msg == msg2);
                }
                _ => assert!(false),
            }
        }
    }

    fn unpark_noop() -> Arc<executor::Unpark> {
        struct Foo;

        impl executor::Unpark for Foo {
            fn unpark(&self) {}
        }

        Arc::new(Foo)
    }
}
