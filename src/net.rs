use std::net::{ToSocketAddrs, SocketAddr, TcpStream, TcpListener};
use std::time::Duration;
use std::marker::Send;
use std::io::{self, Write, BufReader, BufWriter, BufRead, Lines};
use serde_json;
use std::fmt;

use protocol::*;

static LF: &'static [u8] = b"\n";

// @TODO: Add Drop to ProtocolConnection that sends QUIT? Potential for deadlock waiting if so?
pub struct ProtocolConnection {
    pub timeouts: Timeouts,
    stream: TcpStream,
    reader: Lines<BufReader<TcpStream>>,
    writer: BufWriter<TcpStream>,
}

impl ProtocolConnection {
    pub fn new(stream: TcpStream, timeouts: Option<Timeouts>) -> io::Result<ProtocolConnection> {
        Ok(ProtocolConnection {
            timeouts: timeouts.unwrap_or(Default::default()),
            stream: stream.try_clone()?,
            reader: BufReader::new(stream.try_clone()?).lines(),
            writer: BufWriter::new(stream),
        })
    }

    pub fn recieve<T: TypedMessage>(&mut self) -> ProtocolResult<T> {
        match self.recieve_plain() {
            Ok(plain_msg) => plain_msg.to_typed(),
            Err(e) => Err(e),
        }
    }

    pub fn recieve_plain(&mut self) -> ProtocolResult<PlainMessage> {
        self.stream.set_read_timeout(self.timeouts.read)?;

        let line = self.reader.next().ok_or(ProtocolError::NothingReadFromStream)??;
        println!("{:?}", line);
        match serde_json::from_str(&line) {
            Ok(v) => Ok(v),
            Err(e) => Err(From::from(e)),
        }
    }

    pub fn send<T: TypedMessage>(&mut self, message: T) -> ProtocolResult<()> {
        self.send_plain(&PlainMessage::from_typed(message))
    }

    pub fn send_plain(&mut self, plain_msg: &PlainMessage) -> ProtocolResult<()> {
        self.stream.set_write_timeout(self.timeouts.write)?;

        self.writer.write_all(serde_json::to_string(&plain_msg)?.as_bytes())?;
        self.writer.write_all(LF)?;
        self.writer.flush()?;
        Ok(())
    }
}

impl fmt::Debug for ProtocolConnection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ProtocolConnection {{ ??? }}")
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
