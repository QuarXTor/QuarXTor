# ROADMAP-TARGET.md
## QuarXTor — Target Execution Roadmap (Auto-Progress)

Status: TARGET (authoritative for execution)
Scope: Engineering execution, sequencing, progress tracking, evidence ledger
Authority: MUST NOT contradict ARCH-TARGET.md

This file is the **single operational ledger** of the project.
If something exists, is planned, or is being worked on — it must appear here.

Primary requirement:
- **Progress must be auto-computable** from checkboxes / counters in this file.
- Every task has explicit evidence items. If evidence is missing, task is not done.

No speculative scope.
No parallel realities.
No silent changes.

---

## 0. Auto-Progress Rules (Hard)

### 0.1 Units of Progress
- Progress is computed from **atomic items**:
  - `[ ]` / `[x]` checkboxes
  - counters like `DONE: 3/10`
- Each epic has a fixed number of atomic items and a weight.

### 0.2 Global Project Progress Formula
Let each Epic `Ei` have:
- `Wi` weight (integer)
- `Pi` epic progress in `[0..1]`

Then:

`PROJECT_PROGRESS = (Σ(Wi * Pi) / Σ(Wi)) * 100%`

### 0.3 Epic Progress Formula
For each epic, define a list of atomic items `Ai`.
- Each atomic item is one checkbox line: `- [ ] ...`
- Epic progress is:

`Pi = checked_items / total_items`

If an epic uses counters, they must also be expressible as checkboxes or `DONE: a/b`.

### 0.4 Definition of DONE
A task is DONE only if:
- All its checkboxes are checked
- Required evidence links/paths are present (as literal file paths or command outputs)

If it compiles but no evidence is recorded → not DONE.

### 0.5 Evidence Format
Evidence items must be one of:
- file path: `EVIDENCE: path/to/file.ext`
- command that was run + short output snippet:
  - `CMD: <command>`
  - `OUT: <1-10 lines>`
- benchmark summary:
  - `BENCH: <name>`
  - `RESULT: <numbers>`

### 0.6 Scope Lock
- No new epics without explicitly stating:
  - why it is required by ARCH-TARGET
  - what epic weight changes
- Dropped items are never deleted; marked `[DROP]` and checkboxes remain (unchecked).

## 0.7 Tooling (Authoritative, MUST USE)

These tools are part of the execution contract. Codex MUST use them to select work and to report progress.

### 0.7.1 Progress calculator
Tool: `tools/roadmap_progress.py`

Run:
- `python3 tools/roadmap_progress.py --file ROADMAP-TARGET.md`
- (optional) `./tools/roadmap_progress.py --file ROADMAP-TARGET.md`

Output is authoritative for:
- PROJECT progress %
- per-EPIC progress %
- Gate state (BLOCKED/OK)

### 0.7.2 Next-task selector (deterministic)
Tool: `tools/roadmap_next.py`

Run:
- `python3 tools/roadmap_next.py --file ROADMAP-TARGET.md --print-range`
- (optional) `./tools/roadmap_next.py --file ROADMAP-TARGET.md --print-range`

Output is authoritative for:
- current SCOPE (Gate A/B/C or unlocked)
- selected EPIC + section
- NEXT checkbox line and its line number
- excerpt range for human review

### 0.7.3 Mandatory usage rule (hard)
For every execution pass:
1) Determine NEXT work item ONLY via `tools/roadmap_next.py`.
2) Implement changes and record EVIDENCE under the task/section.
3) Check ONLY the completed checkbox(es).
4) Print updated progress via `tools/roadmap_progress.py` and include the output in the session log.
5) If a task is checked without EVIDENCE → revert the checkbox (task not done).

### 0.7.4 Tool stability
- Tool changes are not allowed ad-hoc.
- Any modification of tooling requires an explicit ROADMAP item under EPIC E0 and its own EVIDENCE.

---

## 1. Status Legend

- `[PLAN]` defined & ready
- `[DOING]` in progress
- `[WAIT]` blocked
- `[DONE]` completed
- `[DROP]` intentionally dropped (reason)

---

## 2. Known Existing Artifacts (Inventory Snapshot)

These are already considered “exists” and must be referenced by tasks that depend on them:

