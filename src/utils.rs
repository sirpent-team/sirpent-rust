use std::io;
use std::fmt;
use std::error;
use serde_json;
use std::result;
use std::ops::Deref;
use std::convert::Into;
use std::time::Duration;
use std::collections::HashMap;
use serde::{Serialize, Serializer, Deserialize, Deserializer};
use serde::de::{self, Visitor};
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

pub fn roman_numerals(mut value: u64) -> String {
    let mut numerals = "".to_string();
    while value > 0 {
        let (numeral, sub) = match value {
            v if v >= 1000 => ("M", 1000),
            v if v >= 500 => ("D", 500),
            v if v >= 100 => ("C", 100),
            v if v >= 50 => ("L", 50),
            v if v >= 10 => ("X", 10),
            v if v >= 5 => ("V", 5),
            v if v >= 1 => ("I", 1),
            _ => break,
        };
        numerals.push_str(numeral);
        value -= sub;
    }
    numerals
}

// Constants from https://github.com/rust-lang-deprecated/time/blob/master/src/duration.rs
/// The number of nanoseconds in a millisecond.
const NANOS_PER_MILLI: u64 = 1000_000;
/// The number of milliseconds per second.
const MILLIS_PER_SEC: u64 = 1000;

/// Represents a duration in milliseconds.
#[derive(PartialEq, Clone, Copy, Debug)]
pub struct Milliseconds {
    inner: Duration,
}

impl Milliseconds {
    pub fn new(milliseconds: u64) -> Milliseconds {
        Milliseconds { inner: Duration::from_millis(milliseconds) }
    }

    pub fn millis(&self) -> u64 {
        let seconds_part_as_millis: u64 = self.inner.as_secs() * MILLIS_PER_SEC;
        let nanos_part_as_millis: u64 = (self.inner.subsec_nanos() as u64) / NANOS_PER_MILLI;
        seconds_part_as_millis + nanos_part_as_millis
    }
}

impl Deref for Milliseconds {
    type Target = Duration;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Into<Duration> for Milliseconds {
    fn into(self) -> Duration {
        self.inner
    }
}

impl Serialize for Milliseconds {
    fn serialize<S>(&self, serializer: S) -> result::Result<S::Ok, S::Error>
        where S: Serializer
    {
        serializer.serialize_u64(self.millis())
    }
}

impl Deserialize for Milliseconds {
    fn deserialize<D>(deserializer: D) -> result::Result<Self, D::Error>
        where D: Deserializer
    {
        struct MillisecondsVisitor;

        impl Visitor for MillisecondsVisitor {
            type Value = Milliseconds;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Milliseconds as u64")
            }

            fn visit_u64<E>(self, value: u64) -> result::Result<Milliseconds, E>
                where E: de::Error
            {
                Ok(Milliseconds::new(value.into()))
            }
        }

        deserializer.deserialize_u64(MillisecondsVisitor)
    }
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
