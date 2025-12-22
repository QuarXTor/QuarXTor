//! End-to-end test: quarxnet <-> quarxdrive ZNode flow via quarxproto and real TCP server.
use std::env;
use std::fs;
use std::io::ErrorKind;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use quarxnet::backend::{FileBackend, Repo};
use quarxnet::transport::TcpTransport;
use quarxnet::{
    client_get_znode, client_hello, client_push_znode, protocol::decode_get_object,
    protocol::decode_hello, protocol::decode_push_object, protocol::encode_hello,
    protocol::encode_push_object, protocol::recv_frame, protocol::send_frame,
};
use quarxproto::{ZNode, ZNodeKind};
use quarxtor_core::net_core::{
    frames::Frame, frames::FrameHeader, frames::FrameKind, HelloPayload, NetError, NetResult,
    ProtocolVersion, PushObjectPayload,
};
use uuid::Uuid;

fn temp_dir(tag: &str) -> PathBuf {
    let mut p = env::temp_dir();
    p.push(format!("quarxnet_znode_e2e_{}_{}", tag, Uuid::new_v4()));
    p
}

#[test]
fn znode_roundtrip_over_net_and_drive() {
    // Shared state between server and test for repo location and last object id/raw.
    let store_dir = temp_dir("store");
    fs::create_dir_all(&store_dir).unwrap();
    let last_object_id: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let last_object_raw: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));

    // Bind on ephemeral port.
    let listener = match TcpListener::bind(("127.0.0.1", 0)) {
        Ok(l) => l,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            eprintln!("skipping znode e2e: TCP bind not permitted");
            return;
        }
        Err(e) => panic!("bind listener: {e}"),
    };
    let addr = listener.local_addr().unwrap();
    let node_id: u64 = 1;
    let version_server = ProtocolVersion { major: 0, minor: 1 };
    let version_client = ProtocolVersion { major: 0, minor: 1 };

    // Spawn minimal server handling Hello + PushObject/GetObject.
    let server_id = Arc::clone(&last_object_id);
    let server_raw = Arc::clone(&last_object_raw);
    let store_dir_srv = store_dir.clone();
    let server_thread = thread::spawn(move || -> NetResult<()> {
        // Single accept for the test.
        let (stream, _peer) = listener.accept()?;
        let mut transport = TcpTransport::from_stream(stream);
        let repo = Repo::new(FileBackend::new(store_dir_srv).expect("backend init"));

        // Hello handshake.
        let frame = recv_frame(&mut transport)?;
        if !matches!(frame.header.kind, FrameKind::Hello) {
            return Err(NetError::InvalidFrame);
        }
        let _hello = decode_hello(&frame.payload)?;
        let resp_payload = encode_hello(&HelloPayload {
            node: node_id,
            version: version_server,
        });
        let resp = Frame {
            header: FrameHeader {
                kind: FrameKind::Hello,
                flags: 0,
                length: resp_payload.len() as u32,
            },
            payload: resp_payload,
        };
        send_frame(&mut transport, &resp)?;

        // Main loop for PushObject/GetObject.
        loop {
            let frame = match recv_frame(&mut transport) {
                Ok(f) => f,
                Err(NetError::Io(_)) => break, // client closed
                Err(e) => return Err(e),
            };

            match frame.header.kind {
                FrameKind::PushObject => {
                    let po = decode_push_object(&frame.payload)?;
                    let raw = po.raw;
                    let id = repo.put_object_raw(&raw).map_err(|_| NetError::InvalidFrame)?;
                    *server_id.lock().unwrap() = Some(id);
                    *server_raw.lock().unwrap() = Some(raw);
                }
                FrameKind::GetObject => {
                    let _go = decode_get_object(&frame.payload)?;
                    let raw = server_raw.lock().unwrap().clone().unwrap_or_default();
                    let payload = encode_push_object(&PushObjectPayload { raw });
                    let resp = Frame {
                        header: FrameHeader {
                            kind: FrameKind::PushObject,
                            flags: 0,
                            length: payload.len() as u32,
                        },
                        payload,
                    };
                    send_frame(&mut transport, &resp)?;
                }
                _ => break,
            }
        }

        Ok(())
    });

    // Client: hello + push znode + get znode back.
    let mut transport = TcpTransport::from_stream(TcpStream::connect(addr).expect("connect"));
    client_hello(&mut transport, node_id, version_client).expect("hello");

    let zn = ZNode {
        kind: ZNodeKind::Directory,
        size: 2,
        children: vec!["child-a".into(), "child-b".into()],
    };
    client_push_znode(&mut transport, &zn).expect("push znode");
    let zn_back = client_get_znode(&mut transport, 0).expect("get znode");
    assert_eq!(zn_back.kind, zn.kind);
    assert_eq!(zn_back.size, zn.size);
    assert_eq!(zn_back.children, zn.children);

    // Close the client stream so the server loop can exit before join().
    drop(transport);

    // Wait for server thread to finish.
    server_thread.join().expect("server thread").expect("server ok");

    // Read the same object via the quarxnet backend using the stored id.
    let obj_id = last_object_id
        .lock()
        .unwrap()
        .clone()
        .expect("object id stored");
    let drive_backend = FileBackend::new(&store_dir).expect("drive backend");
    let drive_repo = Repo::new(drive_backend);
    let raw = drive_repo
        .get_object_raw(&obj_id)
        .expect("drive get object raw");
    let zn_drive = ZNode::deserialize(&raw).expect("drive znode decode");

    assert_eq!(zn_drive.kind, zn.kind);
    assert_eq!(zn_drive.size, zn.size);
    assert_eq!(zn_drive.children, zn.children);

    let _ = fs::remove_dir_all(&store_dir);
}
