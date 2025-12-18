#!/usr/bin/env bash
# Smoke runbook (keep stable; local-only automation allowed here)
# - stdout discipline: only final summary line from runner; all other output to stderr/log
# - auto-stash policy: dirty trees are stashed automatically before running
# - detach uses setsid+nohup with real BIN (no bash functions)
# - resume waits for summary in log; pid dead + no summary = failure
# - paths: LOG target/tmp/e2e_source_db_tcp.log, PID target/tmp/e2e_source_db_tcp.pid, FILE_LIST target/tmp/source_db_files.txt
# Smoke guard: must prove real TCP (listener + connect). No listener or bypass => non-zero, no summary.
set -euo pipefail

cd "$(dirname "$0")/.."

LOG_DIR="target/tmp"
LOG_FILE="${LOG_DIR}/e2e_source_db_tcp.log"
PID_FILE="${LOG_DIR}/e2e_source_db_tcp.pid"
FILE_LIST="${LOG_DIR}/source_db_files.txt"
DATASET_DIR="${QUARX_DATASET_DIR:-}"
BIN="./target/debug/quarxnet-e2e-source-db"
BIND_ADDR="${QUARX_BIND_ADDR:-0.0.0.0:39090}"
CONNECT_ADDR="${QUARX_CONNECT_ADDR:-192.168.192.30:39090}"
PORT="${QUARX_PORT:-${BIND_ADDR##*:}}"
XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-$PWD/target/tmp/config}"

if [[ "${QUARX_CLEAN_ENV:-0}" != "1" ]]; then
  env_args=(env -i HOME="$HOME" USER="$USER" PATH="$PATH" TERM="$TERM" QUARX_CLEAN_ENV=1 QUARX_BIND_ADDR="$BIND_ADDR" QUARX_CONNECT_ADDR="$CONNECT_ADDR" XDG_CONFIG_HOME="$XDG_CONFIG_HOME")
  if [[ -n "$DATASET_DIR" ]]; then
    env_args+=(QUARX_DATASET_DIR="$DATASET_DIR")
  fi
  exec "${env_args[@]}" "$0" "$@"
fi

mode=""
use_file_list=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --smoke) mode="smoke" ;;
    --full) mode="full" ;;
    --resume) mode="resume" ;;
    --stop) mode="stop" ;;
    --status) mode="status" ;;
    --file-list) use_file_list=1 ;;
    *)
      echo "unknown arg: $1" >&2
      exit 1
      ;;
  esac
  shift
done

if [[ -z "$mode" ]]; then
  echo "usage: $0 [--smoke|--full|--resume|--stop|--status] [--file-list]" >&2
  exit 1
fi

if [[ "$mode" == "smoke" && -z "${QUARX_PORT:-}" ]]; then
  PORT=39091
  BIND_ADDR="${BIND_ADDR%:*}:$PORT"
  CONNECT_ADDR="${CONNECT_ADDR%:*}:$PORT"
fi

git status -sb
{ git reflog --date=iso | head -30; } || true

if ! git diff --quiet --ignore-submodules --exit-code || [ -n "$(git status --porcelain)" ]; then
  git diff > /tmp/quarxtor-wip.diff || true
fi

mkdir -p "$LOG_DIR"

summarize() {
  local line
  line=$(grep -E '^FILES=[0-9]+ BLOCKS=[0-9]+ BYTES=[0-9]+ HASH_OK=(true|false)$' "$LOG_FILE" | tail -n1 || true)
  if [[ -z "$line" ]]; then
    return 1
  fi
  echo "$line"
  [[ "$line" == *"HASH_OK=true" ]]
}

is_running() {
  [[ -f "$PID_FILE" ]] || return 1
  local pid
  pid=$(cat "$PID_FILE" 2>/dev/null || true)
  [[ -n "$pid" ]] || return 1
  kill -0 "$pid" 2>/dev/null
}

status_cmd() {
  if is_running; then
    echo "RUNNING PID=$(cat "$PID_FILE") LISTEN=${BIND_ADDR}"
  else
    echo "NOT_RUNNING"
  fi
}

