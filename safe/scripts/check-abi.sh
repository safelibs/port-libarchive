#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
INVENTORY_ONLY=0

while (($#)); do
  case "$1" in
    --inventory-only)
      INVENTORY_ONLY=1
      shift
      ;;
    --help|-h)
      cat <<'EOF'
usage: check-abi.sh [--inventory-only]
EOF
      exit 0
      ;;
    *)
      printf 'unknown option: %s\n' "$1" >&2
      exit 1
      ;;
  esac
done

require_file() {
  [[ -f "$1" ]] || {
    printf 'missing required file: %s\n' "$1" >&2
    exit 1
  }
}

require_file "$ROOT/abi/libarchive.map"
require_file "$ROOT/abi/exported_symbols.txt"
require_file "$ROOT/abi/original_exported_symbols.txt"
require_file "$ROOT/abi/original_version_info.txt"
require_file "$ROOT/generated/api_inventory.json"

map_symbols="$(
  awk '
    /^  global:/ { in_global=1; next }
    /^  local:/ { in_global=0 }
    in_global {
      gsub(/[[:space:]]/, "", $0)
      sub(/;.*/, "", $0)
      if ($0 != "") print $0
    }
  ' "$ROOT/abi/libarchive.map" | sort
)"

exported_symbols="$(sort "$ROOT/abi/exported_symbols.txt")"

diff -u <(printf '%s\n' "$map_symbols") <(printf '%s\n' "$exported_symbols") >/dev/null || {
  echo "safe/abi/libarchive.map and safe/abi/exported_symbols.txt disagree" >&2
  exit 1
}

original_export_count="$(grep -cve '^[[:space:]]*$' "$ROOT/abi/original_exported_symbols.txt")"
[[ "$original_export_count" -eq 421 ]] || {
  printf 'expected 421 original exported symbols, found %s\n' "$original_export_count" >&2
  exit 1
}

if ((INVENTORY_ONLY)); then
  exit 0
fi

echo "non-inventory ABI verification is not implemented in phase 1" >&2
exit 1
