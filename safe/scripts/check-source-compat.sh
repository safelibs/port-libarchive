#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

for path in \
  "$ROOT/include/archive.h" \
  "$ROOT/include/archive_entry.h" \
  "$ROOT/generated/original_build_contract.json" \
  "$ROOT/generated/original_pkgconfig/libarchive.pc" \
  "$ROOT/generated/api_inventory.json"
do
  [[ -f "$path" ]] || {
    printf 'missing required source-compat input: %s\n' "$path" >&2
    exit 1
  }
done

"$ROOT/scripts/render-pkg-config.sh" --mode build-tree --check
python3 "$ROOT/tools/gen_api_inventory.py" --check
