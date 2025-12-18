use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use quarxdrive::repo::error::RepoError;
use quarxdrive::repo::znode::ZNodeKind;
use quarxdrive::repo::RepoBackend;
use quarxdrive::{FileBackend, Repo, ZNode};

// --------------------------- common helpers ---------------------------

fn usage() -> ! {
    eprintln!(
        "Usage:
  quarxdrive <command> [options]

Commands:
  import-l0     Import files/dirs into L0 chunk store with dedup
  import-tree   Import directory as structural ZNode tree (no L0 chunks)
  cat-l0        Dump L0-backed file by ZNode id to stdout

Run:
  quarxdrive import-l0 --help
  quarxdrive import-tree --help
  quarxdrive cat-l0 --help"
    );
    std::process::exit(1);
}

fn usage_import_l0() -> ! {
    eprintln!(
        "Usage:
  quarxdrive import-l0 <STORE_ROOT> <DIR|FILE> \
[--dump] [--skip-hidden] [--chunk=N]

Options:
  --dump          dump annotated tree (QFLAGS)
  --skip-hidden   skip entries starting with '.'
  --chunk=N       chunk size, e.g. 8192, 128k, 4m

Config (ini: ~/.config/quarxtor/quarxctl.ini)
  l0_chunk=<bytes>            # default 8192
  import.skip_hidden=<bool>   # default false

Env:
  QUARXTOR_L0_CHUNK=<bytes>   # overrides l0_chunk"
    );
    std::process::exit(1);
}

fn usage_import_tree() -> ! {
    eprintln!(
        "Usage:
  quarxdrive import-tree <STORE_ROOT> <DIR> \
[--dump] [--skip-hidden] [--skip-git]

Options:
  --dump          dump annotated tree (QFLAGS)
  --skip-hidden   skip entries starting with '.'
  --skip-git      skip '.git' directories"
    );
    std::process::exit(1);
}

fn usage_cat_l0() -> ! {
    eprintln!(
        "Usage:
  quarxdrive cat-l0 <STORE_ROOT> <ZNODE_ID>

Description:
  Reconstruct L0-backed file identified by ZNode id and write to stdout.

Notes:
  - Requires that L0 blocks are stored under <STORE_ROOT>/l0_blocks
  - Requires that L0 index files exist under <STORE_ROOT>/l0_index
    with contents = ZNode id (one per file)."
    );
    std::process::exit(1);
}

fn parse_bytes(s: &str) -> Option<usize> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    if let Ok(n) = s.parse::<usize>() {
        return Some(n);
    }

    if s.len() < 2 {
        return None;
    }

    let (num, suffix) = s.split_at(s.len() - 1);
    let mul = match suffix {
        "k" | "K" => 1024usize,
        "m" | "M" => 1024usize * 1024,
        "g" | "G" => 1024usize * 1024 * 1024,
        _ => return None,
    };

    let base = num.trim().parse::<usize>().ok()?;
    Some(base.saturating_mul(mul))
}

fn parse_bool(s: &str) -> Option<bool> {
    let v = s.trim().to_ascii_lowercase();
    match v.as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn config_path() -> Option<PathBuf> {
    if let Ok(p) = env::var("QUARXTOR_CONFIG") {
        return Some(PathBuf::from(p));
    }

    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        let p = PathBuf::from(xdg).join("quarxtor").join("quarxctl.ini");
        if p.exists() {
            return Some(p);
        }
    }

    if let Ok(home) = env::var("HOME") {
        let p = PathBuf::from(home)
            .join(".config")
            .join("quarxtor")
            .join("quarxctl.ini");
        if p.exists() {
            return Some(p);
        }
    }

    None
}

struct DumpNode {
    raw: String,
    _depth: usize,
    is_dir: bool,
    is_file: bool,
    parent: Option<usize>,
    children: Vec<usize>,
    qflag: char,
}

