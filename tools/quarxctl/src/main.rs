use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::exit;
use std::process::Stdio;
use std::time::Duration;
use std::{
    fs::File,
    io::{Read, Write},
    net::TcpStream,
    thread::{sleep, spawn},
};

use quarxdrive::{DirEntry, QuarxRepo};
use quarxtor_core::block::multi::MultiRecipe;
use quarxtor_core::config::{
    config_contract, ConfigStatus, ConfigValueType, EffectiveConfig, EffectiveConfigError,
    QuarxConfig, write_full_config,
};
use quarxtor_core::graph::ObjectGraph;
use quarxtor_core::store::blockstore::BlockStore;
use quarxtor_core::store::decode::BlockBody;
use quarxtor_core::store::file_store::FileBlockStore;
use quarxtor_core::types::BlockRef;
use quarxtor_core::util::log::ts_local;
use quarxnet::proto0::{decode_header, encode_header, serve_one_proto0};

fn usage() -> ! {
    eprintln!(
        "Usage:
  quarxctl cfg show                        # print effective config (no store-file needed)
  quarxctl cfg describe                    # print config schema (no defaults required)
  quarxctl cfg validate                    # validate config (no defaults if invalid)
  quarxctl cfg dump                        # print raw config keys (no defaults)

  # low-level (by obj-id)
  quarxctl <store-file> put <local-file>
  quarxctl <store-file> get <obj-id> <out-file>
  quarxctl <store-file> inspect <obj-id>

  # FS-like (flat root)
  quarxctl <store-file> fs-put <local-file> <dst-path>   # dst-path like /name
  quarxctl <store-file> fs-get <path> <out-file>         # path like /name
  quarxctl <store-file> fs-ls  <path>                    # currently supports /
  quarxctl <store-file> fs-inspect <path>                # inspect object behind path
  quarxctl <store-file> fs-tree                          # UID FS tree (root level)
  quarxctl <store-file> fs-stats                         # recursive FS stats from root
  quarxctl <store-file> fs-import <host-dir>             # import host FS dir into QuarX root

Config (ini: ~/.config/quarxtor/quarxctl.ini)
  l0_chunk=<bytes>                    # default 8192

  import.min_file_size=<bytes>        # default 0 (no threshold)
  import.skip_hidden=<true/false>     # default false
  import.skip_symlink=<true/false>    # default true
  import.skip_zero=<true/false>       # default true
  import.skip_devices=<true/false>    # default true
  import.skip_special=<true/false>    # default true

  import.include_glob=<csv-globs>         # e.g. *.rs,*.toml
  import.exclude_glob=<csv-globs>         # e.g. *.tmp,*.log

  import.include_dir_glob=<csv-globs>     # e.g. src/**,include/**
  import.exclude_dir_glob=<csv-globs>     # e.g. .git/**,target/**,node_modules/**

  import.glob_mode=<name|path>            # name=file_name, path=rel.path/from/host_root

  ram.limit=<none|full|N[K|M|G]>      # default none (0)
    # examples: full, 16G, 256M, 10M, 0

  fs_import.use_z=<true/false>        # default true
  fs_import.z_threshold=<u64>         # default 10

  analysis.enable_znode=<true/false>  # default true (reserved)
  analysis.fs_stats_fallback=<true/false> # default false/true (see config.rs)

Env overrides (override ini):
  QUARX_L0_CHUNK
  QUARX_IMPORT_MIN_FILE_SIZE
  QUARX_IMPORT_SKIP_HIDDEN
  QUARX_IMPORT_SKIP_SYMLINK
  QUARX_IMPORT_SKIP_ZERO
  QUARX_IMPORT_SKIP_DEVICES
  QUARX_IMPORT_SKIP_SPECIAL

  QUARX_IMPORT_INCLUDE_GLOB
  QUARX_IMPORT_EXCLUDE_GLOB

  QUARX_RAM_LIMIT

  QUARX_FS_IMPORT_USE_Z
  QUARX_FS_IMPORT_Z_THRESHOLD

  QUARX_ANALYSIS_ENABLE_ZNODE
  QUARX_ANALYSIS_FS_STATS_FALLBACK

Examples:
Examples:
Examples:
Examples:
  quarxctl repo.qblk put some.bin
  quarxctl repo.qblk get 2 restored.bin
  quarxctl repo.qblk inspect 2

  quarxctl repo.qblk fs-put some.bin /some.bin
  quarxctl repo.qblk fs-ls /
  quarxctl repo.qblk fs-get /some.bin restored.bin
  quarxctl repo.qblk fs-tree
  quarxctl repo.qblk fs-import ~/SOURCE-DB"
    );
    let is_help = env::args().any(|a| a == "--help" || a == "-h");
    exit(if is_help { 0 } else { 1 });
}

