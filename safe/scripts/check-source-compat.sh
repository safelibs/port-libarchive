#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
CONTRACT="$ROOT/generated/original_build_contract.json"
ORIGINAL_PC="$ROOT/generated/original_pkgconfig/libarchive.pc"
GENERATED_PC="$ROOT/generated/pkgconfig/libarchive.pc"

for path in \
  "$ROOT/include/archive.h" \
  "$ROOT/include/archive_entry.h" \
  "$CONTRACT" \
  "$ORIGINAL_PC"
do
  [[ -f "$path" ]] || {
    printf 'missing required source-compat input: %s\n' "$path" >&2
    exit 1
  }
done

"$ROOT/scripts/render-pkg-config.sh" --mode build-tree

compare_pkgconfig_contract() {
  python3 - "$GENERATED_PC" "$ORIGINAL_PC" <<'PY'
import sys
from pathlib import Path

generated = Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
original = Path(sys.argv[2]).read_text(encoding="utf-8").splitlines()

allowed = {"prefix", "exec_prefix", "libdir", "includedir"}

def normalize(lines):
    normalized = []
    for line in lines:
        for key in allowed:
            if line.startswith(f"{key}="):
                normalized.append(f"{key}=@PATH@")
                break
        else:
            normalized.append(line)
    return normalized

if normalize(generated) != normalize(original):
    import difflib

    diff = difflib.unified_diff(
        normalize(original),
        normalize(generated),
        fromfile="original(normalized)",
        tofile="generated(normalized)",
        lineterm="",
    )
    print("\n".join(diff), file=sys.stderr)
    raise SystemExit("non-path pkg-config contract drifted from the original oracle")
PY
}

compare_pkgconfig_contract

HOST_PKGCONFIG_DIRS=(
  /usr/local/lib/x86_64-linux-gnu/pkgconfig
  /usr/local/lib/pkgconfig
  /usr/local/share/pkgconfig
  /usr/lib/x86_64-linux-gnu/pkgconfig
  /usr/lib/pkgconfig
  /usr/share/pkgconfig
)
PKG_CONFIG_LIBDIR_VALUE="$ROOT/generated/pkgconfig"
for dir in "${HOST_PKGCONFIG_DIRS[@]}"; do
  PKG_CONFIG_LIBDIR_VALUE="${PKG_CONFIG_LIBDIR_VALUE}:$dir"
done
export PKG_CONFIG_LIBDIR="$PKG_CONFIG_LIBDIR_VALUE"
unset PKG_CONFIG_PATH

pkg-config --exists libarchive
pkg-config --libs --static libarchive >/dev/null

cargo build --manifest-path "$ROOT/Cargo.toml" --release >/dev/null

readarray -t EXAMPLE_PATHS < <(
  python3 - "$CONTRACT" <<'PY'
import json
import sys
from pathlib import Path

contract = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
print(contract["link_targets"]["minitar"]["source"])
print(contract["link_targets"]["untar"]["source"])
PY
)

resolve_vendored_source() {
  local recorded_path="$1"
  case "$recorded_path" in
    original/libarchive-3.7.2/*)
      ;;
    *)
      printf 'unsupported non-vendored source path: %s\n' "$recorded_path" >&2
      exit 1
      ;;
  esac

  local vendored_path="$ROOT/${recorded_path#original/libarchive-3.7.2/}"
  [[ -f "$vendored_path" ]] || {
    printf 'missing vendored source: %s\n' "$vendored_path" >&2
    exit 1
  }

  printf '%s\n' "$vendored_path"
}

MINITAR_SRC="$(resolve_vendored_source "${EXAMPLE_PATHS[0]}")"
UNTAR_SRC="$(resolve_vendored_source "${EXAMPLE_PATHS[1]}")"
[[ -f "$MINITAR_SRC" ]] || {
  printf 'missing example source: %s\n' "$MINITAR_SRC" >&2
  exit 1
}
[[ -f "$UNTAR_SRC" ]] || {
  printf 'missing example source: %s\n' "$UNTAR_SRC" >&2
  exit 1
}

BUILD_DIR="$ROOT/target/source-compat"
mkdir -p "$BUILD_DIR"
CC_BIN="${CC:-cc}"
read -r -a CFLAGS_ARR <<<"$(pkg-config --cflags libarchive)"
read -r -a LIBS_ARR <<<"$(pkg-config --libs libarchive)"

"$CC_BIN" "${CFLAGS_ARR[@]}" "$MINITAR_SRC" "${LIBS_ARR[@]}" -o "$BUILD_DIR/minitar"
"$CC_BIN" "${CFLAGS_ARR[@]}" "$UNTAR_SRC" "${LIBS_ARR[@]}" -o "$BUILD_DIR/untar"

WORK_DIR="$(mktemp -d "$BUILD_DIR/work.XXXXXX")"
trap 'rm -rf "$WORK_DIR"' EXIT
printf 'source-compat payload\n' > "$WORK_DIR/sample.txt"

run_example() {
  (
    cd "$WORK_DIR"
    LD_LIBRARY_PATH="$ROOT/target/release${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" "$@"
  )
}

run_example "$BUILD_DIR/minitar" -cf sample.tar sample.txt >/dev/null
LIST_OUTPUT="$(run_example "$BUILD_DIR/minitar" -tf sample.tar)"
grep -Eq '^sample\.txt[[:space:]]*$' <<<"$LIST_OUTPUT" >/dev/null || {
  printf 'minitar did not list the expected member\n' >&2
  exit 1
}

rm -f "$WORK_DIR/sample.txt"
run_example "$BUILD_DIR/untar" -xf sample.tar >/dev/null

[[ -f "$WORK_DIR/sample.txt" ]] || {
  printf 'untar did not recreate sample.txt\n' >&2
  exit 1
}
[[ "$(cat "$WORK_DIR/sample.txt")" == "source-compat payload" ]] || {
  printf 'untar restored unexpected file contents\n' >&2
  exit 1
}
