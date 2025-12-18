use crate::fs_root::ensure_root_dir;
use crate::QuarxRepo;
use crate::{OBJ_TYPE_BYTES, OBJ_TYPE_DIR};

use quarxtor_core::config::QuarxConfig;
use quarxtor_core::store::blockstore::BlockStore;
use quarxtor_core::store::decode::BlockBody;
use quarxtor_core::types::BlockKind;

use std::path::Path;

/// Summary statistics over the FS tree.
#[derive(Debug, Clone, Copy, Default)]
pub struct FsStats {
    /// Number of directories (including root).
    pub dirs: u64,
    /// Number of files (Object type BYTES).
    pub files: u64,
    /// Total useful data size (bytes).
    ///
    /// Behavior:
    ///   - while Z-node/cheap-size is not implemented, real size is
    ///     computed only if cfg.analysis_fs_stats_fallback = true;
    ///   - if false — bytes stays 0 (cheap and fast).
    pub bytes: u64,
}

fn walk_dir<S: BlockStore>(
    repo: &mut QuarxRepo<S>,
    dir_id: u64,
    stats: &mut FsStats,
    cfg: &QuarxConfig,
) -> Result<(), String> {
    let entries = repo
        .read_dir(dir_id)
        .map_err(|e| format!("read_dir({dir_id}) failed: {e:?}"))?;

    for e in entries {
        let uid = e.target;

        let mut obj_ty: Option<u32> = None;

        {
            // Short immutable borrow of store inside this block to avoid &mut repo conflict later.
            let store = repo.store();
            let (kind, _hash, body) = store
                .get_typed(uid)
                .map_err(|er| format!("get_typed({uid}) failed: {er:?}"))?;

            if let (BlockKind::Object, BlockBody::Object(op)) = (kind, body) {
                obj_ty = Some(op.obj_type);
            }
        }

        match obj_ty {
            Some(OBJ_TYPE_DIR) => {
                stats.dirs += 1;
                // Recursive traversal after immutable borrow ended.
                walk_dir(repo, uid, stats, cfg)?;
            }
            Some(OBJ_TYPE_BYTES) => {
                stats.files += 1;

                // While cheap-size/Z-node is absent: if fallback enabled,
                // compute size via get_bytes_simple (expensive).
                if cfg.analysis_fs_stats_fallback {
                    let data = repo
                        .get_bytes_simple(uid)
                        .map_err(|er| format!("get_bytes_simple({uid}) failed: {er:?}"))?;
                    stats.bytes += data.len() as u64;
                }
            }
            _ => {
                // Ignore other Object types.
            }
        }
    }

    Ok(())
}

/// Compute stats for the whole FS tree starting from logical root.
///
/// Returns:
///   (root_dir_id, FsStats)
pub fn compute_root_stats<S: BlockStore>(
    store_path: &Path,
    repo: &mut QuarxRepo<S>,
    cfg: &QuarxConfig,
) -> Result<(u64, FsStats), String> {
    let root_id = ensure_root_dir(store_path, repo)?;

    let mut stats = FsStats {
        dirs: 1,
        ..FsStats::default()
    };

    walk_dir(repo, root_id, &mut stats, cfg)?;

    Ok((root_id, stats))
}