fn cfg_show() -> ! {
    let cfg = match EffectiveConfig::load_default() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            exit(1);
        }
    };

    for k in config_contract() {
        let value = match (k.section, k.key) {
            ("core", "l0_chunk") => cfg.l0_chunk.to_string(),
            ("core", "ram.limit") => cfg.ram_limit.clone(),
            ("core", "analysis.enable_znode") => cfg.analysis_enable_znode.to_string(),
            ("core", "analysis.fs_stats_fallback") => {
                cfg.analysis_fs_stats_fallback.to_string()
            }
            ("core", "repo_root") => cfg.repo_root.clone().unwrap_or_default(),
            ("import", "root") => cfg.dataset_root.clone(),
            ("import", "min_file_size") => cfg.import_min_file_size.to_string(),
            ("import", "skip_hidden") => cfg.import_skip_hidden.to_string(),
            ("import", "skip_symlink") => cfg.import_skip_symlink.to_string(),
            ("import", "skip_zero") => cfg.import_skip_zero.to_string(),
            ("import", "skip_devices") => cfg.import_skip_devices.to_string(),
            ("import", "skip_special") => cfg.import_skip_special.to_string(),
            ("import", "include_glob") => cfg.include_glob.clone().unwrap_or_default(),
            ("import", "exclude_glob") => cfg.exclude_glob.clone().unwrap_or_default(),
            ("import", "include_dir_glob") => {
                cfg.include_dir_glob.clone().unwrap_or_default()
            }
            ("import", "exclude_dir_glob") => {
                cfg.exclude_dir_glob.clone().unwrap_or_default()
            }
            ("import", "import_glob_mode") => cfg.import_glob_mode.clone(),
            ("import", "use_z") => cfg.fs_import_use_z.to_string(),
            ("import", "z_threshold") => cfg.fs_import_z_threshold.to_string(),
            ("paths", "data_dir") => cfg.paths_data_dir.clone().unwrap_or_default(),
            ("paths", "cache_dir") => cfg.paths_cache_dir.clone().unwrap_or_default(),
            ("paths", "log_dir") => cfg.paths_log_dir.clone().unwrap_or_default(),
            ("paths", "runtime_dir") => cfg.paths_runtime_dir.clone().unwrap_or_default(),
            ("net", "addr") => cfg.net_addr.clone(),
            ("drive", "mountpoint") => cfg.drive_mountpoint.clone().unwrap_or_default(),
            ("drive", "strategy") => cfg.drive_strategy.clone().unwrap_or_default(),
            ("drive", "fs_cache_bytes") => cfg.drive_fs_cache_bytes.to_string(),
            ("daemon", "enable_netd") => cfg.daemon_enable_netd.to_string(),
            ("cloud", "seed_nodes") => cfg.cloud_seed_nodes.clone().unwrap_or_default(),
            _ => String::new(),
        };
        let desc = k.description.first().cloned().unwrap_or("");
        println!(
            "{}.{} = {}  # {}  [{}]",
            k.section,
            k.key,
            value,
            desc,
            match k.status {
                ConfigStatus::Implemented => "IMPLEMENTED",
                ConfigStatus::Partial => "PARTIAL",
                ConfigStatus::Reserved => "RESERVED",
                ConfigStatus::NotImplemented => "NOT_IMPLEMENTED",
            }
        );
    }
    exit(0);
}

fn cfg_validate() -> ! {
    match EffectiveConfig::load_strict() {
        Ok(_) => exit(0),
        Err(EffectiveConfigError::MissingDatasetRoot) => {
            eprintln!(
                "DATASET ROOT NOT SET (config missing and QUARX_DATASET_DIR not set)"
            );
            exit(1);
        }
        Err(e) => {
            eprintln!("{e}");
            exit(1);
        }
    }
}

fn cfg_init(force: bool) -> ! {
    match write_full_config(force) {
        Ok(path) => {
            println!("{}", path.display());
            exit(0);
        }
        Err(e) => {
            eprintln!("{e}");
            exit(1);
        }
    }
}

fn cfg_describe() -> ! {
    fn type_str(v: &ConfigValueType) -> &'static str {
        match v {
            ConfigValueType::Bool => "bool",
            ConfigValueType::U64 => "u64",
            ConfigValueType::String => "string",
            ConfigValueType::Path => "path",
            ConfigValueType::Enum(_) => "enum",
        }
    }

    for k in config_contract() {
        println!(
            "{}.{} type={} default={} status={}",
            k.section,
            k.key,
            type_str(&k.value_type),
            k.default_value,
            k.status.as_str()
        );
        if let ConfigValueType::Enum(opts) = k.value_type {
            println!("  enum_values={}", opts.join(","));
        }
        if let Some(env) = k.env_override {
            println!("  env_override={env}");
        }
        for line in k.description {
            println!("  desc: {line}");
        }
        println!();
    }
    exit(0);
}

