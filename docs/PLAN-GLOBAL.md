## QuarXTor Global Plan (authoritative snapshot)

### Vision & Scope
- Hash-graph storage stack with deduplication and multi-tier storage.
- Modular layers with strict boundaries:
  - **QuarXCore**: block/object engine (LocalRepo, FsBlockStore, validation).
  - **QuarXDrive**: client/import/pseudo-FS + future FUSE.
  - **QuarXNet**: transports + sync (HTTP/Framed/IPC).
  - **QuarXCloud**: cluster/orchestrator/scheduler.
  - **CLI/Tools**: quarxctl and automation entrypoints.

### Module overview (role / deps / maturity)
- **QuarXCore** — owns on-disk model; no external deps; maturity ~80% (LocalRepo/FsBlockStore working, validation skeleton).
- **QuarXDrive** — depends on core; offers DriveRepo, import, pseudo-fs; maturity ~70% (local path solid, NetDrive usable).
- **QuarXNet** — depends on core for types; provides transports + sync; maturity ~65–70% (dummy + HTTP sync paths working, DeltaDescriptor inventory + capabilities; framed pending).
- **QuarXCloud** — depends on quarxnet; orchestrator/scheduler contracts only; maturity ~20% (interfaces only).
- **CLI/Tools** — depends on drive/net/cloud; maturity ~70% (local workflows + net ping/health/ls-roots/push/pull/caps for single HTTP remote).

### Phase breakdown (1.x → 5.x)
- **Phase 1.x — Core engine**
  - Goal: stable block/object model, LocalRepo, FsBlockStore.
  - Status: **Done** (basic validation still evolving).
- **Phase 2.x — Local drive + CLI**
  - Goal: import/pseudo-fs + local CLI (quarxctl).
  - Status: **Done/usable** (local round-trips, drive CLI, import, pseudo-fs).
- **Phase 3.x — Networking & sync**
  - 3.1: transport baseline (envelope, error model) — **Done**.
  - 3.2: transport wiring (HTTP/framed stubs) — **Done**.
  - 3.3A: minimal sync client + NetDrive + DummySyncClient — **Done**.
  - 3.3B: HTTP sync path end-to-end — **Done**.
  - 3.3C: quarxctl net push/pull on NetDrive — **Done (basic single-remote)**.
  - 3.3D-lite: DeltaDescriptor inventory + HTTP get_delta/capabilities — **Done (inventory level)**.
  - 3.4+: auth/TLS, framed transport parity, smarter deltas — **Planned**.
- **Phase 4.x — QuarXCloud**
  - Goal: coordinator/follower/gateway roles, scheduling, fencing.
  - Status: **Planned** (contracts exist).
- **Phase 5.x — Integrations**
  - Goal: FUSE gateway, S3-like API, user-facing tooling.
  - Status: **Planned**.

### Current progress snapshot
- Core: LocalRepo + FsBlockStore working; IDs/model stable.
- Drive: Local backend + import + pseudo-fs + CLI local commands working.
- Net: Dummy + HTTP sync paths working; DeltaDescriptor inventory + capabilities; framed pending; no auth/TLS yet.
- Cloud: Contracts (orchestrator/scheduler/health) only.
- CLI: Local workflows + net ping/health/ls-roots/push/pull/caps for single HTTP remote.
Overall progress: ~60–65% toward 1.0.

### Next milestones
- **3.3D** — advanced sync (deltas, resumable, multi-remote).
- **3.4** — Transport auth/TLS & capabilities.
- **4.0** — QuarXCloud orchestrator MVP (membership, scheduling, HA).
- **4.x** — cluster operations (health, metrics, balancing).
- **5.0** — public integrations (FUSE/S3/etc).
> **Status:** Legacy snapshot. An up-to-date plan is maintained separately alongside machine-readable snapshots.
