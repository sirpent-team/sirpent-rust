use std::io;
use std::error;
use serde_json;
use serde::Serialize;
use std::collections::HashMap;
use futures::{Future, Stream, Sink, Poll, StartSend};

use errors::*;

pub fn json<T>(value: T) -> Result<String>
    where T: Serialize
{
    serde_json::to_string(&value).chain_err(|| "serialising into json")
}

pub fn io_error_from_str(desc: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, desc)
}

pub fn io_error_from_error<E: error::Error>(e: E) -> io::Error {
    io_error_from_str(&*format!("{:?}", e))
}

pub fn io_error_broken_pipe() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "Broken channel.")
}

pub fn retain_oks<O>(h: HashMap<String, Result<O>>) -> HashMap<String, O> {
    h.into_iter()
        .filter_map(|(id, result)| {
            match result {
                Ok(o) => Some((id, o)),
                Err(_) => None,
            }
        })
        .collect()
}

pub fn map2error<S>(inner: S) -> MapToError<S> {
    MapToError::new(inner)
}

pub struct MapToError<S> {
    inner: S,
}

impl<S> MapToError<S> {
    pub fn new(inner: S) -> MapToError<S> {
        MapToError { inner: inner }
    }
}

impl<S> Future for MapToError<S>
    where S: Future,
          S::Error: Into<Error>
{
    type Item = S::Item;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.inner.poll() {
            Ok(v) => Ok(v),
            Err(e) => Err(e.into()),
        }
    }
}

impl<S> Stream for MapToError<S>
    where S: Stream,
          S::Error: Into<Error>
{
    type Item = S::Item;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.inner.poll() {
            Ok(v) => Ok(v),
            Err(e) => Err(e.into()),
        }
    }
}

impl<S> Sink for MapToError<S>
    where S: Sink,
          S::SinkError: Into<Error>
{
    type SinkItem = S::SinkItem;
    type SinkError = Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        match self.inner.start_send(item) {
            Ok(v) => Ok(v),
            Err(e) => Err(e.into()),
        }
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        match self.inner.poll_complete() {
            Ok(v) => Ok(v),
            Err(e) => Err(e.into()),
        }
    }
}
