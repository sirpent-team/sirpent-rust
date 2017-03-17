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
    pub fn new(clients: Vec<Client<T, R>>) -> Room<T, R> {
        let clients = clients.into_iter().map(|c| (c.id(), c)).collect();
        Room { clients: clients }
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

impl<T, R> Default for Room<T, R>
    where T: Send + 'static,
          R: Send + 'static
{
    fn default() -> Room<T, R> {
        Room { clients: HashMap::new() }
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
    use super::*;
    use super::test::*;
    use futures::Stream;

    #[test]
    fn can_transmit() {
        let (rx0, client0) = mock_client_channelled();
        let mut client0_rx = rx0.wait().peekable();
        let client0_id = client0.id();

        let (rx1, client1) = mock_client_channelled();
        let mut client1_rx = rx1.wait().peekable();
        let client1_id = client1.id();

        let mut room = Room::new(vec![client0, client1]);

        let mut msgs = HashMap::new();
        msgs.insert(client0_id, TinyMsg::A);
        msgs.insert(client1_id, TinyMsg::B("entropy".to_string()));
        room.transmit(msgs).wait().unwrap();
        match (client0_rx.next(), client1_rx.next()) {
            (Some(Ok(_)), Some(Ok(_))) => {}
            _ => assert!(false),
        }
    }
}
