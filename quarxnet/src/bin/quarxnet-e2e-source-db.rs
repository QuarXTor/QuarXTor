use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::sync_channel;
use std::thread;
use std::time::Duration;

use smallvec::{smallvec, SmallVec};
use quarxtor_core::block::multi::MultiRecipe;
use quarxtor_core::codec::{ObjectPayload, ZPayload};
use quarxtor_core::config::QuarxConfig;
use quarxtor_core::net_core::frames::{Frame, FrameHeader, FrameKind, HelloPayload};
use quarxtor_core::net_core::types::ProtocolVersion;
use quarxtor_core::store::blockstore::{decode_frame_typed, BlockStore, StoreError};
use quarxtor_core::store::decode::BlockBody;
use quarxtor_core::store::file_store::FileBlockStore;
use quarxtor_core::types::{BlockId, BlockKind, BlockRef};
use quarxnet::protocol::{
    decode_hello, decode_push_blocks, encode_hello, recv_frame, send_frame,
};
use quarxnet::transport::TcpTransport;

const CHUNK_SIZE: usize = 8 * 1024;
// Keep object type tag local to avoid a back-edge on quarxdrive.
const OBJ_TYPE_BYTES: u32 = 1;

struct Config {
    bind_addr: String,
    connect_addr: String,
    smoke: bool,
}

fn load_config() -> Config {
    let mut bind_addr = env::var("QUARX_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:39090".to_string());
    let mut connect_addr =
        env::var("QUARX_CONNECT_ADDR").unwrap_or_else(|_| "192.168.192.30:39090".to_string());
    let mut smoke = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => {
                bind_addr = args
                    .next()
                    .unwrap_or_else(|| panic!("--bind requires an address value"));
            }
            "--connect" => {
                connect_addr = args
                    .next()
                    .unwrap_or_else(|| panic!("--connect requires an address value"));
            }
            "--smoke" => {
                smoke = true;
            }
            other => {
                panic!("unknown arg: {}", other);
            }
        }
    }

    Config {
        bind_addr,
        connect_addr,
        smoke,
    }
}

fn resolve_dataset_root(cfg: &QuarxConfig) -> String {
    if let Ok(path) = env::var("QUARX_DATASET_DIR") {
        return path;
    }
    if let Some(root) = &cfg.import_root {
        return root.clone();
    }
    eprintln!("DATASET ROOT NOT SET (config missing and QUARX_DATASET_DIR not set)");
    std::process::exit(4);
}

fn resolve_max_inflight() -> usize {
    let default_limit = 128usize;
    match env::var("QUARX_MAX_INFLIGHT_BLOCKS") {
        Ok(v) => v.parse::<usize>().unwrap_or(default_limit),
        Err(_) => default_limit,
    }
}

fn resolve_net_window() -> usize {
    match env::var("QUARX_NET_WINDOW") {
        Ok(v) => v.parse::<usize>().unwrap_or(32).max(1),
        Err(_) => 32,
    }
}

fn main() {
    let config = load_config();
    let cfg = QuarxConfig::load_default();
    let dataset_root = resolve_dataset_root(&cfg);
    let root = Path::new(&dataset_root);
    if !root.is_dir() {
        panic!("dataset missing: {}", dataset_root);
    }
    let bind_addr: SocketAddr = config
        .bind_addr
        .parse()
        .unwrap_or_else(|e| panic!("invalid bind addr {}: {}", config.bind_addr, e));
    let connect_addr: SocketAddr = config
        .connect_addr
        .parse()
        .unwrap_or_else(|e| panic!("invalid connect addr {}: {}", config.connect_addr, e));

    // Smoke guard: requires a real TCP listener; any bind/connect failure must stop the run.
    let listener = TcpListener::bind(bind_addr).expect("bind listener");
    eprintln!("READY on {}", listener.local_addr().unwrap_or(bind_addr));

    let max_inflight = resolve_max_inflight();
    let net_window = resolve_net_window();
    let (files, total_bytes, store_path, hashes, block_count) =
        ingest_source(root, config.smoke, max_inflight);
    if block_count > max_inflight {
        eprintln!(
            "max inflight blocks {} exceeded: {} blocks queued; raise QUARX_MAX_INFLIGHT_BLOCKS",
            max_inflight, block_count
        );
        std::process::exit(6);
    }
    if block_count == 0 {
        panic!("no frames produced from dataset; check contents");
    }

    let (_addr, _recv_path, handle) = start_receiver(listener, block_count, hashes.clone());
    send_frames(connect_addr, &store_path, block_count, net_window);
    let recv_store = handle.join().expect("receiver thread join");

    let (hash_ok, block_count_recv, byte_count) =
        verify_store(&store_path, &recv_store, block_count, &hashes);

    println!(
        "FILES={} BLOCKS={} BYTES={} HASH_OK={}",
        files, block_count_recv, total_bytes, hash_ok
    );
    if !(hash_ok && byte_count == total_bytes) {
        std::process::exit(1);
    }
}