- [x] `ARCH-TARGET.md` (Target Architecture Contract)
- [x] `core-rs/` exists (Core implementation)
- [x] `quarxdrive/` exists (Drive layer)
- [x] `quarxnet/` exists (Net layer skeleton / partial)
- [x] `core-rs/src/store/file_store.rs` (file-backed store with dense offset index `Vec<u64>`)
- [x] `core-rs/src/store/mem_store.rs` (in-memory store using `HashMap<BlockId, Vec<u8>>` — target for densification)
- [x] `core-rs/src/graph/object_graph.rs` (graph traversal / closure)
- [x] `core-rs/src/gc.rs` (GC logic with reachability)
- [x] `core-rs/src/validate.rs` (validation primitives)
- [x] typed block encoding/decoding (`core-rs/src/store/encode.rs`, `decode.rs`, `blockstore.rs`)
- [x] tests exist:
  - `core-rs/tests/gc_*`
  - `core-rs/tests/analysis_patterns.rs`
  - `core-rs/tests/store_roundtrip.rs`

EVIDENCE:
- `core-rs/src/store/file_store.rs`
- `core-rs/src/store/mem_store.rs`
- `core-rs/src/graph/object_graph.rs`
- `core-rs/src/gc.rs`
- `core-rs/src/validate.rs`

---

## 3. Epics, Weights, and Gates

### Weights (must remain stable unless explicitly changed)
- E0 Control Plane & Discipline: `W=3`
- E1 Core Correctness & Invariants: `W=12`
- E2 Repo Format & Recovery: `W=10`
- E3 Drive FS UX & Determinism: `W=10`
- E4 Dedup & Re-import Proof: `W=6`
- E5 Z:* Structural Blocks & Analysis hooks: `W=6`
- E6 Telemetry & Analytics (Binary-first): `W=6`
- E7 Stability & Long-run hardening: `W=7`
- E8 Performance Scaling (after gates): `W=5`
- E9 Networking Minimal Transport: `W=4`
- E10 Cloud/Cluster Coordination: `W=2`
- E11 Reporting & UX truthfulness: `W=4`

### Phase Gates (Hard)
Gate A (MVP correctness gate):
- E1 ≥ 95%
- E2 ≥ 70%
- E3 ≥ 60%
- `cargo test` green for core + drive baseline

Gate B (pre-net gate):
- E4 ≥ 90%
- E6 ≥ 80%
- E7 ≥ 70%

Gate C (pre-cloud gate):
- E9 ≥ 80%

---

# EPIC E0 (W=3) — Control Plane & Project Discipline
Goal: prevent drift, duplication, and context loss.

## E0.1 [PLAN] Canonical docs are enforced
- [x] `ARCH-TARGET.md` exists in repo root
- [x] `ROADMAP-TARGET.md` exists in repo root
- [ ] `AGENTS.md` references both as authoritative
- [ ] `ARCH-TARGET.md` listed as architecture authority in `AGENTS.md`
- [ ] `ROADMAP-TARGET.md` listed as execution authority in `AGENTS.md`

EVIDENCE:
- `AGENTS.md`

## E0.2 [PLAN] Repo hygiene: scripts + transient files policy
- [ ] Root contains no ad-hoc `*.sh` scripts (except documented tooling)
- [ ] `.gitignore` explicitly covers `backups/`, `target/`, and known temp artifacts
- [ ] External runner convention documented (`../qx-run/` or similar)

EVIDENCE:
- `.gitignore`
- `docs/` or `AGENTS.md` section

## E0.3 [PLAN] Stable checkpoints rule
- [ ] Tagging policy written (what triggers tag, naming scheme)
- [ ] First baseline tag created (e.g., `v0.1.0-mvp-baseline`)
- [ ] “Safety guard” rules for resets are recorded (git status, reflog, save diff before reset)

EVIDENCE:
- `AGENTS.md` (policy section)
- `git tag --list` output in a log entry (optional file `docs/progress-log.md`)

---

# EPIC E1 (W=12) — Core Correctness & Invariants
Goal: graph + store + verify + GC correct before speed.

## E1.1 [DONE] Typed block model and IDs are stable (already implemented)
- [x] `BlockId=u64` used throughout core
- [x] typed blocks exist (`L0`, `Multi`, `Z`, `Object`)
- [x] encode/decode exists for typed frames
- [x] traversal reads child IDs from typed payloads