fn parse_id(s: &str, what: &str) -> u64 {
    match s.parse::<u64>() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("Invalid {what}: {s}");
            exit(1);
        }
    }
}

fn short_hash(h: &[u8; 32]) -> String {
    h[..8].iter().map(|b| format!("{:02x}", b)).collect()
}

fn print_block_line(store: &impl BlockStore, id: u64) {
    let (_kind, hash, body) = match store.get_typed(id) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("  {id}: <error reading block> {e:?}");
            return;
        }
    };

    match body {
        BlockBody::L0(raw) => {
            println!(
                "  {id}: L0      len={} hash={}",
                raw.len(),
                short_hash(&hash)
            );
        }

        BlockBody::Multi(recipe) => match recipe {
            MultiRecipe::Aggregate { blocks } => {
                println!(
                    "  {id}: Multi   Aggregate n_blocks={} hash={}",
                    blocks.len(),
                    short_hash(&hash)
                );
            }
            MultiRecipe::CodecRecipe {
                codec,
                dict,
                blocks,
                ..
            } => {
                let n = blocks.as_ref().map(|b| b.len()).unwrap_or(0);
                println!(
                    "  {id}: Multi   CodecRecipe codec={:?} dict={:?} n_blocks={} hash={}",
                    codec,
                    dict,
                    n,
                    short_hash(&hash)
                );
            }
            MultiRecipe::Custom { .. } => {
                println!(
                    "  {id}: Multi   Custom(<opaque>) hash={}",
                    short_hash(&hash)
                );
            }
        },

        BlockBody::Z(zp) => {
            println!(
                "  {id}: Z       range=[{}..{}] z_type={} hash={}",
                zp.first_l0,
                zp.last_l0,
                zp.z_type,
                short_hash(&hash)
            );
        }

        BlockBody::Object(op) => {
            let root_str = match op.root {
                BlockRef::L0(bid) => format!("L0({})", bid),
                BlockRef::Multi(bid) => format!("Multi({})", bid),
                BlockRef::Z(bid) => format!("Z({})", bid),
                BlockRef::Object(bid) => format!("Object({})", bid),
            };
            println!(
                "  {id}: Object  type={} root={} meta_len={} hash={}",
                op.obj_type,
                root_str,
                op.meta.len(),
                short_hash(&hash)
            );
        }
    }
}

// --- low-level bytes API ---

fn cmd_put(store_path: PathBuf, local_path: PathBuf) {
    let data = match fs::read(&local_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to read {:?}: {e}", local_path);
            exit(1);
        }
    };

    let cfg = QuarxConfig::load();

    let store = match FileBlockStore::open(store_path.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to open store {:?}: {e:?}", store_path);
            exit(1);
        }
    };
    let mut repo = QuarxRepo::with_chunk(store, cfg.l0_chunk as usize);

    match repo.put_bytes_simple(&data) {
        Ok(obj_id) => {
            println!("{obj_id}");
        }
        Err(e) => {
            eprintln!("put_bytes_simple failed: {e:?}");
            exit(1);
        }
    }
}

fn cmd_get(store_path: PathBuf, obj_id: u64, out_path: PathBuf) {
    let cfg = QuarxConfig::load();

    let store = match FileBlockStore::open(store_path.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to open store {:?}: {e:?}", store_path);
            exit(1);
        }
    };
    let repo = QuarxRepo::with_chunk(store, cfg.l0_chunk as usize);

    let data = match repo.get_bytes_simple(obj_id) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("get_bytes_simple failed for obj-id {obj_id}: {e:?}");
            exit(1);
        }
    };

    if let Err(e) = fs::write(&out_path, &data) {
        eprintln!("Failed to write {:?}: {e}", out_path);
        exit(1);
    }
}

fn cmd_inspect(store_path: PathBuf, root_id: u64) {
    let store = match FileBlockStore::open(store_path.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to open store {:?}: {e:?}", store_path);
            exit(1);
        }
    };

    let graph = ObjectGraph::new(&store);

    let closure = match graph.compute_closure_from_block(root_id) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("compute_closure_from_block({root_id}) failed: {e:?}");
            exit(1);
        }
    };

    println!("Graph closure from root block {root_id}:");
    println!("  roots     : {:?}", closure.roots);
    println!("  n_blocks  : {}", closure.blocks.len());
    println!("  blocks (DFS order):");
    for bid in &closure.blocks {
        print_block_line(&store, *bid);
    }
}

// --- FS-like helpers ---

/// Flat path resolver: `/` or `/name`
fn split_single_level(path: &str) -> Option<String> {
    let p = path.trim();
    if p.is_empty() || p == "/" {
        return None;
    }
    let p = p.trim_start_matches('/');
    if p.is_empty() {
        return None;
    }
    if p.contains('/') {
        eprintln!(
            "Nested directories are not implemented yet (flat root only). Got path: {}",
            path
        );
        exit(1);
    }
    Some(p.to_string())
}

