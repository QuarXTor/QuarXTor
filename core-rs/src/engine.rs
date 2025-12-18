use std::sync::atomic::{AtomicU64, Ordering};

use crate::net_core::RepoStats;

/// Minimal storage engine:
/// only block/byte counters for now, without a real BlockStore.
#[derive(Debug)]
pub struct Engine {
    blocks: AtomicU64,
    bytes: AtomicU64,
}

impl Default for Engine {
    fn default() -> Self {
        Engine {
            blocks: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
        }
    }
}

impl Engine {
    /// Open Engine on top of a real repository.
    pub fn open_from_repo(path: &str) -> Self {
        use crate::store::file_store::FileBlockStore;

        match FileBlockStore::open(std::path::PathBuf::from(path)) {
            Ok(_) => {
                eprintln!("[engine] repo_root OK: {}", path);
                Engine::default()
            }
            Err(e) => {
                eprintln!("[engine] cannot open repo_root={:?}: {:?}", path, e);
                Engine::default()
            }
        }
    }

    /// Default in-memory engine.
    pub fn new_in_memory() -> Self {
        Self::default()
    }

    /// Register a successful block write of length `len` bytes.
    pub fn account_block_put(&self, len: u64) {
        self.blocks.fetch_add(1, Ordering::Relaxed);
        self.bytes.fetch_add(len, Ordering::Relaxed);
    }

    /// Directly set counters (future use when loading a repository).
    pub fn set_stats(&self, blocks: u64, bytes: u64) {
        self.blocks.store(blocks, Ordering::Relaxed);
        self.bytes.store(bytes, Ordering::Relaxed);
    }

    /// Get current RepoStats.
    pub fn repo_stats(&self) -> RepoStats {
        RepoStats {
            blocks: self.blocks.load(Ordering::Relaxed),
            bytes: self.bytes.load(Ordering::Relaxed),
        }
    }
}