EVIDENCE:
- `core-rs/src/types.rs`
- `core-rs/src/store/encode.rs`
- `core-rs/src/store/decode.rs`
- `core-rs/src/graph/object_graph.rs`

## E1.2 [PLAN] In-memory store densification (productive structure)
Goal: eliminate `HashMap<BlockId, Vec<u8>>` as primary storage.

- [ ] Baseline benchmark recorded (current mem_store behavior)
- [ ] New dense layout chosen:
  - [ ] Option A: `Vec<Vec<u8>>` (only if no deletes)
  - [ ] Option B: `Vec<Option<Vec<u8>>>` (allows delete)
  - [ ] Option C: slab (`Vec<Entry>` + free list)
- [ ] Implementation completed in `core-rs/src/store/mem_store.rs`
- [ ] `list_block_ids()` remains supported and deterministic
- [ ] All core tests pass
- [ ] Memory usage improvement measured (RSS) OR explicit “no regression” statement with numbers

EVIDENCE:
- `core-rs/src/store/mem_store.rs`
- `core-rs/tests/*`
- `docs/bench/mem_store.md` (or similar)

## E1.3 [PLAN] Verify/Validate contract is complete and deterministic
- [ ] Verify detects missing blocks referenced by graph
- [ ] Verify detects hash mismatch for stored frames
- [ ] Verify detects kind mismatches (stored kind vs expected)
- [ ] Verify detects broken references in recipes (bad child IDs)
- [ ] Error reporting is stable (same repo state → same output ordering)
- [ ] A CLI or callable API exists to run verify end-to-end

EVIDENCE:
- `core-rs/src/validate.rs`
- `core-rs/src/repo_api.rs` or equivalent verify entrypoint
- test: `core-rs/tests/verify_*` (must exist)

## E1.4 [PLAN] GC invariants and reproducibility
- [ ] GC closure algorithm validated on:
  - [ ] simple repo graph
  - [ ] branching graph
  - [ ] graph with Z blocks
- [ ] GC dry-run mode exists (report only)
- [ ] GC apply mode exists (mutating)
- [ ] GC never deletes reachable blocks
- [ ] GC detects and reports unreachable blocks deterministically
- [ ] GC produces stable output ordering (by BlockId)

EVIDENCE:
- `core-rs/src/gc.rs`
- tests:
  - `core-rs/tests/gc_invariants.rs`
  - `core-rs/tests/gc_*`

## E1.5 [PLAN] Graph closure API is stable
- [ ] `compute_closure_from_object` is deterministic
- [ ] `compute_closure_from_block` is deterministic
- [ ] closure results contain:
  - [ ] roots
  - [ ] visited set
  - [ ] order (topo/dfs stable order)
- [ ] handles missing blocks gracefully (error path tested)

EVIDENCE:
- `core-rs/src/graph/object_graph.rs`
- `core-rs/tests/object_graph.rs` (must exist / updated)

---

# EPIC E2 (W=10) — Repository Format & Recovery
Goal: portable, rebuildable repo.qblk + repo.qroot contract.

## E2.1 [PLAN] repo.qblk append contract + partial frame detection
- [ ] Define frame boundary and minimal header format in docs
- [ ] Ensure decoder rejects partial frame at EOF safely
- [ ] Create test that truncates file mid-frame and verifies detection
- [ ] Ensure `file_store` can continue reading up to last valid frame

EVIDENCE:
- `core-rs/src/store/file_store.rs`
- `core-rs/tests/repo_truncate.rs` (must exist)
- doc: `docs/format/qblk.md` (or inline section)

## E2.2 [PLAN] repo.qroot rebuild from qblk
- [ ] Define what qroot contains (minimal index, offsets, root pointers)
- [ ] Implement `rebuild_qroot_from_qblk`
- [ ] Rebuild is deterministic (same qblk → identical qroot bytes)
- [ ] Add test: build repo → delete qroot → rebuild → verify

EVIDENCE:
- code location (to be defined)
- test: `core-rs/tests/qroot_rebuild.rs`

## E2.3 [PLAN] Versioning and compatibility
- [ ] qblk format version stamp exists
- [ ] qroot format version stamp exists
- [ ] unknown versions rejected with explicit error code
- [ ] version values recorded in `docs/format/versions.md`

EVIDENCE:
- format docs
- decoder code path