fn ingest_source(
    root: &Path,
    smoke: bool,
    max_inflight: usize,
) -> (usize, u64, PathBuf, Vec<[u8; 32]>, usize) {
    let mut files = Vec::new();
    walk_sorted(root, &mut files);

    let tmp_dir = Path::new("target/tmp/e2e-sender-store");
    let _ = fs::remove_dir_all(tmp_dir);
    fs::create_dir_all(tmp_dir).expect("create sender tmp dir");
    let store_path = tmp_dir.join("store.qblk");
    let mut store = FileBlockStore::open(store_path.clone()).expect("open sender store");
    let mut buf = vec![0u8; CHUNK_SIZE];

    let mut total_bytes = 0u64;
    let mut ingested_files = 0usize;
    for path in files.iter() {
        if smoke && ingested_files >= 1 {
            break;
        }
        let mut f = File::open(path).unwrap_or_else(|e| panic!("open {:?}: {}", path, e));
        let mut l0_ids: SmallVec<[BlockId; 8]> = smallvec![];
        loop {
            let n = f
                .read(&mut buf)
                .unwrap_or_else(|e| panic!("read {:?}: {}", path, e));
            if n == 0 {
                break;
            }
            let id = store
                .put_l0(&buf[..n])
                .unwrap_or_else(|e| panic!("put_l0 {:?}: {:?}", path, e));
            l0_ids.push(id);
            total_bytes += n as u64;
            if l0_ids.len() > max_inflight {
                eprintln!(
                    "max inflight blocks {} exceeded while ingesting {:?}; raise QUARX_MAX_INFLIGHT_BLOCKS",
                    max_inflight, path
                );
                std::process::exit(6);
            }
        }
        if l0_ids.is_empty() {
            let recipe = MultiRecipe::Aggregate { blocks: smallvec![] };
            let multi_id = store
                .put_multi(&recipe)
                .unwrap_or_else(|e| panic!("put_multi empty {:?}: {:?}", path, e));
            let obj = ObjectPayload {
                root: BlockRef::Multi(multi_id),
                obj_type: OBJ_TYPE_BYTES,
                meta: Vec::new(),
            };
            store
                .put_object(&obj)
                .unwrap_or_else(|e| panic!("put_object empty {:?}: {:?}", path, e));
        } else {
            let first_l0 = *l0_ids.first().expect("non-empty l0 list");
            let last_l0 = *l0_ids.last().expect("non-empty l0 list");
            let z = ZPayload {
                first_l0,
                last_l0,
                z_type: 1,
                meta: Vec::new(),
            };
            let z_id = store
                .put_z(&z)
                .unwrap_or_else(|e| panic!("put_z {:?}: {:?}", path, e));
            let recipe = MultiRecipe::Aggregate {
                blocks: smallvec![z_id],
            };
            let multi_id = store
                .put_multi(&recipe)
                .unwrap_or_else(|e| panic!("put_multi {:?}: {:?}", path, e));
            let obj = ObjectPayload {
                root: BlockRef::Multi(multi_id),
                obj_type: OBJ_TYPE_BYTES,
                meta: Vec::new(),
            };
            store
                .put_object(&obj)
                .unwrap_or_else(|e| panic!("put_object {:?}: {:?}", path, e));
        }
        ingested_files += 1;
    }

    let mut hashes = Vec::new();
    let total_blocks = store.len();
    for id in 0..total_blocks {
        let frame = store
            .get_frame(id as BlockId)
            .unwrap_or_else(|e| panic!("get_frame {}: {:?}", id, e));
        let (_kind, _id, hash, _body) =
            decode_frame_typed(&frame).unwrap_or_else(|e| panic!("decode frame {}: {:?}", id, e));
        hashes.push(hash);
    }

    (
        ingested_files,
        total_bytes,
        store_path,
        hashes,
        total_blocks,
    )
}

