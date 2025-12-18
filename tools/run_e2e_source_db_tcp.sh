#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

# Safety guard
git status -sb
{ git reflog --date=iso | head -30; } || true

if ! git diff --quiet --ignore-submodules --exit-code || [ -n "$(git status --porcelain)" ]; then
  echo "Working tree is dirty; please stash/commit manually before running." >&2
  exit 2
fi

log_dir="target/tmp"
log_file="${log_dir}/e2e_source_db_tcp.log"
mkdir -p "$log_dir"

cargo build -q --locked

set +e
cargo run -q --locked -p quarxnet --bin quarxnet-e2e-source-db >"$log_file" 2>&1
run_rc=$?
set -e

if [[ $run_rc -ne 0 ]]; then
  echo "RUN FAILED" >&2
  tail -n 200 "$log_file" >&2 || true
  exit "$run_rc"
fi

summary_line=$(grep -E '^FILES=[0-9]+ BLOCKS=[0-9]+ BYTES=[0-9]+ HASH_OK=(true|false)$' "$log_file" | tail -n1 || true)

if [[ -z "$summary_line" ]]; then
  echo "SUMMARY MISSING" >&2
  tail -n 200 "$log_file" >&2 || true
  exit 3
fi

echo "$summary_line"

if [[ "$summary_line" != *"HASH_OK=true" ]]; then
  exit 1
fi
