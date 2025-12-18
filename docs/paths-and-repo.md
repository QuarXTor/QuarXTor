## Appendix A. QuarXTor path and repository model

### A.1. Terms and invariants

QuarXTor distinguishes three kinds of paths:
- Physical repository root (`[core].repo_root`),
- Import source tree (`[import].root`),
- Service/layout dirs (`[paths].data_dir/cache_dir/log_dir/runtime_dir`).
These are tied together by invariants that `QuarxConfig::validate()` enforces at startup.

**Base entities:**

* **`[paths].data_dir`**  
  Base directory for *persistent data* of QuarXTor:
  * repositories (repo files/directories),
  * index structures,
  * auxiliary DBs and metadata.

* **`[paths].cache_dir`**  
  Directory for caches / temporary structures that are safe to delete:
  * secondary indexes,
  * intermediate analysis structures,
  * warmed snapshots that can be rebuilt.

* **`[paths].log_dir`**  
  Directory for QuarXTor logs:
  * daemon logs (`quarxnetd`, future `quarxcloudd`, `drived`),
  * core logs (as integration progresses).

* **`[paths].runtime_dir`**  
  Runtime artifacts:
  * sockets (UDS / named pipe),
  * PID files,
  * temporary lock files.

* **`[core].repo_root`**  
  *Root of the HGSS/QuarXTor repository*:
  * physical location of the storage itself (arena/pagefile, indexes, dictionaries, etc.),
  * NOT the directory with source data.

* **`[import].root`**  
  *Source directory for import (“quarking”)*:
  * host FS tree that is read and ingested into the repository.

**Hard invariants:**

1. `core.repo_root` **≠** `import.root`. The repo cannot live inside the tree it imports.

2. `core.repo_root` **must not reside inside** `import.root`. Even if names differ, the case below is forbidden:
   * `repo_root = /data/repo`
   * `import.root = /data`
   * → the repo will start consuming itself.

3. Best practice:
   * `core.repo_root` should reside under `[paths].data_dir`,
   * `[paths].data_dir`, `[paths].cache_dir`, `[paths].log_dir`, `[paths].runtime_dir`
     should **not** be inside `import.root`.

4. During import QuarXTor additionally self-filters:
   * any paths under `repo_root`,
   * any paths under `data_dir`, `cache_dir`, `log_dir`, `runtime_dir`.  
     Even if the user mistakenly sets `import.root` too wide.

---

### A.2. Path model: OS mapping

#### A.2.1. Linux (user scope)

Recommended layout for a typical user:

```ini
[paths]
data_dir    = $HOME/.local/share/quarxtor
cache_dir   = $HOME/.cache/quarxtor
log_dir     = $HOME/.local/share/quarxtor/logs
runtime_dir = /run/user/$UID/quarxtor

[core]
repo_root = $HOME/.local/share/quarxtor/repo

[import]
root = /home/USER/SOURCE-DB   ; example, real path is project-specific
```

Semantics:
* `data_dir` — “permanent” data (repos + important metadata).
* `cache_dir` — can be deleted without breaking repository integrity.
* `log_dir` — isolate logs from actual data.
* `runtime_dir` — environment-specific, for Linux via XDG — `/run/user/$UID/…`.

#### A.2.2. Windows

Approximate mapping (example, not the final implementation):

```ini
[paths]
data_dir    = C:\Users\USER\AppData\Local\QuarXTor\data
cache_dir   = C:\Users\USER\AppData\Local\QuarXTor\cache
log_dir     = C:\Users\USER\AppData\Local\QuarXTor\logs
runtime_dir = C:\Users\USER\AppData\Local\QuarXTor\runtime

[core]
repo_root = C:\Users\USER\AppData\Local\QuarXTor\data\repo.hgss

[import]
root = D:\Data\SourceTree
```

* Actual paths will be adjusted to `%LOCALAPPDATA%` / `%APPDATA%`.
* Separation of data/cache/log/runtime is preserved.

#### A.2.3. Other OS / system-wide

* For system-wide installs (Linux):
  * `data_dir` could be `/var/lib/quarxtor`,
  * `cache_dir` — `/var/cache/quarxtor`,
  * `log_dir` — `/var/log/quarxtor`,
  * `runtime_dir` — `/run/quarxtor`.

