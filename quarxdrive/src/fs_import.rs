use crate::fs_root::{ensure_root_dir, set_root_dir_id};
use crate::{DirEntry, QuarxRepo};
use quarxtor_core::config::QuarxConfig;
use quarxtor_core::store::blockstore::BlockStore;

use globset::{GlobBuilder, GlobSetBuilder};
use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct ImportStats {
    pub files: u64,
    pub dirs: u64,

    pub skipped_zero: u64,
    pub skipped_symlink: u64,
    pub skipped_hidden: u64,
    pub skipped_devices: u64,
    pub skipped_special: u64,
    pub skipped_small: u64,
    pub skipped_other: u64,
}

fn is_hidden_name(name: &str) -> bool {
    name.starts_with('.')
}

fn entry_name(path: &Path) -> Option<String> {
    path.file_name().map(|s| s.to_string_lossy().to_string())
}

fn build_globset(csv: &Option<String>) -> Option<globset::GlobSet> {
    let s = match csv {
        Some(s) => s,
        None => return None,
    };

    let mut builder = GlobSetBuilder::new();
    for pat in s.split(',').map(|p| p.trim()).filter(|p| !p.is_empty()) {
        let g = match GlobBuilder::new(pat).case_insensitive(true).build() {
            Ok(g) => g,
            Err(_) => continue,
        };
        builder.add(g);
    }

    builder.build().ok()
}

fn rel_path_str(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.to_string_lossy().to_string()
}

fn glob_key<'a>(cfg: &QuarxConfig, name: &'a str, rel: &'a str) -> &'a str {
    match cfg.import_glob_mode.as_str() {
        "path" => rel,
        _ => name,
    }
}

/// Recursive import of a host FS directory into a QuarX Dir object.
///
/// Returns the directory BlockId.
#[allow(clippy::too_many_arguments)]
fn import_dir_recursive<S: BlockStore>(
    cfg: &QuarxConfig,
    repo: &mut QuarxRepo<S>,
    host_root: &Path,
    host_dir: &Path,
    stats: &mut ImportStats,
    glob_inc_file: &Option<globset::GlobSet>,
    glob_exc_file: &Option<globset::GlobSet>,
    glob_inc_dir: &Option<globset::GlobSet>,
    glob_exc_dir: &Option<globset::GlobSet>,
    debug_import: bool,
) -> Result<u64, String> {
    let mut entries: Vec<DirEntry> = Vec::new();

    let rd = fs::read_dir(host_dir).map_err(|e| format!("read_dir({:?}) failed: {e}", host_dir))?;

    for ent in rd {
        let ent = ent.map_err(|e| format!("read_dir entry error in {:?}: {e}", host_dir))?;
        let path = ent.path();
        let name = match entry_name(&path) {
            Some(n) => n,
            None => continue,
        };

        let rel = rel_path_str(host_root, &path);
        let key = glob_key(cfg, &name, &rel);

        // hidden?
        if cfg.import_skip_hidden && is_hidden_name(&name) {
            if debug_import {
                eprintln!("[import][SKIP][hidden] {} (name={})", rel, name);
            }
            stats.skipped_hidden += 1;
            continue;
        }

        let md = fs::symlink_metadata(&path)
            .map_err(|e| format!("symlink_metadata({:?}) failed: {e}", path))?;
        let ft = md.file_type();

        // symlink
        if ft.is_symlink() {
            if cfg.import_skip_symlink {
                if debug_import {
                    eprintln!("[import][SKIP][symlink] {} (name={})", rel, name);
                }
                stats.skipped_symlink += 1;
                continue;
            } else {
                if debug_import {
                    eprintln!("[import][SKIP][symlink-other] {} (name={})", rel, name);
                }
                stats.skipped_other += 1;
                continue;
            }
        }

        // directory
        if ft.is_dir() {
            // dir-globs
            if let Some(gs) = glob_exc_dir {
                if gs.is_match(key) {
                    if debug_import {
                        eprintln!("[import][SKIP][dir-exclude] {} (key={})", rel, key);
                    }
                    stats.skipped_other += 1;
                    continue;
                }
            }
            if let Some(gs) = glob_inc_dir {
                if !gs.is_match(key) {
                    if debug_import {
                        eprintln!("[import][SKIP][dir-include-miss] {} (key={})", rel, key);
                    }
                    stats.skipped_other += 1;
                    continue;
                }
            }

            stats.dirs += 1;
            let child_id = import_dir_recursive(
                cfg,
                repo,
                host_root,
                &path,
                stats,
                glob_inc_file,
                glob_exc_file,
                glob_inc_dir,
                glob_exc_dir,
                debug_import,
            )?;

            if debug_import {
                eprintln!("[import][MATCH][dir] {} -> {}", rel, child_id);
            }

            entries.push(DirEntry {
                name,
                target: child_id,
            });
            continue;
        }

        // regular file
        if ft.is_file() {
            // file-globs
            if let Some(gs) = glob_inc_file {
                if !gs.is_match(key) {
                    if debug_import {
                        eprintln!("[import][SKIP][file-include-miss] {} (key={})", rel, key);
                    }
                    stats.skipped_other += 1;
                    continue;
                }
            }
            if let Some(gs) = glob_exc_file {
                if gs.is_match(key) {
                    if debug_import {
                        eprintln!("[import][SKIP][file-exclude] {} (key={})", rel, key);
                    }
                    stats.skipped_other += 1;
                    continue;
                }
            }

            let len = md.len();

            if len == 0 && cfg.import_skip_zero {
                if debug_import {
                    eprintln!("[import][SKIP][zero] {} (len=0)", rel);
                }
                stats.skipped_zero += 1;
                continue;
            }

            if cfg.import_min_file_size > 0 && len < cfg.import_min_file_size {
                if debug_import {
                    eprintln!(
                        "[import][SKIP][small] {} (len={}, min={})",
                        rel, len, cfg.import_min_file_size
                    );
                }
                stats.skipped_small += 1;
                continue;
            }

            let data = fs::read(&path).map_err(|e| format!("read({:?}) failed: {e}", path))?;
            let obj_id = repo
                .put_bytes_simple(&data)
                .map_err(|e| format!("put_bytes_simple({:?}) failed: {e:?}", path))?;

            stats.files += 1;

            if debug_import {
                eprintln!(
                    "[import][MATCH][file] {} (len={}) -> obj={}",
                    rel, len, obj_id
                );
            }

            entries.push(DirEntry {
                name,
                target: obj_id,
            });
            continue;
        }

        // devices / special types / other
        if ft.is_block_device() || ft.is_char_device() {
            if cfg.import_skip_devices {
                if debug_import {
                    eprintln!("[import][SKIP][device] {} (key={})", rel, key);
                }
                stats.skipped_devices += 1;
                continue;
            }
        } else if (ft.is_fifo() || ft.is_socket()) && cfg.import_skip_special {
            if debug_import {
                eprintln!("[import][SKIP][special] {} (key={})", rel, key);
            }
            stats.skipped_special += 1;
            continue;
        }

        if debug_import {
            eprintln!("[import][SKIP][other] {} (key={})", rel, key);
        }
        stats.skipped_other += 1;
    }

    let dir_id = repo
        .put_dir(&entries)
        .map_err(|e| format!("put_dir({:?}) failed: {e:?}", host_dir))?;

    Ok(dir_id)
}

