use crate::types::BlockId;

/// ZBlock: structural block (structural zone over an L0 range).
/// Stable on-disk ABI structure; see `docs/core-formats.md` (Structural / Z-blocks).
#[derive(Clone, Debug)]
pub struct ZBlock {
    pub id: BlockId,
    pub hash: [u8; 32],

    /// Range of L0 blocks covered by this Z-block (inclusive).
    pub first_l0: BlockId,
    pub last_l0: BlockId,

    /// Type of structural block (JSON, TAR, ELF-section, user-defined, etc.).
    pub z_type: u32,

    /// Opaque metadata understood only by the corresponding analyzer/plugin.
    pub meta: Vec<u8>,
}
