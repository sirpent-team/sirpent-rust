use futures::{future, Future};
use tokio_timer;
use std::net::SocketAddr;
use state::GridEnum;
use kabuki::{Actor, ActorRef};
use std::fmt::Debug;

use net::*;
use utils::*;

#[derive(Clone)]
pub struct Handshake {
    grid: GridEnum,
    timeout: Milliseconds,
    timer: tokio_timer::Timer,
    nameserver: ActorRef<String, String, ()>,
}

impl Handshake {
    pub fn new<G>(grid: G,
                  timeout: Milliseconds,
                  timer: tokio_timer::Timer,
                  nameserver: ActorRef<String, String, ()>)
                  -> Handshake
        where G: Into<GridEnum>
    {
        Handshake {
            grid: grid.into(),
            timeout: timeout,
            timer: timer,
            nameserver: nameserver,
        }
    }

    fn transmit<I>(client: MsgClient<I>, msg: Msg) -> Box<Future<Item = MsgClient<I>, Error = ()>>
        where I: Clone + Send + Debug
    {
        Box::new(client.transmit(msg).map_err(|_| ()))
    }

    fn receive<I>(client: MsgClient<I>,
                  timeout: Milliseconds,
                  timer: tokio_timer::Timer)
                  -> Box<Future<Item = (Msg, MsgClient<I>), Error = ()>>
        where I: Clone + Send + Debug
    {
        Box::new(client
                     .receive()
                     .with_hard_timeout(timeout.into(), &timer)
                     .map_err(|_| ()))
    }

    fn rename_and_welcome(unnamed_client: MsgClient<SocketAddr>,
                          desired_name: String,
                          grid: GridEnum,
                          timeout: Milliseconds,
                          mut nameserver: ActorRef<String, String, ()>)
                          -> Box<Future<Item = MsgClient<String>, Error = ()>> {
        let fut = nameserver
            .call(desired_name)
            .and_then(move |final_name| {
                let client = unnamed_client.rename(final_name);
                let welcome_msg = Msg::Welcome {
                    name: client.id(),
                    grid: grid,
                    timeout_millis: Some(timeout),
                };
                Self::transmit(client, welcome_msg)
            });
        Box::new(fut)
    }
}

impl Actor for Handshake {
    type Request = MsgClient<SocketAddr>;
    type Response = (MsgClient<String>, ClientKind);
    type Error = ();
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&mut self, unnamed_client: Self::Request) -> Self::Future {
        let Handshake {
            grid,
            timeout,
            timer,
            nameserver,
        } = self.clone();

        let version = Self::transmit(unnamed_client, Msg::version());
        let registration_fn = move |unnamed_client| {
            Self::receive(unnamed_client, timeout, timer).and_then(move |(msg, unnamed_client)| -> Box<Future<Item = (MsgClient<String>, ClientKind), Error = ()>> {
                if let Msg::Register { desired_name, kind } = msg {
                    Box::new(Self::rename_and_welcome(unnamed_client, desired_name, grid, timeout, nameserver)
                        .map(move |client| (client, kind)))
                } else {
                    Box::new(future::err(()))
                }
            })
        };
        Box::new(version.and_then(registration_fn))
    }
}
