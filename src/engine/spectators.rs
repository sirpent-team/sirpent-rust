use std::collections::VecDeque;
use futures::{Future, Sink, Stream, Poll, Async, AsyncSink};
use futures::sync::mpsc;

use net::*;

pub struct Spectators {
    spectator_rx: mpsc::Receiver<Client<String>>,
    spectators: Room<String>,
    msg_rx: mpsc::Receiver<Msg>,
    msg_queue: VecDeque<Msg>,
}

impl Spectators {
    pub fn new(spectator_rx: mpsc::Receiver<Client<String>>,
               msg_rx: mpsc::Receiver<Msg>)
               -> Spectators {
        Spectators {
            spectator_rx: spectator_rx,
            spectators: Room::default(),
            msg_rx: msg_rx,
            msg_queue: VecDeque::new(),
        }
    }
}

impl Future for Spectators {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        println!("spectators wakeup {:?}", self.spectators.ready_ids());

        loop {
            match self.spectator_rx.poll() {
                Ok(Async::NotReady) => break,
                Ok(Async::Ready(Some(client))) => {
                    client.join(&mut self.spectators);
                }
                Ok(Async::Ready(None)) => {
                    // If stream closed, shutdown this future.
                    // @TODO: Guard this a bit better with panics.
                    self.spectator_rx.close();
                    self.spectators.close();
                }
                Err(_) => {}
            }
        }

        loop {
            match self.msg_rx.poll() {
                Ok(Async::NotReady) => break,
                Ok(Async::Ready(Some(msg))) => self.msg_queue.push_back(msg),
                Ok(Async::Ready(None)) => {
                    // If stream closed, shutdown this future.
                    // @TODO: Guard this a bit better with panics.
                    self.spectator_rx.close();
                    self.spectators.close();
                }
                Err(_) => {}
            }
        }

        match self.spectators.poll_complete() {
            Ok(Async::Ready(())) => {}
            Ok(Async::NotReady) => {},
            Err(_) => {}
        }

        while let Some(msg) = self.msg_queue.pop_front() {
            let msgs = self.spectators.ids().into_iter().map(|id| (id, msg.clone())).collect();
            println!("{:?}", msgs);
            match self.spectators.start_send(msgs) {
                Ok(AsyncSink::NotReady(_)) |
                Err(_) => {
                    self.msg_queue.push_front(msg);
                    break;
                },
                Ok(AsyncSink::Ready) => {}
            }

            match self.spectators.poll_complete() {
                Ok(Async::Ready(())) => continue,
                Ok(Async::NotReady) => break,
                Err(_) => {}
            }
        }

        Ok(Async::NotReady)
    }
}
