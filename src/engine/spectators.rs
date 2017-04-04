use std::collections::VecDeque;
use futures::{Future, Sink, Stream, Poll, Async, AsyncSink};
use futures::sync::mpsc;
use comms::Room;

use net::*;

pub struct Spectators {
    spectator_rx: mpsc::Receiver<MsgClient<String>>,
    spectators: MsgRoom<String>,
    msg_rx: mpsc::Receiver<Msg>,
    msg_queue: VecDeque<Msg>,
}

impl Spectators {
    pub fn new(spectator_rx: mpsc::Receiver<MsgClient<String>>,
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
        println!("spectators wakeup {:?}", self.spectators.ids());

        loop {
            match self.spectator_rx.poll() {
                Ok(Async::NotReady) => break,
                Ok(Async::Ready(Some(client))) => {
                    self.spectators.insert(client);
                }
                Ok(Async::Ready(None)) => {
                    // If stream closed, shutdown this future.
                    // @TODO: Guard this a bit better with panics.
                    self.spectator_rx.close();
                    self.spectators.close_all();
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
                    self.spectators.close_all();
                }
                Err(_) => {}
            }
        }

        // If any spectator sends a message, disconnect them as that behaviour is not
        // consistent with spectating.
        match self.spectators.poll() {
            Ok(Async::Ready(Some((id, _)))) => {
                // @TODO: Would be nice to have a `close_one` method to avoid the heap Vec.
                self.spectators.close(vec![id].into_iter().collect());
            }
            _ => {}
        }

        match self.spectators.poll_complete() {
            Ok(Async::Ready(())) => {}
            Ok(Async::NotReady) => {}
            Err(_) => {}
        }

        while let Some(msg) = self.msg_queue.pop_front() {
            for id in self.spectators.ids() {
                match self.spectators.start_send((id.clone(), msg.clone())) {
                    Ok(AsyncSink::NotReady(_)) |
                    Err(_) => {
                        self.spectators.close(vec![id].into_iter().collect());
                    }
                    Ok(AsyncSink::Ready) => {}
                }
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
