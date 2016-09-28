//! Exposes the `Iron` type, the main entrance point of the
//! `Iron` library.

use std::net::{ToSocketAddrs, SocketAddr, TcpStream, TcpListener};
use std::time::Duration;
use std::path::PathBuf;
use openssl::ssl::{SslContext, SslMethod, SslStream, MaybeSslStream};
use openssl::ssl::error::SslError;
use openssl::x509::X509FileType;
use std::result::Result as StdResult;
use std::io::{Result, Error, ErrorKind, BufReader, BufWriter};
use std::error::Error as StdError;

#[derive(Clone)]
pub enum Transport {
    Normal,
    Ssl {
        /// Path to SSL certificate file
        certificate: PathBuf,
        /// Path to SSL private key file
        key: PathBuf,
    },
}

impl Transport {
    fn encapsulate(&self, stream: TcpStream) -> Result<MaybeSslStream<TcpStream>> {
        match *self {
            Transport::Normal => Ok(MaybeSslStream::Normal(stream)),
            Transport::Ssl { ref certificate, ref key } => {
                let mut context = SslContext::new(SslMethod::Sslv23).unwrap();
                ssl_to_io(context.set_cipher_list("ALL!EXPORT!EXPORT40!EXPORT56!aNULL!LOW!RC4@STRENGTH"))?;
                ssl_to_io(context.set_certificate_file(certificate, X509FileType::PEM))?;
                ssl_to_io(context.set_private_key_file(key, X509FileType::PEM))?;
                Ok(MaybeSslStream::Ssl(ssl_to_io(SslStream::accept(&context, stream))?))
            }
        }
    }
}

/// Converts a Result<T, SslError> into an Result<T>.
pub fn ssl_to_io<T>(res: StdResult<T, SslError>) -> Result<T> {
    match res {
        Ok(x) => Ok(x),
        Err(e) => {
            Err(Error::new(ErrorKind::Other,
                           &format!("An SSL error occurred. ({})", e.description())[..]))
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

    /// Once listening, the protocol used to serve content.
    pub transport: Transport,
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

        Ok(SirpentServer {
            addr: Some(sock_addr),
            transport: Transport::Normal,
        })
    }

    /// Kick off the server process using the HTTPS protocol.
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
    pub fn tls<A: ToSocketAddrs>(certificate: PathBuf,
                                 key: PathBuf,
                                 addr: A)
                                 -> Result<SirpentServer> {
        let sock_addr = addr.to_socket_addrs()
            .ok()
            .and_then(|mut addrs| addrs.next())
            .expect("Could not parse socket address.");

        Ok(SirpentServer {
            addr: Some(sock_addr),
            transport: Transport::Ssl {
                certificate: certificate,
                key: key,
            },
        })
    }

    /// Kick off the server process with X threads.
    ///
    /// ## Panics
    ///
    /// Panics if the provided address does not parse. To avoid this
    /// call `to_socket_addrs` yourself and pass a parsed `SocketAddr`.
    pub fn listen<F>(&self, mut f: F, timeouts: Option<Timeouts>)
        where F: FnMut(MaybeSslStream<TcpStream>,
                       BufReader<MaybeSslStream<TcpStream>>,
                       BufWriter<MaybeSslStream<TcpStream>>)
    {
        let transport = self.transport.clone();
        let listener = TcpListener::bind(self.addr.unwrap()).unwrap();
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let encapsulated_stream = transport.encapsulate(stream).unwrap();
                    let reader = BufReader::new(encapsulated_stream.try_clone().unwrap());
                    let writer = BufWriter::new(encapsulated_stream.try_clone().unwrap());

                    f(encapsulated_stream, reader, writer);
                }
                Err(_) => {}
            }
        }
    }
}
