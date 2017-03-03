#[derive(Clone, Debug, PartialEq)]
struct Client<S>
    where S: Sink<SinkItem = Command, SinkError = Error> + Send + 'static
{
    // Clients are identified by UUID.
    uuid: Uuid,
    // Clients can also have an ID specific to their communication form, e.g., SocketAddr.
    kind_id: Option<Box<Clone>>,
    // Channel to command communications with.
    cmd_tx: S
}

impl Client<S>
    where S: Sink<SinkItem = Command, SinkError = Error> + Send + 'static
{
    fn new(uuid: Uuid, kind_id: Option<Box<Clone>>, cmd_tx: S) -> Client<S> {
        Client {
            uuid: uuid,
            kind_id: kind_id,
            cmd_tx: cmd_tx
        }
    }

    fn command(&mut self, cmd: Command) -> Future<Item = (), Error = Error> {
        self.cmd_tx.send(cmd)
    }
}

impl Commander for Client {
    type Transmit = Msg;
    type Receive = Msg;
    type Status = Status;

    fn transmit(&mut self, msg: Self::Transmit) -> Future<Item = (), Error = Error> {
        self.command(Command::Transmit(self.uuid, msg))
    }

    fn receive(&mut self, timeout: Timeout)
        -> Future<Item = Self::Receive, Error = Error>
    {
        let (msg_forward_tx, msg_forward_rx) = oneshot::channel();
        let cmd = Command::ReceiveInto((self.uuid, msg_forward_tx), Timeout);
        self.command(cmd).and_then(|_| msg_forward_rx)
    }

    fn status(&mut self) -> Future<Item = Self::Status, Error = Error> {
        let (status_tx, status_rx) = oneshot::channel();
        let cmd = Command::StatusInto((self.uuid, status_tx))
        self.command(cmd).and_then(|_| status_rx)
    }

    fn close(&mut self) -> Future<Item = (), Error = Error> {
        self.command(Command::Close(self.uuid))
    }
}
