# QuarXTor — Hash-Graph Storage Engine  
*(Name origin: “Quark Store” → “QuarX-Tor”)*

─────────────────────────────────────────────────────────────────────────
## Onboarding

### What is QuarXTor

QuarXTor is a layered hash-graph storage engine for high-speed ingest,  
RAM-aware deduplication, stable on-disk formats, and long-term integrity.

It is built around a multi-level block hierarchy (L0 / L1 / Z / Object) forming a deterministic, verifiable graph.  
The design goal: **predictable performance** → **predictable storage efficiency** → **predictable evolution**.

### Quick Start (Demo)

Prereqs: Rust toolchain (cargo + rustc) and python3.  
From the repo root:

```bash
./quarxtor quickstart
```

Notes:
• Generates a small dataset, imports it, and prints stats/tree.  
• On Windows, use WSL or adapt the script.

### Install / Build

Build the CLI locally:

```bash
cargo build -p quarxdrive --release --locked
```

### Packaging / Releases

No published installers or package-manager releases yet.  
Build a local tarball (Linux/macOS):

```bash
./packaging/tarball/build-dist.sh
```

See `docs/packaging.md` for cross-platform notes and install steps.

### Contributor / Dev

Contributor workflow and strict checks live in `CONTRIBUTING.md`.  
Dev checks: `tools/dev-check.sh` (Cyrillic scan + build + tests).

─────────────────────────────────────────────────────────────────────────
## 🔭 Architecture at a Glance

    ┌──────────────────┐
    │    QuarXDrive    │  Client / SDKs / Tools
    └────────┬─────────┘
             │  RPC / API
    ┌────────▼─────────┐
    │     QuarXNet     │  network, replication, cluster
    └────────┬─────────┘
             │  Engine API
    ┌────────▼─────────┐
    │    QuarXCore     │  engine: blocks, graph, dedup
    └────────┬─────────┘
             │
    ┌────────▼─────────┐
    │   Block Store    │  stable on-disk ABI
    └──────────────────┘

Strict layering:  
`core-rs → quarxnet → quarxdrive → quarxcloud → tools`  
(no back-edges, no bypassing the engine)

─────────────────────────────────────────────────────────────────────────
## 🧱 Block Hierarchy — L0 → L1 → Z → Object

QuarXTor uses a **multi-level block graph**:

• **L0 — physical atomic blocks**  
  Raw data fragments. Content-addressed (hash → block). Fully deduped.

• **L1 — logical aggregates**  
  Multi-blocks combining L0 pieces or codec/dictionary references.  
  (Think: file segments, chunk groups, reconstruction recipes.)

• **Z — structural overlays**  
  Metadata layers describing structure, boundaries, sequences, analysis hints.

• **Object — top-level entities**  
  Files, directories, snapshots, datasets, archives.  
  Objects can represent structures similar to ZIP/7z/tar — but as a *clean,
  transparent graph*, not opaque binary blobs.

Graph intuition:

    [L0]──┐
          ├──>[L1]──┐
    [L0]──┘         │
                    ├──>[Z]──>[Object]
    [L0]────────────┘

Everything is explicit. No magic headers. No hidden offsets.

─────────────────────────────────────────────────────────────────────────
## 📉 Why QuarXTor? (Real Gains)

### Deduplication that *actually* works

The hash-graph model ensures deterministic reuse of identical blocks  
across RAM / SSD / HDD / NVMe tiers.

    Repeated data  ──┐
                     ├─> same L0 hashes
    Similar files ───┘

Different files, backups or VM images can share large parts of their block graph,  
saving space without losing structure.

### Verified storage savings

In early engine-level tests (RAM mode + SSD mode), QuarXTor already showed:

• noticeable reduction in raw L0 storage  
• fewer duplicated fragments across similar datasets  
• reduction in multi-layer aggregates (L1/Z/Object)  
• improved SSD / NVMe footprint over time on repeated ingest

Even before heavy optimization, total footprint was clearly lower than the raw input size,  
especially on mixed binary/text sets and repeated backups.

### Predictable on-disk ABI

The engine owns its on-disk format:

