use smallvec::{smallvec, SmallVec};

use quarxtor_core::block::multi::MultiRecipe;
use quarxtor_core::codec::{ObjectPayload, ZPayload};
use quarxtor_core::store::blockstore::{BlockStore, StoreError, StoreResult};
use quarxtor_core::store::decode::BlockBody;
use quarxtor_core::types::{BlockId, BlockKind, BlockRef};

/// Object type "raw bytes".
pub const OBJ_TYPE_BYTES: u32 = 1;
/// Object type "directory".
pub const OBJ_TYPE_DIR: u32 = 2;
/// Object type "Z-node" (cheap-size / analytics).
/// Directory entry: name -> BlockId of target Object block.
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub target: BlockId,
}

/// Simple repository "bytes/directories <-> block DAG" on top of BlockStore.
///
/// For files:
///   data -> [L0...] -> Z(first_l0..last_l0) -> Multi(Aggregate{[Z]}) -> Object(type=BYTES, root=Multi)
///
/// For directories:
///   DirEntry[target=*] -> Multi(Aggregate{ [target_ids...] }) -> Object(type=DIR, root=Multi, meta=dir-entries)
pub struct QuarxRepo<S: BlockStore> {
    store: S,
    l0_chunk: usize,
}

impl<S: BlockStore> QuarxRepo<S> {
    /// Create repository with default L0 chunk size (8 KiB).
    pub fn new(store: S) -> Self {
        Self {
            store,
            l0_chunk: 8 * 1024,
        }
    }

    /// Set arbitrary L0 chunk size.
    pub fn with_chunk(store: S, l0_chunk: usize) -> Self {
        Self { store, l0_chunk }
    }

    /// Extract the underlying store.
    pub fn into_store(self) -> S {
        self.store
    }

    /// Write arbitrary buffer as a "raw bytes" object.
    ///
    /// Returns BlockId of root Object block.
    pub fn put_bytes_simple(&mut self, data: &[u8]) -> StoreResult<BlockId> {
        if data.is_empty() {
            // Empty object — currently plain empty Multi without L0/Z.
            let recipe = MultiRecipe::Aggregate {
                blocks: smallvec![],
            };
            let multi_id = self.store.put_multi(&recipe)?;

            let obj = ObjectPayload {
                root: BlockRef::Multi(multi_id),
                obj_type: OBJ_TYPE_BYTES,
                meta: Vec::new(),
            };
            let obj_id = self.store.put_object(&obj)?;
            return Ok(obj_id);
        }

        // 1) split into L0 chunks
        let mut l0_ids: SmallVec<[BlockId; 8]> = smallvec![];
        let mut offset = 0;
        while offset < data.len() {
            let end = usize::min(offset + self.l0_chunk, data.len());
            let chunk = &data[offset..end];
            let id = self.store.put_l0(chunk)?;
            l0_ids.push(id);
            offset = end;
        }

        // 2) Z-block over range [first_l0..last_l0]
        let first_l0 = *l0_ids.first().expect("non-empty data must produce L0");
        let last_l0 = *l0_ids.last().expect("non-empty data must produce L0");

        let z = ZPayload {
            first_l0,
            last_l0,
            // Base Z type; can be split later by types/structures
            z_type: 1,
            meta: Vec::new(),
        };
        let z_id = self.store.put_z(&z)?;

        // 3) Multi(Aggregate) over Z (mixed L0+Z may be added later)
        let recipe = MultiRecipe::Aggregate {
            blocks: smallvec![z_id],
        };
        let multi_id = self.store.put_multi(&recipe)?;

        // 4) Object(root=Multi)
        let obj = ObjectPayload {
            root: BlockRef::Multi(multi_id),
            obj_type: OBJ_TYPE_BYTES,
            meta: Vec::new(),
        };
        let obj_id = self.store.put_object(&obj)?;

        Ok(obj_id)
    }