fn walk_sorted(root: &Path, out: &mut Vec<PathBuf>) {
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let mut entries = fs::read_dir(&path)
            .unwrap_or_else(|e| panic!("read_dir {:?}: {}", path, e))
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_else(|e| panic!("collect dir {:?}: {}", path, e));
        entries.sort_by(|a, b| a.path().cmp(&b.path()));
        for entry in entries {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.is_file() {
                out.push(p);
            }
        }
    }
    out.sort();
}

fn start_receiver(
    listener: TcpListener,
    expected_frames: usize,
    expected_hashes: Vec<[u8; 32]>,
) -> (std::net::SocketAddr, PathBuf, thread::JoinHandle<PathBuf>) {
    let addr = listener.local_addr().expect("local addr");

    let recv_dir = Path::new("target/tmp/e2e-recv-store");
    let _ = fs::remove_dir_all(recv_dir);
    fs::create_dir_all(recv_dir).expect("create recv dir");
    let store_path = recv_dir.join("store.qblk");
    let store_path_clone = store_path.clone();

    let handle = thread::spawn(move || {
        let ret_path = store_path_clone.clone();
        let (stream, _) = listener.accept().expect("accept");
        run_receiver(stream, store_path_clone.clone(), expected_frames, expected_hashes).unwrap();
        ret_path
    });

    // Give listener a moment to start
    thread::sleep(Duration::from_millis(50));

    (addr, store_path, handle)
}

fn run_receiver(
    stream: TcpStream,
    store_path: PathBuf,
    expected_frames: usize,
    expected_hashes: Vec<[u8; 32]>,
) -> Result<(), StoreError> {
    let mut transport = TcpTransport::from_stream(stream);

    // Handshake: expect Hello, reply Hello.
    let hello = recv_frame(&mut transport).map_err(|e| StoreError::Corrupt(format!("{:?}", e)))?;
    if !matches!(hello.header.kind, FrameKind::Hello) {
        return Err(StoreError::Corrupt("expected Hello".into()));
    }
    decode_hello(&hello.payload).map_err(|e| StoreError::Corrupt(format!("{:?}", e)))?;
    let resp_payload = encode_hello(&HelloPayload {
        node: 1,
        version: ProtocolVersion { major: 0, minor: 1 },
    });
    let resp = Frame {
        header: FrameHeader {
            kind: FrameKind::Hello,
            flags: 0,
            length: resp_payload.len() as u32,
        },
        payload: resp_payload,
    };
    send_frame(&mut transport, &resp).map_err(|e| StoreError::Corrupt(format!("{:?}", e)))?;

    let mut store = FileBlockStore::open(store_path)?;

    for idx in 0..expected_frames {
        let frame = recv_frame(&mut transport)
            .map_err(|e| StoreError::Corrupt(format!("{:?}", e)))?;
        if !matches!(frame.header.kind, FrameKind::PushBlocks) {
            return Err(StoreError::Corrupt("expected PushBlocks".into()));
        }
        let pb = decode_push_blocks(&frame.payload)
            .map_err(|e| StoreError::Corrupt(format!("{:?}", e)))?;
        let raw = pb.raw;
        let (kind, id, hash, body) = decode_frame_typed(&raw)?;
        if hash != expected_hashes[idx] {
            return Err(StoreError::Corrupt("hash mismatch".into()));
        }
        let next_id = store.len() as BlockId;
        if id != next_id {
            return Err(StoreError::Corrupt(format!(
                "id mismatch: incoming {}, expected {}",
                id, next_id
            )));
        }
        match (kind, body) {
            (BlockKind::L0, BlockBody::L0(raw_l0)) => {
                store.put_l0(&raw_l0)?;
            }
            (BlockKind::Multi, BlockBody::Multi(recipe)) => {
                store.put_multi(&recipe)?;
            }
            (BlockKind::Z, BlockBody::Z(zp)) => {
                store.put_z(&zp)?;
            }
            (BlockKind::Object, BlockBody::Object(op)) => {
                store.put_object(&op)?;
            }
            _ => return Err(StoreError::Corrupt("unsupported block kind".into())),
        }
    }

    Ok(())
}

