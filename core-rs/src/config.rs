use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Canonical INI path:
///   $XDG_CONFIG_HOME/quarxtor/quarxctl.ini
///   or $HOME/.config/quarxtor/quarxctl.ini
fn default_ini_path() -> Option<PathBuf> {
    let base = if let Ok(p) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(p)
    } else {
        let home = std::env::var("HOME").ok()?;
        PathBuf::from(home).join(".config")
    };
    let primary = base.join("quarxtor").join("quarxctl.ini");
    let fallback = PathBuf::from("/tmp").join("quarxtor").join("quarxctl.ini");
    if primary.exists() {
        return Some(primary);
    }
    if fallback.exists() {
        return Some(fallback);
    }
    Some(primary)
}

fn parse_bool(v: &str, default: bool) -> bool {
    let v = v.trim().to_ascii_lowercase();
    match v.as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => default,
    }
}

fn parse_u64(v: &str, default: u64) -> u64 {
    v.trim().parse::<u64>().unwrap_or(default)
}

/// Flat config compatible with quarxdrive/fs_import.
///
/// INI sections:
///   [core]
///     l0_chunk = 16384
///     ram.limit = full | limit | auto
///     analysis.enable_znode = true/false
///     analysis.fs_stats_fallback = true/false
///
///   [paths]
///     data_dir      = ...
///     cache_dir     = ...
///     log_dir       = ...
///     runtime_dir   = ...
///
///   [import]
///     min_file_size   = 4096
///     skip_hidden     = false
///     skip_symlink    = true
///     skip_zero       = true
///     skip_devices    = true
///     skip_special    = true
///     root            = <source tree for import>
///     include_glob    = "*.so,*.exe,*.iso"
///     exclude_glob    = "*.tmp,*.log,*.bak"
///     include_dir_glob = ...
///     exclude_dir_glob = ...
///     import_glob_mode = file-only | dir-only | both
///     use_z           = true
///     z_threshold     = 10
#[derive(Debug, Clone)]
pub struct QuarxConfig {
    // [core]
    pub l0_chunk: u64,
    pub ram_limit: String,
    pub ram_limit_bytes: u64,
    pub analysis_enable_znode: bool,
    pub analysis_fs_stats_fallback: bool,

    // [import] — file filters
    pub repo_root: Option<String>,
    pub import_root: Option<String>,
    // [paths]
    pub paths_data_dir: Option<String>,
    pub paths_cache_dir: Option<String>,
    pub paths_log_dir: Option<String>,
    pub paths_runtime_dir: Option<String>,
    pub import_min_file_size: u64,
    pub import_skip_hidden: bool,
    pub import_skip_symlink: bool,
    pub import_skip_zero: bool,
    pub import_skip_devices: bool,
    pub import_skip_special: bool,

    pub include_glob: Option<String>,
    pub exclude_glob: Option<String>,
    pub include_dir_glob: Option<String>,
    pub exclude_dir_glob: Option<String>,

    /// Glob strategy:
    ///   "file-only" (default) / "dir-only" / "both"
    pub import_glob_mode: String,

    /// Z compression on import (fs_import)
    pub fs_import_use_z: bool,
    pub fs_import_z_threshold: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum QuarxConfigError {
    InvalidPaths(String),
}

impl std::fmt::Display for QuarxConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuarxConfigError::InvalidPaths(msg) => write!(f, "invalid config paths: {msg}"),
        }
    }
}

impl Default for QuarxConfig {
    fn default() -> Self {
        QuarxConfig {
            // core
            l0_chunk: 16384,
            ram_limit: "full".to_string(),
            ram_limit_bytes: 0,
            analysis_enable_znode: true,
            analysis_fs_stats_fallback: true,

            repo_root: None,
            import_root: Some("/tmp/quarx-dataset".to_string()),
            paths_data_dir: None,
            paths_cache_dir: None,
            paths_log_dir: None,
            paths_runtime_dir: None,

            // import
            import_min_file_size: 4096,
            import_skip_hidden: false,
            import_skip_symlink: true,
            import_skip_zero: true,
            import_skip_devices: true,
            import_skip_special: true,

            include_glob: Some("*.so,*.exe,*.iso".to_string()),
            exclude_glob: None,
            include_dir_glob: None,
            exclude_dir_glob: None,
            import_glob_mode: "file-only".to_string(),

            fs_import_use_z: true,
            fs_import_z_threshold: 10,
        }
    }
}

impl QuarxConfig {
    /// Backward-compatible alias for old callers (quarxctl).
    pub fn load() -> Self {
        Self::load_default()
    }

    /// Load config from default INI.
    /// On any error returns Default().
    pub fn load_default() -> Self {
        let path = match default_ini_path() {
            Some(p) => p,
            None => return QuarxConfig::default(),
        };

        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => return QuarxConfig::default(),
        };

        Self::from_ini_str(&text)
    }

    /// Parse INI string with [core]/[import] sections.
    pub fn from_ini_str(text: &str) -> Self {
        let mut cfg = QuarxConfig::default();
        let mut section: Option<String> = None;

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

        // [section]
            if line.starts_with('[') && line.ends_with(']') {
                let s = line[1..line.len() - 1].trim().to_ascii_lowercase();
                section = Some(s);
                continue;
            }

            let eq = match line.find('=') {
                Some(i) => i,
                None => continue,
            };

            let key = line[..eq].trim();
            let val = line[eq + 1..].trim();

            match section.as_deref() {
                Some("core") => apply_core_kv(&mut cfg, key, val),
                Some("paths") => apply_paths_kv(&mut cfg, key, val),
                Some("import") => apply_import_kv(&mut cfg, key, val),
                _ => {}
            }
        }

        cfg.validate()
            .unwrap_or_else(|e| panic!("invalid QuarxConfig (path invariants): {e}"));
        cfg
    }

    /// Validate core/import/path invariants:
    /// - paths must be absolute (tilde expanded)
    /// - repo_root must not equal import.root
    /// - repo_root must not reside inside import.root
    pub fn validate(&self) -> Result<(), QuarxConfigError> {
        let repo_root = normalize_path(self.repo_root.as_deref(), "repo_root")?;
        let import_root = normalize_path(self.import_root.as_deref(), "import.root")?;
        let data_dir = normalize_optional_path(self.paths_data_dir.as_deref(), "paths.data_dir")?;
        let cache_dir = normalize_optional_path(self.paths_cache_dir.as_deref(), "paths.cache_dir")?;
        let log_dir = normalize_optional_path(self.paths_log_dir.as_deref(), "paths.log_dir")?;
        let runtime_dir =
            normalize_optional_path(self.paths_runtime_dir.as_deref(), "paths.runtime_dir")?;

        if let (Some(repo), Some(import)) = (&repo_root, &import_root) {
            if repo == import {
                return Err(QuarxConfigError::InvalidPaths(format!(
                    "repo_root must not equal import.root (repo_root={}, import.root={})",
                    repo.display(),
                    import.display()
                )));
            }

            if repo.starts_with(import) {
                return Err(QuarxConfigError::InvalidPaths(format!(
                    "repo_root must not reside inside import.root (repo_root={}, import.root={})",
                    repo.display(),
                    import.display()
                )));
            }
        }

        if let Some(import) = &import_root {
            if let Some(p) = &data_dir {
                if p.starts_with(import) {
                    return Err(QuarxConfigError::InvalidPaths(format!(
                        "paths.data_dir must not reside inside import.root (data_dir={}, import.root={})",
                        p.display(),
                        import.display()
                    )));
                }
            }
            if let Some(p) = &cache_dir {
                if p.starts_with(import) {
                    return Err(QuarxConfigError::InvalidPaths(format!(
                        "paths.cache_dir must not reside inside import.root (cache_dir={}, import.root={})",
                        p.display(),
                        import.display()
                    )));
                }
            }
            if let Some(p) = &log_dir {
                if p.starts_with(import) {
                    return Err(QuarxConfigError::InvalidPaths(format!(
                        "paths.log_dir must not reside inside import.root (log_dir={}, import.root={})",
                        p.display(),
                        import.display()
                    )));
                }
            }
            if let Some(p) = &runtime_dir {
                if p.starts_with(import) {
                    return Err(QuarxConfigError::InvalidPaths(format!(
                        "paths.runtime_dir must not reside inside import.root (runtime_dir={}, import.root={})",
                        p.display(),
                        import.display()
                    )));
                }
            }
        }

        Ok(())
    }
}

