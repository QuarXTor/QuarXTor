# QuarXTor Architecture

QuarXTor is a hash-graph-based storage stack with RAM-aware deduplication and multi-tier layout (RAM / SSD / HDD).

The system is split into focused components:

- **QuarXCore** – core storage engine (blocks, hash-graph, dedup, on-disk/on-RAM layout)
- **QuarXNet** – networking / RPC / cluster coordination
- **QuarXDrive** – client-facing layer (CLI, FUSE, OS integration, SDKs)
- **QuarXCloud** – multi-node orchestration, deployment models, HA

This repository (`QuarXTor/QuarXTor`) is the **meta / coordination** repo:
architecture docs, specs, RFCs, and roadmap.

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
- RAM tier:
  - hot block cache
  - index cache
- Telemetry:
  - per-block stats (entropy, size, type hint)
  - per-job stats (throughput, dedup ratio, latencies)

QuarXCore exposes a **local API** (library or low-level RPC) used by QuarXNet.

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

1. Input data is chunked into blocks.
2. For each block:
   - compute hash
   - check existance in core index
   - if new → allocate in tier (e.g. SSD), write, index
   - if existing → bump refcount, update reference matrix
3. Update hash-graph edges for structural relationships.
4. Expose logical object / version via QuarXNet.

### 4.2 Read

1. Request arrives (object path / id).
2. QuarXNet resolves logical object into block sequence via QuarXCore.
3. For each block:
   - served from RAM tier if present
   - otherwise from SSD/HDD (with potential promotion to RAM)
4. Stream back to client (via QuarXDrive or API).

### 4.3 Rebalancing / Tier Management

Background tasks:

- Detect hot blocks → promote to RAM
- Detect cold blocks → demote to SSD/HDD
- Compact / rewrite segments if needed
- Maintain target ratios per tier

Policies are pluggable and telemetry-driven.

---

## 5. Repository Mapping

- [`QuarXTor/QuarXTor`](https://github.com/QuarXTor/QuarXTor)
  - ARCH.md, ROADMAP.md, RFCs, global issues, meta-docs

- [`QuarXTor/QuarXCore`](https://github.com/QuarXTor/QuarXCore)
  - Engine implementation
  - Block store, hash-graph, reference structures
  - On-disk format and local APIs

- [`QuarXTor/QuarXNet`](https://github.com/QuarXTor/QuarXNet)
  - RPC definitions
  - Server(s) exposing QuarXCore
  - Cluster protocols

- [`QuarXTor/QuarXDrive`](https://github.com/QuarXTor/QuarXDrive)
  - CLI tools
  - FUSE, OS integration
  - SDKs

- [`QuarXTor/QuarXCloud`](https://github.com/QuarXTor/QuarXCloud)
  - Deployment specs / Helm charts / orchestration code
  - HA / scaling logic

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
