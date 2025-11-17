# QuarXTor Roadmap

This document tracks the high-level roadmap of the QuarXTor storage stack.

The actual implementation order may adjust over time, but the core idea is:

1. Get a solid single-node engine.
2. Add introspection and tooling.
3. Open it via a clean network API.
4. Integrate with clients / OS.
5. Evolve into a multi-node, cloud-ready system.

---

## Milestone 0.1 – Engine Skeleton (Single Node, Local Only)

**Scope:**

- `QuarXCore`:
  - Basic project layout (language/tooling to be confirmed).
  - Simple block store:
    - fixed-size blocks
    - content hash (e.g. BLAKE3)
    - files on local disk
  - In-memory index:
    - hash → location
    - naive refcount
  - Simple CLI (can be in the same repo or temporary script):
    - ingest path → split into blocks → store
    - read back by internal id (for debugging)

**Goals:**

- Prove end-to-end path:
  - input data → blocks → disk → read back
- Get basic measurements:
  - throughput
  - dedup ratio on sample dataset

**Out of scope:**

- Networking
- Cluster
- Advanced tiering
- Persistent metadata engine

---

## Milestone 0.2 – Telemetry and Structure Introspection

**Scope:**

- Extend `QuarXCore`:
  - track basic stats per block:
    - size
    - simple entropy metric
  - record logical objects (files) and their block lists
  - export basic summary:
    - number of blocks
    - dedup ratio
    - per-file stats

- CLI tools:
  - `list-blocks`, `list-files`, `stats`
  - dump internal view in JSON for further analysis

**Goals:**

- Have a clear picture of how data is represented.
- Enable experimentation on real datasets.
- Prepare for designing the hash-graph and reference matrix more formally.

---

## Milestone 0.3 – Hash-Graph & Reference Structures (Prototype)

**Scope:**

- Introduce hash-graph abstraction in `QuarXCore`:
  - define data structures for vertices/edges
  - link consecutive blocks of the same object
  - optional semantic edges (e.g. snapshots, clones)

- Introduce a reference matrix / equivalent:
  - efficient mapping logical object → block ids
  - refcount updates when objects are added/removed

- CLI / debug tools:
  - export graph information for visualization
  - simple queries (e.g. show structure of an object)

**Goals:**

- Move from “bag of blocks” to a structured graph model.
- Prepare foundation for advanced dedup and analysis.

---

## Milestone 0.4 – Tiering & RAM Awareness (Single Node)

**Scope:**

- Introduce notion of tiers in `QuarXCore`:
  - RAM cache
  - SSD
  - HDD (or just “cold” vs “hot” for start)

- Implement a very simple policy:
  - recently accessed blocks → promote to RAM
  - rarely accessed blocks → demote

- Basic metrics:
  - cache hit ratio
  - latency comparison

**Goals:**

- Validate RAM-aware design.
- Understand impact on real workloads.

---

## Milestone 0.5 – Local API & Library Boundary

**Scope:**

- Define a stable internal API for `QuarXCore`:
  - ingest object / file
  - fetch object
  - query stats
  - delete object

- Package `QuarXCore` as a library:
  - API boundary clearly separated from any CLI or network code

**Goals:**

- Make it possible for other components (QuarXNet, QuarXDrive) to depend on QuarXCore consistently.
- Clean up layering.

---

## Milestone 0.6 – QuarXNet v0 (Basic RPC)

**Scope:**

- Create `QuarXNet`:
  - define simple RPC / HTTP API:
    - `PUT /object`
    - `GET /object`
    - `GET /stats`
  - server that wraps QuarXCore API and exposes it over the network

- Minimal auth / security model (can be “none” for lab usage).

**Goals:**

- Remote access to a single-node engine.
- Start using QuarXTor from other machines / containers.

---

## Milestone 0.7 – QuarXDrive v0 (CLI / Tooling)

**Scope:**

- Create `QuarXDrive`:
  - CLI tools using QuarXNet:
    - `quarx put <path>`
    - `quarx get <id>`
    - `quarx stats`
  - Optional: initial experiments with FUSE mount (read-only).

**Goals:**

- Usable developer tooling.
- Demonstrate real workflows (backup, dataset import, etc.).

---

## Milestone 0.8 – QuarXCloud v0 (Multi-Node Sketch)

**Scope:**

- Create `QuarXCloud`:
  - basic multi-node layout:
    - configuration model (N nodes, roles)
  - very simple replication:
    - replicate blocks to 2 nodes
  - cluster health view:
    - which nodes are online
    - which data is where

**Goals:**

- Validate feasibility of moving to multi-node world.
- Identify constraints on QuarXCore and QuarXNet for distributed usage.

---

## Milestone 1.0 – Stable Single-Node + Experimental Cluster

**Scope:**

- Stabilize:
  - QuarXCore APIs and on-disk format (v1)
  - QuarXNet API (v1)
  - QuarXDrive CLI

- Provide:
  - docs, examples, recommended topologies
  - packaging / deployment recipes

**Goals:**

- Have a robust, documented single-node product.
- Have an experimental cluster mode for early adopters.
- Decide on next big directions (deep analytics, GPU, advanced compression, etc.).

---

## Tracking

- Architecture and design docs live in `ARCH.md` and under `docs/` here.
- Each milestone should map to GitHub milestones / issues in this repo.
