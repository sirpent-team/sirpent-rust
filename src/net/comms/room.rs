use futures::{Sink, Future};
use errors::Error;
use super::*;
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

    // Unfortunately this has to take a pointer to give the option of keeping the `Client`
    // around. I'd rather taken it by value and force people to explictly clone to do that,
    // but with `futures::sync::mpsc::SendError` not being `Clone` one cannot clone that
    // natural case of `Client`.
    pub fn join(&mut self, client: &Client<S, E>) -> Result<bool, Error> {
        self.insert(client.id())
    }

    pub fn insert(&mut self, id: ClientId) -> Result<bool, Error> {
        if !self.cmd_tx.can_command(&id) {
            return Err(format!("Attempted to add a client to a room using a different listener")
                .into());
        }
        Ok(self.client_ids.insert(id))
    }

    pub fn contains(&self, id: &ClientId) -> bool {
        self.client_ids.contains(&id)
    }

    fn command(&mut self, cmd: Command) -> Box<Future<Item = (), Error = Error>> {
        Box::new(self.cmd_tx.clone().send(cmd).map(|_| ()).map_err(|e| e.into()))
    }
}

impl<S, E> Communicator for Room<S, E>
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
        CommandChannel {
            id: Uuid::new_v4(),
            cmd_tx: tx,
        }
    }

    fn mock_client(command_channel: &CommandChannel<mpsc::Sender<Command>>)
                   -> Client<mpsc::Sender<Command>, mpsc::SendError<Command>> {
        let client_id = ClientId {
            client: Uuid::new_v4(),
            communicator: command_channel.id(),
        };
        Client::new(client_id, None, command_channel.clone()).unwrap()
    }

    fn mock_room(command_channel: &CommandChannel<mpsc::Sender<Command>>)
                 -> Room<mpsc::Sender<Command>, mpsc::SendError<Command>> {
        Room::new(command_channel.clone())
    }

    #[test]
    fn can_insert() {
        let (tx, _) = mpsc::channel(1);
        let cmd_tx = mock_command_channel(tx.clone());
        let client = mock_client(&cmd_tx);

        // First adding of a `ClientId` to a Room returns `Ok(true)`.
        // Second indicates the `ClientId` was already present with `Ok(false)`.
        let mut room = mock_room(&cmd_tx);
        assert!(!room.contains(&client.id()));
        assert!(room.insert(client.id()).unwrap());
        assert!(room.contains(&client.id()));
        assert!(!room.insert(client.id()).unwrap());
        assert!(room.contains(&client.id()));

        // This tests that a `ClientId` cannot be added to a room unless the communicator IDs
        // match. Notably this isn't the same as, "the underlying Sink is not the same," for
        // unfortunate implementation details mentioned below.
        let (tx2, _) = mpsc::channel(1);
        let mut invalid_room2 = mock_room(&mock_command_channel(tx2));
        assert!(!invalid_room2.contains(&client.id()));
        assert!(invalid_room2.insert(client.id()).is_err());
        assert!(!invalid_room2.contains(&client.id()));

        // This tests an important implementation detail: that at present `Room`s are identified
        // using an ID randomly generated when instantiating from a `CommandChannel`. Thus even
        // if the same `CommandChannel` or inner `Sink` is in use we should get an error that the
        // communicator ids do not match.
        //
        // It would be nice to resolve this. Perhaps `Room` should be created by the far side
        // using a `Command`, thus linking things up nicer. But this doesn't absolutely resolve
        // the issue. Only requiring the use of a `Sink` implementing `PartialEq` would do this
        // properly - and even `futures::sync::mpsc::Sender` does not do this.
        let mut invalid_room1 = mock_room(&mock_command_channel(tx));
        assert!(!invalid_room1.contains(&client.id()));
        assert!(invalid_room1.insert(client.id()).is_err());
        assert!(!invalid_room1.contains(&client.id()));
    }

    // Duplicates `can_insert` but for usage of `Room::join`.
    #[test]
    fn can_join() {
        let (tx, _) = mpsc::channel(1);
        let cmd_tx = mock_command_channel(tx.clone());
        let client = mock_client(&cmd_tx);

        // First adding of a `ClientId` to a Room returns `Ok(true)`.
        // Second indicates the `ClientId` was already present with `Ok(false)`.
        let mut room = mock_room(&cmd_tx);
        assert!(!room.contains(&client.id()));
        assert!(room.join(&client).unwrap());
        assert!(room.contains(&client.id()));
        assert!(!room.join(&client).unwrap());
        assert!(room.contains(&client.id()));

        // This tests that a `ClientId` cannot be added to a room unless the communicator IDs
        // match. Notably this isn't the same as, "the underlying Sink is not the same," for
        // unfortunate implementation details mentioned below.
        let (tx2, _) = mpsc::channel(1);
        let mut invalid_room2 = mock_room(&mock_command_channel(tx2));
        assert!(!invalid_room2.contains(&client.id()));
        assert!(invalid_room2.join(&client).is_err());
        assert!(!invalid_room2.contains(&client.id()));

        // This tests an important implementation detail: that at present `Room`s are identified
        // using an ID randomly generated when instantiating from a `CommandChannel`. Thus even
        // if the same `CommandChannel` or inner `Sink` is in use we should get an error that the
        // communicator ids do not match.
        //
        // It would be nice to resolve this. Perhaps `Room` should be created by the far side
        // using a `Command`, thus linking things up nicer. But this doesn't absolutely resolve
        // the issue. Only requiring the use of a `Sink` implementing `PartialEq` would do this
        // properly - and even `futures::sync::mpsc::Sender` does not do this.
        let mut invalid_room1 = mock_room(&mock_command_channel(tx));
        assert!(!invalid_room1.contains(&client.id()));
        assert!(invalid_room1.join(&client).is_err());
        assert!(!invalid_room1.contains(&client.id()));
    }

    /*
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
    */
}
