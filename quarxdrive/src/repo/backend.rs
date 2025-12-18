use super::error::RepoError;
use std::path::Path;

/// QuarxRepo backend:
///   - can be file-based,
///   - can be in-memory,
///   - can be on top of HGSS/QuarXCore.
///
/// All operations are low-level raw access before Z-node logic.
pub trait RepoBackend: Send + Sync {
    /// Write arbitrary object blob, return its id/addr.
    fn put_raw(&self, raw: &[u8]) -> Result<String, RepoError>;

    /// Read previously stored object by id/addr.
    fn get_raw(&self, id: &str) -> Result<Vec<u8>, RepoError>;

    /// Optional hint where repo physically lives (for self-import protection).
    fn repo_root_hint(&self) -> Option<&Path> {
        None
    }
}
