use crate::config::QuarxConfig;
use crate::engine::Engine;
use crate::net_core::RepoStats;

/// QuarxCore: thin wrapper around Engine + QuarxConfig.
/// External layers talk through this instead of touching Engine directly.
pub struct QuarxCore {
    pub config: QuarxConfig,
    pub engine: Engine,
}

impl QuarxCore {
    /// Pure in-memory mode without on-disk repository.
    /// Used for tests / debugging.
    pub fn open_in_memory() -> Self {
        Self {
            config: QuarxConfig::default(),
            engine: Engine::default(),
        }
    }

    /// Open using canonical config:
    ///   - read INI (QuarxConfig::load_default())
    ///   - take core.repo_root
    ///   - open Engine over on-disk repo or in-memory if path is absent
    ///
    /// Semantics:
    ///   - repo_root = None  -> Engine picks default / in-memory.
    ///   - repo_root = Some  -> Engine opens/creates real HGSS repository.
    pub fn open_with_default_config() -> Self {
        let cfg = QuarxConfig::load_default();

        let engine = match cfg.repo_root.as_deref() {
            Some(path) => Engine::open_from_repo(path),
            None => {
                eprintln!("[engine] repo_root = <none>");
                Engine::new_in_memory()
            }
        };

        Self { config: cfg, engine }
    }

    /// Expose RepoStats outward (for quarxnetd and other layers).
    pub fn repo_stats(&self) -> RepoStats {
        self.engine.repo_stats()
    }
}
