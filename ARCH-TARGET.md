# ARCH-TARGET.md
## QuarXTor — Target Architecture Specification

Status: TARGET (authoritative)
Scope: Architecture / invariants / responsibilities
Language: English (discussion may be in Russian)

---

## 0. Purpose

This document defines the **final converged architecture** of the QuarXTor project
(formerly HGSS QuarkStore) after all iterations, refactors, renames, and scope corrections.

This is **not a roadmap** and **not an implementation guide**.
It is the **architectural contract** the system must satisfy.

No speculative features.
No future fantasies.
Only validated concepts and frozen decisions.

---

## 1. Terminology & Naming

- **HGSS** — *Hash Graph Storage System*  
  Technical name of the underlying storage technology.

- **QuarXTor** — Product / engine name built on HGSS.

- **Quark** — Minimal fixed-size content-addressed block.

- **BlockId** — Compact internal identifier (`u64`) used everywhere inside the engine.

- **BlockKey** — External identity of a block: `(hash, size)`.

- **BlockKind** — Stable kind tag (e.g., `L0`, `Multi`, `Z`, `Object`).

- **Frame** — Stable encoded representation of a typed block (id/hash/kind + payload).

- **Z-block (Z:*)** — Structural block representing implicit/structural data
  (zeros, sparse holes, repeated patterns, low-entropy regions).

- **Repository** — A self-contained QuarXTor storage instance.

- **Store** — A persistence backend for block frames (file-backed, memory-backed, etc.).

- **Overlay / Tier** — A logical storage layer with policy-driven placement.

---

## 2. High-Level System Goal

QuarXTor is a **hash-graph-based storage engine** with:

- fixed-size block deduplication,
- RAM-aware tiered storage,
- structural block optimization (Z:*),
- deterministic recovery and analysis-first design.

The system must work efficiently on **real data**, not synthetic benchmarks.

---

## 3. Layered Architecture (Hard Invariant)

Strict dependency order:

```

QuarXCore (core-rs)
↓
QuarXNet
↓
QuarXDrive
↓
QuarXCloud
↓
CLI / Tools / UI

```

### Invariants

- Lower layers **must not depend** on upper layers.
- `core-rs` is storage-agnostic and filesystem-agnostic.
- Networking, filesystem paths, FUSE, UI **never leak into core**.
- No circular dependencies are allowed.
- Core may expose stable, minimal APIs; upper layers adapt user-facing behavior.
- Storage layout (paths, placement, mounts) is owned by Drive/Cloud, not Core.

---

## 4. Repository Model

A repository consists of **two logical artifacts**:

### 4.1 `repo.qblk`
- Main packed container for all block frames/payloads.
- Authoritative source of data.
- Append-oriented, segment-based.
- Independent of filesystem layout.

### 4.2 `repo.qroot`
- Compact metadata root / index.
- Rebuildable from `repo.qblk` (expensive but deterministic).
- May reside on disk or in RAM (policy-driven).

### Guarantees

- Repository is **self-describing** (format versioned, forward-safe).
- Loss of `repo.qroot` must not imply data loss.
- Integrity is verified by content hashes, not trust in metadata.
- Ordering and serialization must be deterministic (reproducible builds/tests).

---

## 5. Identity, Indexing, and Storage Structures

This section defines the required **productive** data representation model.

### 5.1 External Identity vs Internal IDs

- External identity is `BlockKey = (hash, size)`.
- Internal identity is `BlockId` (`u64`), used everywhere in graphs, recipes, GC, traversal.

### 5.2 Index Principle

- Hash maps are allowed **only as lookup indices**.
- Hash maps must not be used as primary storage for the graph or for dense block sets.

Required baseline:

- `BlockKey → BlockId` as a lookup index (hash map or deterministic map).
- `BlockId → metadata` stored densely (e.g., `Vec` / slab-like).
- `BlockId → payload location` stored densely for file-backed stores (e.g., offset arrays).
- In-memory stores should prefer dense `Vec`-like layouts over `HashMap<BlockId, ...>`.

### 5.3 Determinism

- Any persistent index must have deterministic layout.
- Persistent formats must not depend on hash map iteration order.
- Stable file formats must be versioned; decoding must reject unknown/invalid versions safely.

---

## 6. Block & Graph Model

### 6.1 Quarks (Blocks)

- Fixed-size blocks (default target: 64 KiB).
- Content-addressed via cryptographic hash (BLAKE3 baseline).
- Identified globally by `(hash, size)`; mapped internally to `BlockId`.
- Blocks are stored and exchanged as **typed frames**.

### 6.2 Hash Graph