/// Effective, env-overridable config contract.
#[derive(Debug, Clone)]
pub struct EffectiveConfig {
    pub l0_chunk: u64,
    pub ram_limit: String,
    pub analysis_enable_znode: bool,
    pub analysis_fs_stats_fallback: bool,
    pub repo_root: Option<String>,

    pub dataset_root: String,
    pub ram_limit_bytes: Option<u64>,

    pub paths_data_dir: Option<String>,
    pub paths_cache_dir: Option<String>,
    pub paths_log_dir: Option<String>,
    pub paths_runtime_dir: Option<String>,

    pub import_min_file_size: u64,
    pub import_skip_hidden: bool,
    pub import_skip_symlink: bool,
    pub import_skip_zero: bool,
    pub import_skip_devices: bool,
    pub import_skip_special: bool,
    pub include_glob: Option<String>,
    pub exclude_glob: Option<String>,
    pub include_dir_glob: Option<String>,
    pub exclude_dir_glob: Option<String>,
    pub import_glob_mode: String,
    pub fs_import_use_z: bool,
    pub fs_import_z_threshold: u64,

    pub net_addr: String,
    pub net_transport: String,
    pub net_unix_path: String,

    // Reserved/placeholder sections
    pub drive_mountpoint: Option<String>,
    pub drive_strategy: Option<String>,
    pub drive_fs_cache_bytes: u64,
    pub daemon_enable_netd: bool,
    pub cloud_seed_nodes: Option<String>,
}

#[derive(Debug)]
pub enum EffectiveConfigError {
    InvalidPaths(String),
    MissingDatasetRoot,
    InvalidNetAddr(String),
    UnknownKeys(Vec<String>),
    MissingConfigFile,
    MissingKey(String),
    MissingKeys(Vec<String>),
}

impl std::fmt::Display for EffectiveConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EffectiveConfigError::InvalidPaths(msg) => write!(f, "{msg}"),
            EffectiveConfigError::MissingDatasetRoot => write!(
                f,
                "DATASET ROOT NOT SET (config missing and QUARX_DATASET_DIR not set)"
            ),
            EffectiveConfigError::InvalidNetAddr(msg) => write!(f, "{msg}"),
            EffectiveConfigError::UnknownKeys(keys) => {
                write!(f, "unknown config keys: {}", keys.join(", "))
        }
            EffectiveConfigError::MissingConfigFile => {
                write!(f, "CONFIG FILE NOT FOUND")
            }
            EffectiveConfigError::MissingKey(k) => write!(f, "missing config key: {k}"),
            EffectiveConfigError::MissingKeys(list) => {
                write!(f, "missing config keys: {}", list.join(", "))
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ConfigValueType {
    Bool,
    U64,
    String,
    Path,
    Enum(&'static [&'static str]),
}

#[derive(Debug, Clone, Copy)]
pub enum ConfigStatus {
    Implemented,
    Partial,
    Reserved,
    NotImplemented,
}

impl ConfigStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ConfigStatus::Implemented => "IMPLEMENTED",
            ConfigStatus::Partial => "PARTIAL",
            ConfigStatus::Reserved => "RESERVED",
            ConfigStatus::NotImplemented => "NOT_IMPLEMENTED",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConfigKey {
    pub section: &'static str,
    pub key: &'static str,
    pub value_type: ConfigValueType,
    pub default_value: &'static str,
    pub description: &'static [&'static str],
    pub env_override: Option<&'static str>,
    pub status: ConfigStatus,
    pub role: KeyRole,
}

#[derive(Debug, Clone, Copy)]
pub enum KeyRole {
    Required,
    RequiredWithDefault,
    Optional,
    Reserved,
}

