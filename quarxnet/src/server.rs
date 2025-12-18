use std::env;
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::protocol::{
    decode_get_blocks, decode_get_object, decode_hello, decode_push_blocks, decode_push_object,
    encode_hello, encode_push_blocks, encode_push_object, recv_frame, send_frame,
};
use crate::transport::TcpTransport;
use crate::backend::{FileBackend, NetBackend, Repo, RepoNetBackend};
use quarxtor_core::net_core::*;

fn ts() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let ms = now.subsec_millis();
    let day = secs % 86_400;
    let h = day / 3600;
    let m = (day / 60) % 60;
    let s = day % 60;
    format!("[{:02}:{:02}:{:02}.{:03}]", h, m, s, ms)
}

/// Default TCP port for quarxnetd (dynamic range, does not collide with known services).
pub const DEFAULT_PORT: u16 = 54721;

/// Per-session state: last raw block stream and object.
struct ConnCtx {
    last_blocks: Option<Vec<u8>>,
    last_object: Option<Vec<u8>>,
}

impl ConnCtx {
    fn new() -> Self {
        ConnCtx {
            last_blocks: None,
            last_object: None,
        }
    }
}

/// Handle client Hello and respond with our Hello.
fn handle_hello<T: crate::protocol::Transport>(
    transport: &mut T,
    node_id: NodeId,
    version: ProtocolVersion,
) -> NetResult<()> {
    let frame = recv_frame(transport)?;
    match frame.header.kind {
        FrameKind::Hello => {
            let _hello = decode_hello(&frame.payload)?;

            let resp_payload = HelloPayload {
                node: node_id,
                version,
            };
            let bytes = encode_hello(&resp_payload);

            let resp = Frame {
                header: FrameHeader {
                    kind: FrameKind::Hello,
                    flags: 0,
                    length: bytes.len() as u32,
                },
                payload: bytes,
            };

            send_frame(transport, &resp)?;
            Ok(())
        }
        _ => Err(NetError::InvalidFrame),
    }
}

/// Handle a frame inside the main loop.
fn handle_frame<T: crate::protocol::Transport>(
    transport: &mut T,
    ctx: &mut ConnCtx,
    store: &RepoNetBackend<FileBackend>,
    frame: Frame,
) -> NetResult<()> {
    match frame.header.kind {
        FrameKind::GetBlocks => {
            // Decode request, ignore ids for now — echo/last semantics.
            let _gb = decode_get_blocks(&frame.payload)?;

            let raw = if let Some(buf) = ctx.last_blocks.clone() {
                buf
            } else {
                store.get_blocks_raw().unwrap_or_default()
            };

            let payload = PushBlocksPayload { raw };
            let bytes = encode_push_blocks(&payload);

            let resp = Frame {
                header: FrameHeader {
                    kind: FrameKind::PushBlocks,
                    flags: 0,
                    length: bytes.len() as u32,
                },
                payload: bytes,
            };

            send_frame(transport, &resp)?;
            Ok(())
        }
        FrameKind::PushBlocks => {
            let pb = decode_push_blocks(&frame.payload)?;
            let len = pb.raw.len();

            // Persist raw blocks
            store
                .put_blocks_raw(&pb.raw)
                .map_err(|_| NetError::InvalidFrame)?;
            ctx.last_blocks = Some(pb.raw);

            eprintln!(
                "{} [quarxnetd] PushBlocks received, {} bytes stored (session + store)",
                ts(),
                len
            );
            Ok(())
        }
        FrameKind::GetObject => {
            let _go = decode_get_object(&frame.payload)?;

            let raw = if let Some(buf) = ctx.last_object.clone() {
                buf
            } else {
                store.get_object_raw().unwrap_or_default()
            };

            let payload = PushObjectPayload { raw };
            let bytes = encode_push_object(&payload);

            let resp = Frame {
                header: FrameHeader {
                    kind: FrameKind::PushObject,
                    flags: 0,
                    length: bytes.len() as u32,
                },
                payload: bytes,
            };

            send_frame(transport, &resp)?;
            Ok(())
        }
        FrameKind::PushObject => {
            let po = decode_push_object(&frame.payload)?;
            let len = po.raw.len();

            // Persist raw object
            store
                .put_object_raw(&po.raw)
                .map_err(|_| NetError::InvalidFrame)?;
            ctx.last_object = Some(po.raw);

            eprintln!(
                "{} [quarxnetd] PushObject received, {} bytes stored (session + store)",
                ts(),
                len
            );
            Ok(())
        }
        // Hello in main loop is not supported yet (expected only as the first frame).
        _ => {
            eprintln!(
                "{} [quarxnetd] unexpected frame kind in main loop: {:?}",
                ts(),
                frame.header.kind
            );
            Err(NetError::InvalidFrame)
        }
    }
}

/// Handle one connection: Hello + loop over frames until EOF/error.
fn handle_client(
    stream: TcpStream,
    node_id: NodeId,
    version: ProtocolVersion,
    store: RepoNetBackend<FileBackend>,
) -> NetResult<()> {
    let peer = stream.peer_addr().ok();
    if let Some(addr) = peer {
        eprintln!("{} [quarxnetd] incoming connection from {addr}", ts());
    }

    let mut transport = TcpTransport::from_stream(stream);
    let mut ctx = ConnCtx::new();

    // 1) mandatory Hello
    handle_hello(&mut transport, node_id, version)?;

    // 2) main loop over frames
    loop {
        let frame = match recv_frame(&mut transport) {
            Ok(f) => f,
            Err(e) => {
                eprintln!(
                    "{} [quarxnetd] recv error / connection closed: {:?}",
                    ts(),
                    e
                );
                break;
            }
        };

        if let Err(e) = handle_frame(&mut transport, &mut ctx, &store, frame) {
            eprintln!("{} [quarxnetd] frame handling error: {:?}", ts(), e);
            break;
        }
    }

    Ok(())
}

/// Simple multi-threaded TCP server for quarxnetd.
/// Implements Hello + echo/last for blocks/objects persisted through NetStore.
pub fn run_server(bind_addr: &str, port: u16) -> NetResult<()> {
    let addr = format!("{}:{}", bind_addr, port);
    let listener = TcpListener::bind(&addr)?;
    eprintln!("{} [quarxnetd] listening on {addr}", ts());

    // For now a fixed node_id and protocol 0.1
    let node_id: NodeId = 1;
    let version = ProtocolVersion { major: 0, minor: 1 };

    // Repository backend: file-based, root from QUARXNET_STORE_DIR or /tmp/quarxnet-store.
    let store_root =
        env::var("QUARXNET_STORE_DIR").unwrap_or_else(|_| "/tmp/quarxnet-store".to_string());
    let backend = FileBackend::new(store_root).expect("cannot init FileBackend");
    let repo = Repo::new(backend);
    let store = RepoNetBackend::new(repo);

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let v = version.clone();
                let node = node_id;
                let store_for_conn = store.clone();
                thread::spawn(move || {
                    if let Err(e) = handle_client(s, node, v, store_for_conn) {
                        eprintln!("{} [quarxnetd] connection error: {:?}", ts(), e);
                    }
                });
            }
            Err(e) => {
                eprintln!("{} [quarxnetd] accept error: {:?}", ts(), e);
            }
        }
    }

    Ok(())
}
