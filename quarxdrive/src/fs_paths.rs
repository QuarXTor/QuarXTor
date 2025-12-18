use std::path::Path;

use crate::fs_root::ensure_root_dir;
use crate::{DirEntry, QuarxRepo, OBJ_TYPE_DIR};
use quarxtor_core::store::blockstore::BlockStore;
use quarxtor_core::store::decode::BlockBody;
use quarxtor_core::types::BlockKind;

/// Normalize path into component list.
/// "" and "/" -> empty list (root).
fn normalize_components(path: &str) -> Vec<&str> {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return Vec::new();
    }
    trimmed
        .trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect()
}

/// Resolve path to a directory (for future nesting support).
/// Not used from CLI now, but suitable for FUSE/services.
pub fn resolve_dir<S: BlockStore>(
    store_path: &Path,
    repo: &mut QuarxRepo<S>,
    path: &str,
) -> Result<u64, String> {
    let comps = normalize_components(path);
    if comps.is_empty() {
        return ensure_root_dir(store_path, repo);
    }

    let mut dir_id = ensure_root_dir(store_path, repo)?;
    for comp in comps {
        let entries = repo
            .read_dir(dir_id)
            .map_err(|e| format!("read_dir({dir_id}) failed: {e:?}"))?;
        let ent = entries
            .into_iter()
            .find(|e| e.name == comp)
            .ok_or_else(|| format!("no such entry '{comp}' in dir {dir_id}"))?;

        let (kind, _, body) = repo
            .store()
            .get_typed(ent.target)
            .map_err(|e| format!("get_typed({}) failed: {e:?}", ent.target))?;

        match (kind, body) {
            (BlockKind::Object, BlockBody::Object(op)) if op.obj_type == OBJ_TYPE_DIR => {
                dir_id = ent.target;
            }
            _ => {
                return Err(format!(
                    "entry '{comp}' is not a directory (id={})",
                    ent.target
                ))
            }
        }
    }

    Ok(dir_id)
}

/// For writes in current flat mode: path must be `/name` (single component under root).
///
/// Returns (parent dir id, file name).
pub fn resolve_parent_flat<S: BlockStore>(
    store_path: &Path,
    repo: &mut QuarxRepo<S>,
    path: &str,
) -> Result<(u64, String), String> {
    let comps = normalize_components(path);
    if comps.len() != 1 {
        return Err(format!(
            "nested paths are not supported yet in flat mode (got '{path}')",
        ));
    }
    let name = comps[0].to_string();
    let dir_id = ensure_root_dir(store_path, repo)?;
    Ok((dir_id, name))
}

/// Find a file object under flat root `/name`.
///
/// Returns (dir id, file id).
pub fn resolve_file_flat<S: BlockStore>(
    store_path: &Path,
    repo: &mut QuarxRepo<S>,
    path: &str,
) -> Result<(u64, u64), String> {
    let (dir_id, name) = resolve_parent_flat(store_path, repo, path)?;

    let entries: Vec<DirEntry> = repo
        .read_dir(dir_id)
        .map_err(|e| format!("read_dir(root) failed: {e:?}"))?;

    let ent = entries
        .into_iter()
        .find(|e| e.name == name)
        .ok_or_else(|| format!("no such entry in root: '{name}'"))?;

    Ok((dir_id, ent.target))
}