pub fn config_contract() -> &'static [ConfigKey] {
    &[
        ConfigKey {
            section: "core",
            key: "l0_chunk",
            value_type: ConfigValueType::U64,
            default_value: "16384",
            description: &["L0 chunk size in bytes for new data blocks."],
            env_override: Some("QUARX_L0_CHUNK"),
            status: ConfigStatus::Implemented,
            role: KeyRole::RequiredWithDefault,
        },
        ConfigKey {
            section: "core",
            key: "ram.limit",
            value_type: ConfigValueType::String,
            default_value: "full",
            description: &["RAM tier policy: full / limit / auto (future)."],
            env_override: Some("QUARX_RAM_LIMIT"),
            status: ConfigStatus::Partial,
            role: KeyRole::RequiredWithDefault,
        },
        ConfigKey {
            section: "core",
            key: "analysis.enable_znode",
            value_type: ConfigValueType::Bool,
            default_value: "true",
            description: &["Enable ZNode / cheap-size analysis during import."],
            env_override: Some("QUARX_ANALYSIS_ENABLE_ZNODE"),
            status: ConfigStatus::Implemented,
            role: KeyRole::Optional,
        },
        ConfigKey {
            section: "core",
            key: "analysis.fs_stats_fallback",
            value_type: ConfigValueType::Bool,
            default_value: "true",
            description: &["Fallback to computing real file sizes when cheap-size is missing."],
            env_override: Some("QUARX_ANALYSIS_FS_STATS_FALLBACK"),
            status: ConfigStatus::Implemented,
            role: KeyRole::Optional,
        },
        ConfigKey {
            section: "core",
            key: "repo_root",
            value_type: ConfigValueType::Path,
            default_value: "",
            description: &[
                "Canonical repository root.",
                "Must not equal or reside under import.root.",
            ],
            env_override: Some("QUARX_REPO_ROOT"),
            status: ConfigStatus::Implemented,
            role: KeyRole::Optional,
        },
        ConfigKey {
            section: "import",
            key: "root",
            value_type: ConfigValueType::Path,
            default_value: "",
            description: &[
                "Root of the source tree to import.",
                "Must be absolute; required if QUARX_DATASET_DIR not set.",
            ],
            env_override: Some("QUARX_DATASET_DIR"),
            status: ConfigStatus::Implemented,
            role: KeyRole::Required,
        },
        ConfigKey {
            section: "import",
            key: "min_file_size",
            value_type: ConfigValueType::U64,
            default_value: "4096",
            description: &["Minimum file size to import in bytes."],
            env_override: Some("QUARX_IMPORT_MIN_FILE_SIZE"),
            status: ConfigStatus::Implemented,
            role: KeyRole::RequiredWithDefault,
        },
        ConfigKey {
            section: "import",
            key: "skip_hidden",
            value_type: ConfigValueType::Bool,
            default_value: "false",
            description: &["Skip hidden files/directories (leading dot)."],
            env_override: Some("QUARX_IMPORT_SKIP_HIDDEN"),
            status: ConfigStatus::Implemented,
            role: KeyRole::RequiredWithDefault,
        },
        ConfigKey {
            section: "import",
            key: "skip_symlink",
            value_type: ConfigValueType::Bool,
            default_value: "true",
            description: &["Do not follow symlinks."],
            env_override: Some("QUARX_IMPORT_SKIP_SYMLINK"),
            status: ConfigStatus::Implemented,
            role: KeyRole::RequiredWithDefault,
        },
        ConfigKey {
            section: "import",
            key: "skip_zero",
            value_type: ConfigValueType::Bool,
            default_value: "true",
            description: &["Skip zero-sized files."],
            env_override: Some("QUARX_IMPORT_SKIP_ZERO"),
            status: ConfigStatus::Implemented,
            role: KeyRole::RequiredWithDefault,
        },
        ConfigKey {
            section: "import",
            key: "skip_devices",
            value_type: ConfigValueType::Bool,
            default_value: "true",
            description: &["Skip device files (block/char)."],
            env_override: Some("QUARX_IMPORT_SKIP_DEVICES"),
            status: ConfigStatus::Implemented,
            role: KeyRole::RequiredWithDefault,
        },
        ConfigKey {
            section: "import",
            key: "skip_special",
            value_type: ConfigValueType::Bool,
            default_value: "true",
            description: &["Skip sockets/pipes and other special types."],
            env_override: Some("QUARX_IMPORT_SKIP_SPECIAL"),
            status: ConfigStatus::Implemented,
            role: KeyRole::RequiredWithDefault,
        },
        ConfigKey {
            section: "import",
            key: "include_glob",
            value_type: ConfigValueType::String,
            default_value: "*.so,*.exe,*.iso",
            description: &["CSV glob filter for files to include."],
            env_override: Some("QUARX_IMPORT_INCLUDE_GLOB"),
            status: ConfigStatus::Implemented,
            role: KeyRole::RequiredWithDefault,
        },
        ConfigKey {
            section: "import",
            key: "exclude_glob",
            value_type: ConfigValueType::String,
            default_value: "",
            description: &["CSV glob filter for files to exclude."],
            env_override: Some("QUARX_IMPORT_EXCLUDE_GLOB"),
            status: ConfigStatus::Implemented,
            role: KeyRole::RequiredWithDefault,
        },
        ConfigKey {
            section: "import",
            key: "include_dir_glob",
            value_type: ConfigValueType::String,
            default_value: "",
            description: &["CSV glob filter for directories to include."],
            env_override: Some("QUARX_IMPORT_INCLUDE_DIR_GLOB"),
            status: ConfigStatus::Implemented,
            role: KeyRole::RequiredWithDefault,
        },
        ConfigKey {
            section: "import",
            key: "exclude_dir_glob",
            value_type: ConfigValueType::String,
            default_value: "",
            description: &["CSV glob filter for directories to exclude."],
            env_override: Some("QUARX_IMPORT_EXCLUDE_DIR_GLOB"),
            status: ConfigStatus::Implemented,
            role: KeyRole::RequiredWithDefault,
        },
        ConfigKey {
            section: "import",
            key: "import_glob_mode",
            value_type: ConfigValueType::Enum(&["file-only", "dir-only", "both"]),
            default_value: "file-only",
            description: &["Glob application mode: file-only / dir-only / both."],
            env_override: Some("QUARX_IMPORT_GLOB_MODE"),
            status: ConfigStatus::Implemented,
            role: KeyRole::RequiredWithDefault,
        },
        ConfigKey {
            section: "import",
            key: "use_z",
            value_type: ConfigValueType::Bool,
            default_value: "true",
            description: &["Enable Z compression on import."],
            env_override: Some("QUARX_IMPORT_USE_Z"),
            status: ConfigStatus::Implemented,
            role: KeyRole::RequiredWithDefault,
        },
        ConfigKey {
            section: "import",
            key: "z_threshold",
            value_type: ConfigValueType::U64,
            default_value: "10",
            description: &["Z compression usefulness threshold (heuristic)."],
            env_override: Some("QUARX_IMPORT_Z_THRESHOLD"),
            status: ConfigStatus::Implemented,
            role: KeyRole::RequiredWithDefault,
        },
        ConfigKey {
            section: "paths",
            key: "data_dir",
            value_type: ConfigValueType::Path,
            default_value: "",
            description: &["Base directory for repository data (absolute)."],
            env_override: Some("QUARX_DATA_DIR"),
            status: ConfigStatus::Implemented,
            role: KeyRole::Optional,
        },
        ConfigKey {
            section: "paths",
            key: "cache_dir",
            value_type: ConfigValueType::Path,
            default_value: "",
            description: &["Cache directory (absolute)."],
            env_override: Some("QUARX_CACHE_DIR"),
            status: ConfigStatus::Implemented,
            role: KeyRole::Optional,
        },
        ConfigKey {
            section: "paths",
            key: "log_dir",
            value_type: ConfigValueType::Path,
            default_value: "",
            description: &["Log directory (absolute)."],
            env_override: Some("QUARX_LOG_DIR"),
            status: ConfigStatus::Implemented,
            role: KeyRole::Optional,
        },
        ConfigKey {
            section: "paths",
            key: "runtime_dir",
            value_type: ConfigValueType::Path,
            default_value: "",
            description: &["Runtime directory for pid/log sockets (absolute)."],
            env_override: Some("QUARX_RUNTIME_DIR"),
            status: ConfigStatus::Implemented,
            role: KeyRole::Optional,
        },
        ConfigKey {
            section: "net",
            key: "addr",
            value_type: ConfigValueType::String,
            default_value: "0.0.0.0:54721",
            description: &["Canonical listen address host:port for quarxnetd."],
            env_override: Some("QUARX_NET_ADDR"),
            status: ConfigStatus::Implemented,
            role: KeyRole::RequiredWithDefault,
        },
        ConfigKey {
            section: "net",
            key: "transport",
            value_type: ConfigValueType::Enum(&["tcp", "unix"]),
            default_value: "unix",
            description: &["Network transport: tcp or unix."],
            env_override: Some("QUARX_NET_TRANSPORT"),
            status: ConfigStatus::Implemented,
            role: KeyRole::RequiredWithDefault,
        },
        ConfigKey {
            section: "net",
            key: "unix_path",
            value_type: ConfigValueType::Path,
            default_value: "/tmp/quarxtor/quarxnetd.sock",
            description: &["UNIX socket path for local transport."],
            env_override: Some("QUARX_NET_UNIX_PATH"),
            status: ConfigStatus::Implemented,
            role: KeyRole::RequiredWithDefault,
        },
        // Reserved / placeholders
        ConfigKey {
            section: "drive",
            key: "mountpoint",
            value_type: ConfigValueType::Path,
            default_value: "",
            description: &["Reserved for drive mountpoint settings."],
            env_override: None,
            status: ConfigStatus::Reserved,
            role: KeyRole::Reserved,
        },
        ConfigKey {
            section: "drive",
            key: "strategy",
            value_type: ConfigValueType::String,
            default_value: "",
            description: &["Reserved for drive caching/overlay strategy."],
            env_override: None,
            status: ConfigStatus::Reserved,
            role: KeyRole::Reserved,
        },
        ConfigKey {
            section: "drive",
            key: "fs_cache_bytes",
            value_type: ConfigValueType::U64,
            default_value: "0",
            description: &["Reserved for drive cache limit."],
            env_override: None,
            status: ConfigStatus::Reserved,
            role: KeyRole::Reserved,
        },
        ConfigKey {
            section: "daemon",
            key: "enable_netd",
            value_type: ConfigValueType::Bool,
            default_value: "false",
            description: &["Reserved daemon flag for quarxnetd enablement."],
            env_override: None,
            status: ConfigStatus::Reserved,
            role: KeyRole::Reserved,
        },
        ConfigKey {
            section: "cloud",
            key: "seed_nodes",
            value_type: ConfigValueType::String,
            default_value: "",
            description: &["Reserved for future clustering seed nodes."],
            env_override: None,
            status: ConfigStatus::Reserved,
            role: KeyRole::Reserved,
        },
    ]
}

