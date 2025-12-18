use crate::fs_root::ensure_root_dir;
use crate::{DirEntry, QuarxRepo, OBJ_TYPE_BYTES, OBJ_TYPE_DIR};
use quarxtor_core::store::blockstore::BlockStore;
use quarxtor_core::store::decode::BlockBody;
use quarxtor_core::types::BlockKind;
use std::path::Path;

/// Root-level FS entry description.
#[derive(Debug, Clone)]
pub struct FsEntry {
    pub uid: u64,
    pub name: String,
    pub kind: FsEntryKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsEntryKind {
    File,
    Dir,
    Object,
    Unknown,
}

/// Build UID-FS tree for root (currently single level).
///
/// Returns:
///   (root_dir_id, Vec<FsEntry>)
pub fn build_root_tree<S: BlockStore>(
    store_path: &Path,
    repo: &mut QuarxRepo<S>,
) -> Result<(u64, Vec<FsEntry>), String> {
    let root_id = ensure_root_dir(store_path, repo)?;

    let entries: Vec<DirEntry> = repo
        .read_dir(root_id)
        .map_err(|e| format!("read_dir(root) failed: {e:?}"))?;

    let store = repo.store();
    let mut out = Vec::with_capacity(entries.len());

    for e in entries {
        let uid = e.target;
        let (kind, _h, body) = match store.get_typed(uid) {
            Ok(v) => v,
            Err(err) => {
                out.push(FsEntry {
                    uid,
                    name: e.name,
                    kind: FsEntryKind::Unknown,
                });
                eprintln!("build_root_tree: get_typed({uid}) failed: {err:?}");
                continue;
            }
        };

        let mut t = FsEntryKind::Unknown;

        if let (BlockKind::Object, BlockBody::Object(op)) = (kind, body) {
            t = match op.obj_type {
                OBJ_TYPE_BYTES => FsEntryKind::File,
                OBJ_TYPE_DIR => FsEntryKind::Dir,
                _ => FsEntryKind::Object,
            };
        }

        out.push(FsEntry {
            uid,
            name: e.name,
            kind: t,
        });
    }

    Ok((root_id, out))
}