• stable, versioned and deterministic  
• documented in `docs/core-formats.md`  
• safe for long-term archival and replication

No accidental format drift, no silent incompatibilities.

### High ingest throughput

Ingest pipeline:

    Source → Chunk → Hash → Dedup → Encode → Append → Link → Commit

Hashing, dedup, block IO and telemetry are designed to be parallelizable
across CPU cores and storage tiers.

### Transparent reads

    Object → Z → L1 → L0 → (RAM → SSD → HDD)

• Hot blocks are promoted automatically.  
• Cold blocks stay cheap on slower tiers.  
• The read path is just walking the graph.

─────────────────────────────────────────────────────────────────────────
## 📦 Repository Layout (Mono-Repo)

    core-rs/      → QuarXCore (engine: blocks, graph, dedup, on-disk ABI)
    quarxnet/     → QuarXNet (RPC, node-to-node sync, cluster)
    quarxdrive/   → QuarXDrive (CLI, SDKs, FUSE integration)
    quarxcloud/   → QuarXCloud (deployment / orchestration)
    quarxproto/   → shared schemas / IDL for RPC + clients
    tools/        → leaf tools: admin, inspection, debugging
    docs/         → architecture, formats, roadmap

Rust is the primary implementation language.

─────────────────────────────────────────────────────────────────────────
## 🔥 Ingest Flow (Engine Perspective)

1. Client sends data via QuarXDrive → QuarXNet.  
2. QuarXCore chunks input into L0 blocks.  
3. For each L0:

       • compute hash
       • check index
       • reuse or append

4. Build L1 / Z overlays.  
5. Produce Object root.  
6. Update reference graph and telemetry.

─────────────────────────────────────────────────────────────────────────
## 📤 Read Flow

1. Client requests object/path via QuarXDrive → QuarXNet.  
2. QuarXNet resolves: `Object → Z → L1 → L0`.  
3. QuarXCore loads blocks from RAM / SSD / HDD (with promotion on hot paths).  
4. Data is streamed back to the client with minimal overhead.

─────────────────────────────────────────────────────────────────────────
## 📊 Telemetry

QuarXTor exposes detailed engine-level metrics:

• entropy per block  
• dedup effectiveness and savings  
• block/category statistics  
• ingest throughput and latencies  
• cache behavior and promotion patterns  
• object / graph inspection via overlays (Z-blocks)

This makes it possible to understand *why* storage is saved, not just *how much*.

─────────────────────────────────────────────────────────────────────────
## 📘 Getting Started

Quick Start commands are near the top; this section is the short pointer.

    git clone https://github.com/QuarXTor/QuarXTor.git
    cd QuarXTor
    ./quarxtor quickstart

• CLI and FUSE integration live in `quarxdrive/`.  
• RPC server and cluster plumbing live in `quarxnet/`.

─────────────────────────────────────────────────────────────────────────
## 📚 Documentation

• `ARCH.md` — architecture overview  
• `docs/core-formats.md` — stable on-disk ABI  
• `docs/packaging.md` — packaging and install details  
• `ROADMAP.md` — implementation stages and priorities  
• `CONTRIBUTING.md` — dev workflow  
• `AGENTS.md` — invariants and expectations

─────────────────────────────────────────────────────────────────────────
## 🤝 Funding QuarXTor

QuarXTor is independently developed and maintained.  
If this engine benefits you — support is appreciated.

### Open Collective  
https://opencollective.com/quarxtor

### GitHub Sponsors  
https://github.com/sponsors/QuarXTor

### Cryptocurrency

Bitcoin (BTC):  
bc1q53gchh5x9d80yd6ae7kud7fq4pxmnawsvsrzs8

Ethereum (ETH / ERC-20):  
0xFf276a4b769C71C183B7c40d4383a151efdf5cF9

*(You may use “QuarXTor Development Support” as a payment reference.)*

─────────────────────────────────────────────────────────────────────────
## 👤 Maintainer

Jee — system-level architecture and engine design.  
Contributions, benchmarks and review are welcome.

─────────────────────────────────────────────────────────────────────────
License: MIT
