#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd -- "$ROOT/.." && pwd)"
ORIGINAL_TREE="$REPO_ROOT/original/libarchive-3.7.2"

ABI_DIR="$ROOT/abi"
GENERATED_DIR="$ROOT/generated"
ORIGINAL_C_BUILD_DIR="$GENERATED_DIR/original_c_build"
ORIGINAL_LINK_OBJECTS_DIR="$GENERATED_DIR/original_link_objects"
ORIGINAL_PKGCONFIG_DIR="$GENERATED_DIR/original_pkgconfig"
ORIGINAL_PKGCONFIG_FILE="$ORIGINAL_PKGCONFIG_DIR/libarchive.pc"
PACKAGE_METADATA_FILE="$GENERATED_DIR/original_package_metadata.json"
BUILD_CONTRACT_FILE="$GENERATED_DIR/original_build_contract.json"

CHECK=0

usage() {
  cat <<'EOF'
usage: build-original-oracle.sh [--check]

Build and capture the phase-1 oracle artifacts from the local vendored libarchive
source package, or validate the checked-in oracle outputs in read-only mode.
EOF
}

while (($#)); do
  case "$1" in
    --check)
      CHECK=1
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

suite_define_count() {
  python3 - "$1" <<'PY'
import re
import sys
from pathlib import Path

text = Path(sys.argv[1]).read_text(encoding="utf-8")
print(len(re.findall(r"DEFINE_TEST\(([^)]+)\)", text)))
PY
}

if ((CHECK)); then
  for path in \
    "$ABI_DIR/original_exported_symbols.txt" \
    "$ABI_DIR/original_version_info.txt" \
    "$PACKAGE_METADATA_FILE" \
    "$ORIGINAL_PKGCONFIG_FILE" \
    "$ORIGINAL_C_BUILD_DIR/config.h" \
    "$ORIGINAL_C_BUILD_DIR/libarchive/test/list.h" \
    "$ORIGINAL_C_BUILD_DIR/tar/test/list.h" \
    "$ORIGINAL_C_BUILD_DIR/cpio/test/list.h" \
    "$ORIGINAL_C_BUILD_DIR/cat/test/list.h" \
    "$ORIGINAL_C_BUILD_DIR/unzip/test/list.h" \
    "$BUILD_CONTRACT_FILE" \
    "$GENERATED_DIR/api_inventory.json" \
    "$GENERATED_DIR/test_manifest.json" \
    "$GENERATED_DIR/link_compat_manifest.json" \
    "$GENERATED_DIR/pkgconfig/libarchive.pc"
  do
    require_file "$path"
  done

  export_count="$(grep -cve '^[[:space:]]*$' "$ABI_DIR/original_exported_symbols.txt")"
  [[ "$export_count" -eq 421 ]] || {
    printf 'expected 421 exported symbols, found %s\n' "$export_count" >&2
    exit 1
  }

  [[ "$(suite_define_count "$ORIGINAL_C_BUILD_DIR/libarchive/test/list.h")" -eq 604 ]]
  [[ "$(suite_define_count "$ORIGINAL_C_BUILD_DIR/tar/test/list.h")" -eq 70 ]]
  [[ "$(suite_define_count "$ORIGINAL_C_BUILD_DIR/cpio/test/list.h")" -eq 48 ]]
  [[ "$(suite_define_count "$ORIGINAL_C_BUILD_DIR/cat/test/list.h")" -eq 18 ]]
  [[ "$(suite_define_count "$ORIGINAL_C_BUILD_DIR/unzip/test/list.h")" -eq 22 ]]

  python3 - "$ROOT" <<'PY'
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])
manifest = json.loads((root / "generated" / "link_compat_manifest.json").read_text(encoding="utf-8"))
allowed_prefixes = (
    "original/libarchive-3.7.2/test_utils/",
    "original/libarchive-3.7.2/libarchive/test/read_open_memory.c",
    "original/libarchive-3.7.2/libarchive/test/test_",
)
for target in manifest["targets"]:
    if target["target_name"] != "libarchive_test":
        continue
    for item in target["ordered_objects"]:
        source_path = item["source_path"]
        if source_path.startswith("original/libarchive-3.7.2/test_utils/"):
            continue
        if source_path == "original/libarchive-3.7.2/libarchive/test/read_open_memory.c":
            continue
        if source_path.startswith("original/libarchive-3.7.2/libarchive/test/test_"):
            continue
        raise SystemExit(f"unexpected preserved libarchive_test source: {source_path}")
