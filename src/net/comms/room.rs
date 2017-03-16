use futures::{Future, BoxFuture};
use futures::future::join_all;
use super::*;
use std::collections::HashMap;

#[derive(Clone)]
pub struct Room<T, R>
    where T: Send + 'static,
          R: Send + 'static
{
    clients: HashMap<ClientId, Client<T, R>>,
}

impl<T, R> Room<T, R>
    where T: Send + 'static,
          R: Send + 'static
{
    pub fn new() -> Room<T, R> {
        Room { clients: HashMap::new() }
    }

    pub fn client_ids(&self) -> Vec<ClientId> {
        self.clients.keys().cloned().collect()
    }

    // Unfortunately this has to take a pointer to give the option of keeping the `Client`
    // around. I'd rather taken it by value and force people to explictly clone to do that,
    // but with `futures::sync::mpsc::SendError` not being `Clone` one cannot clone that
    // natural case of `Client`.
    pub fn insert(&mut self, client: Client<T, R>) -> bool {
        if self.contains(&client.id()) {
            return false;
        }
        self.clients.insert(client.id(), client);
        true
    }

    pub fn contains(&self, id: &ClientId) -> bool {
        self.clients.contains_key(id)
    }
}

impl<T, R> Communicator for Room<T, R>
    where T: Send + 'static,
          R: Send + 'static
{
    type Transmit = HashMap<ClientId, T>;
    type Receive = (HashMap<ClientId, ClientStatus>, HashMap<ClientId, R>);
    type Status = HashMap<ClientId, ClientStatus>;
    type Error = ();

    fn transmit(&mut self, msgs: Self::Transmit) -> BoxFuture<Self::Status, ()> {
        let client_futures = msgs.into_iter()
            .filter_map(|(id, msg)| self.clients.get_mut(&id).map(|client| client.transmit(msg)))
            .collect::<Vec<_>>();
        join_all(client_futures).map(|results| results.into_iter().collect()).boxed()
    }

    fn receive(&mut self, timeout: ClientTimeout) -> BoxFuture<Self::Receive, ()> {
        let client_futures =
            self.clients.iter_mut().map(|(_, client)| client.receive(timeout)).collect::<Vec<_>>();
        join_all(client_futures)
            .map(|results| {
                let mut statuses = HashMap::new();
                let mut msgs = HashMap::new();
                for (id, (status, msg)) in results.into_iter() {
                    statuses.insert(id, status);
                    msg.and_then(|msg| msgs.insert(id, msg));
                }
                (statuses, msgs)
            })
            .boxed()
    }

    fn status(&mut self) -> BoxFuture<Self::Status, ()> {
        let client_futures =
            self.clients.iter_mut().map(|(_, client)| client.status()).collect::<Vec<_>>();
        join_all(client_futures).map(|results| results.into_iter().collect()).boxed()
    }

    fn close(&mut self) -> BoxFuture<Self::Status, ()> {
        let client_futures =
            self.clients.iter_mut().map(|(_, client)| client.close()).collect::<Vec<_>>();
        join_all(client_futures).map(|results| results.into_iter().collect()).boxed()
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

    fn mock_room(command_channel: &CommandChannel<mpsc::Sender<Command>>)
                 -> Room<mpsc::Sender<Command>, mpsc::SendError<Command>> {
        Room::new(command_channel.clone())
    }

    #[test]
    fn can_insert() {
        let (tx, _) = mpsc::channel(1);
        let cmd_tx = mock_command_channel(tx.clone());
        let client = mock_client(&cmd_tx);

        // First adding of a `CommunicationId` to a Room returns `Ok(true)`.
        // Second indicates the `CommunicationId` was already present with `Ok(false)`.
        let mut room = mock_room(&cmd_tx);
        assert!(!room.contains(&client.id()));
        assert!(room.insert(client.id()).unwrap());
        assert!(room.contains(&client.id()));
        assert!(!room.insert(client.id()).unwrap());
        assert!(room.contains(&client.id()));

        // This tests that a `CommunicationId` cannot be added to a room unless the communicator IDs
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

        // First adding of a `CommunicationId` to a Room returns `Ok(true)`.
        // Second indicates the `CommunicationId` was already present with `Ok(false)`.
        let mut room = mock_room(&cmd_tx);
        assert!(!room.contains(&client.id()));
        assert!(room.join(&client).unwrap());
        assert!(room.contains(&client.id()));
        assert!(!room.join(&client).unwrap());
        assert!(room.contains(&client.id()));

        // This tests that a `CommunicationId` cannot be added to a room unless the communicator IDs
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