impl EffectiveConfig {
    fn defaults() -> Self {
        EffectiveConfig {
            l0_chunk: 16384,
            ram_limit: "full".to_string(),
            analysis_enable_znode: true,
            analysis_fs_stats_fallback: true,
            repo_root: None,

            dataset_root: String::new(),
            ram_limit_bytes: None,

            paths_data_dir: None,
            paths_cache_dir: None,
            paths_log_dir: None,
            paths_runtime_dir: None,

            import_min_file_size: 4096,
            import_skip_hidden: false,
            import_skip_symlink: true,
            import_skip_zero: true,
            import_skip_devices: true,
            import_skip_special: true,
            include_glob: Some("*.so,*.exe,*.iso".to_string()),
            exclude_glob: None,
            include_dir_glob: None,
            exclude_dir_glob: None,
            import_glob_mode: "file-only".to_string(),
            fs_import_use_z: true,
            fs_import_z_threshold: 10,

            net_addr: "0.0.0.0:54721".to_string(),
            net_transport: "unix".to_string(),
            net_unix_path: "/tmp/quarxtor/quarxnetd.sock".to_string(),

            drive_mountpoint: None,
            drive_strategy: None,
            drive_fs_cache_bytes: 0,
            daemon_enable_netd: false,
            cloud_seed_nodes: None,
        }
    }

    /// Load from default config path (XDG then HOME), apply env overrides, validate.
    pub fn load_default() -> Result<Self, EffectiveConfigError> {
        let path = default_ini_path();
        let text = path
            .as_ref()
            .and_then(|p| fs::read_to_string(p).ok())
            .unwrap_or_default();
        Self::from_ini_str(&text)
    }

    /// Strict load: config file must exist and required keys must be present.
    pub fn load_strict() -> Result<Self, EffectiveConfigError> {
        let path = default_ini_path().ok_or(EffectiveConfigError::MissingConfigFile)?;
        let text = fs::read_to_string(&path).map_err(|_| EffectiveConfigError::MissingConfigFile)?;
        Self::from_ini_str_strict(&text)
    }