/// Import host directory `host_root` into QuarX FS root.
///
/// Returns:
///   (new_root_id, ImportStats)
pub fn import_host_dir<S: BlockStore>(
    cfg: &QuarxConfig,
    store_path: &Path,
    repo: &mut QuarxRepo<S>,
    host_root: &Path,
    debug_import: bool,
) -> Result<(u64, ImportStats), String> {
    let meta =
        fs::metadata(host_root).map_err(|e| format!("metadata({:?}) failed: {e}", host_root))?;
    if !meta.is_dir() {
        return Err(format!("source path is not a directory: {:?}", host_root));
    }

    let mut stats = ImportStats::default();
    let glob_inc_file = build_globset(&cfg.include_glob);
    let glob_exc_file = build_globset(&cfg.exclude_glob);
    let glob_inc_dir = build_globset(&cfg.include_dir_glob);
    let glob_exc_dir = build_globset(&cfg.exclude_dir_glob);

    // Ensure root exists (old id retrieved but not used)
    let _old_root_id = ensure_root_dir(store_path, repo)?;

    let mut root_entries: Vec<DirEntry> = Vec::new();

    let rd =
        fs::read_dir(host_root).map_err(|e| format!("read_dir({:?}) failed: {e}", host_root))?;

    for ent in rd {
        let ent = ent.map_err(|e| format!("read_dir entry error in {:?}: {e}", host_root))?;
        let path = ent.path();
        let name = match entry_name(&path) {
            Some(n) => n,
            None => continue,
        };

        let rel = rel_path_str(host_root, &path);
        let key = glob_key(cfg, &name, &rel);

        if cfg.import_skip_hidden && is_hidden_name(&name) {
            if debug_import {
                eprintln!("[import][SKIP][root-hidden] {} (name={})", rel, name);
            }
            stats.skipped_hidden += 1;
            continue;
        }

        let md = fs::symlink_metadata(&path)
            .map_err(|e| format!("symlink_metadata({:?}) failed: {e}", path))?;
        let ft = md.file_type();

        if ft.is_symlink() {
            if cfg.import_skip_symlink {
                if debug_import {
                    eprintln!("[import][SKIP][root-symlink] {} (name={})", rel, name);
                }
                stats.skipped_symlink += 1;
                continue;
            } else {
                if debug_import {
                    eprintln!("[import][SKIP][root-symlink-other] {} (name={})", rel, name);
                }
                stats.skipped_other += 1;
                continue;
            }
        }

        if ft.is_dir() {
            // dir globs at root level
            if let Some(gs) = &glob_exc_dir {
                if gs.is_match(key) {
                    if debug_import {
                        eprintln!("[import][SKIP][root-dir-exclude] {} (key={})", rel, key);
                    }
                    stats.skipped_other += 1;
                    continue;
                }
            }
            if let Some(gs) = &glob_inc_dir {
                if !gs.is_match(key) {
                    if debug_import {
                        eprintln!(
                            "[import][SKIP][root-dir-include-miss] {} (key={})",
                            rel, key
                        );
                    }
                    stats.skipped_other += 1;
                    continue;
                }
            }

            let child_id = import_dir_recursive(
                cfg,
                repo,
                host_root,
                &path,
                &mut stats,
                &glob_inc_file,
                &glob_exc_file,
                &glob_inc_dir,
                &glob_exc_dir,
                debug_import,
            )?;
            if debug_import {
                eprintln!("[import][MATCH][root-dir] {} -> {}", rel, child_id);
            }
            root_entries.push(DirEntry {
                name,
                target: child_id,
            });
            continue;
        }

        if ft.is_file() {
            // file globs at root level
            if let Some(gs) = &glob_inc_file {
                if !gs.is_match(key) {
                    if debug_import {
                        eprintln!(
                            "[import][SKIP][root-file-include-miss] {} (key={})",
                            rel, key
                        );
                    }
                    stats.skipped_other += 1;
                    continue;
                }
            }
            if let Some(gs) = &glob_exc_file {
                if gs.is_match(key) {
                    if debug_import {
                        eprintln!("[import][SKIP][root-file-exclude] {} (key={})", rel, key);
                    }
                    stats.skipped_other += 1;
                    continue;
                }
            }

            let len = md.len();

            if len == 0 && cfg.import_skip_zero {
                if debug_import {
                    eprintln!("[import][SKIP][root-zero] {} (len=0)", rel);
                }
                stats.skipped_zero += 1;
                continue;
            }

            if cfg.import_min_file_size > 0 && len < cfg.import_min_file_size {
                if debug_import {
                    eprintln!(
                        "[import][SKIP][root-small] {} (len={}, min={})",
                        rel, len, cfg.import_min_file_size
                    );
                }
                stats.skipped_small += 1;
                continue;
            }

            let data = fs::read(&path).map_err(|e| format!("read({:?}) failed: {e}", path))?;
            let obj_id = repo
                .put_bytes_simple(&data)
                .map_err(|e| format!("put_bytes_simple({:?}) failed: {e:?}", path))?;

            stats.files += 1;

            if debug_import {
                eprintln!(
                    "[import][MATCH][root-file] {} (len={}) -> obj={}",
                    rel, len, obj_id
                );
            }

            root_entries.push(DirEntry {
                name,
                target: obj_id,
            });
            continue;
        }

        if ft.is_block_device() || ft.is_char_device() {
            if cfg.import_skip_devices {
                if debug_import {
                    eprintln!("[import][SKIP][root-device] {} (key={})", rel, key);
                }
                stats.skipped_devices += 1;
                continue;
            }
        } else if (ft.is_fifo() || ft.is_socket()) && cfg.import_skip_special {
            if debug_import {
                eprintln!("[import][SKIP][root-special] {} (key={})", rel, key);
            }
            stats.skipped_special += 1;
            continue;
        }

        if debug_import {
            eprintln!("[import][SKIP][root-other] {} (key={})", rel, key);
        }
        stats.skipped_other += 1;
    }

    let new_root_id = repo
        .put_dir(&root_entries)
        .map_err(|e| format!("put_dir(root) failed: {e:?}"))?;

    set_root_dir_id(store_path, repo, new_root_id)?;

    Ok((new_root_id, stats))
}