* User-specific settings still live in:
  * `$XDG_CONFIG_HOME/quarxtor/…` or `$HOME/.config/quarxtor/…`.

---

### A.3. Repository: layout and backend

#### A.3.1. Semantics of `repo_root`

`repo_root` defines **one logical repository** HGSS/QuarXTor:
* This is *not a random file tree*, but a *structured store*:
  * arena/pagefile with blocks,
  * index tables,
  * dictionaries,
  * service metadata.

* Repository is managed through the `BlockStore` abstraction:
  * core (`core-rs`) and network layer (`quarxnet`) see only the interface,
  * physical implementation (files/directories) is a detail of `FileBlockStore` and future backends.
* `repo_root` is the canonical on-disk repository location; it must not be equal to `[import].root` and must not reside inside `[import].root` (enforced by validation).

#### A.3.2. Option 1: `repo_root` as a file

Example:

```ini
[core]
repo_root = /home/jee/.local/share/quarxtor/repo.hgss
```

* Single entity (file) `repo.hgss`:
  * inside — pagefile/arena,
  * tables/indexes may be embedded or located nearby with agreed naming.

From user perspective:
* Repo is a “monolithic” blob,
* convenient to move/backup as one file.

#### A.3.3. Option 2: `repo_root` as a directory

Example:

```ini
[core]
repo_root = /home/jee/.local/share/quarxtor/repo
```

* Inside the directory — structured storage:
  * `repo.hgss` (arena),
  * `index/…`,
  * `meta/…`,
  * `dict/…`,
  * etc.

From user perspective:
* Repo is a directory with predictable structure,
* you can operate on parts separately (e.g., backup only `repo.hgss`).

#### A.3.4. Hot files and RAM

Design level:
* Repository may have *hot components*:
  * frequently used tables/indexes,
  * hot dictionary,
  * small reference structures.

Planned direction:
* `[core]` or `[drive]` will have flags:
  * `hot.tables_in_ram = true/false`
  * `hot.dict_in_ram = true/false`
  * `hot.cache_bytes = <bytes>` — RAM limit for these objects.

* Implementation can use:
  * mmap + OS page cache,
  * full RAM layer (`RamStore`) over file backend.

**Important:** placement of these objects is still tied to `repo_root`:
* all “hot” persistent components live inside the repository,
* RAM is about *reflection* / access acceleration, not about a separate “repo in memory”.

---

### A.4. Import: `import.root` and filtering

#### A.4.1. `import.root`

`[import].root` — *source data tree* to be imported:

```ini
[import]
root = /home/jee/SOURCE-DB
```

When importer runs:
* QuarXDrive / import tools:
  * walk `import.root` recursively,
  * apply filters (size, glob, object types),
  * convert files to blocks/objects and write into repository (`repo_root`).
* `repo_root` and any internal directories (data_dir/cache_dir/log_dir/runtime_dir) must live outside this subtree to avoid self-import/recursion.

#### A.4.2. Type filters

Parameters:

```ini
skip_hidden   = false
skip_symlink  = true
skip_zero     = true
skip_devices  = true
skip_special  = true
```

* Work strictly at host FS level:
  * what to ignore entirely,
  * where not to descend recursively (symlink / special dirs, etc.).

#### A.4.3. Name filters (glob)

```ini
include_glob      = *.so,*.exe,*.iso
#exclude_glob    = *.tmp,*.log,*.bak
#include_dir_glob = src,include
#exclude_dir_glob= .git,node_modules
import_glob_mode  = file-only
```

* `include_glob` / `exclude_glob` — for files.
* `include_dir_glob` / `exclude_dir_glob` — for directories.
* `import_glob_mode` defines target (files / dirs / both).

#### A.4.4. Self-cannibalization guard

Even with incorrect config:
* If a path `P` during traversal of `import.root`:
  * matches `repo_root`, or
  * lies under `repo_root`, or
  * lies under `data_dir`, `cache_dir`, `log_dir`, `runtime_dir`,

then QuarXTor **must**:
* not descend there recursively,
* not import files from there,
* optionally log a warning.

This is guaranteed at importer level even if user set `import.root` too wide (`/home/jee`, etc.).