    fn apply_env_overrides(mut cfg: Self) -> Result<Self, EffectiveConfigError> {
        if let Ok(v) = std::env::var("QUARX_L0_CHUNK") {
            cfg.l0_chunk = parse_u64(&v, cfg.l0_chunk);
        }
        if let Ok(v) = std::env::var("QUARX_RAM_LIMIT") {
            if !v.trim().is_empty() {
                cfg.ram_limit = v;
            }
        }
        if let Ok(v) = std::env::var("QUARX_ANALYSIS_ENABLE_ZNODE") {
            cfg.analysis_enable_znode = parse_bool(&v, cfg.analysis_enable_znode);
        }
        if let Ok(v) = std::env::var("QUARX_ANALYSIS_FS_STATS_FALLBACK") {
            cfg.analysis_fs_stats_fallback =
                parse_bool(&v, cfg.analysis_fs_stats_fallback);
        }
        if let Ok(v) = std::env::var("QUARX_REPO_ROOT") {
            if !v.trim().is_empty() {
                cfg.repo_root = Some(v);
            }
        }

        if let Ok(v) = std::env::var("QUARX_DATA_DIR") {
            if !v.trim().is_empty() {
                cfg.paths_data_dir = Some(v);
            }
        }
        if let Ok(v) = std::env::var("QUARX_CACHE_DIR") {
            if !v.trim().is_empty() {
                cfg.paths_cache_dir = Some(v);
            }
        }
        if let Ok(v) = std::env::var("QUARX_LOG_DIR") {
            if !v.trim().is_empty() {
                cfg.paths_log_dir = Some(v);
            }
        }
        if let Ok(v) = std::env::var("QUARX_RUNTIME_DIR") {
            if !v.trim().is_empty() {
                cfg.paths_runtime_dir = Some(v);
            }
        }

        if let Ok(v) = std::env::var("QUARX_IMPORT_MIN_FILE_SIZE") {
            cfg.import_min_file_size = parse_u64(&v, cfg.import_min_file_size);
        }
        if let Ok(v) = std::env::var("QUARX_DATASET_DIR") {
            if !v.trim().is_empty() {
                cfg.dataset_root = v;
            }
        }
        if let Ok(v) = std::env::var("QUARX_IMPORT_SKIP_HIDDEN") {
            cfg.import_skip_hidden = parse_bool(&v, cfg.import_skip_hidden);
        }
        if let Ok(v) = std::env::var("QUARX_IMPORT_SKIP_SYMLINK") {
            cfg.import_skip_symlink = parse_bool(&v, cfg.import_skip_symlink);
        }
        if let Ok(v) = std::env::var("QUARX_IMPORT_SKIP_ZERO") {
            cfg.import_skip_zero = parse_bool(&v, cfg.import_skip_zero);
        }
        if let Ok(v) = std::env::var("QUARX_IMPORT_SKIP_DEVICES") {
            cfg.import_skip_devices = parse_bool(&v, cfg.import_skip_devices);
        }
        if let Ok(v) = std::env::var("QUARX_IMPORT_SKIP_SPECIAL") {
            cfg.import_skip_special = parse_bool(&v, cfg.import_skip_special);
        }
        if let Ok(v) = std::env::var("QUARX_IMPORT_INCLUDE_GLOB") {
            if !v.trim().is_empty() {
                cfg.include_glob = Some(v);
            }
        }
        if let Ok(v) = std::env::var("QUARX_IMPORT_EXCLUDE_GLOB") {
            if !v.trim().is_empty() {
                cfg.exclude_glob = Some(v);
            }
        }
        if let Ok(v) = std::env::var("QUARX_IMPORT_INCLUDE_DIR_GLOB") {
            if !v.trim().is_empty() {
                cfg.include_dir_glob = Some(v);
            }
        }
        if let Ok(v) = std::env::var("QUARX_IMPORT_EXCLUDE_DIR_GLOB") {
            if !v.trim().is_empty() {
                cfg.exclude_dir_glob = Some(v);
            }
        }
        if let Ok(v) = std::env::var("QUARX_IMPORT_GLOB_MODE") {
            if !v.trim().is_empty() {
                cfg.import_glob_mode = v;
            }
        }
        if let Ok(v) = std::env::var("QUARX_IMPORT_USE_Z") {
            cfg.fs_import_use_z = parse_bool(&v, cfg.fs_import_use_z);
        }
        if let Ok(v) = std::env::var("QUARX_IMPORT_Z_THRESHOLD") {
            cfg.fs_import_z_threshold = parse_u64(&v, cfg.fs_import_z_threshold);
        }
        if let Ok(v) = std::env::var("QUARX_NET_ADDR") {
            if !v.trim().is_empty() {
                cfg.net_addr = v;
            }
        }
        if let Ok(v) = std::env::var("QUARX_NET_TRANSPORT") {
            if !v.trim().is_empty() {
                cfg.net_transport = v;
            }
        }
        if let Ok(v) = std::env::var("QUARX_NET_UNIX_PATH") {
            if !v.trim().is_empty() {
                cfg.net_unix_path = v;
            }
        }

        Ok(cfg)
    }

    pub fn from_ini_str(text: &str) -> Result<Self, EffectiveConfigError> {
        Self::from_ini_str_inner(text, false)
    }

    pub fn from_ini_str_strict(text: &str) -> Result<Self, EffectiveConfigError> {
        Self::from_ini_str_inner(text, true)
    }