stop_cmd() {
  if is_running; then
    local pid
    pid=$(cat "$PID_FILE")
    kill "$pid" 2>/dev/null || true
    sleep 0.2
    if kill -0 "$pid" 2>/dev/null; then
      kill -9 "$pid" 2>/dev/null || true
    fi
    rm -f "$PID_FILE"
    echo "stopped"
  else
    echo "not running"
  fi
}

make_file_list() {
  if [[ -z "$DATASET_DIR" ]]; then
    echo "DATASET ROOT NOT SET (config missing and QUARX_DATASET_DIR not set)" >&2
    exit 4
  fi
  find "$DATASET_DIR" -type f -print0 | sort -z | tr '\0' '\n' > "$FILE_LIST"
}

build_runner() {
  cargo build -q --locked -p quarxnet --bin quarxnet-e2e-source-db
}

ensure_cmd_array() {
  cmd=("$BIN")
  if [[ "$mode" == "smoke" ]]; then
    cmd+=("--smoke")
  fi
  if [[ "$use_file_list" -eq 1 ]]; then
    cmd+=("--file-list" "$FILE_LIST")
  fi
  cmd+=("--bind" "$BIND_ADDR" "--connect" "$CONNECT_ADDR")
}

has_listener() {
  if ss -ltnp 2>/dev/null | grep -F ":${PORT}" >/dev/null; then
    return 0
  fi
  port_hex=$(printf "%04X" "$PORT")
  grep -E ":${port_hex} .* 0A" /proc/net/tcp >/dev/null 2>&1
}

case "$mode" in
  status)
    status_cmd
    exit 0
    ;;
  stop)
    stop_cmd
    exit 0
    ;;
  resume)
    if ! [[ -f "$LOG_FILE" ]]; then
      echo "no log file to resume" >&2
      exit 1
    fi
    while true; do
      if summarize; then
        rm -f "$PID_FILE"
        exit 0
      fi
      if ! is_running; then
        echo "SUMMARY MISSING" >&2
        tail -n 200 "$LOG_FILE" >&2 || true
        exit 3
      fi
      sleep 2
    done
    ;;
esac

if [[ "$mode" != "smoke" ]]; then
  if is_running; then
    echo "reusing running server pid $(cat "$PID_FILE")" >&2
    exit 0
  fi
fi

if [[ "$use_file_list" -eq 1 ]]; then
  make_file_list
fi

build_runner
ensure_cmd_array

if [[ "$mode" == "smoke" ]]; then
  export QUARX_SMOKE=1
  : >"$LOG_FILE"
  set +e
  "${cmd[@]}" >"$LOG_FILE" 2>&1 &
  runner_pid=$!
  deadline=$((SECONDS + 3))
  listener_ok=0
  while [[ $SECONDS -lt $deadline ]]; do
    if has_listener; then
      listener_ok=1
      break
    fi
    if ! kill -0 "$runner_pid" 2>/dev/null; then
      break
    fi
    sleep 0.1
  done
  if [[ $listener_ok -ne 1 ]]; then
    echo "NO_LISTENER" >&2
    kill "$runner_pid" 2>/dev/null || true
    wait "$runner_pid" 2>/dev/null || true
    exit 2
  fi
  wait "$runner_pid"
  rc=$?
  set -e
  if [[ $rc -ne 0 ]]; then
    echo "RUN FAILED" >&2
    tail -n 200 "$LOG_FILE" >&2 || true
    exit $rc
  fi
  if ! summarize; then
    echo "SUMMARY MISSING" >&2
    tail -n 200 "$LOG_FILE" >&2 || true
    exit 3
  fi
  exit 0
fi

# full (detached start only)
: >"$LOG_FILE"
setsid nohup bash -c '
  "$@"
  exec sleep 1000000
' _ "${cmd[@]}" >"$LOG_FILE" 2>&1 < /dev/null &
echo $! > "$PID_FILE"
deadline=$((SECONDS + 3))
while [[ $SECONDS -lt $deadline ]]; do
  if has_listener || kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
    echo "started pid=$(cat "$PID_FILE")" >&2
    exit 0
  fi
  sleep 0.1
done
echo "NO_LISTENER" >&2
tail -n 200 "$LOG_FILE" >&2 || true
exit 2
