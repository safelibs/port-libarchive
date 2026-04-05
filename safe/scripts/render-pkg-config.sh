#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
TEMPLATE="$ROOT/pkgconfig/libarchive.pc.in"
ORIGINAL_PC="$ROOT/generated/original_pkgconfig/libarchive.pc"
OUTPUT="$ROOT/generated/pkgconfig/libarchive.pc"

MODE="build-tree"
CHECK=0
PREFIX=""
RENDERED_OUTPUT="$OUTPUT"

usage() {
  cat <<'EOF'
usage: render-pkg-config.sh [--mode build-tree|staged-sysroot] [--prefix <path>] [--output <path>] [--check]

Render safe/generated/pkgconfig/libarchive.pc from the checked-in template and
the captured original pkg-config oracle.
EOF
}

while (($#)); do
  case "$1" in
    --mode)
      MODE="${2:?missing value for --mode}"
      shift 2
      ;;
    --prefix)
      PREFIX="${2:?missing value for --prefix}"
      shift 2
      ;;
    --output)
      RENDERED_OUTPUT="${2:?missing value for --output}"
      shift 2
      ;;
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

[[ -f "$TEMPLATE" ]] || {
  printf 'missing template: %s\n' "$TEMPLATE" >&2
  exit 1
}
[[ -f "$ORIGINAL_PC" ]] || {
  printf 'missing original pkg-config oracle: %s\n' "$ORIGINAL_PC" >&2
  exit 1
}

if [[ "$MODE" == "build-tree" ]]; then
  PREFIX="${PREFIX:-$ROOT}"
elif [[ "$MODE" == "staged-sysroot" ]]; then
  [[ -n "$PREFIX" ]] || {
    echo "--prefix is required for --mode staged-sysroot" >&2
    exit 1
  }
else
  printf 'unsupported mode: %s\n' "$MODE" >&2
  exit 1
fi

TMP_RENDER="$(mktemp)"
trap 'rm -f "$TMP_RENDER"' EXIT

python3 - "$TEMPLATE" "$ORIGINAL_PC" "$TMP_RENDER" "$MODE" "$PREFIX" <<'PY'
import sys
from pathlib import Path

template_path = Path(sys.argv[1])
original_pc_path = Path(sys.argv[2])
output_path = Path(sys.argv[3])
mode = sys.argv[4]
prefix = Path(sys.argv[5]).resolve()

template = template_path.read_text(encoding="utf-8")
original_lines = original_pc_path.read_text(encoding="utf-8").splitlines()

fields: dict[str, str] = {}
for line in original_lines:
    if not line.strip():
        continue
    if "=" in line and ":" not in line.split("=", 1)[0]:
        key, value = line.split("=", 1)
        fields[key.strip()] = value
    elif ":" in line:
        key, value = line.split(":", 1)
        if value.startswith(" "):
            value = value[1:]
        fields[key.strip()] = value
    else:
        continue

if mode == "build-tree":
    replacements = {
        "@prefix@": str(prefix),
        "@exec_prefix@": "${prefix}",
        "@libdir@": "${exec_prefix}/target/release",
        "@includedir@": "${prefix}/include",
    }
else:
    replacements = {
        "@prefix@": str(prefix),
        "@exec_prefix@": fields.get("exec_prefix", "${prefix}"),
        "@libdir@": fields.get("libdir", "${exec_prefix}/lib"),
        "@includedir@": fields.get("includedir", "${prefix}/include"),
    }

rendered = template
rendered = rendered.replace("@VERSION@", fields["Version"])
rendered = rendered.replace("@LIBS@", fields.get("Libs.private", ""))
rendered = rendered.replace("@LIBSREQUIRED@", fields.get("Requires.private", ""))
for needle, replacement in replacements.items():
    rendered = rendered.replace(needle, replacement)

normalized_lines = []
for line in rendered.splitlines():
    if line.startswith("Name:"):
        normalized_lines.append(f"Name: {fields['Name']}")
    elif line.startswith("Description:"):
        normalized_lines.append(f"Description: {fields['Description']}")
    elif line.startswith("Version:"):
        normalized_lines.append(f"Version: {fields['Version']}")
    elif line.startswith("Cflags:"):
        normalized_lines.append(f"Cflags: {fields['Cflags']}")
    elif line.startswith("Cflags.private:"):
        normalized_lines.append(f"Cflags.private: {fields['Cflags.private']}")
    elif line.startswith("Libs:"):
        normalized_lines.append(f"Libs: {fields['Libs']}")
    elif line.startswith("Libs.private:"):
        normalized_lines.append(f"Libs.private: {fields.get('Libs.private', '')}")
    elif line.startswith("Requires.private:"):
        normalized_lines.append(f"Requires.private: {fields.get('Requires.private', '')}")
    else:
        normalized_lines.append(line)

output_path.write_text("\n".join(normalized_lines) + "\n", encoding="utf-8")
PY

normalize_contract() {
  python3 - "$1" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
for line in path.read_text(encoding="utf-8").splitlines():
    if line.startswith(("prefix=", "exec_prefix=", "libdir=", "includedir=")):
        continue
    print(line.rstrip())
PY
}

if ((CHECK)); then
  [[ -f "$RENDERED_OUTPUT" ]] || {
    printf 'rendered pkg-config file is missing: %s\n' "$RENDERED_OUTPUT" >&2
    exit 1
  }
  cmp -s "$TMP_RENDER" "$RENDERED_OUTPUT" || {
    echo "rendered pkg-config file is out of date" >&2
    diff -u "$RENDERED_OUTPUT" "$TMP_RENDER" || true
    exit 1
  }
  diff -u <(normalize_contract "$TMP_RENDER") <(normalize_contract "$ORIGINAL_PC") >/dev/null || {
    echo "non-path pkg-config contract drifted from the original oracle" >&2
    exit 1
  }
else
  mkdir -p "$(dirname -- "$RENDERED_OUTPUT")"
  mv "$TMP_RENDER" "$RENDERED_OUTPUT"
  trap - EXIT
fi