fn annotate_tree(tree: &str) -> String {
    let mut nodes: Vec<DumpNode> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();

    for line in tree.lines() {
        let raw = line.to_string();
        let mut indent = 0usize;
        for c in line.chars() {
            if c == ' ' {
                indent += 1;
            } else {
                break;
            }
        }

        let depth = indent / 2;
        let c_mode = line.chars().nth(indent).unwrap_or('?');
        let is_dir = c_mode == 'd';
        let is_file = c_mode == '-';

        while stack.len() > depth {
            stack.pop();
        }

        let parent = stack.last().cloned();

        let idx = nodes.len();
        if let Some(p) = parent {
            nodes[p].children.push(idx);
        }

        nodes.push(DumpNode {
            raw,
            _depth: depth,
            is_dir,
            is_file,
            parent,
            children: Vec::new(),
            qflag: '?',
        });

        stack.push(idx);
    }

    for i in 0..nodes.len() {
        if nodes[i].is_dir {
            nodes[i].qflag = 'D';
        } else if nodes[i].is_file {
            let parent_is_file = nodes[i].parent.map(|p| nodes[p].is_file).unwrap_or(false);

            if parent_is_file {
                nodes[i].qflag = 'C';
            } else {
                let has_file_child = nodes[i]
                    .children
                    .iter()
                    .copied()
                    .any(|ch| nodes[ch].is_file);
                nodes[i].qflag = if has_file_child { 'L' } else { 'F' };
            }
        } else {
            nodes[i].qflag = '?';
        }
    }

    let mut out = String::new();
    for n in nodes {
        out.push_str(&format!("[{}] {}\n", n.qflag, n.raw));
    }
    out
}

// --------------------------- import-l0 ---------------------------

const DEFAULT_L0_CHUNK: usize = 8192;

struct ImportConfig {
    chunk_size: Option<usize>,
    skip_hidden: Option<bool>,
}

fn load_import_config() -> ImportConfig {
    let mut cfg = ImportConfig {
        chunk_size: None,
        skip_hidden: None,
    };

    let path = match config_path() {
        Some(p) => p,
        None => return cfg,
    };

    let contents = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return cfg,
    };

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        let mut parts = line.splitn(2, '=');
        let key = parts.next().unwrap().trim();
        let val = parts.next().map(|v| v.trim()).unwrap_or("");

        match key {
            "l0_chunk" => {
                if let Some(n) = parse_bytes(val) {
                    cfg.chunk_size = Some(n);
                }
            }
            "import.skip_hidden" => {
                if let Some(b) = parse_bool(val) {
                    cfg.skip_hidden = Some(b);
                }
            }
            _ => {}
        }
    }

    cfg
}

fn detect_chunk_size(cfg: &ImportConfig) -> usize {
    if let Some(n) = cfg.chunk_size {
        return n;
    }

    if let Ok(v) = env::var("QUARXTOR_L0_CHUNK") {
        if let Some(n) = parse_bytes(&v) {
            if n > 0 {
                return n;
            }
        }
    }

    DEFAULT_L0_CHUNK
}

fn chunk_hex(data: &[u8]) -> String {
    use blake3::Hasher;
    use std::fmt::Write as _;

    let mut hasher = Hasher::new();
    hasher.update(data);
    let hash = hasher.finalize();
    let mut s = String::with_capacity(hash.as_bytes().len() * 2);
    for b in hash.as_bytes() {
        let _ = write!(&mut s, "{:02x}", b);
    }
    s
}

fn load_index_entry(path: &Path) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(s) => {
            let id = s.trim();
            if id.is_empty() {
                None
            } else {
                Some(id.to_string())
            }
        }
        Err(_) => None,
    }
}

fn store_index_entry(path: &Path, id: &str) -> Result<(), RepoError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(RepoError::Io)?;
    }
    fs::write(path, id).map_err(RepoError::Io)
}

fn store_block(path: &Path, data: &[u8]) -> Result<(), RepoError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(RepoError::Io)?;
    }
    fs::write(path, data).map_err(RepoError::Io)
}

fn shard_path(root: &Path, hex: &str) -> PathBuf {
    if hex.len() >= 4 {
        root.join(&hex[0..2]).join(&hex[2..4]).join(hex)
    } else {
        root.join(hex)
    }
}

struct ImportOptionsL0 {
    skip_hidden: bool,
    chunk_size: usize,
    index_root: PathBuf,
    blocks_root: PathBuf,
}

struct ImportStats {
    total_input_bytes: u64,
    unique_bytes: u64,
    reused_bytes: u64,
    total_chunks: u64,
    unique_chunks: u64,
    reused_chunks: u64,
}

impl ImportStats {
    fn new() -> Self {
        Self {
            total_input_bytes: 0,
            unique_bytes: 0,
            reused_bytes: 0,
            total_chunks: 0,
            unique_chunks: 0,
            reused_chunks: 0,
        }
    }
}

