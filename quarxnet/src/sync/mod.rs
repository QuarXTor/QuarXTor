use crate::protocol::{
    decode_hello, decode_push_blocks, decode_push_object, encode_get_blocks, encode_get_object,
    encode_hello, encode_push_blocks, encode_push_object, recv_frame, send_frame, Transport,
};
use quarxproto::ZNode;
use quarxtor_core::net_core::*;
use quarxtor_core::types::{BlockId, ObjectId};

/// Send Hello and receive Hello back (handshake).
pub fn client_hello<T: Transport>(
    transport: &mut T,
    node: NodeId,
    version: ProtocolVersion,
) -> NetResult<HelloPayload> {
    let payload = HelloPayload { node, version };
    let bytes = encode_hello(&payload);

    let frame = Frame {
        header: FrameHeader {
            kind: FrameKind::Hello,
            flags: 0,
            length: bytes.len() as u32,
        },
        payload: bytes,
    };

    // Send Hello
    send_frame(transport, &frame)?;

    // Wait for response
    let resp = recv_frame(transport)?;
    if !matches!(resp.header.kind, FrameKind::Hello) {
        return Err(NetError::InvalidFrame);
    }

    decode_hello(&resp.payload)
}

/// Request a set of blocks by their BlockId.
///
/// Returns raw stream of encoded blocks (see PushBlocksPayload::raw).
pub fn client_get_blocks<T: Transport>(transport: &mut T, ids: &[BlockId]) -> NetResult<Vec<u8>> {
    let payload = GetBlocksPayload { ids: ids.to_vec() };
    let bytes = encode_get_blocks(&payload);

    let frame = Frame {
        header: FrameHeader {
            kind: FrameKind::GetBlocks,
            flags: 0,
            length: bytes.len() as u32,
        },
        payload: bytes,
    };

    send_frame(transport, &frame)?;

    let resp = recv_frame(transport)?;
    if !matches!(resp.header.kind, FrameKind::PushBlocks) {
        return Err(NetError::InvalidFrame);
    }

    let pb = decode_push_blocks(&resp.payload)?;
    Ok(pb.raw)
}

/// Send raw stream of encoded blocks (L0/Multi/Z).
pub fn client_push_blocks<T: Transport>(transport: &mut T, raw: &[u8]) -> NetResult<()> {
    let payload = PushBlocksPayload { raw: raw.to_vec() };
    let bytes = encode_push_blocks(&payload);

    let frame = Frame {
        header: FrameHeader {
            kind: FrameKind::PushBlocks,
            flags: 0,
            length: bytes.len() as u32,
        },
        payload: bytes,
    };

    send_frame(transport, &frame)
}

/// Request object by ObjectId.
///
/// Returns raw blob (see PushObjectPayload::raw).
pub fn client_get_object<T: Transport>(transport: &mut T, id: ObjectId) -> NetResult<Vec<u8>> {
    let payload = GetObjectPayload { id };
    let bytes = encode_get_object(&payload);

    let frame = Frame {
        header: FrameHeader {
            kind: FrameKind::GetObject,
            flags: 0,
            length: bytes.len() as u32,
        },
        payload: bytes,
    };

    send_frame(transport, &frame)?;

    let resp = recv_frame(transport)?;
    if !matches!(resp.header.kind, FrameKind::PushObject) {
        return Err(NetError::InvalidFrame);
    }

    let po = decode_push_object(&resp.payload)?;
    Ok(po.raw)
}

/// Send object (blob + tree) to remote node.
pub fn client_push_object<T: Transport>(transport: &mut T, raw: &[u8]) -> NetResult<()> {
    let payload = PushObjectPayload { raw: raw.to_vec() };
    let bytes = encode_push_object(&payload);

    let frame = Frame {
        header: FrameHeader {
            kind: FrameKind::PushObject,
            flags: 0,
            length: bytes.len() as u32,
        },
        payload: bytes,
    };

    send_frame(transport, &frame)
}

/// Send ZNode over the network as an object (PushObject).
pub fn client_push_znode<T: Transport>(transport: &mut T, znode: &ZNode) -> NetResult<()> {
    let raw = znode.serialize();
    client_push_object(transport, &raw)
}

/// Request ZNode by ObjectId (via GetObject/PushObject).
pub fn client_get_znode<T: Transport>(transport: &mut T, id: ObjectId) -> NetResult<ZNode> {
    let raw = client_get_object(transport, id)?;
    ZNode::deserialize(&raw).map_err(|_| NetError::InvalidFrame)
}
