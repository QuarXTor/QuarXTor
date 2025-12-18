use std::fs;
use std::path::PathBuf;

use quarxdrive::QuarxRepo;
use quarxtor_core::graph::ObjectGraph;
use quarxtor_core::store::file_store::FileBlockStore;

#[test]
fn bytes_roundtrip_simple() {
    let path: PathBuf = std::env::temp_dir().join("quarxdrive_bytes_roundtrip.qblk");
    let _ = fs::remove_file(&path);

    let store = FileBlockStore::open(path.clone()).expect("open store");
    let mut repo = QuarxRepo::new(store);

    let data = b"hello-quarxtor-drive-0123456789";
    let root_obj = repo.put_bytes_simple(data).expect("put_bytes_simple");

    let restored = repo.get_bytes_simple(root_obj).expect("get_bytes_simple");
    assert_eq!(restored, data);

    // Verify ObjectGraph from core-rs sees closure from this root
    let graph = ObjectGraph::new(repo.store());
    let closure = graph
        .compute_closure_from_object(root_obj)
        .expect("closure");

    assert!(closure.blocks.len() >= 2); // at least Object + Multi (+ L0 chunks)

    drop(repo);
    let _ = fs::remove_file(&path);
}
