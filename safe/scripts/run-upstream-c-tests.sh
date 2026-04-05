#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/generated/test_manifest.json"
CONTRACT="$ROOT/generated/original_build_contract.json"
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

python3 - "$ROOT" "$MANIFEST" "$CONTRACT" "$SUITE" "$PHASE_GROUP" "$BUILD_DIR" <<'PY'
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])
manifest_path = Path(sys.argv[2])
contract_path = Path(sys.argv[3])
suite = sys.argv[4]
phase_group = sys.argv[5]
build_dir = Path(sys.argv[6])

manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
contract = json.loads(contract_path.read_text(encoding="utf-8"))

def resolve(path: str) -> Path:
    artifact = Path(path)
    if artifact.is_absolute():
        return artifact
    if str(artifact).startswith("safe/"):
        return root / artifact.relative_to("safe")
    return root.parent / artifact

rows = [
    row for row in manifest["rows"]
    if row["suite"] == suite and row["phase_group"] == phase_group
]
if not rows:
    raise SystemExit(f"no tests selected for suite={suite!r} phase_group={phase_group!r}")

selected_names = {row["define_test"] for row in rows}
selected_sources = []
seen_sources = set()
for row in rows:
    source = row["source_file"]
    if source in seen_sources:
        continue
    seen_sources.add(source)
    if source.startswith("original/libarchive-3.7.2/"):
        selected_sources.append(root / "c_src" / Path(source).relative_to("original/libarchive-3.7.2"))
    else:
        selected_sources.append(resolve(source))
generated_list = resolve(contract["generated_headers"]["list_h_by_suite"][suite])

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
(build_dir / "sources.txt").write_text(
    "".join(f"{path}\n" for path in selected_sources),
    encoding="utf-8",
)
(build_dir / "tests.txt").write_text(
    "".join(f"{name}\n" for name in ordered_names),
    encoding="utf-8",
)
PY

printf 'selected tests:\n'
sed 's/^/  /' "$BUILD_DIR/tests.txt"

cargo build --release >/dev/null
"$ROOT/scripts/build-c-frontends.sh" --suite "$SUITE" --build-dir "$BUILD_DIR/frontends"

CONFIG_DIR="$ROOT/generated/original_c_build"
SAFE_INCLUDE_DIR="$ROOT/include"
LIBARCHIVE_DIR="$ROOT/c_src/libarchive"
TEST_UTILS_DIR="$ROOT/c_src/test_utils"
SUITE_DIR="$ROOT/c_src/$SUITE"
SUITE_TEST_DIR="$SUITE_DIR/test"
TEST_BIN="$BUILD_DIR/${SUITE}-${PHASE_GROUP}-tests"
CC_BIN="${CC:-cc}"

read -r -a COMMON_FLAG_ARR <<<"${CPPFLAGS:-} ${CFLAGS:-}"
read -r -a LDFLAG_ARR <<<"${LDFLAGS:-}"
COMMON_FLAG_ARR+=(
  -DHAVE_CONFIG_H=1
  -DLIST_H
  -I"$BUILD_DIR"
  -I"$CONFIG_DIR"
  -I"$SAFE_INCLUDE_DIR"
  -I"$LIBARCHIVE_DIR"
  -I"$TEST_UTILS_DIR"
  -I"$ROOT/c_src/libarchive_fe"
  -I"$SUITE_DIR"
  -I"$SUITE_TEST_DIR"
)

EXTRA_TEST_LIBS=()
if grep -Eq '^#define HAVE_LIBACL 1$' "$CONFIG_DIR/config.h"; then
  EXTRA_TEST_LIBS+=(-lacl)
fi

mapfile -t SELECTED_SOURCES < "$BUILD_DIR/sources.txt"

case "$SUITE" in
  tar)
    FRONTEND_BIN="$BUILD_DIR/frontends/bsdtar"
    SUPPORT_SOURCES=()
    ;;
  cpio)
    FRONTEND_BIN="$BUILD_DIR/frontends/bsdcpio"
    SUPPORT_SOURCES=(
      "$ROOT/c_src/cpio/cmdline.c"
      "$ROOT/c_src/libarchive_fe/err.c"
    )
    ;;
  cat)
    FRONTEND_BIN="$BUILD_DIR/frontends/bsdcat"
    SUPPORT_SOURCES=()
    ;;
  unzip)
    FRONTEND_BIN="$BUILD_DIR/frontends/bsdunzip"
    SUPPORT_SOURCES=(
      "$ROOT/c_src/libarchive_fe/err.c"
    )
    ;;
  *)
    printf 'unsupported suite: %s\n' "$SUITE" >&2
    exit 1
    ;;
esac

"$CC_BIN" \
  "${COMMON_FLAG_ARR[@]}" \
  "$TEST_UTILS_DIR/test_utils.c" \
  "$TEST_UTILS_DIR/test_main.c" \
  "${SUPPORT_SOURCES[@]}" \
  "${SELECTED_SOURCES[@]}" \
  "${EXTRA_TEST_LIBS[@]}" \
  "${LDFLAG_ARR[@]}" \
  -o "$TEST_BIN"

EXPECTED_ARCHIVE="$(readlink -f "$ROOT/target/release/libarchive.so.13")"
RESOLVED_ARCHIVE="$(
  LD_LIBRARY_PATH="$ROOT/target/release${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    ldd "$FRONTEND_BIN" | awk '/libarchive\.so\.13/ {print $3; exit}'
)"
if [[ -z "$RESOLVED_ARCHIVE" ]]; then
  printf 'failed to resolve libarchive.so.13 for %s\n' "$FRONTEND_BIN" >&2
  exit 1
fi
if [[ "$(readlink -f "$RESOLVED_ARCHIVE")" != "$EXPECTED_ARCHIVE" ]]; then
  printf 'frontend %s resolved libarchive to %s, expected %s\n' \
    "$FRONTEND_BIN" \
    "$RESOLVED_ARCHIVE" \
    "$EXPECTED_ARCHIVE" >&2
  exit 1
fi

LD_LIBRARY_PATH="$ROOT/target/release${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  "$TEST_BIN" -p "$FRONTEND_BIN" -r "$SUITE_TEST_DIR" -vv
