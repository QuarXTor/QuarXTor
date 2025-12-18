use super::error::RepoError;
pub use quarxproto::{ZNode, ZNodeKind};

/// Bincode deserialization with error mapping to RepoError.
#[inline]
pub fn deserialize_znode(raw: &[u8]) -> Result<ZNode, RepoError> {
    quarxproto::ZNode::deserialize(raw).map_err(|_| RepoError::InvalidInput)
}
