use std::io::{self, Read, Write};

use bincode;
use serde::{de::DeserializeOwned, Serialize};

/// Send message: length (u32 big-endian) + payload (bincode).
pub fn send_message<W, T>(mut w: W, msg: &T) -> io::Result<()>
where
    W: Write,
    T: Serialize,
{
    let data = bincode::serialize(msg).map_err(|e| io::Error::other(format!("bincode::serialize: {e}")))?;
    let len = data.len() as u32;
    let len_bytes = len.to_be_bytes();
    w.write_all(&len_bytes)?;
    w.write_all(&data)?;
    Ok(())
}

/// Receive message: read length (u32 big-endian), then payload, decode via bincode.
pub fn recv_message<R, T>(mut r: R) -> io::Result<T>
where
    R: Read,
    T: DeserializeOwned,
{
    let mut len_bytes = [0u8; 4];
    r.read_exact(&mut len_bytes)?;
    let len = u32::from_be_bytes(len_bytes) as usize;

    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;

    bincode::deserialize(&buf).map_err(|e| io::Error::other(format!("bincode::deserialize: {e}")))
}