    fn from_ini_str_inner(text: &str, strict: bool) -> Result<Self, EffectiveConfigError> {
        let mut cfg = Self::defaults();
        let mut section: Option<String> = None;
        let mut kv_map: HashMap<String, HashMap<String, String>> = HashMap::new();
        let mut unknown_keys: Vec<String> = Vec::new();
        let mut allowed: HashSet<(String, String)> = HashSet::new();
        for k in config_contract() {
            allowed.insert((k.section.to_string(), k.key.to_string()));
        }
        // legacy aliases that are accepted but mapped to canonical keys
        let alias_map: HashMap<(&str, &str), (&str, &str)> = HashMap::from([
            (("core", "l0.chunk"), ("core", "l0_chunk")),
            (("core", "ram_limit"), ("core", "ram.limit")),
            (("core", "analysis.enable_z"), ("core", "analysis.enable_znode")),
            (
                ("core", "analysis.fs_stats.fallback"),
                ("core", "analysis.fs_stats_fallback"),
            ),
        ]);
        let mut net_addr_set = false;

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = Some(line[1..line.len() - 1].trim().to_ascii_lowercase());
                continue;
            }
            let eq = match line.find('=') {
                Some(i) => i,
                None => continue,
            };
            let key_raw = line[..eq].trim().to_ascii_lowercase();
            let val = line[eq + 1..].trim().to_string();
            if let Some(sec) = section.as_deref() {
                let (sec_canon, key_canon) = if let Some((s, k)) =
                    alias_map.get(&(sec, key_raw.as_str()))
                {
                    (s.to_string(), k.to_string())
                } else {
                    (sec.to_string(), key_raw.clone())
                };

                if !allowed.contains(&(sec_canon.clone(), key_canon.clone())) {
                    unknown_keys.push(format!("{sec}.{key_raw}"));
                    continue;
                }
                if sec_canon == "net" && key_canon == "addr" {
                    net_addr_set = true;
                }
                kv_map
                    .entry(sec_canon)
                    .or_default()
                    .insert(key_canon, val);
            }
        }

        if !unknown_keys.is_empty() {
            return Err(EffectiveConfigError::UnknownKeys(unknown_keys));
        }

        if let Some(core) = kv_map.get("core") {
            if let Some(v) = core.get("l0_chunk") {
                cfg.l0_chunk = parse_u64(v, cfg.l0_chunk);
            }
            if let Some(v) = core.get("l0.chunk") {
                cfg.l0_chunk = parse_u64(v, cfg.l0_chunk);
            }
            if let Some(v) = core.get("ram.limit") {
                cfg.ram_limit = v.clone();
            }
            if let Some(v) = core.get("ram_limit") {
                cfg.ram_limit = v.clone();
            }
            if let Some(v) = core.get("analysis.enable_znode") {
                cfg.analysis_enable_znode = parse_bool(v, cfg.analysis_enable_znode);
            }
            if let Some(v) = core.get("analysis.fs_stats_fallback") {
                cfg.analysis_fs_stats_fallback =
                    parse_bool(v, cfg.analysis_fs_stats_fallback);
            }
            if let Some(v) = core.get("repo_root") {
                cfg.repo_root = Some(v.clone());
            }
        }

        if let Some(paths) = kv_map.get("paths") {
            if let Some(v) = paths.get("data_dir") {
                cfg.paths_data_dir = Some(v.clone());
            }
            if let Some(v) = paths.get("cache_dir") {
                cfg.paths_cache_dir = Some(v.clone());
            }
            if let Some(v) = paths.get("log_dir") {
                cfg.paths_log_dir = Some(v.clone());
            }
            if let Some(v) = paths.get("runtime_dir") {
                cfg.paths_runtime_dir = Some(v.clone());
            }
        }

        if let Some(import) = kv_map.get("import") {
            if let Some(v) = import.get("min_file_size") {
                cfg.import_min_file_size = parse_u64(v, cfg.import_min_file_size);
            }
            if let Some(v) = import.get("root") {
                cfg.dataset_root = v.clone();
            }
            if let Some(v) = import.get("skip_hidden") {
                cfg.import_skip_hidden = parse_bool(v, cfg.import_skip_hidden);
            }
            if let Some(v) = import.get("skip_symlink") {
                cfg.import_skip_symlink = parse_bool(v, cfg.import_skip_symlink);
            }
            if let Some(v) = import.get("skip_zero") {
                cfg.import_skip_zero = parse_bool(v, cfg.import_skip_zero);
            }
            if let Some(v) = import.get("skip_devices") {
                cfg.import_skip_devices = parse_bool(v, cfg.import_skip_devices);
            }
            if let Some(v) = import.get("skip_special") {
                cfg.import_skip_special = parse_bool(v, cfg.import_skip_special);
            }
            if let Some(v) = import.get("include_glob") {
                cfg.include_glob = Some(v.clone());
            }
            if let Some(v) = import.get("exclude_glob") {
                cfg.exclude_glob = Some(v.clone());
            }
            if let Some(v) = import.get("include_dir_glob") {
                cfg.include_dir_glob = Some(v.clone());
            }
            if let Some(v) = import.get("exclude_dir_glob") {
                cfg.exclude_dir_glob = Some(v.clone());
            }
            if let Some(v) = import.get("import_glob_mode") {
                cfg.import_glob_mode = v.clone();
            }
            if let Some(v) = import.get("use_z") {
                cfg.fs_import_use_z = parse_bool(v, cfg.fs_import_use_z);
            }
            if let Some(v) = import.get("z_threshold") {
                cfg.fs_import_z_threshold = parse_u64(v, cfg.fs_import_z_threshold);
            }
        }

        if let Some(net) = kv_map.get("net") {
            if let Some(v) = net.get("addr") {
                cfg.net_addr = v.clone();
            }
            if let Some(v) = net.get("transport") {
                cfg.net_transport = v.clone();
            }
            if let Some(v) = net.get("unix_path") {
                cfg.net_unix_path = v.clone();
            }
        }

        if let Some(drive) = kv_map.get("drive") {
            if let Some(v) = drive.get("mountpoint") {
                cfg.drive_mountpoint = Some(v.clone());
            }
            if let Some(v) = drive.get("strategy") {
                cfg.drive_strategy = Some(v.clone());
            }
            if let Some(v) = drive.get("fs_cache_bytes") {
                cfg.drive_fs_cache_bytes = parse_u64(v, cfg.drive_fs_cache_bytes);
            }
        }

        if let Some(daemon) = kv_map.get("daemon") {
            if let Some(v) = daemon.get("enable_netd") {
                cfg.daemon_enable_netd = parse_bool(v, cfg.daemon_enable_netd);
            }
        }

        if let Some(cloud) = kv_map.get("cloud") {
            if let Some(v) = cloud.get("seed_nodes") {
                if !v.trim().is_empty() {
                    cfg.cloud_seed_nodes = Some(v.clone());
                }
            }
        }

        cfg = Self::apply_env_overrides(cfg)?;
        if std::env::var("QUARX_NET_ADDR").is_ok() {
            net_addr_set = true;
        }
        if strict && !net_addr_set {
            return Err(EffectiveConfigError::MissingKey("net.addr".to_string()));
        }
        cfg.validate()
    }

    fn validate(self) -> Result<Self, EffectiveConfigError> {
        let repo_root = normalize_path(self.repo_root.as_deref(), "repo_root")
            .map_err(|e| EffectiveConfigError::InvalidPaths(e.to_string()))?;
        let dataset_root_env = if self.dataset_root.trim().is_empty() {
            None
        } else {
            Some(self.dataset_root.clone())
        };
        let dataset_root = dataset_root_env
            .or_else(|| self.dataset_root_from_import())
            .ok_or(EffectiveConfigError::MissingDatasetRoot)?;

        let dataset_root_norm = normalize_single_path(&dataset_root, "import.root")
            .map_err(|e| EffectiveConfigError::InvalidPaths(e.to_string()))?;

        let paths_data_dir = normalize_optional_path(
            self.paths_data_dir.as_deref(),
            "paths.data_dir",
        )
        .map_err(|e| EffectiveConfigError::InvalidPaths(e.to_string()))?;
        let paths_cache_dir = normalize_optional_path(
            self.paths_cache_dir.as_deref(),
            "paths.cache_dir",
        )
        .map_err(|e| EffectiveConfigError::InvalidPaths(e.to_string()))?;
        let paths_log_dir = normalize_optional_path(
            self.paths_log_dir.as_deref(),
            "paths.log_dir",
        )
        .map_err(|e| EffectiveConfigError::InvalidPaths(e.to_string()))?;
        let paths_runtime_dir = normalize_optional_path(
            self.paths_runtime_dir.as_deref(),
            "paths.runtime_dir",
        )
        .map_err(|e| EffectiveConfigError::InvalidPaths(e.to_string()))?;

        if let Some(ref repo) = repo_root {
            if repo.as_path() == dataset_root_norm.as_path() {
                return Err(EffectiveConfigError::InvalidPaths(
                    "repo_root must not equal import.root".to_string(),
                ));
            }
            if repo.starts_with(dataset_root_norm.as_path()) {
                return Err(EffectiveConfigError::InvalidPaths(
                    "repo_root must not reside inside import.root".to_string(),
                ));
            }
        }

        for (name, maybe_path) in [
            ("paths.data_dir", paths_data_dir.as_ref()),
            ("paths.cache_dir", paths_cache_dir.as_ref()),
            ("paths.log_dir", paths_log_dir.as_ref()),
            ("paths.runtime_dir", paths_runtime_dir.as_ref()),
        ] {
            if let Some(p) = maybe_path {
                if p.starts_with(&dataset_root_norm) {
                    return Err(EffectiveConfigError::InvalidPaths(format!(
                        "{name} must not reside inside import.root"
                    )));
                }
            }
        }

        if self.net_transport != "tcp" && self.net_transport != "unix" {
            return Err(EffectiveConfigError::InvalidNetAddr(
                "net.transport must be tcp or unix".to_string(),
            ));
        }
        if self.net_addr.trim().is_empty() || !self.net_addr.contains(':') {
            return Err(EffectiveConfigError::InvalidNetAddr(
                "net.addr must be host:port".to_string(),
            ));
        }
        let net_unix_path_norm = if self.net_transport == "unix" {
            let p = normalize_single_path(&self.net_unix_path, "net.unix_path")
                .map_err(|e| EffectiveConfigError::InvalidPaths(e.to_string()))?;
            Some(p.to_string_lossy().to_string())
        } else {
            None
        };

        let ram_limit_bytes = resolve_ram_limit(&self.ram_limit)?;

        if let Some(mp) = &self.drive_mountpoint {
            normalize_single_path(mp, "drive.mountpoint")
                .map_err(|e| EffectiveConfigError::InvalidPaths(e.to_string()))?;
        }

        Ok(EffectiveConfig {
            dataset_root: dataset_root_norm.to_string_lossy().to_string(),
            repo_root: repo_root.map(|p| p.to_string_lossy().to_string()),
            paths_data_dir: paths_data_dir.map(|p| p.to_string_lossy().to_string()),
            paths_cache_dir: paths_cache_dir.map(|p| p.to_string_lossy().to_string()),
            paths_log_dir: paths_log_dir.map(|p| p.to_string_lossy().to_string()),
            paths_runtime_dir: paths_runtime_dir
                .map(|p| p.to_string_lossy().to_string()),
            ram_limit_bytes,
            net_unix_path: net_unix_path_norm.unwrap_or(self.net_unix_path),
            ..self
        })
    }

    fn dataset_root_from_import(&self) -> Option<String> {
        if self.dataset_root.trim().is_empty() {
            None
        } else {
            Some(self.dataset_root.clone())
        }
    }
}