// fs-put
fn fs_cmd_put(store_path: PathBuf, local_path: PathBuf, dst_path: String) {
    let data = match fs::read(&local_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to read {:?}: {e}", local_path);
            exit(1);
        }
    };

    let cfg = QuarxConfig::load();

    let store = match FileBlockStore::open(store_path.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to open store {:?}: {e:?}", store_path);
            exit(1);
        }
    };
    let mut repo = QuarxRepo::with_chunk(store, cfg.l0_chunk as usize);

    let name = match split_single_level(&dst_path) {
        Some(n) => n,
        None => {
            eprintln!("Destination path must be '/name', got '{}'", dst_path);
            exit(1);
        }
    };

    let root_id = match quarxdrive::fs_root::ensure_root_dir(&store_path, &mut repo) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("ensure_root_dir failed: {e}");
            exit(1);
        }
    };

    let obj_id = match repo.put_bytes_simple(&data) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("put_bytes_simple (fs-put) failed: {e:?}");
            exit(1);
        }
    };

    let mut entries = match repo.read_dir(root_id) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("read_dir(root) failed: {e:?}");
            exit(1);
        }
    };

    let mut updated = false;
    for e in &mut entries {
        if e.name == name {
            e.target = obj_id;
            updated = true;
            break;
        }
    }
    if !updated {
        entries.push(DirEntry {
            name: name.clone(),
            target: obj_id,
        });
    }

    let new_root_id = match repo.put_dir(&entries) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("put_dir(updated root) failed: {e:?}");
            exit(1);
        }
    };

    if let Err(e) = quarxdrive::fs_root::set_root_dir_id(&store_path, &mut repo, new_root_id) {
        eprintln!("set_root_dir_id failed: {e}");
        exit(1);
    }

    println!("{dst_path} -> object {obj_id} (root={new_root_id})");
}

// fs-get
fn fs_cmd_get(store_path: PathBuf, path: String, out_path: PathBuf) {
    let cfg = QuarxConfig::load();

    let store = match FileBlockStore::open(store_path.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to open store {:?}: {e:?}", store_path);
            exit(1);
        }
    };
    let mut repo = QuarxRepo::with_chunk(store, cfg.l0_chunk as usize);

    let name = match split_single_level(&path) {
        Some(n) => n,
        None => {
            eprintln!("FS-get expects '/name', got '{}'", path);
            exit(1);
        }
    };

    let root_id = match quarxdrive::fs_root::ensure_root_dir(&store_path, &mut repo) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("ensure_root_dir failed: {e}");
            exit(1);
        }
    };

    let entries = match repo.read_dir(root_id) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("read_dir(root) failed: {e:?}");
            exit(1);
        }
    };

    let target = match entries.iter().find(|e| e.name == name) {
        Some(e) => e.target,
        None => {
            eprintln!("No such entry in root: '{}'", name);
            exit(1);
        }
    };

    let data = match repo.get_bytes_simple(target) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("get_bytes_simple({target}) failed: {e:?}");
            exit(1);
        }
    };

    if let Err(e) = fs::write(&out_path, &data) {
        eprintln!("Failed to write {:?}: {e}", out_path);
        exit(1);
    }
}

// fs-ls
fn fs_cmd_ls(store_path: PathBuf, path: String) {
    let cfg = QuarxConfig::load();

    let store = match FileBlockStore::open(store_path.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to open store {:?}: {e:?}", store_path);
            exit(1);
        }
    };
    let mut repo = QuarxRepo::with_chunk(store, cfg.l0_chunk as usize);

    let root_id = match quarxdrive::fs_root::ensure_root_dir(&store_path, &mut repo) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("ensure_root_dir failed: {e}");
            exit(1);
        }
    };

    if split_single_level(&path).is_some() && path != "/" {
        eprintln!(
            "fs-ls: only root '/' is supported in flat mode (got '{}')",
            path
        );
        exit(1);
    }

    let entries = match repo.read_dir(root_id) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("read_dir(root) failed: {e:?}");
            exit(1);
        }
    };

    for e in entries {
        println!("{}", e.name);
    }
}

// fs-inspect
fn fs_cmd_inspect(store_path: PathBuf, path: String) {
    let cfg = QuarxConfig::load();

    let store = match FileBlockStore::open(store_path.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to open store {:?}: {e:?}", store_path);
            exit(1);
        }
    };
    let mut repo = QuarxRepo::with_chunk(store, cfg.l0_chunk as usize);

    let name = match split_single_level(&path) {
        Some(n) => n,
        None => {
            eprintln!("fs-inspect expects '/name', got '{}'", path);
            exit(1);
        }
    };

    let root_id = match quarxdrive::fs_root::ensure_root_dir(&store_path, &mut repo) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("ensure_root_dir failed: {e}");
            exit(1);
        }
    };

    let entries = match repo.read_dir(root_id) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("read_dir(root) failed: {e:?}");
            exit(1);
        }
    };

    let target = match entries.iter().find(|e| e.name == name) {
        Some(e) => e.target,
        None => {
            eprintln!("No such entry in root: '{}'", name);
            exit(1);
        }
    };

    cmd_inspect(store_path, target);
}