- Objects are defined as **graphs of blocks**.
- Graph edges encode containment and structure via `BlockId` links.
- No implicit hierarchy inside core.
- Traversal is defined in terms of typed payloads that reference child `BlockId`s.

### 6.3 Recipes

- Multi/Object/Z payloads may contain child links.
- Child links are stored as `Vec<BlockId>` or compact small-vectors (e.g., `SmallVec`).
- Custom recipes are allowed; if Core cannot interpret links, they are treated as opaque
  (only safe operations allowed; analysis/link-closure must be conservative).

### 6.4 Reference Tracking

- Reference counts track block liveness.
- Deletion decrements references.
- Garbage collection removes blocks with zero references.
- Consistency rules are enforced by graph invariants, not heuristics.
- Core must provide a **validate/verify** capability (detect missing blocks, hash mismatches,
  broken references, kind mismatches).

---

## 7. Structural Blocks (Z:*)

Z-blocks represent **implicit data**:

- zero-filled regions,
- low-entropy patterns,
- sparse filesystem holes,
- repeated structural sequences.

Properties:

- Stored as compact descriptors, not raw payloads.
- Integrated into the same graph model and traversed the same way.
- Do **not** bypass deduplication logic.
- Detection is deterministic and reproducible.

---

## 8. Tiered Storage & Overlays

### 8.1 Storage Tiers

- RAM
- SSD / NVMe
- HDD
- Network (future overlay)

RAM is a **first-class tier**, not merely a cache.

### 8.2 Overlay Model

Logical overlay stack:

```

RAM overlay
↓
Drive overlay
↓
Network overlay

```

Rules:

- Reads traverse top → bottom.
- Writes go to top overlay.
- Migration between tiers is policy-driven.
- Logic is independent of physical device type.
- Policies must be explicit (configurable), never implicit magic.

---

## 9. Analytics & Telemetry Layer

Analytics is a **separate architectural subsystem**.

### Properties

- Read-only by default.
- Explicitly enabled (opt-in).
- No background daemon in MVP.
- No automatic mutation of storage state.

### Collected Metrics

- Throughput (blk/s, MB/s)
- RSS
- CPU usage (normalized by core count)
- Unique blocks
- Structural bytes (Z:*)
- Store entries / bytes

### Data Representation

- Binary/raw formats for performance.
- JSON only as derived/debug/report output.
- Heavy JSON dumps are explicitly forbidden as primary storage.

---

## 10. Analysis Subsystem

- Pattern analysis (byte repetition, entropy, structure).
- Implemented as an independent analysis family.
- Results may *inform* Z:* heuristics and future quarks.
- Disabled by default.

No global background intelligence.
No self-modifying behavior.

---

## 11. Import / Export & Filesystem Layer

### QuarXDrive Responsibilities

- Filesystem import/export.
- File recipes (object → block list).
- User-facing CLI operations.
- Optional FUSE/VFS integration.

### Import Controls

- Include/exclude file globs.
- Include/exclude directory globs.
- Configurable via INI and environment.

Core never sees filesystem paths.

---

## 12. Networking Layer (QuarXNet)

- Provides transport for block/object exchange.
- Uses a **strictly limited** set of protocols.
- Baseline: HTTP/HTTPS.
- RPC/gRPC considered only if strictly justified.

Security model:

- Hash-based integrity verification.
- Encryption/authentication planned but not assumed in MVP.

---

## 13. Cloud / Cluster Layer (QuarXCloud)

- Node coordination.
- Health checks and heartbeats.
- Replication orchestration.
- No storage logic inside cloud layer.

Cloud is optional; local operation is first-class.

---

## 14. Configuration Model

- Unified configuration service.
- Sources:
  - INI files
  - Environment variables
- Deterministic resolution order.
- All configuration keys are explicitly documented.

---

## 15. Language, Style, and Process Rules

- All code and documentation: **English only**.
- Discussion and task framing: Russian allowed.
- No AI/tool-brand mentions (e.g., ChatGPT) in user-facing artifacts.
- No partial patches or magic edits.
- Stable checkpoints must be preserved.
- Existing behavior is frozen unless explicitly changed.

---

## 16. Licensing & Ownership

- License: MIT.
- Author attribution preserved.
- Project identity: independent storage engine.
- No dependency on external ecosystems.

---

## 17. Non-Goals (Explicit)

- No speculative future architectures.
- No implicit background intelligence.
- No protocol zoo.
- No automatic self-optimization.
- No breaking changes without explicit architectural decision.

---

## 18. Architectural Authority

This document overrides:
- older plans,
- partial drafts,
- chat-local decisions.

Any change to this document is an **architectural event** and must be explicit.

---
End of ARCH-TARGET.md