---

### A.5. Interaction core / net / drive

#### A.5.1. QuarxCore and Engine

* `QuarxCore` loads `QuarxConfig` (single INI).
* Based on `[core].repo_root` opens backend (via `Engine::open_from_repo()`).
* `Engine` stores aggregated repo stats (`RepoStats`, etc.).

#### A.5.2. quarxnetd

* `quarxnetd` runs `QuarxCore::open_with_default_config()`:
  * reads INI,
  * opens repository by `repo_root`,
  * returns `RepoStats` to clients via Net API.

* `import.root` is not critical for `quarxnetd`:
  * it is a source of data for individual commands (import/analysis),
  * repo lives separately.

#### A.5.3. QuarXDrive

* Uses `QuarxConfig`:
  * `core` — L0 params, repo_root, Z layers,
  * `import` — filters and source directory,
  * `drive` — mounting, cache strategy (as implemented).

* When mounting:
  * reads data from `repo_root`,
  * may use `cache_dir` for local overlays/caches.

---

### A.6. `[paths]` layout and relation to import

- `data_dir`: preferred parent for `repo_root` (repository data).
- `cache_dir`: caches and transient artifacts.
- `log_dir`: logs.
- `runtime_dir`: pid/state/runtime files.
- None of these may reside inside `[import].root`; validation rejects such configs.

Example layout:

```text
/mnt/source-tree        # [import].root
/var/lib/quarxtor       # [paths].data_dir
  /var/lib/quarxtor/repo    # [core].repo_root
/var/cache/quarxtor     # [paths].cache_dir
/var/log/quarxtor       # [paths].log_dir
/run/quarxtor           # [paths].runtime_dir
```

In this layout, import walks only the source tree; internal repo/cache/log/runtime stay outside.

### A.7. Summary / invariants

- `repo_root != [import].root`.
- `repo_root` must not be under `[import].root`.
- `data_dir`/`cache_dir`/`log_dir`/`runtime_dir` must not be under `[import].root`.
- Import tooling must never traverse into repo/log/cache/runtime paths.

---

### A.6. Practical examples

#### A.6.1. Workstation (single user)

```ini
[paths]
data_dir    = /home/jee/.local/share/quarxtor
cache_dir   = /home/jee/.cache/quarxtor
log_dir     = /home/jee/.local/share/quarxtor/logs
runtime_dir = /run/user/1000/quarxtor

[core]
l0_chunk   = 16384
repo_root  = /home/jee/.local/share/quarxtor/repo

[import]
root             = /home/jee/SOURCE-DB
min_file_size    = 4096
skip_hidden      = false
skip_symlink     = true
skip_zero        = true
skip_devices     = true
skip_special     = true
include_glob     = *.so,*.exe,*.iso
import_glob_mode = file-only

[net]
addr = 127.0.0.1:54721
```

* Repository:
  * isolated at `~/.local/share/quarxtor/repo`,
* Source data:
  * live in `/home/jee/SOURCE-DB`,
* Import and analysis:
  * never touch QuarXTor internals.

#### A.6.2. Server / cluster (sketch)

```ini
[paths]
data_dir    = /var/lib/quarxtor
cache_dir   = /var/cache/quarxtor
log_dir     = /var/log/quarxtor
runtime_dir = /run/quarxtor

[core]
repo_root = /var/lib/quarxtor/main-repo.hgss

[import]
root = /srv/datasets

[net]
addr = 0.0.0.0:54721
```

* Repository — single file `main-repo.hgss`.
* Import from `/srv/datasets`.
* Server listens on all interfaces, ready for LAN/cluster.

---

If this looks fine — this chunk can be dropped into docs as “Appendix A” and treated as the baseline path/repository model.

Next priorities (brief):

1. **Config validator**:
   * on INI load check invariants `repo_root` / `import.root` / `[paths]`,
   * log and fail/warn on violations.

2. **Hard skip of internal paths during import**:
   * importer should check against `repo_root`, `data_dir`, `cache_dir`, `log_dir`, `runtime_dir`.

3. **“Config v2” doc page**:
   * move description of all sections (`[paths]/[core]/[import]/[net]/[drive]/[daemon]/[cloud]`) into a dedicated reference.
