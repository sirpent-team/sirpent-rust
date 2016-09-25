use std::net::{SocketAddr, TcpStream};
use std::io::Result;
use std::time;
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    #[serde(skip_serializing)]
    pub server_address: Option<SocketAddr>,
    pub snake_uuid: Uuid,
}

impl Player {
    pub fn new(name: String, server_address: SocketAddr, snake_uuid: Uuid) -> Player {
        Player{
            name: name,
            server_address: Some(server_address),
            snake_uuid: snake_uuid,
        }
    }
}

pub struct PlayerConnection {
    pub socket: TcpStream,
    pub timeout: Option<time::Duration>,
}

impl PlayerConnection {
    pub fn open(server_address: SocketAddr, timeout: Option<time::Duration>) -> Result<PlayerConnection> {
        Ok(PlayerConnection{
            socket: try!(Self::connect(server_address, timeout)),
            timeout: timeout
        })
    }

    fn connect(server_address: SocketAddr, timeout: Option<time::Duration>) -> Result<TcpStream> {
        let socket = try!(TcpStream::connect(server_address));
        try!(socket.set_read_timeout(timeout));
        try!(socket.set_write_timeout(timeout));
        Ok(socket)
    }
}
