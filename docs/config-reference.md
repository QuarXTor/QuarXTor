# QuarXTor configuration contract

This page mirrors the Rust config contract in `core-rs/src/config.rs`. Every key below is defined in code. Keys not listed here are unsupported.

## Key reference

| Key | Type | Default | Env Override | Status | Description |
| --- | ---- | ------- | ------------ | ------ | ----------- |
| core.l0_chunk | u64 | 16384 | `QUARX_L0_CHUNK` | IMPLEMENTED | L0 chunk size in bytes for new data blocks. |
| core.ram.limit | string | full | `QUARX_RAM_LIMIT` | PARTIAL | RAM tier policy: full / limit / auto (future). Effective cap is computed; enforcement is limited. |
| core.analysis.enable_znode | bool | true | `QUARX_ANALYSIS_ENABLE_ZNODE` | IMPLEMENTED | Enable ZNode / cheap-size analysis during import. |
| core.analysis.fs_stats_fallback | bool | true | `QUARX_ANALYSIS_FS_STATS_FALLBACK` | IMPLEMENTED | Fallback to computing real file sizes when cheap-size is missing. |
| core.repo_root | path |  | `QUARX_REPO_ROOT` | IMPLEMENTED | Canonical repository root. Must not equal or reside under import.root. |
| import.root | path |  | `QUARX_DATASET_DIR` | IMPLEMENTED | Root of the source tree to import. Must be absolute; required if QUARX_DATASET_DIR not set. |
| import.min_file_size | u64 | 4096 | `QUARX_IMPORT_MIN_FILE_SIZE` | IMPLEMENTED | Minimum file size to import in bytes. |
| import.skip_hidden | bool | false | `QUARX_IMPORT_SKIP_HIDDEN` | IMPLEMENTED | Skip hidden files/directories (leading dot). |
| import.skip_symlink | bool | true | `QUARX_IMPORT_SKIP_SYMLINK` | IMPLEMENTED | Do not follow symlinks. |
| import.skip_zero | bool | true | `QUARX_IMPORT_SKIP_ZERO` | IMPLEMENTED | Skip zero-sized files. |
| import.skip_devices | bool | true | `QUARX_IMPORT_SKIP_DEVICES` | IMPLEMENTED | Skip device files (block/char). |
| import.skip_special | bool | true | `QUARX_IMPORT_SKIP_SPECIAL` | IMPLEMENTED | Skip sockets/pipes and other special types. |
| import.include_glob | string | \*.so,\*.exe,\*.iso | `QUARX_IMPORT_INCLUDE_GLOB` | IMPLEMENTED | CSV glob filter for files to include. |
| import.exclude_glob | string |  | `QUARX_IMPORT_EXCLUDE_GLOB` | IMPLEMENTED | CSV glob filter for files to exclude. |
| import.include_dir_glob | string |  | `QUARX_IMPORT_INCLUDE_DIR_GLOB` | IMPLEMENTED | CSV glob filter for directories to include. |
| import.exclude_dir_glob | string |  | `QUARX_IMPORT_EXCLUDE_DIR_GLOB` | IMPLEMENTED | CSV glob filter for directories to exclude. |
| import.import_glob_mode | enum | file-only | `QUARX_IMPORT_GLOB_MODE` | IMPLEMENTED | Glob application mode: file-only / dir-only / both. |
| import.use_z | bool | true | `QUARX_IMPORT_USE_Z` | IMPLEMENTED | Enable Z compression on import. |
| import.z_threshold | u64 | 10 | `QUARX_IMPORT_Z_THRESHOLD` | IMPLEMENTED | Z compression usefulness threshold (heuristic). |
| paths.data_dir | path |  | `QUARX_DATA_DIR` | IMPLEMENTED | Base directory for repository data (absolute). |
| paths.cache_dir | path |  | `QUARX_CACHE_DIR` | IMPLEMENTED | Cache directory (absolute). |
| paths.log_dir | path |  | `QUARX_LOG_DIR` | IMPLEMENTED | Log directory (absolute). |
| paths.runtime_dir | path |  | `QUARX_RUNTIME_DIR` | IMPLEMENTED | Runtime directory for pid/log sockets (absolute). |
| net.addr | string | 0.0.0.0:54721 | `QUARX_NET_ADDR` | IMPLEMENTED | Canonical listen address host:port for quarxnetd. |
| drive.mountpoint | path |  |  | RESERVED | Reserved for drive mountpoint settings. |
| drive.strategy | string |  |  | RESERVED | Reserved for drive caching/overlay strategy. |
| drive.fs_cache_bytes | u64 | 0 |  | RESERVED | Reserved for drive cache limit. |
| daemon.enable_netd | bool | false |  | RESERVED | Reserved daemon flag for quarxnetd enablement. |
| cloud.seed_nodes | string |  |  | RESERVED | Reserved for future clustering seed nodes. |

## Gap audit

- RESERVED: drive.mountpoint, drive.strategy, drive.fs_cache_bytes, daemon.enable_netd, cloud.seed_nodes
- NOT IMPLEMENTED: (none)
