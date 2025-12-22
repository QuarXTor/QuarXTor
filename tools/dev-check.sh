#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

echo "[1/3] Cyrillic check..."
./tools/check-cyrillic.sh

echo "[2/3] cargo build --workspace..."
cargo build --workspace

echo "[3/3] cargo test --workspace..."
cargo test --workspace
