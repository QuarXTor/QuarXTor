use quarxproto::{ZNode, ZNodeKind};

/// Sanity: ZNode bincode format round-trips and matches across users (drive/net).
#[test]
fn znode_bincode_roundtrip_is_compatible() {
    // Emulate quarxdrive usage: directory with two children.
    let zn_drive = ZNode {
        kind: ZNodeKind::Directory,
        size: 42,
        children: vec!["child-a".to_string(), "child-b".to_string()],
    };

    // Emulate quarxnet usage: same payload goes over the wire.
    let raw = zn_drive.serialize();
    let zn_net = ZNode::deserialize(&raw).expect("net side should decode");

    assert_eq!(zn_net.kind, ZNodeKind::Directory);
    assert_eq!(zn_net.size, 42);
    assert_eq!(zn_net.children, vec!["child-a".to_string(), "child-b".to_string()]);

    // And full roundtrip equality.
    let raw2 = zn_net.serialize();
    let zn2 = ZNode::deserialize(&raw2).expect("decode after re-encode");
    assert_eq!(zn2.pretty(), zn_drive.pretty());
}