PY

  exit 0
fi

export LC_ALL=C.UTF-8
export LANG=C.UTF-8
export DEB_BUILD_OPTIONS=nocheck
export DEB_BUILD_PROFILES=nocheck

tmpdir="$(mktemp -d "${TMPDIR:-/tmp}/libarchive-safe-oracle.XXXXXX")"
trap 'rm -rf "$tmpdir"' EXIT

package_src="$tmpdir/package-src"
package_extract_runtime="$tmpdir/package-runtime"
package_extract_dev="$tmpdir/package-dev"
autotools_src="$tmpdir/autotools-src"
autotools_build="$tmpdir/autotools-build"
example_build="$tmpdir/examples"
consumer_sources_json="$tmpdir/libarchive_consumer_sources.json"
package_metadata_tmp="$tmpdir/original_package_metadata.json"
build_contract_tmp="$tmpdir/original_build_contract.json"
make_vars_dump="$tmpdir/autotools-make-vars.txt"

rm -rf \
  "$ORIGINAL_C_BUILD_DIR" \
  "$ORIGINAL_LINK_OBJECTS_DIR" \
  "$ORIGINAL_PKGCONFIG_DIR"

mkdir -p \
  "$ORIGINAL_C_BUILD_DIR/libarchive/test" \
  "$ORIGINAL_C_BUILD_DIR/tar/test" \
  "$ORIGINAL_C_BUILD_DIR/cpio/test" \
  "$ORIGINAL_C_BUILD_DIR/cat/test" \
  "$ORIGINAL_C_BUILD_DIR/unzip/test" \
  "$ORIGINAL_LINK_OBJECTS_DIR/libarchive_test/test_utils" \
  "$ORIGINAL_LINK_OBJECTS_DIR/libarchive_test/libarchive/test" \
  "$ORIGINAL_LINK_OBJECTS_DIR/examples" \
  "$ORIGINAL_PKGCONFIG_DIR" \
  "$example_build"

cp -a "$ORIGINAL_TREE" "$package_src"
(
  cd "$package_src"
  dpkg-buildpackage -us -uc -b -Pnocheck
)

multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
runtime_deb="$(find "$tmpdir" -maxdepth 1 -type f -name 'libarchive13t64_*.deb' | sort | head -n 1)"
dev_deb="$(find "$tmpdir" -maxdepth 1 -type f -name 'libarchive-dev_*.deb' | sort | head -n 1)"
tools_deb="$(find "$tmpdir" -maxdepth 1 -type f -name 'libarchive-tools_*.deb' | sort | head -n 1)"

[[ -n "$runtime_deb" && -n "$dev_deb" && -n "$tools_deb" ]] || {
  echo "failed to locate built Debian packages" >&2
  exit 1
}

mkdir -p "$package_extract_runtime" "$package_extract_dev"
dpkg-deb -x "$runtime_deb" "$package_extract_runtime"
dpkg-deb -x "$dev_deb" "$package_extract_dev"

runtime_shared_object="$package_extract_runtime/usr/lib/$multiarch/libarchive.so.13.7.2"
runtime_install_path="/usr/lib/$multiarch/libarchive.so.13.7.2"
runtime_soname_path="/usr/lib/$multiarch/libarchive.so.13"
pkgconfig_install_path="/usr/lib/$multiarch/pkgconfig/libarchive.pc"
dev_pkgconfig_file="$package_extract_dev$pkgconfig_install_path"

