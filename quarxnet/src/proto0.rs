use std::io::{Read, Write};

/// Encode proto0 header with given opcode.
pub fn encode_header(opc: u8) -> [u8; 8] {
    let mut buf = [0u8; 8];
    buf[0..4].copy_from_slice(b"QXRT");
    buf[4] = 0;
    buf[5] = opc;
    buf[6..8].copy_from_slice(&0u16.to_le_bytes());
    buf
}

/// Decode proto0 header; return (ver, opc, len) or error string.
pub fn decode_header(buf: [u8; 8]) -> Result<(u8, u8, u16), String> {
    if &buf[0..4] != b"QXRT" {
        return Err("bad magic".into());
    }
    let ver = buf[4];
    let opc = buf[5];
    let len = u16::from_le_bytes([buf[6], buf[7]]);
    if ver != 0 {
        return Err("bad version".into());
    }
    Ok((ver, opc, len))
}

/// Serve a single proto0 exchange: PING -> PONG.
pub fn serve_one_proto0<S: Read + Write>(stream: &mut S) -> Result<(), String> {
    let mut hdr = [0u8; 8];
    stream.read_exact(&mut hdr).map_err(|e| e.to_string())?;
    let (_ver, opc, len) = decode_header(hdr)?;
    if opc == 1 && len == 0 {
        let pong = encode_header(2);
        stream
            .write_all(&pong)
            .and_then(|_| stream.flush())
            .map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("unexpected opcode/len".into())
    }
}
