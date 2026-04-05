#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
INVENTORY_ONLY=0
STRICT=0

usage() {
  cat <<'EOF'
usage: check-abi.sh [--inventory-only] [--strict]

  --inventory-only  Validate the recorded ABI contract artifacts only.
  --strict          Build the release shared library and compare the live ABI
                    against the recorded original-export contract.
EOF
}

while (($#)); do
  case "$1" in
    --inventory-only)
      INVENTORY_ONLY=1
      shift
      ;;
    --strict)
      STRICT=1
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

require_file() {
  [[ -f "$1" ]] || {
    printf 'missing required file: %s\n' "$1" >&2
    exit 1
  }
}

for path in \
  "$ROOT/abi/libarchive.map" \
  "$ROOT/abi/exported_symbols.txt" \
  "$ROOT/abi/original_exported_symbols.txt" \
  "$ROOT/abi/original_version_info.txt"
do
  require_file "$path"
done

python3 - "$ROOT" "$INVENTORY_ONLY" "$STRICT" <<'PY'
import subprocess
import sys
from pathlib import Path

root = Path(sys.argv[1])
inventory_only = sys.argv[2] == "1"
strict = sys.argv[3] == "1"


def load_plain_symbols(path: Path) -> list[str]:
    return [
        line.strip()
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]


def load_map_symbols(path: Path) -> list[str]:
    symbols: list[str] = []
    in_global = False
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if raw_line.startswith("  global:"):
            in_global = True
            continue
        if raw_line.startswith("  local:"):
            in_global = False
            continue
        if not in_global:
            continue
        line = line.rstrip(";")
        if line:
            symbols.append(line)
    return symbols


def extract_live_symbols(path: Path) -> list[str]:
    output = subprocess.run(
        ["readelf", "--dyn-syms", "--wide", str(path)],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout
    symbols: list[str] = []
    seen: set[str] = set()
    for raw_line in output.splitlines():
        line = raw_line.strip()
        if not line or ":" not in line:
            continue
        columns = line.split()
        if len(columns) < 8:
            continue
        if columns[4] != "GLOBAL" or columns[6] == "UND":
            continue
        name = columns[-1].split("@", 1)[0]
        if name and name not in seen:
            seen.add(name)
            symbols.append(name)
    return sorted(symbols)


def extract_defined_version_names(contents: str) -> list[str]:
    names: set[str] = set()
    in_definitions = False
    for line in contents.splitlines():
        if line.startswith("Version definition section "):
            in_definitions = True
            continue
        if line.startswith("Version needs section ") or line.startswith("Version symbols section "):
            in_definitions = False
        if not in_definitions or "Name:" not in line:
            continue
        names.add(line.split("Name:", 1)[1].split()[0])
    return sorted(names)


def extract_live_defined_version_names(path: Path) -> list[str]:
    output = subprocess.run(
        ["readelf", "-V", str(path)],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout
    return extract_defined_version_names(output)


def extract_soname(path: Path) -> str:
    output = subprocess.run(
        ["readelf", "-d", str(path)],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout
    for line in output.splitlines():
        if "(SONAME)" not in line:
            continue
        start = line.rfind("[")
        end = line.rfind("]")
        if start != -1 and end != -1 and end > start:
            return line[start + 1 : end]
    raise SystemExit(f"missing SONAME in {path}")


def fail(label: str, actual: list[str], expected: list[str]) -> None:
    actual_set = set(actual)
    expected_set = set(expected)
    missing = sorted(expected_set - actual_set)
    extra = sorted(actual_set - expected_set)
    raise SystemExit(
        f"{label} drifted from the recorded contract\n"
        f"missing: {missing[:20]}\n"
        f"extra: {extra[:20]}"
    )


map_symbols = load_map_symbols(root / "abi/libarchive.map")
recorded_symbols = load_plain_symbols(root / "abi/exported_symbols.txt")
original_symbols = load_plain_symbols(root / "abi/original_exported_symbols.txt")

if map_symbols != recorded_symbols:
    fail("safe/abi/libarchive.map", map_symbols, recorded_symbols)
if recorded_symbols != original_symbols:
    fail("safe/abi/exported_symbols.txt", recorded_symbols, original_symbols)

original_defined_version_names = extract_defined_version_names(
    (root / "abi/original_version_info.txt").read_text(encoding="utf-8")
)

if inventory_only and not strict:
    print(
        f"validated recorded ABI inventory: {len(recorded_symbols)} exported symbols, "
        f"{len(original_defined_version_names)} defined version names"
    )
    raise SystemExit(0)

subprocess.run(["cargo", "build", "--release"], cwd=root, check=True, stdout=subprocess.DEVNULL)

live_library = root / "target/release/libarchive.so"
if not live_library.is_file():
    raise SystemExit(f"missing release shared library: {live_library}")

live_symbols = extract_live_symbols(live_library)
if live_symbols != original_symbols:
    fail("live exported symbols", live_symbols, original_symbols)

live_defined_version_names = extract_live_defined_version_names(live_library)
if live_defined_version_names != original_defined_version_names:
    fail(
        "live defined version names",
        live_defined_version_names,
        original_defined_version_names,
    )

soname = extract_soname(live_library)
if soname != "libarchive.so.13":
    raise SystemExit(f"unexpected SONAME {soname!r}, expected 'libarchive.so.13'")

print(
    f"validated ABI: {len(live_symbols)} exported symbols, "
    f"{len(live_defined_version_names)} defined version names, SONAME {soname}"
)
PY