#[allow(dead_code)]
fn resolve_path(raw: &str) -> PathBuf {
    normalize_single_path(raw, "path").unwrap_or_else(|e| {
        panic!("invalid QuarxConfig path: {e}");
    })
}

fn normalize_single_path(raw: &str, key: &str) -> Result<PathBuf, QuarxConfigError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(QuarxConfigError::InvalidPaths(format!(
            "{key} must not be empty"
        )));
    }

    let expanded = if trimmed.starts_with("~/") || trimmed == "~" {
        let home = std::env::var("HOME").unwrap_or_default();
        if home.is_empty() {
            return Err(QuarxConfigError::InvalidPaths(format!(
                "{key} requires HOME to be set for tilde expansion ({trimmed})"
            )));
        }
        let suffix = trimmed.trim_start_matches("~/");
        PathBuf::from(home).join(suffix)
    } else {
        PathBuf::from(trimmed)
    };

    if !expanded.is_absolute() {
        return Err(QuarxConfigError::InvalidPaths(format!(
            "{key} must be absolute ({})",
            expanded.display()
        )));
    }

    if expanded.exists() {
        Ok(expanded.canonicalize().unwrap_or(expanded))
    } else {
        Ok(expanded)
    }
}

fn normalize_path(raw: Option<&str>, key: &str) -> Result<Option<PathBuf>, QuarxConfigError> {
    match raw {
        Some(r) => Ok(Some(normalize_single_path(r, key)?)),
        None => Ok(None),
    }
}

fn normalize_optional_path(
    raw: Option<&str>,
    key: &str,
) -> Result<Option<PathBuf>, QuarxConfigError> {
    match raw {
        Some(r) => Ok(Some(normalize_single_path(r, key)?)),
        None => Ok(None),
    }
}

fn resolve_ram_limit(raw: &str) -> Result<Option<u64>, EffectiveConfigError> {
    let v = raw.trim().to_ascii_lowercase();
    if v.is_empty() || v == "full" {
        return Ok(None);
    }
    if let Some(percent_str) = v.strip_prefix("percent:") {
        let p = percent_str
            .trim()
            .parse::<u64>()
            .map_err(|_| EffectiveConfigError::InvalidPaths("core.ram.limit invalid percent".to_string()))?;
        if p == 0 || p > 100 {
            return Err(EffectiveConfigError::InvalidPaths(
                "core.ram.limit percent must be 1..100".to_string(),
            ));
        }
        let free = mem_available_bytes().unwrap_or(0);
        let cap = free.saturating_mul(p).saturating_div(100);
        return Ok(Some(cap));
    }
    if let Some(bytes_str) = v.strip_prefix("bytes:") {
        let b = bytes_str
            .trim()
            .parse::<u64>()
            .map_err(|_| EffectiveConfigError::InvalidPaths("core.ram.limit invalid bytes".to_string()))?;
        return Ok(Some(b));
    }
    Err(EffectiveConfigError::InvalidPaths(
        "core.ram.limit must be full | percent:N | bytes:X".to_string(),
    ))
}

fn mem_available_bytes() -> Option<u64> {
    let data = fs::read_to_string("/proc/meminfo").ok()?;
    for line in data.lines() {
        if line.starts_with("MemAvailable:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(kb) = parts[1].parse::<u64>() {
                    return Some(kb * 1024);
                }
            }
        }
    }
    None
}

fn full_ini_defaults() -> String {
    let mut out = String::new();
    out.push_str("[core]\n");
    out.push_str("l0_chunk = 16384\n");
    out.push_str("ram.limit = full\n");
    out.push_str("analysis.enable_znode = true\n");
    out.push_str("analysis.fs_stats_fallback = true\n");
    out.push_str("\n");
    out.push_str("[import]\n");
    out.push_str("min_file_size = 4096\n");
    out.push_str("root = /tmp/quarx-dataset\n");
    out.push_str("skip_hidden = false\n");
    out.push_str("skip_symlink = true\n");
    out.push_str("skip_zero = true\n");
    out.push_str("skip_devices = true\n");
    out.push_str("skip_special = true\n");
    out.push_str("include_glob = *.so,*.exe,*.iso\n");
    out.push_str("import_glob_mode = file-only\n");
    out.push_str("use_z = true\n");
    out.push_str("z_threshold = 10\n");
    out.push_str("\n");
    out.push_str("[paths]\n");
    out.push_str("#data_dir = /var/lib/quarxtor\n");
    out.push_str("#cache_dir = /var/cache/quarxtor\n");
    out.push_str("#log_dir = /var/log/quarxtor\n");
    out.push_str("#runtime_dir = /run/quarxtor\n");
    out.push_str("\n");
    out.push_str("[net]\n");
    out.push_str("transport = unix\n");
    out.push_str("unix_path = /tmp/quarxtor/quarxnetd.sock\n");
    out.push_str("addr = 127.0.0.1:39090\n");
    out.push_str("\n");
    out.push_str("[drive]\n");
    out.push_str("\n[daemon]\n");
    out.push_str("\n[cloud]\n");
    out
}

pub fn write_full_config(force: bool) -> Result<PathBuf, EffectiveConfigError> {
    let primary = default_ini_path().ok_or(EffectiveConfigError::MissingConfigFile)?;
    for target in [primary.clone(), PathBuf::from("/tmp").join("quarxtor").join("quarxctl.ini")] {
        if target.exists() && !force {
            return Err(EffectiveConfigError::InvalidPaths(
                "config exists; use --force to overwrite".to_string(),
            ));
        }
        if let Some(parent) = target.parent() {
            if let Err(_) = fs::create_dir_all(parent) {
                continue;
            }
        }
        if target.exists() && force {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let bak_parent = target
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            let bak = bak_parent.join(format!("quarxctl.ini.bak.{ts}"));
            fs::copy(&target, bak)
                .map_err(|e| EffectiveConfigError::InvalidPaths(e.to_string()))?;
        }
        if fs::write(&target, full_ini_defaults()).is_ok() {
            return Ok(target);
        }
    }
    Err(EffectiveConfigError::InvalidPaths(
        "failed to write config".to_string(),
    ))
}

