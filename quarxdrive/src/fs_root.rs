use crate::{DirEntry, QuarxRepo};
use quarxtor_core::store::blockstore::BlockStore;

use std::fs;
use std::path::{Path, PathBuf};

fn root_sidecar_path(store_path: &Path) -> PathBuf {
    let mut p = store_path.to_owned();
    p.set_extension("qroot");
    p
}

fn read_root_id(store_path: &Path) -> Option<u64> {
    let p = root_sidecar_path(store_path);
    let text = fs::read_to_string(&p).ok()?;
    text.trim().parse::<u64>().ok()
}

fn write_root_id(store_path: &Path, id: u64) -> Result<(), String> {
    let p = root_sidecar_path(store_path);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create_dir_all({:?}): {e}", parent))?;
    }
    fs::write(&p, format!("{id}\n")).map_err(|e| format!("write_root_id({:?}): {e}", p))?;
    Ok(())
}

/// Get root-dir id if it exists (no creation).
pub fn get_root_dir_id<S: BlockStore>(
    store_path: &Path,
    _repo: &mut QuarxRepo<S>,
) -> Result<u64, String> {
    if let Some(id) = read_root_id(store_path) {
        Ok(id)
    } else {
        Err("root-dir not initialized".to_string())
    }
}

/// Update root-dir id (rewrite sidecar).
pub fn set_root_dir_id<S: BlockStore>(
    store_path: &Path,
    _repo: &mut QuarxRepo<S>,
    id: u64,
) -> Result<(), String> {
    write_root_id(store_path, id)
}

/// Get root-dir; if absent — create empty directory and persist id.
pub fn ensure_root_dir<S: BlockStore>(
    store_path: &Path,
    repo: &mut QuarxRepo<S>,
) -> Result<u64, String> {
    if let Some(id) = read_root_id(store_path) {
        return Ok(id);
    }

    let entries: Vec<DirEntry> = Vec::new();
    let dir_id = repo
        .put_dir(&entries)
        .map_err(|e| format!("put_dir(empty root) failed: {e:?}"))?;

    write_root_id(store_path, dir_id)?;
    Ok(dir_id)
}