fn import_file_l0<B: RepoBackend>(
    repo: &Repo<B>,
    path: &Path,
    opts: &ImportOptionsL0,
    stats: &mut ImportStats,
) -> Result<String, RepoError> {
    let mut f = File::open(path).map_err(RepoError::Io)?;
    let mut buf = vec![0u8; opts.chunk_size];
    let mut total_size: u64 = 0;
    let mut chunk_nodes: Vec<String> = Vec::new();

    loop {
        let n = match f.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(RepoError::Io(e)),
        };

        total_size += n as u64;
        stats.total_input_bytes += n as u64;
        stats.total_chunks += 1;

        let slice = &buf[..n];

        let hex = chunk_hex(slice);
        let idx_path = shard_path(&opts.index_root, &hex);
        let blk_path = shard_path(&opts.blocks_root, &hex);

        if let Some(existing_id) = load_index_entry(&idx_path) {
            stats.reused_chunks += 1;
            stats.reused_bytes += n as u64;
            chunk_nodes.push(existing_id);
            continue;
        }

        store_block(&blk_path, slice)?;

        let chunk_zn = ZNode {
            kind: ZNodeKind::Chunk,
            size: n as u64,
            children: Vec::new(),
        };
        let chunk_id = repo.put_znode(&chunk_zn)?;
        store_index_entry(&idx_path, &chunk_id)?;

        stats.unique_chunks += 1;
        stats.unique_bytes += n as u64;

        chunk_nodes.push(chunk_id);
    }

    let file_zn = ZNode {
        kind: ZNodeKind::L0File,
        size: total_size,
        children: chunk_nodes,
    };
    repo.put_znode(&file_zn)
}

fn import_path_l0<B: RepoBackend>(
    repo: &Repo<B>,
    path: &Path,
    opts: &ImportOptionsL0,
    stats: &mut ImportStats,
) -> Result<String, RepoError> {
    let meta = fs::symlink_metadata(path).map_err(RepoError::Io)?;

    if meta.is_file() {
        return import_file_l0(repo, path, opts, stats);
    }

    if meta.is_dir() {
        let mut children_ids = Vec::new();

        let rd = fs::read_dir(path).map_err(RepoError::Io)?;
        for entry in rd {
            let entry = entry.map_err(RepoError::Io)?;
            let child_path = entry.path();

            if let Some(name) = child_path.file_name().and_then(|s| s.to_str()) {
                if opts.skip_hidden && name.starts_with('.') {
                    continue;
                }
            }

            let child_id = import_path_l0(repo, &child_path, opts, stats)?;
            children_ids.push(child_id);
        }

        let zn = ZNode {
            kind: ZNodeKind::Directory,
            size: 0,
            children: children_ids,
        };
        return repo.put_znode(&zn);
    }

    let zn = ZNode {
        kind: ZNodeKind::File,
        size: 0,
        children: Vec::new(),
    };
    repo.put_znode(&zn)
}

fn cmd_import_l0(args: &[String]) {
    if args.len() < 2 {
        usage_import_l0();
    }

    let store_root = &args[0];
    let dir_or_file = &args[1];

    let cfg = load_import_config();

    let mut dump_full = false;
    let mut skip_hidden = cfg.skip_hidden.unwrap_or(false);
    let mut chunk_size = detect_chunk_size(&cfg);

    for arg in &args[2..] {
        if arg == "--dump" {
            dump_full = true;
        } else if arg == "--skip-hidden" {
            skip_hidden = true;
        } else if let Some(v) = arg.strip_prefix("--chunk=") {
            if let Some(n) = parse_bytes(v) {
                if n > 0 {
                    chunk_size = n;
                }
            }
        } else if arg == "--help" || arg == "-h" {
            usage_import_l0();
        } else {
            eprintln!("Unknown arg for import-l0: {}", arg);
            usage_import_l0();
        }
    }

    let index_root = PathBuf::from(store_root).join("l0_index");
    let blocks_root = PathBuf::from(store_root).join("l0_blocks");

    let opts = ImportOptionsL0 {
        skip_hidden,
        chunk_size,
        index_root,
        blocks_root,
    };

    let backend = match FileBackend::new(store_root) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Failed to init FileBackend at {}: {:?}", store_root, e);
            std::process::exit(1);
        }
    };

    let repo = Repo::new(backend);
    let root_path = PathBuf::from(dir_or_file);

    let mut stats = ImportStats::new();

    let root_id = match import_path_l0(&repo, &root_path, &opts, &mut stats) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("import_path (L0) error for {:?}: {:?}", root_path, e);
            std::process::exit(1);
        }
    };

    let stored_bytes = stats.unique_bytes;
    let logical_bytes = stats.total_input_bytes;
    let saved_bytes = logical_bytes.saturating_sub(stored_bytes);
    let dedup_factor = if stored_bytes > 0 {
        logical_bytes as f64 / stored_bytes as f64
    } else {
        1.0
    };

    println!(
        "root_id={} (chunk_size={} bytes, skip_hidden={})",
        root_id, opts.chunk_size, opts.skip_hidden
    );
    println!(
        "L0 stats: logical_bytes={}, stored_bytes={}, saved_bytes={}, \
total_chunks={}, unique_chunks={}, reused_chunks={}, dedup_factor={:.2}",
        logical_bytes,
        stored_bytes,
        saved_bytes,
        stats.total_chunks,
        stats.unique_chunks,
        stats.reused_chunks,
        dedup_factor
    );

    if dump_full {
        match repo.dump_tree(&root_id, 0) {
            Ok(tree) => {
                println!("--- tree dump (QFLAGS: D=dir, F=file, L=L0-file, C=chunk) ---");
                let annotated = annotate_tree(&tree);
                print!("{}", annotated);
            }
            Err(e) => {
                eprintln!("dump_tree error for id {}: {:?}", root_id, e);
                std::process::exit(1);
            }
        }
    }
}