require_file "$runtime_shared_object"
require_file "$dev_pkgconfig_file"

readelf --dyn-syms --wide "$runtime_shared_object" \
  | awk '
      /^[[:space:]]*[0-9]+:/ {
        name=$8
        bind=$5
        vis=$6
        ndx=$7
        if (ndx != "UND" && (bind == "GLOBAL" || bind == "WEAK") && vis == "DEFAULT" && name != "") {
          sub(/@.*/, "", name)
          print name
        }
      }
    ' \
  | sort -u > "$ABI_DIR/original_exported_symbols.txt"

readelf --version-info --wide "$runtime_shared_object" > "$ABI_DIR/original_version_info.txt"
cp "$dev_pkgconfig_file" "$ORIGINAL_PKGCONFIG_FILE"

python3 - "$package_metadata_tmp" "$multiarch" "$runtime_install_path" "$runtime_soname_path" "$pkgconfig_install_path" "$runtime_deb" "$dev_deb" "$tools_deb" <<'PY'
import json
import sys
from pathlib import Path

output = Path(sys.argv[1])
multiarch = sys.argv[2]
runtime_install_path = sys.argv[3]
runtime_soname_path = sys.argv[4]
pkgconfig_install_path = sys.argv[5]
runtime_deb = Path(sys.argv[6]).name
dev_deb = Path(sys.argv[7]).name
tools_deb = Path(sys.argv[8]).name

full_version = runtime_deb.split("_", 2)[1]
package_version, debian_revision = full_version.split("-", 1)

output.write_text(
    json.dumps(
        {
            "schema_version": 1,
            "package_version": package_version,
            "debian_revision": debian_revision,
            "full_version": full_version,
            "multiarch_triplet": multiarch,
            "runtime_shared_library_install_path": runtime_install_path,
            "runtime_soname_install_path": runtime_soname_path,
            "development_pkgconfig_install_path": pkgconfig_install_path,
            "deb_filenames": {
                "runtime": runtime_deb,
                "development": dev_deb,
                "tools": tools_deb,
            },
        },
        indent=2,
        sort_keys=True,
    )
    + "\n",
    encoding="utf-8",
)
PY
mv "$package_metadata_tmp" "$PACKAGE_METADATA_FILE"

cp -a "$ORIGINAL_TREE" "$autotools_src"
mkdir -p "$autotools_build"
(
  cd "$autotools_src"
  ./build/autogen.sh
)
(
  cd "$autotools_build"
  ../autotools-src/configure \
    --without-openssl \
    --with-nettle \
    --enable-bsdtar=shared \
    --enable-bsdcpio=shared \
    --enable-bsdcat=shared \
    --enable-bsdunzip=shared
  make libarchive/test/list.h tar/test/list.h cpio/test/list.h cat/test/list.h unzip/test/list.h
)

cp "$autotools_build/config.h" "$ORIGINAL_C_BUILD_DIR/config.h"
cp "$autotools_build/libarchive/test/list.h" "$ORIGINAL_C_BUILD_DIR/libarchive/test/list.h"
cp "$autotools_build/tar/test/list.h" "$ORIGINAL_C_BUILD_DIR/tar/test/list.h"
cp "$autotools_build/cpio/test/list.h" "$ORIGINAL_C_BUILD_DIR/cpio/test/list.h"
cp "$autotools_build/cat/test/list.h" "$ORIGINAL_C_BUILD_DIR/cat/test/list.h"
cp "$autotools_build/unzip/test/list.h" "$ORIGINAL_C_BUILD_DIR/unzip/test/list.h"

python3 - "$ORIGINAL_TREE/Makefile.am" "$ORIGINAL_TREE/libarchive/test/CMakeLists.txt" "$consumer_sources_json" <<'PY'
from __future__ import annotations

import json
import posixpath
import re
import sys
from pathlib import Path


makefile_am = Path(sys.argv[1]).read_text(encoding="utf-8")
cmake_lists = Path(sys.argv[2]).read_text(encoding="utf-8")
output = Path(sys.argv[3])


