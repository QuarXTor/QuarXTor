### Overview

QuarXTor is a Rust-based storage stack; contributions are welcome via pull requests. The primary workflow is standard GitHub PRs. Before contributing, skim the README and `docs/` for context.

### Configuration invariants

- Config invariants are enforced by `QuarxConfig::validate()` at startup.
- Key rules:
  - `repo_root` must not equal `[import].root` and must not reside inside `[import].root`.
  - `[paths].data_dir/cache_dir/log_dir/runtime_dir` must not reside under `[import].root`.
- Invalid configs panic early so misconfigured nodes do not start.
- See `docs/config-reference.md` and `docs/paths-and-repo.md` for details.

### Example config

- `quarxctl.example.ini` is the canonical example config.
- Any new `QuarxConfig` field must be kept in sync across:
  - `core-rs/src/config.rs` doc comments,
  - `docs/config-reference.md`,
  - `quarxctl.example.ini` (matching section and comments).
- Do not edit the example in place for local tweaks; copy it and adjust in your own configs.

### Dev workflow

- Require a recent Rust stable toolchain (via rustup).
- Local checks:
  - `./tools/check-cyrillic.sh` — ensure no Cyrillic in `.rs`, `docs/*.md`, `.ini`, `.toml`.
  - `./dev-check.sh` — runs the Cyrillic check, then `cargo build --workspace`, then `cargo test --workspace`.
- PRs are expected to pass these checks locally before being opened.

### CI

- GitHub Actions workflow `.github/workflows/ci.yml` runs on every `push` and `pull_request`.
- CI runs `dev-check.sh`, which:
  - validates config/path invariants,
  - enforces English-only docs/sources (no Cyrillic),
  - builds and tests the workspace.
- Fix CI failures (config invariants or Cyrillic) before requesting review.
