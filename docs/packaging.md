# Packaging & Installation (Single-Binary)

This project ships as a small set of **standalone Rust binaries** (no runtime server required for local use).

This document covers:
- a practical **install story** (Linux / macOS / Windows)
- a **cross-platform build plan**
- **deterministic build notes** (best-effort)

## What gets built

The packaging scripts place built binaries into:

- `dist/bin/`

Common binaries (may vary by feature set):
- `quarxdrive` (CLI)
- `quarxctl` (admin/tooling CLI)
- `quarxnetd` (network daemon)
- `quarxdash` (read-only dashboard server)

Verify any installed binary with:

- `<bin> --version` (or `-V`)

## Linux (install story)

Build and produce a tarball:

```bash
./packaging/tarball/build-dist.sh
```

This creates a tarball under `dist/` containing `bin/` with available binaries.

Install (example: user-local):

```bash
mkdir -p ~/.local/bin
tar -xzf dist/quarxtor-*.tar.gz -C /tmp/quarxtor-dist
cp -f /tmp/quarxtor-dist/bin/* ~/.local/bin/
```

## macOS (install story)

On macOS, the same scripts work for local builds:

```bash
./packaging/tarball/build-dist.sh
```

Install by unpacking the tarball and copying the binaries into a directory on your `PATH` (e.g. `~/bin` or `~/.local/bin`).

## Windows (install story)

Build on Windows (PowerShell):

```powershell
cargo build --workspace --release --locked
```

Install by copying the resulting `.exe` files from `target\\release\\` into a directory on your `PATH`.

## Cross-platform build plan

Supported target triples (plan):

- Linux: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`
- Windows: `x86_64-pc-windows-msvc`
- macOS: `x86_64-apple-darwin`, `aarch64-apple-darwin`

The build helper supports cross-target builds via `TARGET`:

```bash
TARGET=x86_64-unknown-linux-gnu ./packaging/common/build-release.sh
```

Cross-compilation requires the corresponding Rust target toolchain (and, for some targets, an external linker). Using a dedicated tool such as `cross` is acceptable for CI/release builds.

## Deterministic build notes (best-effort)

QuarXTor prioritizes **deterministic runtime behavior** (stable outputs, stable ordering, canonical JSON).

For build reproducibility:

- Use `cargo build --locked` to pin dependency resolution to `Cargo.lock`.
- Record the toolchain used (`rustc -Vv`) for any published build artifacts.
- Prefer release builds without debug info when you need bit-for-bit closer reproducibility (debug info may embed paths).