// fs-tree (UID FS TREE)
fn fs_cmd_tree(store_path: PathBuf) {
    let cfg = QuarxConfig::load();

    let store = match FileBlockStore::open(store_path.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to open store {:?}: {e:?}", store_path);
            exit(1);
        }
    };
    let mut repo: QuarxRepo<FileBlockStore> = QuarxRepo::with_chunk(store, cfg.l0_chunk as usize);

    let (root_id, entries) = match quarxdrive::fs_tree::build_root_tree(&store_path, &mut repo) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("fs-tree: {e}");
            exit(1);
        }
    };

    println!("ROOT / (dir_id={root_id})");
    println!("UID\tTYPE\tNAME");

    use quarxdrive::fs_tree::FsEntryKind;
    for e in entries {
        let t = match e.kind {
            FsEntryKind::File => "file",
            FsEntryKind::Dir => "dir",
            FsEntryKind::Object => "obj",
            FsEntryKind::Unknown => "unknown",
        };
        println!("{}\t{}\t{}", e.uid, t, e.name);
    }
}

// fs-import
fn fs_cmd_import(store_path: PathBuf, host_dir: PathBuf, debug_import: bool) {
    let cfg = QuarxConfig::load();

    let store = match FileBlockStore::open(store_path.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to open store {:?}: {e:?}", store_path);
            exit(1);
        }
    };
    let mut repo: QuarxRepo<FileBlockStore> = QuarxRepo::with_chunk(store, cfg.l0_chunk as usize);

    let host_dir_str = host_dir.to_string_lossy().to_string();

    println!("fs-import: host dir {:?} -> QuarX root", host_dir);

    let result = quarxdrive::fs_import::import_host_dir(
        &cfg,
        &store_path,
        &mut repo,
        &host_dir,
        debug_import,
    );

    match result {
        Ok((new_root_id, stats)) => {
            println!("  new_root_id = {}", new_root_id);
            println!("  files       = {}", stats.files);
            println!("  dirs        = {}", stats.dirs);
            println!("  skipped_zero     = {}", stats.skipped_zero);
            println!("  skipped_symlink  = {}", stats.skipped_symlink);
            println!("  skipped_hidden   = {}", stats.skipped_hidden);
            println!("  skipped_devices  = {}", stats.skipped_devices);
            println!("  skipped_special  = {}", stats.skipped_special);
            println!("  skipped_other    = {}", stats.skipped_other);
        }
        Err(e) => {
            eprintln!("fs-import failed for {:?}: {e}", host_dir_str);
            exit(1);
        }
    }
}

/// Print current config (without accessing store file).
/// Print current config (without accessing store file).
#[allow(dead_code)]
fn cmd_cfg() {
    cfg_show();
}

fn config_path() -> Option<PathBuf> {
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg).join("quarxtor").join("quarxctl.ini"));
    }
    env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".config").join("quarxtor").join("quarxctl.ini"))
}

fn state_dir() -> PathBuf {
    if let Ok(base) = env::var("XDG_STATE_HOME") {
        return PathBuf::from(base).join("quarxtor");
    }
    let home = env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".local").join("state").join("quarxtor")
}

fn ensure_state_base() -> PathBuf {
    let primary = state_dir();
    if fs::create_dir_all(&primary).is_ok() {
        return primary;
    }
    let fallback = PathBuf::from("/tmp").join("quarxtor-state");
    let _ = fs::create_dir_all(&fallback);
    fallback
}

fn cmd_cfg_dump() -> ! {
    let path = match config_path() {
        Some(p) => p,
        None => {
            println!("CONFIG FILE NOT FOUND");
            exit(1);
        }
    };
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => {
            println!("CONFIG FILE NOT FOUND");
            exit(1);
        }
    };

    let mut section: Option<String> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            section = Some(trimmed[1..trimmed.len() - 1].trim().to_ascii_lowercase());
            continue;
        }
        let eq = match trimmed.find('=') {
            Some(i) => i,
            None => continue,
        };
        let key = trimmed[..eq].trim();
        let val = trimmed[eq + 1..].trim();
        let sec = match section.as_deref() {
            Some(s) => s,
            None => continue,
        };
        let key_l = key.to_ascii_lowercase();
        let recognized = match sec {
            "core" => matches!(
                key_l.as_str(),
                "l0_chunk"
                    | "l0.chunk"
                    | "ram.limit"
                    | "ram_limit"
                    | "analysis.enable_znode"
                    | "analysis.enable_z"
                    | "analysis.fs_stats_fallback"
                    | "analysis.fs_stats.fallback"
                    | "repo_root"
            ),
            "paths" => matches!(
                key_l.as_str(),
                "data_dir" | "cache_dir" | "log_dir" | "runtime_dir"
            ),
            "import" => matches!(
                key_l.as_str(),
                "min_file_size"
                    | "root"
                    | "skip_hidden"
                    | "skip_symlink"
                    | "skip_zero"
                    | "skip_devices"
                    | "skip_special"
                    | "include_glob"
                    | "exclude_glob"
                    | "include_dir_glob"
                    | "exclude_dir_glob"
                    | "import_glob_mode"
                    | "use_z"
                    | "z_threshold"
            ),
            _ => false,
        };
        if recognized {
            println!("{sec}.{key} = {val}");
        }
    }
    exit(0);
}