## E2.4 [PLAN] Atomic update rules
- [ ] qroot update uses write-new + fsync + atomic rename
- [ ] no in-place qroot mutations that can corrupt on crash
- [ ] tests demonstrate safe update pattern (at least unit-level)

EVIDENCE:
- drive layer or core code implementing update
- tests (or documented invariants)

---

# EPIC E3 (W=10) — Drive FS UX & Determinism
Goal: users operate via filesystem import/export; core never sees paths.

## E3.1 [PLAN] CLI command surface exists and is stable
- [ ] `quarxdrive` CLI exposes:
  - [ ] `repo init` (or equivalent)
  - [ ] `fs-import`
  - [ ] `fs-tree`
  - [ ] `fs-stats`
  - [ ] `fs-export` / `fs-get`
  - [ ] `verify`
- [ ] `--root-path` (or equivalent) exists and is documented
- [ ] user-facing output shows interpreted info (not raw `ls -al repo.qblk`)
  - [ ] repo size
  - [ ] block count
  - [ ] dedup ratio / saved bytes (if computed)
  - [ ] Z:* stats

EVIDENCE:
- `quarxdrive/src/bin/quarxdrive.rs`

## E3.2 [PLAN] Import filters (include/exclude for files and dirs)
- [ ] Config keys exist:
  - [ ] `import.include_glob`
  - [ ] `import.exclude_glob`
  - [ ] `import.include_dir_glob`
  - [ ] `import.exclude_dir_glob`
- [ ] Filters apply deterministically
- [ ] `fs-stats` works after filtered import
- [ ] Tests cover:
  - [ ] skip by extension
  - [ ] skip by directory
  - [ ] include-only list

EVIDENCE:
- config handling code
- tests or fixture runs

## E3.3 [PLAN] fs-tree determinism and scale
- [ ] stable ordering
- [ ] handles large trees without quadratic behavior (basic measure)
- [ ] output can be machine-parsed (optional `--json` derived output)

EVIDENCE:
- code
- sample output log

## E3.4 [PLAN] fs-export correctness (hash-based)
- [ ] Export reproduces bytes exactly
- [ ] random sampling verify (N files)
- [ ] full verify option exists (slow but deterministic)
- [ ] explicit mismatch reporting (path + reason)

EVIDENCE:
- code
- test dataset run log

---

# EPIC E4 (W=6) — Dedup & Re-import Proof (Real Data)
Goal: demonstrate real dedup behavior and incremental updates.

## E4.1 [PLAN] Re-import identical dataset yields minimal growth
- [ ] Create test dataset snapshot (or choose a real tree)
- [ ] Run import #1: record repo stats
- [ ] Run import #2 (same data): record repo stats
- [ ] Show:
  - [ ] block count delta ~0 (or expected minimal)
  - [ ] repo bytes delta ~0 (or expected minimal)

EVIDENCE:
- `docs/bench/reimport_identical.md` with numbers

## E4.2 [PLAN] Re-import with partial changes
- [ ] Modify 10–20% of data (controlled)
- [ ] Import again
- [ ] Show growth proportional to modified content

EVIDENCE:
- `docs/bench/reimport_delta.md`

---

# EPIC E5 (W=6) — Z:* Structural Blocks & Analysis Hooks
Goal: Z:* is correct, measurable, and verifiable.

## E5.1 [DONE] Z-block integrated as first-class block kind
- [x] Z payload exists
- [x] graph traversal includes Z children

EVIDENCE:
- `core-rs/src/block/zblock.rs`
- `core-rs/src/codec/z.rs`
- `core-rs/src/graph/object_graph.rs`

## E5.2 [PLAN] Structural detection measurement surfaced
- [ ] import / scan reports Z bytes and ratio
- [ ] Z metrics included in fs-stats
- [ ] Z metrics included in telemetry line

EVIDENCE:
- drive stats output
- docs: `docs/metrics.md` (or similar)

## E5.3 [PLAN] Z verify and invariants
- [ ] verify checks Z ranges/refs are valid
- [ ] tests: corrupt Z payload triggers deterministic error

EVIDENCE:
- test file `core-rs/tests/z_verify.rs`

## E5.4 [PLAN] Pattern analyzer subsystem boundary (opt-in)
- [ ] Analysis subsystem exists as separate module/family
- [ ] Disabled by default
- [ ] Output available via explicit command (e.g., `repo explain-block --id <id>`)
- [ ] Must not mutate repository state

