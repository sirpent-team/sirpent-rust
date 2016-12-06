use std::net::{ToSocketAddrs, SocketAddr, TcpStream, TcpListener};
use std::time::Duration;
use std::marker::Send;
use std::io::{self, Write, BufReader, BufWriter, BufRead, Lines, Error, ErrorKind};
use std::result::Result as StdResult;
use std::error::Error as StdError;
use std::collections::{HashMap, BTreeMap};
use serde_json;
use rayon::prelude::*;

use player::*;
use protocol::*;

static LF: &'static [u8] = b"\n";

pub struct PlayerConnections {
    connections: HashMap<PlayerName, PlayerConnection>,
}

impl PlayerConnections {
    pub fn new() -> PlayerConnections {
        PlayerConnections { connections: HashMap::new() }
    }

    pub fn add_player(&mut self, player_name: PlayerName, player_connection: PlayerConnection) {
        self.connections.insert(player_name, player_connection);
    }

    pub fn remove_player(&mut self, player_name: PlayerName) -> Option<PlayerConnection> {
        self.connections.remove(&player_name)
    }

    pub fn broadcast(&mut self,
                     command: Command)
                     -> HashMap<PlayerName, StdResult<(), ProtocolError>> {
        let mut result_pairs = Vec::with_capacity(self.connections.len());
        self.connections
            .par_iter_mut()
            .map(|(player_name, connection)| (player_name.clone(), connection.write(&command)))
            .collect_into(&mut result_pairs);
        result_pairs.into_iter().collect()
    }

    pub fn send(&mut self,
                player_name: PlayerName,
                command: Command)
                -> StdResult<(), ProtocolError> {
        self.connections
            .get_mut(&player_name)
            .ok_or(ProtocolError::SendToUnknownPlayer)?
            .write(&command)
    }

    pub fn collect(&mut self) -> HashMap<PlayerName, StdResult<Command, ProtocolError>> {
        let mut result_pairs = Vec::with_capacity(self.connections.len());
        self.connections
            .par_iter_mut()
            .map(|(player_name, connection)| (player_name.clone(), connection.read()))
            .collect_into(&mut result_pairs);
        result_pairs.into_iter().collect()
    }

    pub fn recieve(&mut self, player_name: PlayerName) -> StdResult<Command, ProtocolError> {
        self.connections
            .get_mut(&player_name)
            .ok_or(ProtocolError::RecieveFromUnknownPlayer)?
            .read()
    }
}

// @TODO: Add Drop to PlayerConnection that sends QUIT? Potential for deadlock waiting if so?
pub struct PlayerConnection {
    timeouts: Timeouts,
    stream: TcpStream,
    reader: Lines<BufReader<TcpStream>>,
    writer: BufWriter<TcpStream>,
}

impl PlayerConnection {
    pub fn new(stream: TcpStream, timeouts: Option<Timeouts>) -> io::Result<PlayerConnection> {
        Ok(PlayerConnection {
            timeouts: timeouts.unwrap_or(Default::default()),
            stream: stream.try_clone()?,
            reader: BufReader::new(stream.try_clone()?).lines(),
            writer: BufWriter::new(stream),
        })
    }

    pub fn read(&mut self) -> StdResult<Command, ProtocolError> {
        self.stream.set_read_timeout(self.timeouts.read)?;

        let line = self.reader.next().ok_or(ProtocolError::NothingReadFromStream)??;
        println!("{:?}", line);
        let mut command_value: serde_json::Value = serde_to_io(serde_json::from_str(&line))?;

        let obj = command_value.as_object_mut()
            .ok_or(ProtocolError::MessageReadNotADictionary)?;
        let msg = obj.remove("msg")
            .ok_or(ProtocolError::MessageReadMissingMsgField)?
            .as_str()
            .ok_or(ProtocolError::MessageReadNonStringMsgField)?
            .to_string();
        let data = obj.remove("data")
            .ok_or(ProtocolError::MessageReadMissingDataField)?;

        let mut command_map: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        command_map.insert(msg, data);
        let command_map = serde_json::Value::Object(command_map);

        let command: Command = serde_to_io(serde_json::from_value(command_map))?;
        Ok(command)
    }

