use std::io;
use std::time::Duration;

use futures::{Async, Future, Stream, Poll};
use tokio_timer::{Timer, Sleep};

use protocol::*;

pub struct RxWithTimeout<T>
    where T: Stream<Item = Msg, Error = io::Error> + 'static
{
    stream: Option<T>,
    sleep: Option<Sleep>,
}

impl<T> RxWithTimeout<T>
    where T: Stream<Item = Msg, Error = io::Error> + 'static
{
    pub fn new(stream: T, timer: Timer, timeout: Option<Duration>) -> Self {
        let sleep = match timeout {
            Some(timeout) => Some(timer.sleep(timeout)),
            None => None,
        };
        RxWithTimeout {
            stream: Some(stream),
            sleep: sleep,
        }
    }
}

impl<T> Future for RxWithTimeout<T>
    where T: Stream<Item = Msg, Error = io::Error> + 'static
{
    type Item = (Msg, T);
    type Error = (ProtocolError, Option<T>);

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if let Some(sleep) = self.sleep.as_mut() {
            match sleep.poll() {
                Ok(Async::NotReady) => {}
                Ok(Async::Ready(_)) => {
                    // Timeout has been exceeded.
                    return Err((ProtocolError::Timeout, self.stream.take()));
                }
                Err(e) => {
                    // Timeout errored so stop.
                    return Err((ProtocolError::from(io::Error::from(e)), self.stream.take()));
                }
            }
        }

        return match self.stream.as_mut().unwrap().poll() {
            Ok(Async::Ready(Some(msg))) => {
                // Stream ready before timeout.
                Ok(Async::Ready((msg, self.stream.take().unwrap())))
            }
            Ok(Async::Ready(None)) => {
                // Stream has terminated. Do not return the terminated Stream because:
                // `further calls to poll may result in a panic or other "bad behavior".'
                // https://docs.rs/futures/0.1.7/futures/stream/trait.Stream.html#tymethod.poll
                Err((ProtocolError::StreamFinishedUnexpectedly, None))
            }
            // Stream and timeout not yet ready.
            Ok(Async::NotReady) => Ok(Async::NotReady),
            // Stream errored so stop.
            Err(e) => Err((ProtocolError::from(e), self.stream.take())),
        };
    }
}
