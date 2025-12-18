# QuarXTor Architecture

QuarXTor is a hash-graph-based storage stack with RAM-aware deduplication and multi-tier layout (RAM / SSD / HDD).

This repository (`QuarXTor/QuarXTor`) is the meta / coordination repo: architecture docs, specs, RFCs, and roadmap.

## Components and layering

The stack is implemented as internal modules in this mono-repo:

- **QuarXCore** – core storage engine (blocks, hash-graph, dedup, on-disk/on-RAM layout), implemented in `core-rs/`.
- **QuarXNet** – networking / RPC / cluster coordination, implemented in `quarxnet/`.
- **QuarXDrive** – client-facing layer (CLI, FUSE, OS integration, SDKs), implemented in `quarxdrive/`.
- **QuarXCloud** – multi-node orchestration, deployment models, HA, implemented in `quarxcloud/`.
- **QuarXProto** – shared schema/IDL for network/client payloads, implemented in `quarxproto/`.
- **Tools** – leaf-level binaries/scripts, implemented in `tools/`; must not be used as libraries by lower layers.
- **core-py** – experimental Python core utilities, implemented in `core-py/` (non-ABI, not part of the stable layering).

Layering rule for this mono-repo and the wider QuarXTor organization (applies to crate dependencies, internal module boundaries, and conceptual API/deployment layering):

`core-rs (QuarXCore) → quarxnet (QuarXNet) → quarxdrive (QuarXDrive) → quarxcloud (QuarXCloud) → tools`

- `core-rs` must not depend on `quarxnet`, `quarxdrive`, `quarxcloud`, `tools`, or `quarxproto`.
- `quarxnet` depends only on public APIs from `core-rs` and shared schemas in `quarxproto`; it must not depend on `quarxdrive` or `quarxcloud`.
- `quarxdrive` uses QuarXNet/QuarXCore APIs and shared schemas and must not rely on internal on-disk layout.
- `quarxcloud` orchestrates deployments of the lower layers and does not own on-disk formats or engine invariants.
- `quarxproto` is a shared schema/IDL crate used where layering allows; it must not introduce back-edges.
- Tools are leaf consumers; lower layers must not import them as libraries.

---

## 1. Design Goals

- **Space efficiency**  
  Content-addressed blocks, hash-graph and reference matrices to eliminate duplicates and compress structural redundancy.

- **RAM-aware operation**  
  Make RAM a first-class tier: hot blocks, indexes and small metadata live in RAM for low-latency reads and analytics.

- **Multi-tier storage**  
  Clean separation between logical blocks and physical tiers (RAM, SSD, HDD, remote). Policy-driven placement and migration.

- **Introspectable by design**  
  Telemetry, block statistics, reference graphs and entropy metrics are core features, not add-ons.

- **Embeddable core, pluggable frontends**  
  Engine (QuarXCore) is independent of any particular protocol or filesystem. Different frontends (FUSE, HTTP, gRPC, etc.) plug via QuarXNet and QuarXDrive.

---

## 2. High-Level Components

### 2.1 QuarXCore (Engine)

Responsibilities:

- Block model (fixed or variable-size chunks, content hashes)
- Hash-graph:
  - vertices = blocks
  - edges = structural / reference relationships
- Deduplication:
  - hash index → detect identical blocks
  - reference matrix → track where blocks are used
- On-disk layout:
  - segment files / extents
  - index files (hash → location, refcount, stats)
- Stable on-disk formats (see `docs/core-formats.md` and local agent guidance); QuarXCore is the sole owner of disk layout and must not depend on quarxnet/quarxdrive/quarxcloud/tools.
- RAM tier:
  - hot block cache
  - index cache
- Telemetry:
  - per-block stats (entropy, size, type hint)
  - per-job stats (throughput, dedup ratio, latencies)

QuarXCore exposes a **local API** (library or low-level RPC) used by QuarXNet and engine-level tools.

Core invariants:

- On-disk formats are stable ABI; see `docs/core-formats.md` and the local agent guidance on on-disk format.
- Config path rules: `repo_root` must not equal or be nested under `[import].root`; `[paths].data_dir/cache_dir/log_dir/runtime_dir` must stay outside `[import].root`; all config paths are absolute (tilde expands to `$HOME`).
- Evolution must follow the local agent rules and respect stable public APIs and formats.

### 2.2 QuarXNet (Networking / RPC / Cluster)

Responsibilities:

- External API:
  - store / retrieve / delete objects
  - introspection / stats / debug views
- Node-to-node communication:
  - replication of blocks
  - sync of indices and reference graphs
- Cluster coordination:
  - membership, health checks
  - leader election (if needed)
  - placement decisions (where blocks live)
- Does not own on-disk formats; depends on QuarXCore via stable APIs and shared schemas in `quarxproto`; must not depend on QuarXDrive.

Typical transport: gRPC / HTTP+JSON (to be specified later).

### 2.3 QuarXDrive (Clients / Integration)

Responsibilities:

- Command-line tools:
  - data import/export
  - analysis / inspection
  - admin workflows
- FUSE / virtual filesystem:
  - mount QuarXTor as a filesystem
  - map logical paths to underlying hash-graph
- SDKs:
  - language bindings for QuarXNet APIs
- Uses QuarXNet/QuarXCore APIs and shared schemas; must not bypass on-disk formats or depend on internal layouts.

QuarXDrive never directly touches on-disk layout – it speaks to QuarXNet.

### 2.4 QuarXCloud (Orchestration / Cloud)

Responsibilities:

- Deployments:
  - single-node, multi-node, hybrid layouts
- Scaling:
  - horizontal scaling (add more nodes)
  - shard / replicate graph segments
- High availability:
  - failure detection
  - replication policies
- Integration with external orchestrators:
  - Docker / Kubernetes, etc. (later phases)
