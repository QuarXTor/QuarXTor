use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};

use crate::protocol::Transport;
use quarxtor_core::net_core::NetResult;

/// Simple blocking TCP transport wrapper for QuarXNet.
pub struct TcpTransport {
    stream: TcpStream,
}

impl TcpTransport {
    /// Connect to a remote address.
    pub fn connect<A: ToSocketAddrs>(addr: A) -> NetResult<Self> {
        let stream = TcpStream::connect(addr)?;
        // Best-effort optimization.
        let _ = stream.set_nodelay(true);
        Ok(TcpTransport { stream })
    }

    /// Wrap an existing TcpStream (e.g., accepted by a server).
    pub fn from_stream(stream: TcpStream) -> Self {
        let _ = stream.set_nodelay(true);
        TcpTransport { stream }
    }

    /// Take inner TcpStream.
    pub fn into_inner(self) -> TcpStream {
        self.stream
    }
}

impl Transport for TcpTransport {
    fn send(&mut self, data: &[u8]) -> NetResult<()> {
        self.stream.write_all(data)?;
        self.stream.flush()?;
        Ok(())
    }

    /// Trait implementation: read exactly `len` bytes and return Vec<u8>.
    fn recv_exact(&mut self, len: usize) -> NetResult<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.stream.read_exact(&mut buf)?;
        Ok(buf)
    }
}

pub mod simple;
