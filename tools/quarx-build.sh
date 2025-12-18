#!/usr/bin/env bash
# Smoke guard: canonical entrypoint. Build + TCP smoke gate. Smoke failure stops the run.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MODE="smoke"
FULL_PORT="${QUARX_FULL_PORT:-39090}"
SMOKE_PORT="${QUARX_SMOKE_PORT:-39091}"
FULL_BIND="${QUARX_BIND_ADDR:-0.0.0.0}:${FULL_PORT}"
FULL_CONNECT="${QUARX_CONNECT_ADDR:-192.168.192.30}:${FULL_PORT}"
SMOKE_BIND="0.0.0.0:${SMOKE_PORT}"
SMOKE_CONNECT="192.168.192.30:${SMOKE_PORT}"

if [[ $# -gt 0 ]]; then
  case "$1" in
    --smoke) MODE="smoke" ;;
    --full) MODE="full" ;;
    --status) MODE="status" ;;
    --stop) MODE="stop" ;;
    *)
      echo "usage: $0 [--smoke|--full|--status|--stop]" >&2
      exit 1
      ;;
  esac
fi

cd "$ROOT"

cargo build --locked

if [[ "$MODE" == "smoke" ]]; then
  exec env QUARX_BIND_ADDR="$SMOKE_BIND" QUARX_CONNECT_ADDR="$SMOKE_CONNECT" QUARX_PORT="$SMOKE_PORT" timeout 15s bash tools/e2e0_tcp_source_db.sh --smoke
elif [[ "$MODE" == "status" || "$MODE" == "stop" ]]; then
  exec bash tools/e2e0_tcp_source_db.sh "--${MODE#--}"
else
  # Always run smoke as the gate even in full mode.
  env QUARX_BIND_ADDR="$SMOKE_BIND" QUARX_CONNECT_ADDR="$SMOKE_CONNECT" QUARX_PORT="$SMOKE_PORT" timeout 15s bash tools/e2e0_tcp_source_db.sh --smoke
  exec env QUARX_BIND_ADDR="$FULL_BIND" QUARX_CONNECT_ADDR="$FULL_CONNECT" QUARX_PORT="$FULL_PORT" timeout 15s bash tools/e2e0_tcp_source_db.sh --full
fi