- Operates at deployment/cluster level; does not own on-disk formats or core invariants.

### 2.5 QuarXProto (Schema / IDL)

- Shared lightweight schema/IDL used by QuarXNet/QuarXDrive (and allowed engine-facing surfaces) for payloads such as ZNode/ZNodeKind.
- Lives in `quarxproto/`; must remain free of back-edges and respect layering constraints.

### 2.6 Tools

- Leaf binaries/scripts for inspection, migration, and admin workflows (lives in `tools/`).
- May depend on higher layers but must not be used as libraries by lower layers or introduce invariants that contradict this architecture or the internal agent guidance.

---

## 3. Data Model (Core Concepts)

### 3.1 Block

Minimal unit of storage.

Fields (conceptual):

- `id` – content hash (e.g. BLAKE3)
- `tier` – RAM / SSD / HDD / remote
- `size` – size in bytes
- `entropy` – approximate entropy / type hint
- `refs` – reference count
- `meta` – optional metadata (flags, tags)

### 3.2 Hash-Graph

Graph where:

- vertices = blocks
- edges = relationships:
  - structural (e.g. next block in file extent)
  - semantic (e.g. same logical object, different versions)
  - container relationships (e.g. file → directory → snapshot)

The hash-graph allows:

- deduplication across arbitrary structures
- structural analysis (what patterns repeat)
- temporal / version-aware storage

### 3.3 Reference Matrix

Logical abstraction representing where blocks are used.

Examples:

- rows = logical objects (files, datasets, snapshots)
- columns = blocks
- entries = 0/1 or weight (# of uses)

Physical implementation will be more compact (indices, compressed structures), but conceptually QuarXCore maintains:

- which logical entities depend on which blocks
- how refcounts should be updated upon changes

---

## 4. Operations (Typical Flows)

### 4.1 Ingest / Store

1. A client (QuarXDrive) submits data via QuarXNet (or directly to QuarXCore in local/engine workflows).
2. QuarXCore chunks input into blocks:
   - compute hash
   - check existence in core index
   - if new → allocate in tier (e.g. SSD), write, index
   - if existing → bump refcount, update reference matrix
3. QuarXCore updates hash-graph edges for structural relationships (Multi/Z/Object blocks).
4. QuarXNet exposes the resulting logical object / version.

### 4.2 Read

1. A client (QuarXDrive) requests an object/path via QuarXNet.
2. QuarXNet resolves the logical object into block references via QuarXCore.
3. For each block:
   - served from RAM tier if present
   - otherwise from SSD/HDD (with potential promotion to RAM)
4. QuarXDrive (or another API client) streams data back to the requester.

### 4.3 Rebalancing / Tier Management

Background tasks:

- Detect hot blocks → promote to RAM
- Detect cold blocks → demote to SSD/HDD
- Compact / rewrite segments if needed
- Maintain target ratios per tier

Policies are pluggable and telemetry-driven.

---

## 5. Repository Mapping

- `QuarXTor/QuarXTor` (this mono-repo)
  - `core-rs/` — QuarXCore engine implementation; owns on-disk formats (see `docs/core-formats.md`), config invariants in `docs/config-reference.md` and the local agent guidance.
  - `quarxnet/` — QuarXNet networking/RPC layer; uses QuarXCore APIs and shared schemas; no on-disk ownership.
  - `quarxdrive/` — QuarXDrive CLI/FUSE/SDK layer; uses QuarXNet/QuarXCore APIs; no on-disk ownership.
  - `quarxcloud/` — QuarXCloud orchestration; operates at cluster/deployment level.
  - `quarxproto/` — shared schema/IDL crate for network/client payloads; no back-edges.
  - `core-py/` — experimental Python core utilities (non-ABI).
  - `tools/` — leaf tooling; must not be used as libraries by lower layers.
  - Docs (ARCH, ROADMAP, AGENTS, core-formats) and meta/RFCs.

- Ecosystem/organizational mapping (1:1 conceptual layers):
  - `QuarXTor/QuarXCore` — engine (core-rs equivalent)
  - `QuarXTor/QuarXNet` — networking/RPC (quarxnet equivalent)
  - `QuarXTor/QuarXDrive` — client/CLI/FUSE (quarxdrive equivalent)
  - `QuarXTor/QuarXCloud` — orchestration/cloud (quarxcloud equivalent)

All repos and directories must respect the layering rule: `core-rs → quarxnet → quarxdrive → quarxcloud → tools`, with `quarxproto` as a shared schema layer and no forbidden back-edges.

---

## 6. Implementation Phases (Overview)

Detailed plan is in `ROADMAP.md`. High-level:

1. Single-node engine prototype (QuarXCore):
   - block model, index, basic dedup
   - telemetry core
2. Local API and CLI tools:
   - introspection over local storage
3. Network layer (QuarXNet):
   - simple RPC for remote access
4. Client integration (QuarXDrive):
   - basic CLI / import / export
5. Multi-node / cloud (QuarXCloud):
   - replication and simple cluster semantics

### Dev & CI overview

- Unified dev-check flow:
  - `./tools/check-cyrillic.sh` — enforce English-only in `.rs`, docs, INI/TOML.
  - `./dev-check.sh` — runs Cyrillic check, `cargo build --workspace`, `cargo clippy --workspace --all-targets --all-features`, `cargo test --workspace`.
- CI invokes `dev-check.sh`; clippy warnings, config/path invariant violations, or Cyrillic text are treated as failures.
- Architecture and code changes must respect the internal agent guidance and `docs/core-formats.md`. Architecture evolution (adding/changing modules in `core-rs/`, `quarxnet/`, `quarxdrive/`, `quarxcloud/`, `quarxproto/`) must preserve the layering and invariants defined in this document.
