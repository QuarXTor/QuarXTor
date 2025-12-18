#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
cd "${REPO_ROOT}"

cargo test --workspace

SMOKE_BIN="${REPO_ROOT}/target/debug/quarxctl"
if [[ -x "${SMOKE_BIN}" ]]; then
  "${SMOKE_BIN}" --help >/dev/null
fi

echo "Tests and smoke checks completed"
