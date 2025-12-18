use std::fs;
use std::path::PathBuf;

use quarxdrive::{DirEntry, QuarxRepo, OBJ_TYPE_BYTES};
use quarxtor_core::graph::ObjectGraph;
use quarxtor_core::store::blockstore::BlockStore;
use quarxtor_core::store::file_store::FileBlockStore;
use quarxtor_core::types::BlockKind;

#[test]
fn fs_dir_roundtrip() {
    let path: PathBuf = std::env::temp_dir().join("quarxdrive_fs_dir.qblk");
    let _ = fs::remove_file(&path);

    let store = FileBlockStore::open(path.clone()).expect("open store");
    let mut repo = QuarxRepo::new(store);

    // Two "files" as OBJ_TYPE_BYTES
    let id_a = repo.put_bytes_simple(b"AAA").expect("put_bytes_simple A");
    let id_b = repo.put_bytes_simple(b"BBBB").expect("put_bytes_simple B");

    // Sanity: ensure Object with obj_type = BYTES
    let (k_a, _, body_a) = repo.store().get_typed(id_a).expect("get a typed");
    assert!(matches!(k_a, BlockKind::Object));
    if let quarxtor_core::store::decode::BlockBody::Object(op) = body_a {
        assert_eq!(op.obj_type, OBJ_TYPE_BYTES);
    } else {
        panic!("expected Object body for file A");
    }

    // Directory with two entries
    let entries = vec![
        DirEntry {
            name: "a.bin".to_string(),
            target: id_a,
        },
        DirEntry {
            name: "b.bin".to_string(),
            target: id_b,
        },
    ];

    let dir_id = repo.put_dir(&entries).expect("put_dir");

    // Read directory back
    let got = repo.read_dir(dir_id).expect("read_dir");
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].name, "a.bin");
    assert_eq!(got[0].target, id_a);
    assert_eq!(got[1].name, "b.bin");
    assert_eq!(got[1].target, id_b);

    // Ensure ObjectGraph from dir_id captures files as well
    let graph = ObjectGraph::new(repo.store());
    let closure = graph
        .compute_closure_from_block(dir_id)
        .expect("closure from dir");

    assert!(closure.blocks.contains(&dir_id));
    assert!(closure.blocks.contains(&id_a));
    assert!(closure.blocks.contains(&id_b));

    drop(repo);
    let _ = fs::remove_file(&path);
}