fn apply_core_kv(cfg: &mut QuarxConfig, key: &str, val: &str) {
    let key_l = key.trim().to_lowercase();

    match key_l.as_str() {
        // Base core config
        "l0_chunk" | "l0.chunk" => {
            if let Ok(v) = val.trim().parse::<u64>() {
                cfg.l0_chunk = v;
            }
        }

        "ram.limit" | "ram_limit" => {
            cfg.ram_limit = val.trim().to_string();
        }

        "analysis.enable_znode" | "analysis.enable_z" => {
            let v = parse_bool(val, cfg.analysis_enable_znode);
            cfg.analysis_enable_znode = v;
        }

        "analysis.fs_stats_fallback" | "analysis.fs_stats.fallback" => {
            let v = parse_bool(val, cfg.analysis_fs_stats_fallback);
            cfg.analysis_fs_stats_fallback = v;
        }

        "repo_root" => {
            let v = val.trim();
            if !v.is_empty() {
                let norm = normalize_single_path(v, "repo_root")
                    .unwrap_or_else(|e| panic!("invalid QuarxConfig repo_root: {e}"));
                cfg.repo_root = Some(norm.to_string_lossy().to_string());
            }
        }

        _ => {
            // Silently ignore unknown keys in [core]
        }
    }
}

fn apply_paths_kv(cfg: &mut QuarxConfig, key: &str, val: &str) {
    let key_l = key.trim().to_ascii_lowercase();
    match key_l.as_str() {
        "data_dir" => {
            let v = val.trim();
            if !v.is_empty() {
                cfg.paths_data_dir = Some(v.to_string());
            }
        }
        "cache_dir" => {
            let v = val.trim();
            if !v.is_empty() {
                cfg.paths_cache_dir = Some(v.to_string());
            }
        }
        "log_dir" => {
            let v = val.trim();
            if !v.is_empty() {
                cfg.paths_log_dir = Some(v.to_string());
            }
        }
        "runtime_dir" => {
            let v = val.trim();
            if !v.is_empty() {
                cfg.paths_runtime_dir = Some(v.to_string());
            }
        }
        _ => {}
    }
}

fn apply_import_kv(cfg: &mut QuarxConfig, key: &str, val: &str) {
    let key_l = key.trim().to_ascii_lowercase();
    match key_l.as_str() {
        "min_file_size" => {
            cfg.import_min_file_size = parse_u64(val, cfg.import_min_file_size);
        }
        "root" => {
            let v = val.trim();
            if !v.is_empty() {
                let norm = normalize_single_path(v, "import.root")
                    .unwrap_or_else(|e| panic!("invalid QuarxConfig import.root: {e}"));
                cfg.import_root = Some(norm.to_string_lossy().to_string());
            }
        }
        "skip_hidden" => {
            cfg.import_skip_hidden = parse_bool(val, cfg.import_skip_hidden);
        }
        "skip_symlink" => {
            cfg.import_skip_symlink = parse_bool(val, cfg.import_skip_symlink);
        }
        "skip_zero" => {
            cfg.import_skip_zero = parse_bool(val, cfg.import_skip_zero);
        }
        "skip_devices" => {
            cfg.import_skip_devices = parse_bool(val, cfg.import_skip_devices);
        }
        "skip_special" => {
            cfg.import_skip_special = parse_bool(val, cfg.import_skip_special);
        }
        "include_glob" => {
            let v = val.trim();
            if !v.is_empty() {
                cfg.include_glob = Some(v.to_string());
            }
        }
        "exclude_glob" => {
            let v = val.trim();
            if !v.is_empty() {
                cfg.exclude_glob = Some(v.to_string());
            }
        }
        "include_dir_glob" => {
            let v = val.trim();
            if !v.is_empty() {
                cfg.include_dir_glob = Some(v.to_string());
            }
        }
        "exclude_dir_glob" => {
            let v = val.trim();
            if !v.is_empty() {
                cfg.exclude_dir_glob = Some(v.to_string());
            }
        }
        "import_glob_mode" => {
            let v = val.trim();
            if !v.is_empty() {
                cfg.import_glob_mode = v.to_string();
            }
        }
        // Z compression on import
        "use_z" => {
            cfg.fs_import_use_z = parse_bool(val, cfg.fs_import_use_z);
        }
        "z_threshold" => {
            cfg.fs_import_z_threshold = parse_u64(val, cfg.fs_import_z_threshold);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_err_on_equal_paths() {
        let cfg = QuarxConfig {
            repo_root: Some("/data/repo".to_string()),
            import_root: Some("/data/repo".to_string()),
            ..Default::default()
        };

        assert!(matches!(
            cfg.validate(),
            Err(QuarxConfigError::InvalidPaths(msg)) if msg.contains("must not equal")
        ));
    }

    #[test]
    fn validate_err_on_repo_inside_import() {
        let cfg = QuarxConfig {
            repo_root: Some("/data/repo".to_string()),
            import_root: Some("/data".to_string()),
            ..Default::default()
        };

        assert!(matches!(
            cfg.validate(),
            Err(QuarxConfigError::InvalidPaths(msg)) if msg.contains("inside import.root")
        ));
    }

    #[test]
    fn validate_ok_on_separate_roots() {
        let cfg = QuarxConfig {
            repo_root: Some("/data/repo".to_string()),
            import_root: Some("/mnt/src".to_string()),
            ..Default::default()
        };

        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_err_when_data_dir_under_import_root() {
        let cfg = QuarxConfig {
            import_root: Some("/data/import".to_string()),
            paths_data_dir: Some("/data/import/cache".to_string()),
            ..Default::default()
        };

        assert!(matches!(
            cfg.validate(),
            Err(QuarxConfigError::InvalidPaths(msg)) if msg.contains("paths.data_dir")
        ));
    }

    #[test]
    fn validate_err_when_cache_dir_under_import_root() {
        let cfg = QuarxConfig {
            import_root: Some("/data/import".to_string()),
            paths_cache_dir: Some("/data/import/cache".to_string()),
            ..Default::default()
        };

        assert!(matches!(
            cfg.validate(),
            Err(QuarxConfigError::InvalidPaths(msg)) if msg.contains("paths.cache_dir")
        ));
    }

    #[test]
    fn validate_err_when_log_dir_under_import_root() {
        let cfg = QuarxConfig {
            import_root: Some("/data/import".to_string()),
            paths_log_dir: Some("/data/import/logs".to_string()),
            ..Default::default()
        };

        assert!(matches!(
            cfg.validate(),
            Err(QuarxConfigError::InvalidPaths(msg)) if msg.contains("paths.log_dir")
        ));
    }

    #[test]
    fn validate_err_when_runtime_dir_under_import_root() {
        let cfg = QuarxConfig {
            import_root: Some("/data/import".to_string()),
            paths_runtime_dir: Some("/data/import/run".to_string()),
            ..Default::default()
        };

        assert!(matches!(
            cfg.validate(),
            Err(QuarxConfigError::InvalidPaths(msg)) if msg.contains("paths.runtime_dir")
        ));
    }
}