fn daemon_paths() -> (PathBuf, PathBuf) {
    let base = ensure_state_base();
    (base.join("quarxnetd.pid"), base.join("quarxnetd.log"))
}

fn read_pid(pid_path: &PathBuf) -> Option<i32> {
    fs::read_to_string(pid_path)
        .ok()
        .and_then(|s| s.trim().parse::<i32>().ok())
}

fn pid_alive(pid: i32) -> bool {
    let proc_path = PathBuf::from("/proc").join(pid.to_string());
    proc_path.exists()
}

fn transport_ready(cfg: &EffectiveConfig, timeout_ms: u64) -> bool {
    if cfg.net_transport == "unix" {
        #[cfg(unix)]
        {
            let path = PathBuf::from(&cfg.net_unix_path);
            let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
            while std::time::Instant::now() < deadline {
                if path.exists() {
                    return true;
                }
                sleep(Duration::from_millis(100));
            }
            return false;
        }
        #[cfg(not(unix))]
        {
            return false;
        }
    }
    check_listen(&cfg.net_addr, timeout_ms)
}

fn check_listen(addr: &str, timeout_ms: u64) -> bool {
    let mut parts = addr.split(':');
    let host = parts.next().unwrap_or("127.0.0.1");
    let port = parts
        .next()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(54721);
    let sockaddr = format!("{host}:{port}");
    let socket_addr = match sockaddr.parse() {
        Ok(a) => a,
        Err(_) => return false,
    };
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    while std::time::Instant::now() < deadline {
        if TcpStream::connect_timeout(&socket_addr, Duration::from_millis(200)).is_ok() {
            return true;
        }
        sleep(Duration::from_millis(100));
    }
    false
}

fn net_start() -> ! {
    let cfg = match EffectiveConfig::load_default() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            exit(1);
        }
    };
    let (pid_path, log_path) = daemon_paths();
    if let Some(pid) = read_pid(&pid_path) {
        if pid_alive(pid) && transport_ready(&cfg, 500) {
            let listen = if cfg.net_transport == "unix" {
                cfg.net_unix_path.clone()
            } else {
                cfg.net_addr.clone()
            };
            println!("RUNNING pid={} listen={}", pid, listen);
            exit(0);
        }
    }

    if let Some(parent) = pid_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Some(parent) = log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let exe_dir = env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("target/debug"));
    let netd = exe_dir.join("quarxnetd");

    let mut log_file = File::create(&log_path).unwrap_or_else(|e| {
        eprintln!("failed to open log file {:?}: {e}", log_path);
        exit(1);
    });

    let child = match std::process::Command::new(netd)
        .stdout(Stdio::from(log_file.try_clone().unwrap()))
        .stderr(Stdio::from(log_file.try_clone().unwrap()))
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to start quarxnetd: {e}");
            exit(1);
        }
    };
    let pid = child.id() as i32;
    let _ = writeln!(log_file, "spawned pid={}", pid);
    let _ = fs::write(&pid_path, pid.to_string());

    // detach: drop child handle
    drop(child);

    if transport_ready(&cfg, 3000) {
        let listen = if cfg.net_transport == "unix" {
            cfg.net_unix_path.clone()
        } else {
            cfg.net_addr.clone()
        };
        println!("RUNNING pid={} listen={}", pid, listen);
        exit(0);
    }

    let listen = if cfg.net_transport == "unix" {
        cfg.net_unix_path.clone()
    } else {
        cfg.net_addr.clone()
    };
    eprintln!("quarxnetd failed to start or listen on {}", listen);
    let _ = fs::remove_file(&pid_path);
    exit(1);
}