def parse_make_variable(text: str, variable: str) -> list[str]:
    tokens = []
    lines = text.splitlines()
    index = 0
    seen = False
    while index < len(lines):
        raw_line = lines[index]
        stripped = raw_line.split("#", 1)[0].rstrip()
        if not stripped:
            index += 1
            continue
        match = re.match(rf"^{re.escape(variable)}(\+)?=\s*(.*)$", stripped)
        if not match:
            index += 1
            continue
        seen = True
        remainder = match.group(2)
        while True:
            continued = remainder.endswith("\\")
            if continued:
                remainder = remainder[:-1].rstrip()
            if remainder:
                tokens.extend(remainder.split())
            if not continued:
                break
            index += 1
            if index >= len(lines):
                break
            remainder = lines[index].split("#", 1)[0].rstrip()
        index += 1
    if not seen:
        raise SystemExit(f"failed to locate {variable} in Makefile.am")
    return tokens


def parse_cmake_sources(text: str) -> list[str]:
    match = re.search(
        r"SET\(libarchive_test_SOURCES(?P<body>.*?)\n\s*\)",
        text,
        re.S,
    )
    if not match:
        raise SystemExit("failed to locate libarchive_test_SOURCES in CMakeLists.txt")
    tokens = []
    for raw_line in match.group("body").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        tokens.append(line)
    return tokens


def normalize_from_cmake(token: str) -> str | None:
    if token == "test.h" or not token.endswith(".c"):
        return None
    return posixpath.normpath(posixpath.join("libarchive/test", token))


def normalize_make_source(token: str) -> str | None:
    if token == "test.h" or not token.endswith(".c"):
        return None
    return posixpath.normpath(token)


libarchive_la_sources = {
    token
    for token in (normalize_make_source(token) for token in parse_make_variable(makefile_am, "libarchive_la_SOURCES"))
    if token
}
test_utils_sources = [
    token
    for token in (normalize_make_source(token) for token in parse_make_variable(makefile_am, "test_utils_SOURCES"))
    if token
]

make_tokens = parse_make_variable(makefile_am, "libarchive_test_SOURCES")
expanded_make_sources = []
for token in make_tokens:
    if token == "$(libarchive_la_SOURCES)":
        expanded_make_sources.extend(sorted(libarchive_la_sources))
    elif token == "$(test_utils_SOURCES)":
        expanded_make_sources.extend(test_utils_sources)
    else:
        normalized = normalize_make_source(token)
        if normalized:
            expanded_make_sources.append(normalized)

make_consumer_sources = [
    token
    for token in expanded_make_sources
    if token not in libarchive_la_sources
]

cmake_consumer_sources = []
for token in parse_cmake_sources(cmake_lists):
    normalized = normalize_from_cmake(token)
    if normalized:
        cmake_consumer_sources.append(normalized)

expected_prefixes = (
    "test_utils/test_utils.c",
    "test_utils/test_main.c",
    "libarchive/test/read_open_memory.c",
)
for expected in expected_prefixes:
    if expected not in make_consumer_sources or expected not in cmake_consumer_sources:
        raise SystemExit(f"expected consumer source missing from libarchive_test set: {expected}")

if set(make_consumer_sources) != set(cmake_consumer_sources):
    raise SystemExit(
        "libarchive_test consumer-source set mismatch between Makefile.am and CMakeLists.txt"
    )

for source in make_consumer_sources:
    if source.startswith("libarchive/") and source not in (
        "libarchive/test/read_open_memory.c",
    ) and not source.startswith("libarchive/test/test_"):
        raise SystemExit(f"disallowed libarchive implementation source in consumer set: {source}")

output.write_text(
    json.dumps(make_consumer_sources, indent=2) + "\n",
    encoding="utf-8",
)
PY

(
  cd "$autotools_build"
  make V=1 libarchive_test
)