fn send_frames(addr: SocketAddr, store_path: &Path, expected: usize, window: usize) {
    let store = FileBlockStore::open(store_path.to_path_buf()).expect("open sender store for send");
    let (tx, rx) = sync_channel::<(usize, Vec<u8>)>(window.max(1));
    let producer = thread::spawn(move || {
        for idx in 0..expected {
            let frame = store
                .get_frame(idx as BlockId)
                .unwrap_or_else(|e| panic!("get_frame {}: {:?}", idx, e));
            tx.send((idx, frame)).expect("channel send frame");
        }
    });

    let stream = TcpStream::connect(addr).expect("connect sender");
    let mut transport = TcpTransport::from_stream(stream);

    // Send Hello, expect Hello back.
    let payload = encode_hello(&HelloPayload {
        node: 2,
        version: ProtocolVersion { major: 0, minor: 1 },
    });
    let hello = Frame {
        header: FrameHeader {
            kind: FrameKind::Hello,
            flags: 0,
            length: payload.len() as u32,
        },
        payload,
    };
    send_frame(&mut transport, &hello).expect("send hello");
    let resp = recv_frame(&mut transport).expect("recv hello resp");
    if !matches!(resp.header.kind, FrameKind::Hello) {
        panic!("expected Hello response");
    }

    let mut sent = 0usize;
    for (idx, pb_payload) in rx {
        if idx != sent {
            panic!("frame order mismatch: got {}, expected {}", idx, sent);
        }
        let frame = Frame {
            header: FrameHeader {
                kind: FrameKind::PushBlocks,
                flags: 0,
                length: pb_payload.len() as u32,
            },
            payload: pb_payload,
        };
        send_frame(&mut transport, &frame).expect("send block frame");
        sent += 1;
    }
    producer.join().expect("join frame producer");
    assert_eq!(sent, expected);
}

fn verify_store(
    sender_store_path: &Path,
    recv_store_path: &Path,
    expected_blocks: usize,
    original_hashes: &[[u8; 32]],
) -> (bool, usize, u64) {
    let sender_store =
        FileBlockStore::open(sender_store_path.to_path_buf()).expect("open sender store for verify");
    let store =
        FileBlockStore::open(recv_store_path.to_path_buf()).expect("open receiver store");
    let mut ok = true;
    let mut bytes = 0u64;
    let count = store.len();
    if count != expected_blocks {
        return (false, count, bytes);
    }
    for idx in 0..count {
        let orig_frame = sender_store
            .get_frame(idx as BlockId)
            .unwrap_or_else(|e| panic!("sender get_frame {}: {:?}", idx, e));
        let frame = store
            .get_frame(idx as BlockId)
            .unwrap_or_else(|e| panic!("recv get_frame {}: {:?}", idx, e));
        let (_k, _id, hash, body) = decode_frame_typed(&frame).expect("decode recv frame");
        if hash != original_hashes[idx] {
            ok = false;
        }
        if frame != orig_frame {
            ok = false;
        }
        if let BlockBody::L0(raw) = body {
            bytes += raw.len() as u64;
        }
    }
    (ok, count, bytes)
}
