#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/generated/test_manifest.json"
SUITE=""
PHASE_GROUP=""
LIST_ONLY=0

usage() {
  cat <<'EOF'
usage: run-upstream-c-tests.sh [--suite <name>] [--phase-group <name>] [--list]

Phase 1 provides the shared manifest-driven test selector. Execution of rebuilt
upstream binaries is added in later phases once the safe library is featureful
enough to run the preserved test consumers.
EOF
}

while (($#)); do
  case "$1" in
    --suite)
      SUITE="${2:?missing value for --suite}"
      shift 2
      ;;
    --phase-group)
      PHASE_GROUP="${2:?missing value for --phase-group}"
      shift 2
      ;;
    --list)
      LIST_ONLY=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      printf 'unknown option: %s\n' "$1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

[[ -f "$MANIFEST" ]] || {
  printf 'missing test manifest: %s\n' "$MANIFEST" >&2
  exit 1
}

python3 - "$MANIFEST" "$SUITE" "$PHASE_GROUP" "$LIST_ONLY" <<'PY'
import json
import sys
from pathlib import Path

manifest_path = Path(sys.argv[1])
suite = sys.argv[2]
phase_group = sys.argv[3]
list_only = sys.argv[4] == "1"

manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
rows = manifest["rows"]
selected = [
    row for row in rows
    if (not suite or row["suite"] == suite)
    and (not phase_group or row["phase_group"] == phase_group)
]

for row in selected:
    print(f'{row["suite"]}:{row["define_test"]}:{row["phase_group"]}')

if not list_only:
    raise SystemExit(
        "phase 1 only ships the manifest-driven test selector; relinked execution arrives later"
    )
PY
