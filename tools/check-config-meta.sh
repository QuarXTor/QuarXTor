#!/usr/bin/env sh
set -e

INI_PATH="tools/quarxctl/quarxctl.canonical.example.ini"
META_PATH="docs/config-meta.json"

if [ ! -f "$INI_PATH" ]; then
  echo "Canonical INI not found: $INI_PATH" >&2
  exit 1
fi

if [ ! -f "$META_PATH" ]; then
  echo "Config metadata not found: $META_PATH" >&2
  exit 1
fi

# Extract keys from INI (lines with '=' and no leading ';'/'#')
INI_KEYS=$(grep -E '^[^#;].*=.*' "$INI_PATH" | sed 's/[[:space:]]//g' | cut -d'=' -f1 | sort -u)

# Validate JSON and extract keys via Python
PYTHON_KEYS=$(python - <<'PY'
import json, sys
path = "docs/config-meta.json"
with open(path, "r", encoding="utf-8") as f:
    data = json.load(f)
if not isinstance(data, dict):
    print("config-meta.json must be a JSON object", file=sys.stderr)
    sys.exit(1)
for k, v in data.items():
    if not isinstance(v, dict):
        print(f"metadata for key {k} must be an object", file=sys.stderr)
        sys.exit(1)
    for req in ("group", "type", "level", "label", "description"):
        if req not in v:
            print(f"metadata for key {k} missing '{req}'", file=sys.stderr)
            sys.exit(1)
print("\\n".join(sorted(data.keys())))
PY
)

# Compare sets
MISSING=$(comm -23 <(echo "$INI_KEYS") <(echo "$PYTHON_KEYS"))
EXTRA=$(comm -13 <(echo "$INI_KEYS") <(echo "$PYTHON_KEYS"))

EXIT=0
if [ -n "$MISSING" ]; then
  echo "Keys missing in config-meta.json:" >&2
  echo "$MISSING" >&2
  EXIT=1
fi

if [ -n "$EXTRA" ]; then
  echo "Keys present in config-meta.json but not in canonical INI:" >&2
  echo "$EXTRA" >&2
  EXIT=1
fi

exit $EXIT