fn net_status() -> ! {
    let cfg = match EffectiveConfig::load_default() {
        Ok(c) => c,
        Err(_) => {
            println!("NOT_RUNNING");
            exit(1);
        }
    };
    let (pid_path, _) = daemon_paths();
    if let Some(pid) = read_pid(&pid_path) {
        if pid_alive(pid) && transport_ready(&cfg, 500) {
            let listen = if cfg.net_transport == "unix" {
                cfg.net_unix_path.clone()
            } else {
                cfg.net_addr.clone()
            };
            println!("RUNNING pid={} listen={}", pid, listen);
            exit(0);
        }
    }
    println!("NOT_RUNNING");
    exit(1);
}

fn net_stop() -> ! {
    let cfg = match EffectiveConfig::load_default() {
        Ok(c) => c,
        Err(_) => {
            println!("NOT_RUNNING");
            exit(0);
        }
    };
    let (pid_path, _) = daemon_paths();
    if let Some(pid) = read_pid(&pid_path) {
        let _ = std::process::Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status();
        let deadline = std::time::Instant::now() + Duration::from_millis(3000);
        while std::time::Instant::now() < deadline {
            if !pid_alive(pid) && !transport_ready(&cfg, 100) {
                break;
            }
            sleep(Duration::from_millis(100));
        }
    }
    let _ = fs::remove_file(&pid_path);
    if transport_ready(&cfg, 200) {
        let listen = if cfg.net_transport == "unix" {
            cfg.net_unix_path.clone()
        } else {
            cfg.net_addr.clone()
        };
        println!("RUNNING pid=? listen={}", listen);
        exit(1);
    }
    if cfg.net_transport == "unix" {
        let _ = fs::remove_file(&cfg.net_unix_path);
    }
    println!("NOT_RUNNING");
    exit(0);
}

fn net_e2e_smoke() -> ! {
    let cfg = match EffectiveConfig::load_default() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            exit(1);
        }
    };
    let (pid_path, _) = daemon_paths();
    let pid = read_pid(&pid_path).filter(|p| pid_alive(*p));
    if pid.is_none() {
        eprintln!("daemon not running");
        exit(1);
    }

    let (sent, recv) = if cfg.net_transport == "unix" {
        #[cfg(unix)]
        {
            match std::os::unix::net::UnixStream::connect(&cfg.net_unix_path) {
                Ok(mut s) => {
                    let hdr = encode_header(1);
                    if let Err(e) = s.write_all(&hdr) {
                        eprintln!("send failed: {e}");
                        exit(1);
                    }
                    let mut resp = [0u8; 8];
                    if let Err(e) = s.read_exact(&mut resp) {
                        eprintln!("recv failed: {e}");
                        exit(1);
                    }
                    (hdr, resp)
                }
                Err(e) => {
                    eprintln!("connect failed (unix): {e}");
                    exit(1);
                }
            }
        }
        #[cfg(not(unix))]
        {
            eprintln!("unix transport unsupported on this platform");
            exit(1);
        }
    } else {
        let addr = cfg.net_addr.clone();
        match TcpStream::connect(&addr) {
            Ok(mut s) => {
                let hdr = encode_header(1);
                if let Err(e) = s.write_all(&hdr) {
                    eprintln!("send failed: {e}");
                    exit(1);
                }
                let mut resp = [0u8; 8];
                if let Err(e) = s.read_exact(&mut resp) {
                    eprintln!("recv failed: {e}");
                    exit(1);
                }
                (hdr, resp)
            }
            Err(e) => {
                eprintln!("connect failed (tcp): {e}");
                exit(1);
            }
        }
    };

    match decode_header(recv) {
        Ok((_ver, opc, len)) if opc == 2 && len == 0 => {}
        Ok((_ver, opc, len)) => {
            eprintln!("unexpected response opc={} len={}", opc, len);
            exit(1);
        }
        Err(e) => {
            eprintln!("{e}");
            exit(1);
        }
    }
    println!("PROTO0_OK");
    eprintln!("SENT={:02x?} RECV={:02x?}", sent, recv);
    exit(0);
}

fn net_selftest_proto0() -> ! {
    #[cfg(unix)]
    {
        let (mut a, mut b) = match std::os::unix::net::UnixStream::pair() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("socketpair failed: {e}");
                exit(1);
            }
        };
        let server = spawn(move || serve_one_proto0(&mut b));

        let sent = encode_header(1);
        if let Err(e) = a.write_all(&sent) {
            eprintln!("send failed: {e}");
            exit(1);
        }
        let mut recv = [0u8; 8];
        if let Err(e) = a.read_exact(&mut recv) {
            eprintln!("recv failed: {e}");
            exit(1);
        }
        if let Err(e) = server.join().unwrap_or_else(|_| Err("server panicked".into())) {
            eprintln!("server error: {e}");
            exit(1);
        }
        match decode_header(recv) {
            Ok((_ver, opc, len)) if opc == 2 && len == 0 => {}
            Ok((_ver, opc, len)) => {
                eprintln!("unexpected response opc={} len={}", opc, len);
                exit(1);
            }
            Err(e) => {
                eprintln!("{e}");
                exit(1);
            }
        }
        println!("PROTO0_OK");
        eprintln!("SENT={:02x?} RECV={:02x?}", sent, recv);
        exit(0);
    }
    #[cfg(not(unix))]
    {
        eprintln!("proto0 selftest requires unix socketpair");
        exit(1);
    }
}

