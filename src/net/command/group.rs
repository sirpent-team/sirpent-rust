#[derive(Clone, Debug, PartialEq)]
struct Group<S>
    where S: Sink<SinkItem = Command, SinkError = Error> + Send + 'static
{
    clients: HashMap<Uuid, Client<S>>,
    // Channel to command communications with.
    cmd_tx: S
}

impl Group<S>
    where S: Sink<SinkItem = Command, SinkError = Error> + Send + 'static
{
    fn new(cmd_tx: S) -> Group<S> {
        Group {
            clients: HashMap::new(),
            cmd_tx: cmd_tx
        }
    }

    fn uuids(&self) -> Vec<Uuid> {
        self.clients.keys().collect()
    }

    fn command(&mut self, cmd: Command) -> Future<Item = (), Error = Error> {
        self.cmd_tx.send(cmd)
    }
}

impl Commander for Group {
    type Transmit = HashMap<Uuid, Msg>;
    type Receive = HashMap<Uuid, Msg>;
    type Status = HashMap<Uuid, Status>;

    fn transmit(&mut self, msgs: Self::Transmit) -> Future<Item = (), Error = Error> {
        let cmd = Command::TransmitToGroup(msgs);
        self.command(cmd)
    }

    fn receive(&mut self, timeout: Timeout)
        -> Future<Item = Self::Receive, Error = Error>
    {
        let (msg_forward_tx, msg_forward_rx) = oneshot::channel();
        let cmd = Command::ReceiveFromGroupInto(self.uuids(), msg_forward_tx, Timeout);
        self.command(cmd).and_then(|_| msg_forward_rx)
    }

    fn status(&mut self) -> Future<Item = Self::Status, Error = Error> {
        let (status_tx, status_rx) = oneshot::channel();
        let cmd = Command::StatusFromGroupInto(self.uuids(), status_tx);
        self.command(cmd).and_then(|_| status_rx)
    }

    fn close(&mut self) -> Future<Item = (), Error = Error> {
        self.command(Command::CloseGroup(self.uuids()))
    }
}
