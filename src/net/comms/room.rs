use futures::{Future, BoxFuture};
use futures::future::{join_all, JoinAll};
use super::*;
use std::collections::HashMap;

#[derive(Clone)]
pub struct Room<T, R>
    where T: Send + 'static,
          R: Send + 'static
{
    // @TODO: When RFC1422 is stable, make this `pub(super)`.
    #[doc(hidden)]
    pub clients: HashMap<ClientId, Client<T, R>>,
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

    fn communicate_on_clients<F, G>(&mut self, f: F) -> JoinAll<Vec<G>>
        where F: FnMut(&mut Client<T, R>) -> G,
              G: Future
    {
        join_all(self.clients.values_mut().map(f).collect::<Vec<_>>())
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
        self.communicate_on_clients(|client| client.receive(timeout))
            .map(|results| {
                let mut statuses = HashMap::new();
                let mut msgs = HashMap::new();
                for (id, status, msg) in results.into_iter() {
                    statuses.insert(id, status);
                    msg.and_then(|msg| msgs.insert(id, msg));
                }
                (statuses, msgs)
            })
            .boxed()
    }

    fn status(&mut self) -> BoxFuture<Self::Status, ()> {
        self.communicate_on_clients(|client| client.status())
            .map(|results| results.into_iter().collect())
            .boxed()
    }

    fn close(&mut self) -> BoxFuture<Self::Status, ()> {
        self.communicate_on_clients(|client| client.close())
            .map(|results| results.into_iter().collect())
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    /*
    use super::*;
    use super::test::*;
    use uuid::Uuid;
    use futures::sync::mpsc;
    use futures::{Stream, executor};
    use std::sync::Arc;

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
