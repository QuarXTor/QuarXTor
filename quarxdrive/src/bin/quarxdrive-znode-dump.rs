use std::env;

use quarxdrive::{FileBackend, Repo};

fn main() {
    let mut args = env::args().skip(1);

    let store_root = args.next().unwrap_or_else(|| {
        eprintln!("Usage: quarxdrive-znode-dump <STORE_ROOT> <ZNODE_ID>");
        std::process::exit(1);
    });

    let znode_id = args.next().unwrap_or_else(|| {
        eprintln!("Usage: quarxdrive-znode-dump <STORE_ROOT> <ZNODE_ID>");
        std::process::exit(1);
    });

    let backend = match FileBackend::new(&store_root) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Failed to init FileBackend at {}: {:?}", store_root, e);
            std::process::exit(1);
        }
    };

    let repo = Repo::new(backend);

    match repo.dump_tree(&znode_id, 0) {
        Ok(tree) => print!("{}", tree),
        Err(e) => {
            eprintln!("dump_tree error for id {}: {:?}", znode_id, e);
            std::process::exit(1);
        }
    }
}
