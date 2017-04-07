use super::*;
use futures::{future, Future, Stream, Sink};
use futures::sync::{mpsc, oneshot};
use tokio_timer;
use std::net::SocketAddr;
use state::GridEnum;
use std::collections::HashSet;

pub fn client_handshake<G>(unnamed_client: MsgClient<SocketAddr>,
                           grid: G,
                           timeout: Milliseconds,
                           timer: tokio_timer::Timer,
                           nameserver_tx: mpsc::Sender<(String, oneshot::Sender<String>)>,
                           player_tx: mpsc::Sender<MsgClient<String>>,
                           spectator_tx: mpsc::Sender<MsgClient<String>>)
                           -> Box<Future<Item = (), Error = ()>>
    where G: Into<GridEnum> + 'static
{
    ClientHandshake::handshake(unnamed_client,
                               grid,
                               timeout,
                               timer,
                               nameserver_tx,
                               player_tx,
                               spectator_tx)
}

struct ClientHandshake;

impl ClientHandshake {
    fn handshake<G>(unnamed_client: MsgClient<SocketAddr>,
                    grid: G,
                    timeout: Milliseconds,
                    timer: tokio_timer::Timer,
                    nameserver_tx: mpsc::Sender<(String, oneshot::Sender<String>)>,
                    player_tx: mpsc::Sender<MsgClient<String>>,
                    spectator_tx: mpsc::Sender<MsgClient<String>>)
                    -> Box<Future<Item = (), Error = ()>>
        where G: Into<GridEnum> + 'static
    {
        let version_msg = Msg::version();
        let version_future = unnamed_client.transmit(version_msg).map_err(|_| ());

        let timeout_clone = timeout.clone();
        let registration_future =
            move |unnamed_client| Self::registration(unnamed_client, timeout_clone, timer);

        let welcome_future = move |client| Self::welcome(client, grid, timeout);
        let rename_future = move |(unnamed_client, desired_name, kind)| {
            Self::rename(unnamed_client, desired_name, nameserver_tx)
                .and_then(welcome_future)
                .map(move |client| (client, kind))
        };

        let forward_future = move |(client, kind)| {
            match kind {
                    ClientKind::Player => player_tx.send(client),
                    ClientKind::Spectator => spectator_tx.send(client),
                }
                .map(|_| ())
                .map_err(|_| ())
        };

        let handshake_future = version_future
            .and_then(registration_future)
            .and_then(rename_future)
            .and_then(forward_future);
        Box::new(handshake_future)
    }

    fn registration
        (unnamed_client: MsgClient<SocketAddr>,
         timeout: Milliseconds,
         timer: tokio_timer::Timer)
         -> Box<Future<Item = (MsgClient<SocketAddr>, String, ClientKind), Error = ()>> {
        let register_rx = unnamed_client
            .receive()
            .with_hard_timeout(timeout.into(), &timer)
            .map_err(|_| ());
        let receive_registration_future = register_rx.and_then(|(msg, unnamed_client)| {
            if let Msg::Register { desired_name, kind } = msg {
                future::ok((unnamed_client, desired_name, kind))
            } else {
                future::err(())
            }
        });
        Box::new(receive_registration_future)
    }

    fn rename(unnamed_client: MsgClient<SocketAddr>,
              desired_name: String,
              nameserver_tx: mpsc::Sender<(String, oneshot::Sender<String>)>)
              -> Box<Future<Item = MsgClient<String>, Error = ()>> {
        let (name_tx, name_rx) = oneshot::channel();
        let ns_tx_future = nameserver_tx
            .send((desired_name, name_tx))
            .map_err(|_| ());
        let ns_rx_future = name_rx.map_err(|_| ());
        let rename_future = ns_tx_future
            .join(ns_rx_future)
            .map(|(_, name)| unnamed_client.rename(name));
        Box::new(rename_future)
    }

    fn welcome<G>(client: MsgClient<String>,
                  grid: G,
                  timeout: Milliseconds)
                  -> Box<Future<Item = MsgClient<String>, Error = ()>>
        where G: Into<GridEnum>
    {
        let welcome_msg = Msg::Welcome {
            name: client.id(),
            grid: grid.into(),
            timeout_millis: Some(timeout),
        };
        let welcome_future = client.transmit(welcome_msg).map_err(|_| ());
        Box::new(welcome_future)
    }
}

pub fn client_nameserver(name_desires: mpsc::Receiver<(String, oneshot::Sender<String>)>)
                         -> Box<Future<Item = (), Error = ()>> {
    let mut names: HashSet<String> = HashSet::new();
    let desires_for_each = name_desires.for_each(move |(desired_name, name_tx)| {
        let mut name = desired_name.clone();
        let mut n = 1;
        while names.contains(&name) {
            name = format!("{}_{}", desired_name, roman_numerals(n));
            n += 1;
        }
        names.insert(name.clone());
        let _ = name_tx.send(name);
        future::ok(())
    });
    Box::new(desires_for_each)
}
