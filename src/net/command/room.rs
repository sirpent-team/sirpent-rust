use futures::{Sink, Future};
use net::command::Command;
use errors::Error;
use net::command::*;
use net::Msg;
use std::collections::{HashSet, HashMap};
use futures::sync::oneshot;

#[derive(Clone, Debug, PartialEq)]
pub struct Room<S, E>
    where S: Sink<SinkItem = Command, SinkError = E> + Send + Clone + 'static,
          E: Into<Error> + 'static
{
    client_ids: HashSet<ClientId>,
    // Channel to command communications with.
    cmd_tx: CommandChannel<S>,
}

impl<S, E> Room<S, E>
    where S: Sink<SinkItem = Command, SinkError = E> + Send + Clone + 'static,
          E: Into<Error> + 'static
{
    pub fn new(cmd_tx: CommandChannel<S>) -> Room<S, E> {
        Room {
            client_ids: HashSet::new(),
            cmd_tx: cmd_tx,
        }
    }

    pub fn client_ids(&self) -> HashSet<ClientId> {
        self.client_ids.clone()
    }

    pub fn add(&mut self, id: ClientId) -> Result<bool, Error> {
        if !self.cmd_tx.can_command(&id) {
            return Err(format!("Attempted to add a client to a room using a different listener")
                .into());
        }
        Ok(self.client_ids.insert(id))
    }

    fn command(&mut self, cmd: Command) -> Box<Future<Item = (), Error = Error>> {
        Box::new(self.cmd_tx.clone().send(cmd).map(|_| ()).map_err(|e| e.into()))
    }
}

impl<S, E> Commander for Room<S, E>
    where S: Sink<SinkItem = Command, SinkError = E> + Send + Clone + 'static,
          E: Into<Error> + 'static
{
    type Transmit = HashMap<ClientId, Msg>;
    type Receive = HashMap<ClientId, Msg>;
    type Status = HashMap<ClientId, ClientStatus>;
    type Error = Error;

    fn transmit(&mut self, msgs: Self::Transmit) -> Box<Future<Item = (), Error = Error>> {
        let cmd = Command::TransmitToGroup(msgs);
        Box::new(self.command(cmd))
    }

    fn receive(&mut self,
               timeout: ClientTimeout)
               -> Box<Future<Item = Self::Receive, Error = Error>> {
        let (msg_forward_tx, msg_forward_rx) = oneshot::channel();
        let cmd = Command::ReceiveFromGroupInto(self.client_ids(), msg_forward_tx, timeout);
        Box::new(self.command(cmd).and_then(|_| msg_forward_rx.map_err(|e| e.into())))
    }

    fn status(&mut self) -> Box<Future<Item = Self::Status, Error = Error>> {
        let (status_tx, status_rx) = oneshot::channel();
        let cmd = Command::StatusFromGroupInto(self.client_ids(), status_tx);
        Box::new(self.command(cmd).and_then(|_| status_rx.map_err(|e| e.into())))
    }

    fn close(&mut self) -> Box<Future<Item = (), Error = Error>> {
        let cmd = Command::CloseGroup(self.client_ids());
        Box::new(self.command(cmd))
    }
}

/*
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
        let mut group = Group::new(uuid, None, tx);
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
}
*/
