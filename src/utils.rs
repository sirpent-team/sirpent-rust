use std::io;
use std::error;

pub fn io_error_from_str(desc: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, desc)
}

pub fn io_error_from_error<E: error::Error>(e: E) -> io::Error {
    io_error_from_str(&*format!("{:?}", e))
}

pub fn io_error_broken_pipe() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "Broken channel.")
}
