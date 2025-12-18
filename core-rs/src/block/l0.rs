use crate::types::BlockId;

/// L0Block: atomic physical block (usually 8K).
/// Stable on-disk ABI structure; see `docs/core-formats.md` (L0 blocks).
#[derive(Clone, Debug)]
pub struct L0Block {
    pub id: BlockId,
    pub hash: [u8; 32],
    pub size: u32, // actual length; usually 8192
    pub tier: u8,  // storage tier (RAM/SSD/HDD/...)
                   // TODO: segment, offset, flags, checksum-level, etc.
}
