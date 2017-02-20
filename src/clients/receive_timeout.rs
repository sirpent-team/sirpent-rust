use std::hash::Hash;
use std::fmt::Debug;
use std::time::Duration;
use std::collections::HashMap;
use futures::{Future, Stream, Sink, Poll, Async};
use tokio_timer::{Timer, Sleep};

use clients::*;

pub struct GroupReceiveTimeout<Id, CmdSink>
    where Id: Eq + Hash + Clone + Debug + Send,
          CmdSink: Sink<SinkItem = Cmd, SinkError = Error> + Send + 'static
{
    group_receive: GroupReceive<Id, CmdSink>,
    items: Option<Vec<Vec<(Id, Result<(Msg, CmdSink)>)>>>,
    sleep: Sleep,
}

impl<Id, CmdSink> GroupReceiveTimeout<Id, CmdSink>
    where Id: Eq + Hash + Clone + Debug + Send,
          CmdSink: Sink<SinkItem = Cmd, SinkError = Error> + Send + 'static
{
    pub fn new(clients: HashMap<Id, CmdSink>, timeout: Duration) -> Self {
        GroupReceiveTimeout {
            group_receive: GroupReceive::new(clients),
            items: Some(Vec::new()),
            sleep: Timer::default().sleep(timeout),
        }
    }
}

impl<Id, CmdSink> Future for GroupReceiveTimeout<Id, CmdSink>
    where Id: Eq + Hash + Clone + Debug + Send,
          CmdSink: Sink<SinkItem = Cmd, SinkError = Error> + Send + 'static
{
    type Item = Vec<Vec<(Id, Result<(Msg, CmdSink)>)>>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        assert!(self.items.is_some());

        // Poll the receiving until it is temporarily unavailable - or finished.
        loop {
            match self.group_receive.poll() {
                Ok(Async::Ready(Some(v))) => self.items.as_mut().unwrap().push(v),
                Ok(Async::Ready(None)) => return Ok(Async::Ready(self.items.take().unwrap())),
                Ok(Async::NotReady) => break,
                Err(e) => bail!(e),
            }
        }

        match self.sleep.poll() {
            Ok(Async::Ready(())) => Ok(Async::Ready(self.items.take().unwrap())),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => bail!(e),
        }
    }
}
