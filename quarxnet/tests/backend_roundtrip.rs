use std::env;
use std::fs;
use std::path::PathBuf;

use quarxnet::backend::{FileBackend, Repo, RepoBackend, RepoNetBackend};
use quarxnet::NetBackend;
use uuid::Uuid;

fn temp_dir(name: &str) -> PathBuf {
    let mut p = env::temp_dir();
    p.push(format!("quarxnet_backend_test_{}_{}", name, Uuid::new_v4()));
    p
}

#[test]
fn file_backend_put_get_raw() {
    let dir = temp_dir("file_backend");
    fs::create_dir_all(&dir).unwrap();

    let backend = FileBackend::new(&dir).expect("create backend");
    let id = backend.put_raw(b"hello").expect("put");
    let got = backend.get_raw(&id).expect("get");
    assert_eq!(got, b"hello");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn repo_net_backend_blocks_roundtrip_last_wins() {
    let dir = temp_dir("repo_blocks");
    fs::create_dir_all(&dir).unwrap();
    let backend = FileBackend::new(&dir).expect("create backend");
    let repo = Repo::new(backend);
    let net = RepoNetBackend::new(repo);

    net.put_blocks_raw(b"first").expect("put first");
    net.put_blocks_raw(b"second").expect("put second");

    let got = net.get_blocks_raw().expect("get blocks");
    assert_eq!(got, b"second");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn repo_net_backend_object_roundtrip_last_wins() {
    let dir = temp_dir("repo_objects");
    fs::create_dir_all(&dir).unwrap();
    let backend = FileBackend::new(&dir).expect("create backend");
    let repo = Repo::new(backend);
    let net = RepoNetBackend::new(repo);

    net.put_object_raw(b"obj-a").expect("put a");
    net.put_object_raw(b"obj-b").expect("put b");

    let got = net.get_object_raw().expect("get object");
    assert_eq!(got, b"obj-b");

    let _ = fs::remove_dir_all(&dir);
}
