#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
MATRIX="$ROOT/generated/cve_matrix.json"

python3 - "$MATRIX" <<'PY'
import json
import sys
from pathlib import Path

matrix = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
rows = {row["cve_id"]: row for row in matrix["rows"]}
for cve in ("CVE-2026-5121", "CVE-2026-4426"):
    row = rows.get(cve)
    if row is None:
        raise SystemExit(f"missing {cve} in cve_matrix.json")
    if row["verification"] != "./scripts/check-i686-cve.sh":
        raise SystemExit(f"{cve} must point to ./scripts/check-i686-cve.sh")
PY

cd "$ROOT"
cargo test --test cve_regressions -- --exact i686_zisofs_pointer_table_overflow_is_rejected
cargo test --test cve_regressions -- --exact i686_zisofs_block_shift_is_validated
cargo test --test cve_regressions -- --exact i686_zstd_long_window_matches_ubuntu_patch_context
