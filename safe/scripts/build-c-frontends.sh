#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="$ROOT/target/c-frontends"
SUITE="all"
LIB_DIR="$ROOT/target/release"
CONFIG_DIR="$ROOT/generated/original_c_build"
CC_BIN="${CC:-cc}"

usage() {
  cat <<'EOF'
usage: build-c-frontends.sh [--build-dir <path>] [--suite tar|cpio|cat|unzip|all] [--lib-dir <path>]

Build the vendored C frontend binaries against the safe libarchive build tree.
EOF
}

while (($#)); do
  case "$1" in
    --build-dir)
      BUILD_DIR="${2:?missing value for --build-dir}"
      shift 2
      ;;
    --suite)
      SUITE="${2:?missing value for --suite}"
      shift 2
      ;;
    --lib-dir)
      LIB_DIR="${2:?missing value for --lib-dir}"
      shift 2
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

require_file "$CONFIG_DIR/config.h"
require_file "$LIB_DIR/libarchive.so"
require_file "$ROOT/generated/original_build_contract.json"
require_file "$ROOT/generated/original_package_metadata.json"

mkdir -p "$BUILD_DIR"

readarray -t METADATA_LINES < <(
  python3 - "$ROOT/generated/original_build_contract.json" "$ROOT/generated/original_package_metadata.json" <<'PY'
import json
import sys
from pathlib import Path

contract = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
metadata = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
print(" ".join(contract["link_targets"]["examples"]["extra_libraries"]))
print(Path(metadata["runtime_soname_install_path"]).name)
print(Path(metadata["runtime_shared_library_install_path"]).name)
PY
)

EXTRA_LIBS="${METADATA_LINES[0]}"
SONAME_BASENAME="${METADATA_LINES[1]}"
RUNTIME_BASENAME="${METADATA_LINES[2]}"

ln -sf libarchive.so "$LIB_DIR/$SONAME_BASENAME"
ln -sf libarchive.so "$LIB_DIR/$RUNTIME_BASENAME"

read -r -a COMMON_FLAG_ARR <<<"${CPPFLAGS:-} ${CFLAGS:-}"
read -r -a LDFLAG_ARR <<<"${LDFLAGS:-}"
read -r -a EXTRA_LIB_ARR <<<"$EXTRA_LIBS"

COMMON_FLAG_ARR+=(
  -DHAVE_CONFIG_H=1
  -I"$CONFIG_DIR"
  -I"$ROOT/include"
  -I"$ROOT/c_src/libarchive_fe"
)

build_frontend() {
  local suite_name="$1"
  local binary_name="$2"
  shift 2
  local -a sources=("$@")

  "$CC_BIN" \
    "${COMMON_FLAG_ARR[@]}" \
    "${sources[@]}" \
    -L"$LIB_DIR" \
    -larchive \
    "${EXTRA_LIB_ARR[@]}" \
    "${LDFLAG_ARR[@]}" \
    -o "$BUILD_DIR/$binary_name"
}

case "$SUITE" in
  tar|all)
    build_frontend tar bsdtar \
      "$ROOT/c_src/tar/bsdtar.c" \
      "$ROOT/c_src/tar/cmdline.c" \
      "$ROOT/c_src/tar/creation_set.c" \
      "$ROOT/c_src/tar/read.c" \
      "$ROOT/c_src/tar/subst.c" \
      "$ROOT/c_src/tar/util.c" \
      "$ROOT/c_src/tar/write.c" \
      "$ROOT/c_src/libarchive_fe/err.c" \
      "$ROOT/c_src/libarchive_fe/line_reader.c" \
      "$ROOT/c_src/libarchive_fe/passphrase.c"
    ;&
  cpio|all)
    if [[ "$SUITE" == "cpio" || "$SUITE" == "all" ]]; then
      build_frontend cpio bsdcpio \
        "$ROOT/c_src/cpio/cmdline.c" \
        "$ROOT/c_src/cpio/cpio.c" \
        "$ROOT/c_src/libarchive_fe/err.c" \
        "$ROOT/c_src/libarchive_fe/line_reader.c" \
        "$ROOT/c_src/libarchive_fe/passphrase.c"
    fi
    ;&
  cat|all)
    if [[ "$SUITE" == "cat" || "$SUITE" == "all" ]]; then
      build_frontend cat bsdcat \
        "$ROOT/c_src/cat/bsdcat.c" \
        "$ROOT/c_src/cat/cmdline.c" \
        "$ROOT/c_src/libarchive_fe/err.c"
    fi
    ;&
  unzip|all)
    if [[ "$SUITE" == "unzip" || "$SUITE" == "all" ]]; then
      build_frontend unzip bsdunzip \
        "$ROOT/c_src/unzip/bsdunzip.c" \
        "$ROOT/c_src/unzip/cmdline.c" \
        "$ROOT/c_src/unzip/la_getline.c" \
        "$ROOT/c_src/libarchive_fe/err.c" \
        "$ROOT/c_src/libarchive_fe/passphrase.c"
    fi
    ;;
  *)
    printf 'unsupported suite: %s\n' "$SUITE" >&2
    exit 1
    ;;
esac