    /// Read "raw bytes" object by root Object BlockId.
    ///
    /// Supports:
    ///   Multi(Aggregate{ L0... })
    ///   Multi(Aggregate{ Z... })
    ///   Multi(Aggregate{ any mix of L0/Z })
    pub fn get_bytes_simple(&self, root_obj_block: BlockId) -> StoreResult<Vec<u8>> {
        // 1) Object
        let (kind_obj, _h_obj, body_obj) = self.store.get_typed(root_obj_block)?;
        if !matches!(kind_obj, BlockKind::Object) {
            return Err(StoreError::Corrupt(format!(
                "get_bytes_simple: root {} is not Object (kind={:?})",
                root_obj_block, kind_obj
            )));
        }

        let obj = match body_obj {
            BlockBody::Object(op) => op,
            _ => {
                return Err(StoreError::Corrupt(
                    "get_bytes_simple: expected Object body".into(),
                ))
            }
        };

        if obj.obj_type != OBJ_TYPE_BYTES {
            return Err(StoreError::Corrupt(format!(
                "get_bytes_simple: unsupported obj_type {}",
                obj.obj_type
            )));
        }

        // root must be Multi(...)
        let multi_id = match obj.root {
            BlockRef::Multi(mid) => mid,
            _ => {
                return Err(StoreError::Corrupt(
                    "get_bytes_simple: root is not Multi(..)".into(),
                ))
            }
        };

        // 2) Multi
        let (kind_multi, _h_multi, body_multi) = self.store.get_typed(multi_id)?;
        if !matches!(kind_multi, BlockKind::Multi) {
            return Err(StoreError::Corrupt(format!(
                "get_bytes_simple: multi {} is not Multi (kind={:?})",
                multi_id, kind_multi
            )));
        }

        let recipe = match body_multi {
            BlockBody::Multi(r) => r,
            _ => {
                return Err(StoreError::Corrupt(
                    "get_bytes_simple: expected Multi body".into(),
                ))
            }
        };

        let mut out = Vec::new();

        match recipe {
            MultiRecipe::Aggregate { blocks } => {
                for &bid in blocks.iter() {
                    let (k_b, _h_b, body_b) = self.store.get_typed(bid)?;
                    match (k_b, body_b) {
                        // Direct L0 in Multi list
                        (BlockKind::L0, BlockBody::L0(raw)) => {
                            out.extend_from_slice(&raw);
                        }

                        // Z in Multi list — expand L0 range
                        (BlockKind::Z, BlockBody::Z(zp)) => {
                            if zp.last_l0 < zp.first_l0 {
                                // inconsistent Z — skip for now (could be strict)
                                continue;
                            }
                            let mut cur = zp.first_l0;
                            loop {
                                let (k_l0, _h_l0, body_l0) = self.store.get_typed(cur)?;
                                match (k_l0, body_l0) {
                                    (BlockKind::L0, BlockBody::L0(raw)) => {
                                        out.extend_from_slice(&raw);
                                    }
                                    _ => {
                                        return Err(StoreError::Corrupt(format!(
                                            "get_bytes_simple: expected L0 in Z-range, got kind {:?} at id {}",
                                            k_l0, cur
                                        )));
                                    }
                                }
                                if cur == zp.last_l0 {
                                    break;
                                }
                                cur = cur.saturating_add(1);
                            }
                        }

                        // Other cases not supported yet
                        _ => {
                            return Err(StoreError::Corrupt(format!(
                                "get_bytes_simple: unsupported child kind {:?} in Multi Aggregate",
                                k_b
                            )));
                        }
                    }
                }
            }

            // Other modes not implemented yet
            _ => {
                return Err(StoreError::Corrupt(
                    "get_bytes_simple: unsupported MultiRecipe (expected Aggregate)".into(),
                ));
            }
        }

        Ok(out)
    }

