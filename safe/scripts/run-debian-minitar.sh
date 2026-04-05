#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd -- "$ROOT/.." && pwd)"
PACKAGE_METADATA="$ROOT/generated/original_package_metadata.json"
ORIGINAL_PC="$ROOT/generated/original_pkgconfig/libarchive.pc"

for path in \
  "$PACKAGE_METADATA" \
  "$ORIGINAL_PC" \
  "$ROOT/debian/tests/minitar" \
  "$ROOT/examples/minitar/minitar.c" \
  "$ROOT/examples/untar.c"
do
  [[ -f "$path" ]] || {
    printf 'missing required Debian minitar input: %s\n' "$path" >&2
    exit 1
  }
done

mkdir -p "$ROOT/target"

readarray -t METADATA_LINES < <(
  python3 - "$PACKAGE_METADATA" <<'PY'
import json
import sys
from pathlib import Path

metadata = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
print(metadata["deb_filenames"]["runtime"])
print(metadata["deb_filenames"]["development"])
print(metadata["deb_filenames"]["tools"])
print(metadata["multiarch_triplet"])
print(metadata["development_pkgconfig_install_path"])
PY
)

RUNTIME_DEB="$REPO_ROOT/${METADATA_LINES[0]}"
DEV_DEB="$REPO_ROOT/${METADATA_LINES[1]}"
TOOLS_DEB="$REPO_ROOT/${METADATA_LINES[2]}"
MULTIARCH_TRIPLET="${METADATA_LINES[3]}"
PKGCONFIG_INSTALL_PATH="${METADATA_LINES[4]}"

for path in "$RUNTIME_DEB" "$DEV_DEB" "$TOOLS_DEB"; do
  [[ -f "$path" ]] || {
    printf 'missing built package artifact: %s\n' "$path" >&2
    exit 1
  }
done

SYSROOT="$(mktemp -d "$ROOT/target/debian-minitar-sysroot.XXXXXX")"
WORKROOT="$(mktemp -d "$ROOT/target/debian-minitar-work.XXXXXX")"
cleanup() {
  rm -rf "$SYSROOT" "$WORKROOT"
}
trap cleanup EXIT

dpkg-deb -x "$RUNTIME_DEB" "$SYSROOT"
dpkg-deb -x "$DEV_DEB" "$SYSROOT"
dpkg-deb -x "$TOOLS_DEB" "$SYSROOT"

STAGED_PC="$SYSROOT$PKGCONFIG_INSTALL_PATH"
[[ -f "$STAGED_PC" ]] || {
  printf 'missing staged pkg-config file: %s\n' "$STAGED_PC" >&2
  exit 1
}

python3 - "$STAGED_PC" "$ORIGINAL_PC" <<'PY'
import sys
from pathlib import Path

generated = Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
original = Path(sys.argv[2]).read_text(encoding="utf-8").splitlines()

def normalize(lines):
    normalized = []
    for line in lines:
        if line.startswith(("prefix=", "exec_prefix=", "libdir=", "includedir=")):
            normalized.append(line.split("=", 1)[0] + "=@PATH@")
        else:
            normalized.append(line)
    return normalized

if normalize(generated) != normalize(original):
    import difflib

    diff = difflib.unified_diff(
        normalize(original),
        normalize(generated),
        fromfile="original(normalized)",
        tofile="staged(normalized)",
        lineterm="",
    )
    print("\n".join(diff), file=sys.stderr)
    raise SystemExit("staged libarchive.pc drifted from the original oracle")
PY

WORKTREE="$WORKROOT/source"
mkdir -p "$WORKTREE/debian/tests"
cp -a "$ROOT/debian/tests/minitar" "$WORKTREE/debian/tests/"
cp -a "$ROOT/examples" "$WORKTREE/"

HOST_PKGCONFIG_DIRS=(
  "/usr/local/lib/$MULTIARCH_TRIPLET/pkgconfig"
  /usr/local/lib/pkgconfig
  /usr/local/share/pkgconfig
  "/usr/lib/$MULTIARCH_TRIPLET/pkgconfig"
  /usr/lib/pkgconfig
  /usr/share/pkgconfig
)
PKG_CONFIG_LIBDIR_VALUE="$SYSROOT/usr/lib/$MULTIARCH_TRIPLET/pkgconfig"
for dir in "${HOST_PKGCONFIG_DIRS[@]}"; do
  PKG_CONFIG_LIBDIR_VALUE="${PKG_CONFIG_LIBDIR_VALUE}:$dir"
done

export PKG_CONFIG_SYSROOT_DIR="$SYSROOT"
export PKG_CONFIG_LIBDIR="$PKG_CONFIG_LIBDIR_VALUE"
unset PKG_CONFIG_PATH
export LD_LIBRARY_PATH="$SYSROOT/usr/lib/$MULTIARCH_TRIPLET:$SYSROOT/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PATH="$SYSROOT/usr/bin:$PATH"
export DEB_HOST_MULTIARCH="$MULTIARCH_TRIPLET"

(
  cd "$WORKTREE"
  sh ./debian/tests/minitar
)