// FS-like: recursive FS stats from root.
fn fs_cmd_stats(store_path: PathBuf) {
    let cfg = QuarxConfig::load();

    let store = match FileBlockStore::open(store_path.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to open store {:?}: {e:?}", store_path);
            exit(1);
        }
    };
    let mut repo: QuarxRepo<FileBlockStore> = QuarxRepo::with_chunk(store, cfg.l0_chunk as usize);

    let (root_id, stats) =
        match quarxdrive::fs_stats::compute_root_stats(&store_path, &mut repo, &cfg) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("fs-stats: {e}");
                exit(1);
            }
        };

    println!("FS stats from root dir_id={root_id}:");
    println!("  dirs  : {}", stats.dirs);
    println!("  files : {}", stats.files);
    println!("  bytes : {}", stats.bytes);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() == 2 && args[1] == "--print-ts" {
        println!("{} [quarxctl] [INFO] local-time-ok", ts_local());
        return;
    }
    // special-case: `quarxctl cfg` (no store-file needed)
    if args.len() >= 2 && args[1] == "cfg" {
        if args.len() == 2 || (args.len() == 3 && args[2] == "show") {
            cfg_show();
        }
        if args.len() == 3 && args[2] == "describe" {
            cfg_describe();
        }
        if args.len() == 3 && args[2] == "validate" {
            cfg_validate();
        }
        if args.len() == 3 && args[2] == "dump" {
            cmd_cfg_dump();
        }
        if args.len() >= 3 && args[2] == "init" {
            let force = args.len() == 4 && args[3] == "--force";
            if args.len() == 3 || force {
                cfg_init(force);
            }
        }
        usage();
    }

    // net subcommands
    if args.len() >= 2 && args[1] == "net" {
        if args.len() == 3 && args[2] == "start" {
            net_start();
        }
        if args.len() == 3 && args[2] == "status" {
            net_status();
        }
        if args.len() == 3 && args[2] == "stop" {
            net_stop();
        }
        if args.len() == 4 && args[2] == "e2e" && args[3] == "--smoke" {
            net_e2e_smoke();
        }
        if args.len() == 4 && args[2] == "selftest" && args[3] == "--proto0" {
            net_selftest_proto0();
        }
        usage();
    }

    if args.len() < 3 {
        usage();
    }

    let store_path = PathBuf::from(&args[1]);
    let cmd = args[2].as_str();

    match cmd {
        // low-level
        "put" => {
            if args.len() != 4 {
                usage();
            }
            let local_path = PathBuf::from(&args[3]);
            cmd_put(store_path, local_path);
        }

        "get" => {
            if args.len() != 5 {
                usage();
            }
            let obj_id = parse_id(&args[3], "obj-id");
            let out_path = PathBuf::from(&args[4]);
            cmd_get(store_path, obj_id, out_path);
        }

        "inspect" => {
            if args.len() != 4 {
                usage();
            }
            let root_id = parse_id(&args[3], "root-id");
            cmd_inspect(store_path, root_id);
        }

        // FS-like
        "fs-put" => {
            if args.len() != 5 {
                usage();
            }
            let local_path = PathBuf::from(&args[3]);
            let dst_path = args[4].clone();
            fs_cmd_put(store_path, local_path, dst_path);
        }

        "fs-get" => {
            if args.len() != 5 {
                usage();
            }
            let path = args[3].clone();
            let out_path = PathBuf::from(&args[4]);
            fs_cmd_get(store_path, path, out_path);
        }

        "fs-ls" => {
            if args.len() != 4 {
                usage();
            }
            let path = args[3].clone();
            fs_cmd_ls(store_path, path);
        }

        "fs-inspect" => {
            if args.len() != 4 {
                usage();
            }
            let path = args[3].clone();
            fs_cmd_inspect(store_path, path);
        }

        "fs-tree" => {
            if args.len() != 3 {
                usage();
            }
            fs_cmd_tree(store_path);
        }

        "fs-import" => {
            // fs-import [--debug-import] <host_dir>
            if args.len() < 4 || args.len() > 5 {
                usage();
            }
            let (host_dir, debug_import) = if args.len() == 4 {
                (PathBuf::from(&args[3]), false)
            } else if args[3] == "--debug-import" {
                (PathBuf::from(&args[4]), true)
            } else {
                usage();
            };
            fs_cmd_import(store_path, host_dir, debug_import);
        }

        "fs-stats" => {
            if args.len() != 3 {
                usage();
            }
            fs_cmd_stats(store_path);
        }
        _ => usage(),
    }
}
