#!/usr/bin/env bash
set -euo pipefail

# NOTE: make this script executable (chmod +x tools/check-cyrillic.sh).

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

TARGETS=(core-rs quarxnet quarxdrive quarxcloud tools docs)
PATTERN='[А-Яа-яЁё]'

if command -v rg >/dev/null 2>&1; then
  CMD=(rg --with-filename --line-number --color=never --pcre2)
  CMD+=(--glob '*.rs' --glob '*.md' --glob '*.toml' --glob '*.ini')
  CMD+=(--glob '!backups/**' --glob '!**/backups/**')
  CMD+=(--glob '!target/**' --glob '!.git/**' --glob '!QuarXTor-project-*.zip')
  CMD+=("${PATTERN}" "${TARGETS[@]}")
  if output="$("${CMD[@]}")"; then
    echo "${output}"
    echo "ERROR: Cyrillic text found in sources/docs. Please translate to English."
    exit 1
  else
    status=$?
    if [[ ${status} -eq 1 ]]; then
      echo "OK: no Cyrillic found."
      exit 0
    fi
    exit "${status}"
  fi
else
  CMD=(grep -R --line-number --color=never -E "${PATTERN}")
  CMD+=(--include='*.rs' --include='*.md' --include='*.toml' --include='*.ini')
  CMD+=(--exclude-dir='backups' --exclude-dir='target' --exclude-dir='.git')
  CMD+=(--exclude='QuarXTor-project-*.zip')
  CMD+=("${TARGETS[@]}")
  if output="$("${CMD[@]}")"; then
    echo "${output}"
    echo "ERROR: Cyrillic text found in sources/docs. Please translate to English."
    exit 1
  else
    status=$?
    if [[ ${status} -eq 1 ]]; then
      echo "OK: no Cyrillic found."
      exit 0
    fi
    exit "${status}"
  fi
fi
