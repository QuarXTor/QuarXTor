use quarxdrive::repo::znode::{deserialize_znode, ZNode, ZNodeKind};
use quarxdrive::repo::{InMemBackend, QuarxRepo};

#[test]
fn znode_serde_roundtrip_directory() {
    let zn = ZNode {
        kind: ZNodeKind::Directory,
        size: 123,
        children: vec!["child1".to_string(), "child2".to_string()],
    };

    let raw = zn.serialize();
    let zn2 = deserialize_znode(&raw).expect("deserialize");

    assert!(matches!(zn2.kind, ZNodeKind::Directory));
    assert_eq!(zn2.size, 123);
    assert_eq!(
        zn2.children,
        vec!["child1".to_string(), "child2".to_string()]
    );
}

#[test]
fn znode_serde_roundtrip_chunk() {
    let zn = ZNode {
        kind: ZNodeKind::Chunk,
        size: 4096,
        children: Vec::new(),
    };

    let raw = zn.serialize();
    let zn2 = deserialize_znode(&raw).expect("deserialize");

    assert!(matches!(zn2.kind, ZNodeKind::Chunk));
    assert_eq!(zn2.size, 4096);
    assert!(zn2.children.is_empty());
}

#[test]
fn repo_dump_tree_basic_directory() {
    let backend = InMemBackend::new();
    let repo = QuarxRepo::new(backend);

    let zn = ZNode {
        kind: ZNodeKind::Directory,
        size: 0,
        children: Vec::new(),
    };

    let root_id = repo.put_znode(&zn).expect("put_znode");
    let out = repo.dump_tree(&root_id, 0).expect("dump_tree");

    // Expect output starts with 'd' (Directory) and contains the znode id.
    assert!(
        out.starts_with("d"),
        "dump_tree() should start with 'd' for Directory, got: {out}"
    );
    assert!(
        out.contains(&root_id),
        "dump_tree() output must contain znode id, got: {out}"
    );
}
