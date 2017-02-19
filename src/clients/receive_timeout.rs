use std::io;
use std::time::Duration;
use std::collections::HashMap;
use std::hash::Hash;

use futures::{Future, Stream, Sink, Poll, Async};
use tokio_timer::{Timer, Sleep};

use net::*;
use clients::*;

pub struct GroupReceiveTimeout<Id, CmdSink>
    where Id: Eq + Hash + Clone + Send,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static
{
    group_receive: Option<GroupReceive<Id, CmdSink>>,
    items: Option<Vec<Vec<(Id, Result<(Msg, CmdSink), CmdSink::SinkError>)>>>,
    sleep: Sleep,
}

impl<Id, CmdSink> GroupReceiveTimeout<Id, CmdSink>
    where Id: Eq + Hash + Clone + Send,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static
{
    pub fn new(clients: HashMap<Id, CmdSink>, timeout: Duration) -> Self {
        GroupReceiveTimeout {
            group_receive: Some(GroupReceive::new(clients)),
            items: Some(Vec::new()),
            sleep: Timer::default().sleep(timeout),
        }
    }
}

impl<Id, CmdSink> Future for GroupReceiveTimeout<Id, CmdSink>
    where Id: Eq + Hash + Clone + Send,
          CmdSink: Sink<SinkItem = Cmd> + Send + 'static
{
    type Item = Vec<Vec<(Id, Result<(Msg, CmdSink), CmdSink::SinkError>)>>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if let Some(mut group_receive) = self.group_receive.take() {
            match group_receive.poll() {
                Ok(Async::Ready(Some(v))) => {
                    self.items.as_mut().unwrap().push(v);
                    self.group_receive = Some(group_receive);
                }
                Ok(Async::Ready(None)) => {}
                Ok(Async::NotReady) => {
                    self.group_receive = Some(group_receive);
                }
                Err(e) => return Err(other(e)),
            }
        }

        match self.sleep.poll() {
            // If the timeout has yet to be reached then poll receive.
            Ok(Async::NotReady) => Ok(Async::NotReady),
            // If the timeout has been reached then return what entries we have.
            Ok(Async::Ready(_)) => Ok(Async::Ready(self.items.take().unwrap())),
            // If the timeout errored then return it as an `io::Error`.
            // @TODO: Also return what entries we have?
            Err(e) => Err(other(e)),
        }
    }
}
