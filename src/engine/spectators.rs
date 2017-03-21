pub struct Spectators {
    spectator_rx: mpsc::Receiver<Client<Uuid>>,
    spectators: Room<Uuid>,
    msg_rx: mpsc::Receiver<Msg>,
    msg_queue: VecDeque<Msg>,
}

impl Future for Spectators {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        loop {
            self.spectator_rx.poll() {
                Ok(Async::NotReady) => break,
                Ok(Async::Ready(Some(client))) => {
                    client.join(&mut self.spectators);
                },
                Ok(Async::Ready(None)) => {
                    // If stream closed, shutdown this future.
                    // @TODO: Guard this a bit better with panics.
                    self.spectator_rx.close();
                    self.spectators.close();
                },
                Err(_) => {}
            }
        }

        loop {
            self.msg_rx.poll() {
                Ok(Async::NotReady) => break,
                Ok(Async::Ready(Some(msg))) => self.msg_queue.push_back(msg),
                Ok(Async::Ready(None)) => {
                    // If stream closed, shutdown this future.
                    // @TODO: Guard this a bit better with panics.
                    self.spectator_rx.close();
                    self.spectators.close();
                },
                Err(_) => {}
            }
        }

        while Some(msg) = msg_queue.pop_front() {
            match self.spectator_rx.start_send(msg) {
                Ok(AsyncSink::NotReady(msg)) => msg_queue.push_front(msg),
                Ok(AsyncSink::Ready) => {},
                Err(e) => msg_queue.push_front(msg),
            }
        }
    }
}
