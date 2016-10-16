use std::net::{ToSocketAddrs, SocketAddr, TcpStream, TcpListener};
use std::time::Duration;
use std::marker::Send;
use std::io::{Result, Read, Write, BufReader, BufWriter, Bytes, Error, ErrorKind};
use std::result::Result as StdResult;
use std::error::Error as StdError;
use std::collections::BTreeMap;
use serde_json;

use protocol::*;

// @TODO: Add Drop to PlayerConnection that sends QUIT? Potential for deadlock waiting if so?
pub struct PlayerConnection {
    stream: TcpStream,
    reader: serde_json::StreamDeserializer<serde_json::Value, Bytes<BufReader<TcpStream>>>,
    writer: BufWriter<TcpStream>,
}

impl PlayerConnection {
    pub fn new(stream: TcpStream) -> Result<PlayerConnection> {
        Ok(PlayerConnection {
            stream: stream.try_clone()?,
            reader: serde_json::StreamDeserializer::new(BufReader::new(stream.try_clone()?)
                .bytes()),
            writer: BufWriter::new(stream),
        })
    }

    pub fn read(&mut self) -> Result<Command> {
        let next_recieved_value =
            self.reader.next().ok_or(Error::new(ErrorKind::Other, "Nothing read."))?;
        let mut command_value = serde_to_io(next_recieved_value)?;

        //println!("~~~ {}",
        //         serde_to_io(serde_json::to_string(&command_value))?);

        let obj = command_value.as_object_mut()
            .ok_or(Error::new(ErrorKind::Other, "The msg was not a dictionary."))?;
        let msg = obj.remove("msg")
            .ok_or(Error::new(ErrorKind::Other, "No msg field provided."))?
            .as_str()
            .ok_or(Error::new(ErrorKind::Other, "The msg field was not a string."))?
            .to_string();
        let data = obj.remove("data")
            .ok_or(Error::new(ErrorKind::Other, "No data field provided."))?;

        let mut command_map: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        command_map.insert(msg, data);
        let command_map = serde_json::Value::Object(command_map);

        //println!("~~~ {}", serde_to_io(serde_json::to_string(&command_map))?);

        let command: Command = serde_to_io(serde_json::from_value(command_map))?;
        Ok(command)
    }

    pub fn write(&mut self, command: &Command) -> Result<()> {
        let command_value = serde_json::to_value(command);
        // let command_map = command_value.as_object_mut()
        //     .unwrap_or({
        //         let mut map = BTreeMap::new();
        //         map.insert(command_value.as_str()
        //                        .ok_or(Error::new(ErrorKind::Other,
        //                                          "Serialised Command was not an object and not \
        //                                           a string."))?
        //                        .to_string(),
        //                    serde_json::Value::Object(BTreeMap::new()));
        //         &mut map
        //     });

        let mut data = BTreeMap::new();

        let msg = match command_value {
            serde_json::Value::Object(command_map) => {
                let (msg_, data_) = command_map.iter()
                    .next()
                    .ok_or(Error::new(ErrorKind::Other, "The outer Command object was empty."))?;
                data.append(data_.clone()
                    .as_object_mut()
                    .ok_or(Error::new(ErrorKind::Other, "Command data was not an object."))?);
                msg_.clone()
            }
            serde_json::Value::String(command_msg) => command_msg,
            _ => {
                return Err(Error::new(ErrorKind::Other,
                                      "Serialised Command was not an object or string."))
            }
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
fn serde_to_io<T>(res: StdResult<T, serde_json::Error>) -> Result<T> {
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
    pub fn plain<A: ToSocketAddrs>(addr: A) -> Result<SirpentServer> {
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
    pub fn listen<F>(&self, mut f: F, timeouts: Option<Timeouts>)
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