python3 - "$consumer_sources_json" "$autotools_build" "$ORIGINAL_LINK_OBJECTS_DIR/libarchive_test" <<'PY'
import json
import shutil
import sys
from pathlib import Path

consumer_sources = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
build_root = Path(sys.argv[2])
destination_root = Path(sys.argv[3])

for source in consumer_sources:
    rel = Path(source)
    stem = rel.stem
    if source.startswith("test_utils/"):
        object_rel = Path("test_utils") / f"libarchive_test-{stem}.o"
    elif source.startswith("libarchive/test/"):
        object_rel = Path("libarchive/test") / f"test-{stem}.o"
    else:
        raise SystemExit(f"unexpected consumer source: {source}")

    source_object = build_root / object_rel
    if not source_object.exists():
        raise SystemExit(f"missing built consumer object: {source_object}")

    destination = destination_root / object_rel
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source_object, destination)
PY

make -pn -C "$autotools_build" libarchive_test > "$make_vars_dump"
libarchive_test_libs="$(awk '/^LIBS =/{print substr($0, index($0, "=") + 1)}' "$make_vars_dump")"
[[ -n "$libarchive_test_libs" ]] || {
  echo "failed to derive LIBS from autotools make dump" >&2
  exit 1
}
pkg_static_libs="$(PKG_CONFIG_PATH="$ORIGINAL_PKGCONFIG_DIR" pkg-config --libs --static libarchive)"

PKG_CONFIG_PATH="$ORIGINAL_PKGCONFIG_DIR" \
PKG_CONFIG_SYSROOT_DIR="$package_extract_dev" \
gcc $(PKG_CONFIG_PATH="$ORIGINAL_PKGCONFIG_DIR" PKG_CONFIG_SYSROOT_DIR="$package_extract_dev" pkg-config --cflags libarchive) \
  -c "$ORIGINAL_TREE/examples/minitar/minitar.c" \
  -o "$example_build/minitar.o"

PKG_CONFIG_PATH="$ORIGINAL_PKGCONFIG_DIR" \
PKG_CONFIG_SYSROOT_DIR="$package_extract_dev" \
gcc $(PKG_CONFIG_PATH="$ORIGINAL_PKGCONFIG_DIR" PKG_CONFIG_SYSROOT_DIR="$package_extract_dev" pkg-config --cflags libarchive) \
  -c "$ORIGINAL_TREE/examples/untar.c" \
  -o "$example_build/untar.o"

cp "$example_build/minitar.o" "$ORIGINAL_LINK_OBJECTS_DIR/examples/minitar.o"
cp "$example_build/untar.o" "$ORIGINAL_LINK_OBJECTS_DIR/examples/untar.o"

python3 - "$BUILD_CONTRACT_FILE" "$consumer_sources_json" "$libarchive_test_libs" "$pkg_static_libs" <<'PY'
from __future__ import annotations

import json
import shlex
import sys
from pathlib import Path


output = Path(sys.argv[1])
consumer_sources = [
    f"original/libarchive-3.7.2/{path}"
    for path in json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
]
libarchive_test_libs = [token for token in shlex.split(sys.argv[3]) if token]
pkg_static_libs = [token for token in shlex.split(sys.argv[4]) if token and token != "-larchive"]


def unique(tokens: list[str]) -> list[str]:
    seen = set()
    ordered = []
    for token in tokens:
        if token not in seen:
            seen.add(token)
            ordered.append(token)
    return ordered


