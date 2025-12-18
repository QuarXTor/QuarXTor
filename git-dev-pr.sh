#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

if [ "$#" -lt 1 ]; then
    echo 'Usage: ./git-dev-pr.sh "commit message"'
    exit 1
fi

COMMIT_MSG="$1"

echo "[1/4] dev-check.sh..."
./dev-check.sh

echo "[2/4] git status (before commit)..."
BRANCH="$(git rev-parse --abbrev-ref HEAD)"
echo "[i] Current branch: ${BRANCH}"
git status --short

echo "[3/4] git add + commit..."
git add -A
if git diff --cached --quiet; then
    echo "[i] No changes to commit. Skipping commit/push."
    exit 0
fi
git commit -m "${COMMIT_MSG}"

echo "[4/4] git push..."
git push -u origin "${BRANCH}"
