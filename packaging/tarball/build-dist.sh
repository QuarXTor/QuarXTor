#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
cd "${REPO_ROOT}"

"${REPO_ROOT}/packaging/common/build-release.sh"

VERSION="$(git describe --tags --always 2>/dev/null || date +%Y%m%d-%H%M)"
ARCH="$(uname -m)"

DIST_DIR="${REPO_ROOT}/dist"
DIST_BIN="${DIST_DIR}/bin"
TARBALL="${DIST_DIR}/quarxtor-${VERSION}-${ARCH}.tar.gz"

mkdir -p "${DIST_DIR}"

if [[ ! -d "${DIST_BIN}" ]]; then
  echo "dist/bin not found after build; nothing to package" >&2
  exit 1
fi

tar -czf "${TARBALL}" -C "${DIST_DIR}" bin
echo "Created tarball ${TARBALL}"

# TODO: future passes may add checksums/signing; keep this script upload-free.