// --------------------------- import-tree ---------------------------

struct ImportOptionsTree {
    skip_hidden: bool,
    skip_git: bool,
}

fn import_path_tree<B: RepoBackend>(
    repo: &Repo<B>,
    path: &Path,
    opts: &ImportOptionsTree,
) -> Result<String, RepoError> {
    let meta = fs::symlink_metadata(path).map_err(RepoError::Io)?;

    if meta.is_file() {
        let size = meta.len();
        let zn = ZNode {
            kind: ZNodeKind::File,
            size,
            children: Vec::new(),
        };
        return repo.put_znode(&zn);
    }

    if meta.is_dir() {
        let mut children_ids = Vec::new();

        let rd = fs::read_dir(path).map_err(RepoError::Io)?;
        for entry in rd {
            let entry = entry.map_err(RepoError::Io)?;
            let child_path = entry.path();

            if let Some(name) = child_path.file_name().and_then(|s| s.to_str()) {
                if opts.skip_git && name == ".git" {
                    continue;
                }
                if opts.skip_hidden && name.starts_with('.') {
                    continue;
                }
            }

            let child_id = import_path_tree(repo, &child_path, opts)?;
            children_ids.push(child_id);
        }

        let zn = ZNode {
            kind: ZNodeKind::Directory,
            size: 0,
            children: children_ids,
        };
        return repo.put_znode(&zn);
    }

    let zn = ZNode {
        kind: ZNodeKind::File,
        size: 0,
        children: Vec::new(),
    };
    repo.put_znode(&zn)
}

fn cmd_import_tree(args: &[String]) {
    if args.len() < 2 {
        usage_import_tree();
    }

    let store_root = &args[0];
    let dir = &args[1];

    let mut dump_full = false;
    let mut skip_hidden = false;
    let mut skip_git = false;

    for arg in &args[2..] {
        if arg == "--dump" {
            dump_full = true;
        } else if arg == "--skip-hidden" {
            skip_hidden = true;
        } else if arg == "--skip-git" {
            skip_git = true;
        } else if arg == "--help" || arg == "-h" {
            usage_import_tree();
        } else {
            eprintln!("Unknown arg for import-tree: {}", arg);
            usage_import_tree();
        }
    }

    let opts = ImportOptionsTree {
        skip_hidden,
        skip_git,
    };

    let backend = match FileBackend::new(store_root) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Failed to init FileBackend at {}: {:?}", store_root, e);
            std::process::exit(1);
        }
    };

    let repo = Repo::new(backend);
    let root_path = PathBuf::from(dir);

    let root_id = match import_path_tree(&repo, &root_path, &opts) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("import_path (tree) error for {:?}: {:?}", root_path, e);
            std::process::exit(1);
        }
    };

    println!("root_id={}", root_id);

    if dump_full {
        match repo.dump_tree(&root_id, 0) {
            Ok(tree) => {
                println!("--- tree dump (QFLAGS: D=dir, F=file, L=L0-file-ish, C=chunk-ish) ---");
                let annotated = annotate_tree(&tree);
                print!("{}", annotated);
            }
            Err(e) => {
                eprintln!("dump_tree error for id {}: {:?}", root_id, e);
                std::process::exit(1);
            }
        }
    }
}

// --------------------------- cat-l0 ---------------------------

