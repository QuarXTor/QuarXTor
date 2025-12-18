use std::env;

use quarxdrive::repo::znode::ZNodeKind;
use quarxdrive::{FileBackend, Repo, ZNode};

fn usage_and_exit() -> ! {
    eprintln!("Usage: quarxdrive-znode-mktest-tree <STORE_ROOT>");
    std::process::exit(1);
}

fn main() {
    let store_root = env::args().nth(1).unwrap_or_else(|| usage_and_exit());

    let backend = match FileBackend::new(&store_root) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Failed to init FileBackend at {}: {:?}", store_root, e);
            std::process::exit(1);
        }
    };

    let repo = Repo::new(backend);

    // Two leaf nodes
    let leaf1 = ZNode {
        kind: ZNodeKind::File,
        size: 100,
        children: Vec::new(),
    };
    let leaf2 = ZNode {
        kind: ZNodeKind::File,
        size: 200,
        children: Vec::new(),
    };

    let id_leaf1 = repo.put_znode(&leaf1).expect("put_znode leaf1 failed");
    let id_leaf2 = repo.put_znode(&leaf2).expect("put_znode leaf2 failed");

    // Directory root referencing leaves
    let root = ZNode {
        kind: ZNodeKind::Directory,
        size: 0,
        children: vec![id_leaf1.clone(), id_leaf2.clone()],
    };

    let root_id = repo.put_znode(&root).expect("put_znode root failed");

    println!("root_id={}", root_id);

    match repo.dump_tree(&root_id, 0) {
        Ok(tree) => {
            println!("--- tree dump ---");
            print!("{}", tree);
        }
        Err(e) => {
            eprintln!("dump_tree error for id {}: {:?}", root_id, e);
            std::process::exit(1);
        }
    }
}
