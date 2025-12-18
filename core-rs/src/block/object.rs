use crate::types::{BlockRef, ObjectId};

/// Object: top-level entity (file, snapshot, image, etc.).
/// Stable on-disk ABI structure; see `docs/core-formats.md` (Object blocks).
#[derive(Clone, Debug)]
pub struct Object {
    pub id: ObjectId,
    /// Root block (can be Multi/Z/tree, etc.).
    pub root: BlockRef,
    /// Object type (file, snapshot, vm-image, user-defined, etc.).
    pub obj_type: u32,
    /// Arbitrary metadata (names, timestamps, ACL, user-defined payload).
    pub meta: Vec<u8>,
}
