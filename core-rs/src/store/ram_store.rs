use crate::store::blockstore::BlockStore;
use std::sync::atomic::{AtomicU64, Ordering};

/// Snapshot of RAM-tier statistics.
#[derive(Debug, Clone, Copy)]
pub struct RamStats {
    /// Limit (bytes) with which RamStore was created.
    /// 0        — tier is effectively disabled;
    /// u64::MAX — "full/unlimited".
    pub limit_bytes: u64,

    /// Estimated RAM used by cache (bytes).
    pub used_bytes: u64,

    /// Estimated number of blocks in cache.
    pub blocks: u64,

    pub hits: u64,
    pub misses: u64,
    pub inserts: u64,
    pub evictions: u64,
}

/// RamStore — future RAM-tier / cache over any BlockStore.
///
/// For now:
///   - this is an extension point (type is present in external API),
///   - counters/limit exist, but no real cache yet,
///   - BlockStore for RamStore is not implemented to avoid breaking existing code.
///
/// Later additions:
///   - internal LRU / ARC structure for blocks,
///   - impl BlockStore for transparent get/put caching.
#[derive(Debug)]
pub struct RamStore<S: BlockStore> {
    inner: S,
    /// Configured RAM limit (bytes).
    pub limit_bytes: u64,

    /// Counters and estimates (for future cache).
    used_bytes: AtomicU64,
    blocks: AtomicU64,

    hits: AtomicU64,
    misses: AtomicU64,
    inserts: AtomicU64,
    evictions: AtomicU64,
}

#[allow(dead_code)]
impl<S: BlockStore> RamStore<S> {
    /// Create RAM wrapper over existing BlockStore.
    ///
    /// Currently just a shim with limit and counters.
    pub fn new(inner: S, limit_bytes: u64) -> Self {
        Self {
            inner,
            limit_bytes,
            used_bytes: AtomicU64::new(0),
            blocks: AtomicU64::new(0),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            inserts: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
        }
    }

    /// Access to underlying BlockStore (read-only).
    pub fn inner(&self) -> &S {
        &self.inner
    }

    /// Access to underlying BlockStore (mutable).
    pub fn inner_mut(&mut self) -> &mut S {
        &mut self.inner
    }

    /// Take ownership of the inner store.
    pub fn into_inner(self) -> S {
        self.inner
    }

    /// Current RAM limit.
    pub fn limit(&self) -> u64 {
        self.limit_bytes
    }

    /// Whether RAM-tier is logically enabled (limit != 0).
    pub fn is_enabled(&self) -> bool {
        self.limit_bytes != 0
    }

    /// Whether running in "full/unlimited" mode (limit == u64::MAX).
    pub fn is_unlimited(&self) -> bool {
        self.limit_bytes == u64::MAX
    }

    /// Snapshot of RAM-tier statistics.
    ///
    /// For now all fields remain zero (used_bytes/blocks/hits/...) because
    /// caching logic is not implemented yet.
    pub fn stats(&self) -> RamStats {
        RamStats {
            limit_bytes: self.limit_bytes,
            used_bytes: self.used_bytes.load(Ordering::Relaxed),
            blocks: self.blocks.load(Ordering::Relaxed),
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            inserts: self.inserts.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
        }
    }

    /// Helper methods for the future cache.
    ///
    /// Not used externally yet but kept as a contract.
    pub(crate) fn add_used_bytes(&self, delta: i64) {
        if delta >= 0 {
            self.used_bytes.fetch_add(delta as u64, Ordering::Relaxed);
        } else {
            self.used_bytes
                .fetch_sub((-delta) as u64, Ordering::Relaxed);
        }
    }

    pub(crate) fn inc_blocks(&self, delta: i64) {
        if delta >= 0 {
            self.blocks.fetch_add(delta as u64, Ordering::Relaxed);
        } else {
            self.blocks.fetch_sub((-delta) as u64, Ordering::Relaxed);
        }
    }

    pub(crate) fn inc_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn inc_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn inc_insert(&self) {
        self.inserts.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn inc_eviction(&self) {
        self.evictions.fetch_add(1, Ordering::Relaxed);
    }
}

/// Optional extension to obtain RAM statistics.
///
/// For RamStore returns Some(..); for regular stores — None.
pub trait RamBlockStoreExt {
    fn ram_stats(&self) -> Option<RamStats>;
}

impl<S: BlockStore> RamBlockStoreExt for RamStore<S> {
    fn ram_stats(&self) -> Option<RamStats> {
        Some(self.stats())
    }
}
