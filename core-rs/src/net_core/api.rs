use serde::{Deserialize, Serialize};

/// Version/identity information of a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    /// Logical name of the node/service.
    pub name: String,
    /// Package version (usually CARGO_PKG_VERSION).
    pub version: String,
}

/// Basic repository/core statistics.
/// TODO: extend later (tiers, dedup_ratio, used_ram, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoStats {
    /// Number of blocks in the repository.
    pub blocks: u64,
    /// Total volume of unique data (bytes).
    pub bytes: u64,
}

/// Minimal network API v0.2:
/// - Ping / Pong
/// - GetVersion / Version(VersionInfo)
/// - GetRepoStats / RepoStats(RepoStats)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetRequest {
    /// Connectivity check / healthcheck.
    Ping,
    /// Request server version info.
    GetVersion,
    /// Request repository statistics.
    GetRepoStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetResponse {
    /// Response to Ping.
    Pong,
    /// Response to GetVersion.
    Version(VersionInfo),
    /// Response to GetRepoStats.
    RepoStats(RepoStats),
    /// Simple protocol error.
    Error {
        code: u16,
        message: String,
    },
}
