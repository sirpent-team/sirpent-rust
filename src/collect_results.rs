use std::prelude::v1::*;

use std::mem;

use futures::{Future, Poll, Async};
use futures::stream::Stream;

/// A future which collects all of the outputs of a stream into a vector of Result<Item, Error>.
///
/// This future is created by the `collect_results` function.
#[must_use = "streams do nothing unless polled"]
pub struct CollectResults<S> where S: Stream {
    stream: S,
    items: Vec<Result<S::Item, S::Error>>,
}

pub fn collect_results<S>(s: S) -> CollectResults<S>
    where S: Stream,
{
    CollectResults {
        stream: s,
        items: Vec::new(),
    }
}

impl<S: Stream> CollectResults<S> {
    fn finish(&mut self) -> Vec<Result<S::Item, S::Error>> {
        mem::replace(&mut self.items, Vec::new())
    }
}

impl<S> Future for CollectResults<S>
    where S: Stream
{
    type Item = Vec<Result<S::Item, S::Error>>;
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, ()> {
        loop {
            match self.stream.poll() {
                Ok(Async::Ready(Some(e))) => self.items.push(Ok(e)),
                Ok(Async::Ready(None)) => return Ok(Async::Ready(self.finish())),
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(e) => self.items.push(Err(e))
            }
        }
    }
}