    pub fn write(&mut self, command: &Command) -> Result<(), ProtocolError> {
        self.stream.set_write_timeout(self.timeouts.write)?;

        let command_value = serde_json::to_value(command);

        let mut data = BTreeMap::new();

        let msg = match command_value {
            serde_json::Value::Object(command_map) => {
                let (msg_, data_) = command_map.iter()
                    .next()
                    .ok_or(ProtocolError::CommandWasEmpty)?;
                data.append(data_.clone()
                    .as_object_mut()
                    .ok_or(ProtocolError::CommandDataWasNotObject)?);
                msg_.clone()
            }
            serde_json::Value::String(command_msg) => command_msg,
            _ => return Err(ProtocolError::CommandSerialiseNotObjectNotString),
        };

        // Using a Map here was putting data before msg in output JSON. For developers it is easier to
        // keep things the sane way around even though for clients it probably won't be a big deal unless
        // our payloads got big. Thus an order-preserving struct is used.
        let message = Message {
            msg: msg,
            data: serde_json::Value::Object(data),
        };

        // serde_json:: to_writer seems to never return when using a BufWriter<TcpStream>.
        self.writer.write_all(serde_to_io(serde_json::to_string(&message))?.as_bytes())?;
        self.writer.write_all(LF)?;
        self.writer.flush()?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Message {
    msg: String,
    data: serde_json::Value,
}

/// Converts a Result<T, serde_json::Error> into an Result<T>.
fn serde_to_io<T>(res: StdResult<T, serde_json::Error>) -> io::Result<T> {
    match res {
        Ok(x) => Ok(x),
        Err(e) => {
            Err(Error::new(ErrorKind::Other,
                           &format!("A serde_json error occurred. ({})", e.description())[..]))
        }
    }
}

/// A settings struct containing a set of timeouts which can be applied to a server.
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Timeouts {
    /// Controls the timeout for reads on existing connections.
    ///
    /// The default is `Some(Duration::from_secs(30))`
    pub read: Option<Duration>,

    /// Controls the timeout for writes on existing conncetions.
    ///
    /// The default is `Some(Duration::from_secs(1))`
    pub write: Option<Duration>,
}

impl Default for Timeouts {
    fn default() -> Self {
        Timeouts {
            read: Some(Duration::from_secs(5)),
            write: Some(Duration::from_secs(1)),
        }
    }
}

pub struct SirpentServer {
    /// Iron contains a `Handler`, which it uses to create responses for client
    /// requests.
    // pub handler: H,
    /// Once listening, the local address that this server is bound to.
    pub addr: Option<SocketAddr>,
}

impl SirpentServer {
    /// Kick off the server process using the HTTP protocol.
    ///
    /// Call this once to begin listening for requests on the server.
    /// This consumes the Iron instance, but does the listening on
    /// another task, so is not blocking.
    ///
    /// The thread returns a guard that will automatically join with the parent
    /// once it is dropped, blocking until this happens.
    ///
    /// Defaults to a threadpool of size `8 * num_cpus`.
    ///
    /// ## Panics
    ///
    /// Panics if the provided address does not parse. To avoid this
    /// call `to_socket_addrs` yourself and pass a parsed `SocketAddr`.
    pub fn plain<A: ToSocketAddrs>(addr: A) -> io::Result<SirpentServer> {
        let sock_addr = addr.to_socket_addrs()
            .ok()
            .and_then(|mut addrs| addrs.next())
            .expect("Could not parse socket address.");

        Ok(SirpentServer { addr: Some(sock_addr) })
    }

    /// Kick off the server process with X threads.
    ///
    /// ## Panics
    ///
    /// Panics if the provided address does not parse. To avoid this
    /// call `to_socket_addrs` yourself and pass a parsed `SocketAddr`.
    pub fn listen<F>(&self, mut f: F)
        where F: FnMut(TcpStream) + Send
    {
        let listener = TcpListener::bind(self.addr.unwrap()).unwrap();
        for stream in listener.incoming() {
            match stream {
                Ok(s) => f(s),
                _ => {}
            }
        }
    }
}
