use std::net;
use std::io;
use std::time;
use uuid::Uuid;

pub struct Player {
    pub name: String,
    pub alive: bool,
    pub snake_uuid: Uuid,
    server_address: net::SocketAddr,
    socket: Option<net::TcpStream>,
}

impl Player {
    pub fn new(name: String, server_address: net::SocketAddr) -> Player {
        Player{
            name: name,
            alive: true,
            snake_uuid: Uuid::new_v4(),
            server_address: server_address,
            socket: None,
        }
    }

    pub fn connect(&mut self, timeout: Option<time::Duration>) -> io::Result<()> {
        let socket = try!(net::TcpStream::connect(self.server_address));
        try!(socket.set_read_timeout(timeout));
        try!(socket.set_write_timeout(timeout));
        self.socket = Some(socket);
        Ok(())
    }

    pub fn close(&mut self) -> io::Result<()> {
        // @TODO: if socket open, send a closing message
        self.socket = None;
        Ok(())
    }
}
