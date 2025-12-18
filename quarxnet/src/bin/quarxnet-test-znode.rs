//! Dev/e2e helper: push/get a ZNode over quarxnet for manual protocol sanity.
use std::net::TcpStream;

use quarxnet::transport::TcpTransport;
use quarxnet::{client_get_znode, client_hello, client_push_znode};
use quarxproto::{ZNode, ZNodeKind};
use quarxtor_core::net_core::{NetResult, ProtocolVersion};

fn main() -> NetResult<()> {
    let addr = "127.0.0.1:54721";
    println!("[znode-test] connecting to {}", addr);

    let stream = TcpStream::connect(addr).expect("cannot connect to quarxnetd");
    let mut transport = TcpTransport::from_stream(stream);

    // handshake: node_id=1, protocol=0.1
    let node_id: u64 = 1;
    let version = ProtocolVersion { major: 0, minor: 1 };
    client_hello(&mut transport, node_id, version)?;

    // build test ZNode: directory with two children
    let zn = ZNode {
        kind: ZNodeKind::Directory,
        size: 2,
        children: vec!["child-a".into(), "child-b".into()],
    };

    println!("[znode-test] push znode: {:?}", zn);
    client_push_znode(&mut transport, &zn)?;

    // RepoNetBackend currently ignores id and always returns last_object
    let zn2 = client_get_znode(&mut transport, 0)?;

    println!(
        "[znode-test] got znode back: kind={:?}, size={}, children={:?}",
        zn2.kind, zn2.size, zn2.children
    );

    Ok(())
}