fn build_id_to_hex_map(index_root: &Path) -> io::Result<HashMap<String, String>> {
    let mut map = HashMap::new();

    if !index_root.exists() {
        return Ok(map);
    }

    let mut stack = vec![index_root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let rd = match fs::read_dir(&dir) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("read_dir error on {:?}: {}", dir, e);
                continue;
            }
        };

        for entry in rd {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("read_dir entry error: {}", e);
                    continue;
                }
            };

            let path = entry.path();
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("metadata error for {:?}: {}", path, e);
                    continue;
                }
            };

            if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file() {
                let hex = match path.file_name().and_then(|s| s.to_str()) {
                    Some(s) => s.to_string(),
                    None => continue,
                };

                let contents = match fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("read_to_string error for {:?}: {}", path, e);
                        continue;
                    }
                };

                let id = contents.trim();
                if !id.is_empty() {
                    map.insert(id.to_string(), hex);
                }
            }
        }
    }

    Ok(map)
}

fn cat_l0_recursive<B: RepoBackend>(
    repo: &Repo<B>,
    blocks_root: &Path,
    id: &str,
    id_to_hex: &HashMap<String, String>,
) -> Result<(), RepoError> {
    // Assume Repo has get_znode; adjust if the real API differs.
    let zn = repo.get_znode(id)?;

    match zn.kind {
        ZNodeKind::Directory => {
            // Nothing to emit.
            Ok(())
        }
        ZNodeKind::Chunk => {
            // Leaf is an L0 chunk: find hex by id, read block and write to stdout.
            let hex = match id_to_hex.get(id) {
                Some(h) => h,
                None => {
                    // No entry — silently skip (could be an error if strict is desired).
                    return Ok(());
                }
            };

            let blk_path = shard_path(blocks_root, hex);
            let mut f = File::open(&blk_path).map_err(RepoError::Io)?;
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            io::copy(&mut f, &mut handle).map_err(RepoError::Io)?;
            Ok(())
        }
        ZNodeKind::File | ZNodeKind::L0File => {
            if zn.children.is_empty() {
                // File without children — fallback as single chunk (legacy data).
                let hex = match id_to_hex.get(id) {
                    Some(h) => h,
                    None => {
                        // No entry — silently skip (could be an error if strict is desired).
                        return Ok(());
                    }
                };

                let blk_path = shard_path(blocks_root, hex);
                let mut f = File::open(&blk_path).map_err(RepoError::Io)?;
                let stdout = io::stdout();
                let mut handle = stdout.lock();
                io::copy(&mut f, &mut handle).map_err(RepoError::Io)?;
                Ok(())
            } else {
                // File above chunks — recursively process children in order.
                for child in &zn.children {
                    cat_l0_recursive(repo, blocks_root, child, id_to_hex)?;
                }
                Ok(())
            }
        }
    }
}

fn cmd_cat_l0(args: &[String]) {
    if args.len() < 2 {
        usage_cat_l0();
    }

    let store_root = &args[0];
    let znode_id = &args[1];

    if let Some(arg) = args.get(2) {
        if arg == "--help" || arg == "-h" {
            usage_cat_l0();
        } else {
            eprintln!("Unknown arg for cat-l0: {}", arg);
            usage_cat_l0();
        }
    }

    let backend = match FileBackend::new(store_root) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Failed to init FileBackend at {}: {:?}", store_root, e);
            std::process::exit(1);
        }
    };

    let repo = Repo::new(backend);

    let index_root = PathBuf::from(store_root).join("l0_index");
    let blocks_root = PathBuf::from(store_root).join("l0_blocks");

    let id_to_hex = match build_id_to_hex_map(&index_root) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to build id->hex map from {:?}: {}", index_root, e);
            std::process::exit(1);
        }
    };

    if let Err(e) = cat_l0_recursive(&repo, &blocks_root, znode_id, &id_to_hex) {
        eprintln!("cat-l0 error for id {}: {:?}", znode_id, e);
        std::process::exit(1);
    }
}

// --------------------------- main ---------------------------

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        usage();
    }

    let cmd = &args[1];
    let sub_args = &args[2..];

    match cmd.as_str() {
        "import-l0" => cmd_import_l0(sub_args),
        "import-tree" | "import-dir" => cmd_import_tree(sub_args),
        "cat-l0" => cmd_cat_l0(sub_args),
        "help" | "--help" | "-h" => usage(),
        other => {
            eprintln!("Unknown command: {}", other);
            usage();
        }
    }
}