EVIDENCE:
- code path
- CLI command

---

# EPIC E6 (W=6) — Telemetry & Analytics (Binary-first)
Goal: metrics without heavy JSON; derived JSON allowed.

## E6.1 [PLAN] Live telemetry line
- [ ] Throughput (blk/s, MB/s)
- [ ] RSS
- [ ] CPU normalized (`cpu / cores`)
- [ ] unique blocks
- [ ] Z bytes
- [ ] store entries / bytes

EVIDENCE:
- CLI output sample in `docs/bench/telemetry_samples.md`

## E6.2 [PLAN] Binary analytics storage
- [ ] Define raw/binary record format
- [ ] Write raw logs during import/scan
- [ ] Provide converter to JSON report (derived)
- [ ] Ensure derived JSON is not required for correctness

EVIDENCE:
- `core-rs` or `tools/` implementation
- `docs/format/analytics_raw.md`

---

# EPIC E7 (W=7) — Stability & Long-run Hardening
Goal: predictable long runs, diagnosable failures.

## E7.1 [PLAN] Hang/stall detection
- [ ] Progress counter increments
- [ ] Watchdog tick in logs
- [ ] Detect “no progress in N seconds” and emit warning

EVIDENCE:
- run logs

## E7.2 [PLAN] Memory return measurement
- [ ] After run exit, RSS returns (or explain OS cache behavior)
- [ ] Provide “expected behavior” doc if OS caching persists
- [ ] Record before/after numbers

EVIDENCE:
- `docs/bench/memory_return.md`

## E7.3 [PLAN] Crash/recovery drills
- [ ] Simulate crash during qblk append
- [ ] Verify repo remains readable up to last valid frame
- [ ] Rebuild qroot and verify

EVIDENCE:
- test logs
- `docs/bench/crash_drills.md`

---

# EPIC E8 (W=5) — Performance Scaling (After Gate A)
Goal: faster without breaking determinism.

## E8.1 [WAIT] Parallel hashing and worker control
- [ ] Identify safe parallel regions
- [ ] Worker count config key exists
- [ ] Determinism preserved under parallelism

EVIDENCE:
- perf doc with numbers

## E8.2 [WAIT] I/O strategy controls
- [ ] direct I/O policy is Drive-owned
- [ ] config toggles exist
- [ ] measurements recorded

EVIDENCE:
- `docs/bench/io_policy.md`

---

# EPIC E9 (W=4) — Networking Minimal Transport (After Gate B)
Goal: transport is not a source of truth.

## E9.1 [WAIT] Minimal protocol surface
- [ ] get-block (by BlockKey)
- [ ] put-block (frame upload)
- [ ] list roots / snapshots (optional)
- [ ] reject invalid frames before admit

EVIDENCE:
- `docs/net/protocol.md`

## E9.2 [WAIT] Integrity on receive
- [ ] verify kind + hash
- [ ] reject corrupted payload
- [ ] deterministic error codes

EVIDENCE:
- tests in net module

---

# EPIC E10 (W=2) — Cloud/Cluster Coordination (After Gate C)
Goal: coordination only (no storage logic).

## E10.1 [WAIT] Node registry + heartbeat
- [ ] nodes register
- [ ] heartbeats recorded
- [ ] node list stable

EVIDENCE:
- `docs/cloud/registry.md`

---

# EPIC E11 (W=4) — Reporting & UX Truthfulness
Goal: output that never lies and is interpretable.

## E11.1 [PLAN] Single stable report block for import
- [ ] consistent fields
- [ ] no duplicated sections
- [ ] includes effective config snapshot

EVIDENCE:
- sample output in docs

## E11.2 [PLAN] Saved-bytes math correctness
- [ ] define equations explicitly
- [ ] byte-accurate totals
- [ ] verify totals equal sum of parts

EVIDENCE:
- `docs/metrics.md`

---

## 4. Auto-Progress Extraction Notes (for tools)
To compute progress:
- Parse epic headers `# EPIC Ex (W=n)`
- Count checkboxes until next epic header
- Epic progress = checked/total
- Project progress = weighted average

This file is intentionally machine-readable.

---
End of ROADMAP-TARGET.md
