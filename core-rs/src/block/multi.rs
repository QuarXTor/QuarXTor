use smallvec::SmallVec;

use crate::types::{BlockId, ClusterId, CodecId, DictId, ObjectId};

/// Reference to a codec (abstract, cluster-oriented).
/// Stable on-disk ABI helper embedded into multiblock recipes.
#[derive(Clone, Debug)]
pub struct CodecRef {
    pub codec_id: CodecId,
    pub cluster: Option<ClusterId>,
    // TODO: version, ABI, OS-mask, etc.
}

/// Reference to a dictionary inside a codec.
/// Stable on-disk ABI helper embedded into multiblock recipes.
#[derive(Clone, Debug)]
pub struct DictRef {
    pub dict_id: DictId,
    pub cluster: Option<ClusterId>,
    /// Optional reference to an object graph containing the dictionary as data.
    pub object_id: Option<ObjectId>,
}

/// Recipe of a multiblock (logical block > L0).
/// Stable on-disk ABI payload; see `docs/core-formats.md` (Multi blocks).
#[derive(Clone, Debug)]
pub enum MultiRecipe {
    /// Simple aggregate of a sequence of L0 blocks (B0..Bn).
    Aggregate { blocks: SmallVec<[BlockId; 8]> },

    /// Recipe bound to codec/dictionary/cluster (including custom ones).
    CodecRecipe {
        codec: CodecRef,
        dict: Option<DictRef>,

        /// Recipe identifier in the (codec, dict) space.
        recipe_id: u64,

        /// Extra recipe data (parameters, inline structure, opaque payload).
        recipe_data: Option<Vec<u8>>,

        /// Optional fallback link to L0 blocks (B0..Bn).
        blocks: Option<SmallVec<[BlockId; 8]>>,
    },

    /// Reserved for any custom variants (erasure codes, RAID-like, etc.).
    Custom { kind_id: u32, payload: Vec<u8> },
}

/// MultiBlock: multiblock / logical block (e.g., 64K).
/// Stable on-disk ABI structure; see `docs/core-formats.md` (Multi blocks).
#[derive(Clone, Debug)]
pub struct MultiBlock {
    pub id: BlockId,
    pub hash: [u8; 32],   // hash of the logical block
    pub logical_len: u32, // length in bytes (64K, etc.)
    pub recipe: MultiRecipe,
}
