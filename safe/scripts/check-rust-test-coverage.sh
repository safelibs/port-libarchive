#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
UPSTREAM_MANIFEST="$ROOT/generated/test_manifest.json"
RUST_MANIFEST="$ROOT/generated/rust_test_manifest.json"

[[ -f "$UPSTREAM_MANIFEST" ]] || {
  printf 'missing upstream manifest: %s\n' "$UPSTREAM_MANIFEST" >&2
  exit 1
}
[[ -f "$RUST_MANIFEST" ]] || {
  printf 'missing Rust manifest: %s\n' "$RUST_MANIFEST" >&2
  exit 1
}

tmpdir="$(mktemp -d "${TMPDIR:-/tmp}/libarchive-rust-coverage.XXXXXX")"
trap 'rm -rf "$tmpdir"' EXIT

list_output="$tmpdir/cargo-test-list.txt"

(
  cd "$ROOT"
  cargo test --workspace --all-features -- --list
) >"$list_output" 2>&1

python3 - "$UPSTREAM_MANIFEST" "$RUST_MANIFEST" "$list_output" <<'PY'
import json
import re
import sys
from pathlib import Path

upstream_manifest = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
rust_manifest = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
list_output = Path(sys.argv[3]).read_text(encoding="utf-8")

required_fields = {
    "suite",
    "define_test",
    "source_file",
    "driver_kind",
    "rust_test_target",
    "rust_test_name",
}

def upstream_key(row):
    return (row["suite"], row["define_test"], row["source_file"])

upstream_rows = upstream_manifest["rows"]
rust_rows = rust_manifest["rows"]

if len(upstream_rows) != len(rust_rows):
    raise SystemExit(
        f"row count mismatch: upstream has {len(upstream_rows)} rows, Rust manifest has {len(rust_rows)}"
    )

upstream_keys = set()
for row in upstream_rows:
    key = upstream_key(row)
    if key in upstream_keys:
        raise SystemExit(f"duplicate upstream manifest row: {key}")
    upstream_keys.add(key)

rust_keys = set()
rust_pairs = set()
for index, row in enumerate(rust_rows, start=1):
    missing = sorted(required_fields - row.keys())
    if missing:
        raise SystemExit(f"rust manifest row {index} is missing required fields: {', '.join(missing)}")

    if row["driver_kind"] == "upstream_c_suite":
        raise SystemExit(
            f"rust manifest row {index} still uses disallowed driver_kind upstream_c_suite for "
            f'{row["suite"]}:{row["define_test"]}'
        )

    if row["suite"] in {"tar", "cpio", "cat", "unzip"} and not row.get("frontend_binary"):
        raise SystemExit(
            f"rust manifest row {index} is missing frontend_binary metadata for "
            f'{row["suite"]}:{row["define_test"]}'
        )

    key = upstream_key(row)
    if key in rust_keys:
        raise SystemExit(f"duplicate Rust manifest mapping for upstream row: {key}")
    rust_keys.add(key)

    pair = (row["rust_test_target"], row["rust_test_name"])
    if pair in rust_pairs:
        raise SystemExit(f"duplicate Rust test pair in manifest: {pair}")
    rust_pairs.add(pair)

if rust_keys != upstream_keys:
    missing = sorted(upstream_keys - rust_keys)
    extra = sorted(rust_keys - upstream_keys)
    problems = []
    if missing:
        problems.append(f"missing Rust manifest rows: {missing[:5]}")
    if extra:
        problems.append(f"unexpected Rust manifest rows: {extra[:5]}")
    raise SystemExit("; ".join(problems))

listed_pairs = set()
current_target = None
running_re = re.compile(r"Running tests/([A-Za-z0-9_]+)\.rs\b")

for line in list_output.splitlines():
    running = running_re.search(line)
    if running:
        current_target = running.group(1)
        continue
    stripped = line.strip()
    if stripped.startswith("Running ") or stripped.startswith("Doc-tests "):
        current_target = None
        continue
    if current_target is None:
        continue
    if not stripped.endswith(": test"):
        continue
    test_name = stripped[:-6]
    listed_pairs.add((current_target, test_name))

missing_pairs = sorted(rust_pairs - listed_pairs)
if missing_pairs:
    preview = ", ".join(f"{target}:{name}" for target, name in missing_pairs[:10])
    raise SystemExit(f"Rust manifest references tests absent from cargo test --list: {preview}")

print(
    f"validated {len(rust_rows)} Rust test mappings against {len(listed_pairs)} listed cargo tests"
)
PY