config = {
    "schema_version": 1,
    "config_header": "safe/generated/original_c_build/config.h",
    "generated_headers": {
        "config_h": "safe/generated/original_c_build/config.h",
        "list_h_by_suite": {
            "libarchive": "safe/generated/original_c_build/libarchive/test/list.h",
            "tar": "safe/generated/original_c_build/tar/test/list.h",
            "cpio": "safe/generated/original_c_build/cpio/test/list.h",
            "cat": "safe/generated/original_c_build/cat/test/list.h",
            "unzip": "safe/generated/original_c_build/unzip/test/list.h",
        },
    },
    "suites": {
        "libarchive": {
            "generated_list_h": "safe/generated/original_c_build/libarchive/test/list.h",
            "include_roots": [
                "safe/include",
                "safe/generated/original_c_build",
                "safe/generated/original_c_build/libarchive/test",
                "original/libarchive-3.7.2/libarchive",
                "original/libarchive-3.7.2/test_utils",
                "original/libarchive-3.7.2/libarchive/test",
            ],
            "defines": ["HAVE_CONFIG_H", "LIBARCHIVE_STATIC", "LIST_H"],
        },
        "tar": {
            "generated_list_h": "safe/generated/original_c_build/tar/test/list.h",
            "include_roots": [
                "safe/include",
                "safe/generated/original_c_build",
                "safe/generated/original_c_build/tar/test",
                "original/libarchive-3.7.2/libarchive",
                "original/libarchive-3.7.2/libarchive_fe",
                "original/libarchive-3.7.2/test_utils",
                "original/libarchive-3.7.2/tar",
                "original/libarchive-3.7.2/tar/test",
            ],
            "defines": ["HAVE_CONFIG_H", "LIST_H"],
        },
        "cpio": {
            "generated_list_h": "safe/generated/original_c_build/cpio/test/list.h",
            "include_roots": [
                "safe/include",
                "safe/generated/original_c_build",
                "safe/generated/original_c_build/cpio/test",
                "original/libarchive-3.7.2/libarchive",
                "original/libarchive-3.7.2/libarchive_fe",
                "original/libarchive-3.7.2/test_utils",
                "original/libarchive-3.7.2/cpio",
                "original/libarchive-3.7.2/cpio/test",
            ],
            "defines": ["HAVE_CONFIG_H", "LIST_H"],
        },
        "cat": {
            "generated_list_h": "safe/generated/original_c_build/cat/test/list.h",
            "include_roots": [
                "safe/include",
                "safe/generated/original_c_build",
                "safe/generated/original_c_build/cat/test",
                "original/libarchive-3.7.2/libarchive",
                "original/libarchive-3.7.2/libarchive_fe",
                "original/libarchive-3.7.2/test_utils",
                "original/libarchive-3.7.2/cat",
                "original/libarchive-3.7.2/cat/test",
            ],
            "defines": ["HAVE_CONFIG_H", "LIST_H"],
        },
        "unzip": {
            "generated_list_h": "safe/generated/original_c_build/unzip/test/list.h",
            "include_roots": [
                "safe/include",
                "safe/generated/original_c_build",
                "safe/generated/original_c_build/unzip/test",
                "original/libarchive-3.7.2/libarchive",
                "original/libarchive-3.7.2/libarchive_fe",
                "original/libarchive-3.7.2/test_utils",
                "original/libarchive-3.7.2/unzip",
                "original/libarchive-3.7.2/unzip/test",
            ],
            "defines": ["HAVE_CONFIG_H", "LIST_H"],
        },
    },
    "link_targets": {
        "libarchive_test": {
            "consumer_sources": consumer_sources,
            "extra_libraries": unique(libarchive_test_libs),
        },
        "examples": {
            "extra_libraries": unique(pkg_static_libs),
        },
        "minitar": {
            "source": "original/libarchive-3.7.2/examples/minitar/minitar.c",
            "extra_libraries": unique(pkg_static_libs),
        },
        "untar": {
            "source": "original/libarchive-3.7.2/examples/untar.c",
            "extra_libraries": unique(pkg_static_libs),
        },
    },
}

output.write_text(json.dumps(config, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY

python3 "$ROOT/tools/gen_test_manifest.py" --write-phase-groups
python3 "$ROOT/tools/gen_test_manifest.py"
python3 "$ROOT/tools/gen_api_inventory.py"
"$ROOT/scripts/render-pkg-config.sh" --mode build-tree
python3 "$ROOT/tools/gen_link_compat_manifest.py"
