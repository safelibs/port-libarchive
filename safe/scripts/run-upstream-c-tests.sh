#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/generated/test_manifest.json"
CONTRACT="$ROOT/generated/original_build_contract.json"
LINK_MANIFEST="$ROOT/generated/link_compat_manifest.json"
SUITE=""
PHASE_GROUP=""
LIST_ONLY=0

usage() {
  cat <<'EOF'
usage: run-upstream-c-tests.sh [<suite> <phase-group>] [--suite <name>] [--phase-group <name>] [--list]

Build a filtered upstream libarchive test harness using the preserved phase-1
manifest/build contract artifacts and run only the selected suite/phase-group.
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
    --*)
      printf 'unknown option: %s\n' "$1" >&2
      usage >&2
      exit 1
      ;;
    *)
      if [[ -z "$SUITE" ]]; then
        SUITE="$1"
      elif [[ -z "$PHASE_GROUP" ]]; then
        PHASE_GROUP="$1"
      else
        printf 'unexpected positional argument: %s\n' "$1" >&2
        usage >&2
        exit 1
      fi
      shift
      ;;
  esac
done

[[ -f "$MANIFEST" ]] || {
  printf 'missing test manifest: %s\n' "$MANIFEST" >&2
  exit 1
}
[[ -f "$CONTRACT" ]] || {
  printf 'missing build contract: %s\n' "$CONTRACT" >&2
  exit 1
}
[[ -f "$LINK_MANIFEST" ]] || {
  printf 'missing link manifest: %s\n' "$LINK_MANIFEST" >&2
  exit 1
}

if [[ $LIST_ONLY -eq 1 ]]; then
  python3 - "$MANIFEST" "$SUITE" "$PHASE_GROUP" <<'PY'
import json
import sys
from pathlib import Path

manifest = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
suite = sys.argv[2]
phase_group = sys.argv[3]
rows = [
    row for row in manifest["rows"]
    if (not suite or row["suite"] == suite)
    and (not phase_group or row["phase_group"] == phase_group)
]
for row in rows:
    print(f'{row["suite"]}:{row["define_test"]}:{row["phase_group"]}')
PY
  exit 0
fi

[[ -n "$SUITE" ]] || {
  printf 'suite is required\n' >&2
  usage >&2
  exit 1
}
[[ -n "$PHASE_GROUP" ]] || {
  printf 'phase-group is required\n' >&2
  usage >&2
  exit 1
}

BUILD_DIR="$ROOT/target/upstream-c-tests/${SUITE}-${PHASE_GROUP}"
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"

python3 - "$ROOT" "$MANIFEST" "$CONTRACT" "$LINK_MANIFEST" "$SUITE" "$PHASE_GROUP" "$BUILD_DIR" <<'PY'
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])
manifest_path = Path(sys.argv[2])
contract_path = Path(sys.argv[3])
link_manifest_path = Path(sys.argv[4])
suite = sys.argv[5]
phase_group = sys.argv[6]
build_dir = Path(sys.argv[7])

manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
contract = json.loads(contract_path.read_text(encoding="utf-8"))
link_manifest = json.loads(link_manifest_path.read_text(encoding="utf-8"))

repo_root = root.parent

def resolve_artifact(path: str) -> Path:
    artifact = Path(path)
    if artifact.is_absolute():
        return artifact

    safe_relative = root / artifact
    if safe_relative.exists():
        return safe_relative

    repo_relative = repo_root / artifact
    if repo_relative.exists():
        return repo_relative

    return safe_relative

rows = [
    row for row in manifest["rows"]
    if row["suite"] == suite and row["phase_group"] == phase_group
]
if not rows:
    raise SystemExit(f"no tests selected for suite={suite!r} phase_group={phase_group!r}")

selected_names = {row["define_test"] for row in rows}
selected_sources = {row["source_file"] for row in rows}
generated_list = resolve_artifact(contract["generated_headers"]["list_h_by_suite"][suite])

ordered_names = []
for line in generated_list.read_text(encoding="utf-8").splitlines():
    line = line.strip()
    if not line.startswith("DEFINE_TEST(") or not line.endswith(")"):
        continue
    name = line[len("DEFINE_TEST("):-1]
    if name in selected_names:
        ordered_names.append(name)

list_h = build_dir / "list.h"
list_h.write_text("".join(f"DEFINE_TEST({name})\n" for name in ordered_names), encoding="utf-8")

objects = []
for row in link_manifest["objects"]:
    if row["target_name"] != "libarchive_test":
        continue
    if row["source_path"] == "original/libarchive-3.7.2/test_utils/test_utils.c":
        objects.append((row["link_order"], resolve_artifact(row["preserved_object_path"])))
    elif row["source_path"] in selected_sources:
        objects.append((row["link_order"], resolve_artifact(row["preserved_object_path"])))

objects.sort(key=lambda item: item[0])
(build_dir / "objects.txt").write_text(
    "".join(f"{path}\n" for _, path in objects),
    encoding="utf-8",
)
(build_dir / "tests.txt").write_text(
    "".join(f"{name}\n" for name in ordered_names),
    encoding="utf-8",
)
(build_dir / "extra_libs.txt").write_text(
    " ".join(contract["link_targets"]["libarchive_test"]["extra_libraries"]),
    encoding="utf-8",
)
PY

printf 'selected tests:\n'
sed 's/^/  /' "$BUILD_DIR/tests.txt"

cargo build >/dev/null

CC_BIN="${CC:-cc}"
TEST_MAIN_SRC="$ROOT/../original/libarchive-3.7.2/test_utils/test_main.c"
CONFIG_DIR="$ROOT/generated/original_c_build"
SAFE_INCLUDE_DIR="$ROOT/include"
ORIGINAL_LIBARCHIVE_DIR="$ROOT/../original/libarchive-3.7.2/libarchive"
ORIGINAL_TEST_DIR="$ROOT/../original/libarchive-3.7.2/libarchive/test"
ORIGINAL_TEST_UTILS_DIR="$ROOT/../original/libarchive-3.7.2/test_utils"
TEST_MAIN_OBJ="$BUILD_DIR/test_main.o"
TEST_BIN="$BUILD_DIR/${SUITE}-${PHASE_GROUP}-tests"

"$CC_BIN" -c "$TEST_MAIN_SRC" \
  -o "$TEST_MAIN_OBJ" \
  -DHAVE_CONFIG_H=1 \
  -D__LIBARCHIVE_TEST=1 \
  -I"$BUILD_DIR" \
  -I"$CONFIG_DIR" \
  -I"$SAFE_INCLUDE_DIR" \
  -I"$ORIGINAL_LIBARCHIVE_DIR" \
  -I"$ORIGINAL_TEST_DIR" \
  -I"$ORIGINAL_TEST_UTILS_DIR"

mapfile -t OBJECTS < "$BUILD_DIR/objects.txt"
EXTRA_LIBS="$(<"$BUILD_DIR/extra_libs.txt")"

"$CC_BIN" \
  -o "$TEST_BIN" \
  "$TEST_MAIN_OBJ" \
  "${OBJECTS[@]}" \
  -L"$ROOT/target/debug" \
  -Wl,-rpath,"$ROOT/target/debug" \
  -larchive \
  $EXTRA_LIBS

LD_LIBRARY_PATH="$ROOT/target/debug${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  "$TEST_BIN" -r "$ORIGINAL_TEST_DIR"
