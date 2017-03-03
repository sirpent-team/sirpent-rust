struct AnonymousGroup<S>
    where S: Sink<SinkItem = Command, SinkError = Error> + Send + 'static
{
    cmd_tx: S
}

impl Commander for AnonymousGroup {
    type TransmitMsg = Msg;
    type ReceiveMsg = ();
    type StatusMsg = HashMap<Uuid, ClientStatus>;
    type ClosedMsg = HashMap<Uuid, bool>;

    fn send(&mut self, msg: Self::TransmitMsg) -> Future<Item = (), Error = Error>;

    fn receive(&mut self, optionality: Optional, timeout: Option<Duration>) -> Future<Item = ReceiveMsg, Error = Error>;

    fn status(&mut self) -> Future<Item = StatusMsg, Error = Error>;

    fn close(&mut self) -> Future<Item = Self::ClosedMsg, Error = Error>;
}