    /// Create directory from DirEntry set.
    ///
    /// Multi(Aggregate{targets}) + Object(type=DIR, meta=dir-entries).
    pub fn put_dir(&mut self, entries: &[DirEntry]) -> StoreResult<BlockId> {
        // collect target_id list into Multi
        let mut ids: SmallVec<[BlockId; 8]> = SmallVec::new();
        for e in entries {
            ids.push(e.target);
        }

        let recipe = MultiRecipe::Aggregate { blocks: ids };
        let multi_id = self.store.put_multi(&recipe)?;

        let meta = encode_dir_meta(entries);

        let obj = ObjectPayload {
            root: BlockRef::Multi(multi_id),
            obj_type: OBJ_TYPE_DIR,
            meta,
        };
        let obj_id = self.store.put_object(&obj)?;

        Ok(obj_id)
    }

    /// Read directory: return DirEntry list by directory Object id.
    pub fn read_dir(&self, dir_obj_id: BlockId) -> StoreResult<Vec<DirEntry>> {
        let (kind_obj, _h_obj, body_obj) = self.store.get_typed(dir_obj_id)?;
        if !matches!(kind_obj, BlockKind::Object) {
            return Err(StoreError::Corrupt(format!(
                "read_dir: root {} is not Object (kind={:?})",
                dir_obj_id, kind_obj
            )));
        }

        let obj = match body_obj {
            BlockBody::Object(op) => op,
            _ => {
                return Err(StoreError::Corrupt("read_dir: expected Object body".into()));
            }
        };

        if obj.obj_type != OBJ_TYPE_DIR {
            return Err(StoreError::Corrupt(format!(
                "read_dir: obj_type {} is not DIR",
                obj.obj_type
            )));
        }

        decode_dir_meta(&obj.meta)
    }

    /// Access inner store for low-level operations.
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Mutable access to store (e.g., for direct put_* in tests).
    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }
}

/// Simple binary encoding of DirEntry into Object.meta:
///   [ u16 name_len ][ name_bytes ][ u64 target ] * N
fn encode_dir_meta(entries: &[DirEntry]) -> Vec<u8> {
    let mut out = Vec::new();
    for e in entries {
        let name_bytes = e.name.as_bytes();
        if name_bytes.len() > u16::MAX as usize {
            // At this level a panic is acceptable — developer error.
            panic!("encode_dir_meta: name too long (> u16::MAX)");
        }
        let len = name_bytes.len() as u16;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(&e.target.to_be_bytes());
    }
    out
}

fn decode_dir_meta(meta: &[u8]) -> StoreResult<Vec<DirEntry>> {
    let mut pos = 0usize;
    let mut out = Vec::new();

    while pos < meta.len() {
        if pos + 2 > meta.len() {
            return Err(StoreError::Corrupt(
                "decode_dir_meta: truncated name_len".into(),
            ));
        }
        let len = u16::from_be_bytes([meta[pos], meta[pos + 1]]) as usize;
        pos += 2;

        if pos + len + 8 > meta.len() {
            return Err(StoreError::Corrupt(
                "decode_dir_meta: truncated name/target".into(),
            ));
        }

        let name_bytes = &meta[pos..pos + len];
        pos += len;

        let name = String::from_utf8(name_bytes.to_vec())
            .map_err(|_| StoreError::Corrupt("decode_dir_meta: invalid UTF-8 in name".into()))?;

        let mut id_bytes = [0u8; 8];
        id_bytes.copy_from_slice(&meta[pos..pos + 8]);
        pos += 8;
        let target = u64::from_be_bytes(id_bytes);

        out.push(DirEntry { name, target });
    }

    Ok(out)
}

pub mod fs_root;

pub mod fs_tree;

pub mod fs_paths;

pub mod fs_import;
pub mod repo;

pub mod fs_stats;

pub mod net_backend;
pub mod net_backend_fs;

pub use net_backend::NetBackend;
pub use net_backend_fs::FsNetBackend;

pub mod net_backend_repo;
pub use net_backend_repo::RepoNetBackend;
pub use repo::{FileBackend, InMemBackend, QuarxRepo as Repo, RepoBackend};

pub use repo::znode::ZNode;
