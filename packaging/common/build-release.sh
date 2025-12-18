#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
cd "${REPO_ROOT}"

PROFILE="${PROFILE:-release}"
TARGET="${TARGET:-}"

build_args=(--workspace "--${PROFILE}")
if [[ -n "${TARGET}" ]]; then
  build_args+=(--target "${TARGET}")
fi

cargo build "${build_args[@]}"

DIST_BIN="${REPO_ROOT}/dist/bin"
mkdir -p "${DIST_BIN}"

copy_bin() {
  local bin_name="$1"
  local src_path
  if [[ -n "${TARGET}" ]]; then
    src_path="${REPO_ROOT}/target/${TARGET}/${PROFILE}/${bin_name}"
  else
    src_path="${REPO_ROOT}/target/${PROFILE}/${bin_name}"
  fi
  if [[ -x "${src_path}" ]]; then
    cp -f "${src_path}" "${DIST_BIN}/"
  fi
}

copy_bin "quarxctl"
copy_bin "quarxnetd"
copy_bin "quarxnetd-framed"
copy_bin "quarxdrive"

echo "Binaries placed (where present) in ${DIST_BIN}"
