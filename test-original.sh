#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
IMAGE_TAG="${LIBARCHIVE_ORIGINAL_TEST_IMAGE:-libarchive-original-test:ubuntu24.04}"
ONLY=""
TARGET="safe"

usage() {
  cat <<'EOF'
usage: test-original.sh [--target safe|original] [--only <binary-package>]

Builds the selected local libarchive source package inside Docker, installs the
resulting local .debs into the container, and then smoke-tests the
libarchive-dependent packages recorded in dependents.json.

--target chooses which source package to build. `safe` is the default.
--only runs just one dependent by exact .dependents[].binary_package.
EOF
}

while (($#)); do
  case "$1" in
    --target)
      TARGET="${2:?missing value for --target}"
      shift 2
      ;;
    --only)
      ONLY="${2:?missing value for --only}"
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

case "$TARGET" in
  safe|original)
    ;;
  *)
    printf 'unsupported target: %s\n' "$TARGET" >&2
    usage >&2
    exit 1
    ;;
esac

for tool in docker jq; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'missing required host tool: %s\n' "$tool" >&2
    exit 1
  }
done

if [[ "$TARGET" == "safe" ]]; then
  [[ -d "$ROOT/safe" ]] || {
    echo "missing safe" >&2
    exit 1
  }
else
  [[ -d "$ROOT/original/libarchive-3.7.2" ]] || {
    echo "missing original/libarchive-3.7.2" >&2
    exit 1
  }
fi

[[ -f "$ROOT/dependents.json" ]] || {
  echo "missing dependents.json" >&2
  exit 1
}

[[ -e /dev/fuse ]] || {
  echo "/dev/fuse is required to exercise archivemount inside Docker" >&2
  exit 1
}

expected_packages=(
  "file-roller"
  "ark"
  "archivemount"
  "zathura-cb"
  "kodi-vfs-libarchive"
  "gnome-epub-thumbnailer"
  "libgepub-0.7-0"
  "libgnome-autoar-0-0"
  "fwupd"
  "pacman-package-manager"
  "libextractor-plugin-archive"
  "python3-libarchive-c"
)

expected_sorted="$(printf '%s\n' "${expected_packages[@]}" | sort)"
actual_sorted="$(jq -r '.dependents[].binary_package' "$ROOT/dependents.json" | sort)"

if [[ "$actual_sorted" != "$expected_sorted" ]]; then
  echo "dependents.json does not match the expected binary package set" >&2
  diff -u <(printf '%s\n' "${expected_packages[@]}" | sort) <(jq -r '.dependents[].binary_package' "$ROOT/dependents.json" | sort) || true
  exit 1
fi

if [[ -n "$ONLY" ]]; then
  jq -e --arg pkg "$ONLY" '.dependents[] | select(.binary_package == $pkg)' "$ROOT/dependents.json" >/dev/null || {
    printf 'unknown dependent in dependents.json: %s\n' "$ONLY" >&2
    exit 1
  }
fi

docker build -t "$IMAGE_TAG" - <<'DOCKERFILE'
FROM ubuntu:24.04

ARG DEBIAN_FRONTEND=noninteractive

RUN sed -i 's/^Types: deb$/Types: deb deb-src/' /etc/apt/sources.list.d/ubuntu.sources \
 && apt-get update \
 && apt-get install -y --no-install-recommends \
      ark \
      archivemount \
      binutils \
      build-essential \
      ca-certificates \
      cargo \
      dbus-x11 \
      dpkg-dev \
      extract \
      fakeroot \
      file \
      file-roller \
      fuse \
      fwupd \
      gir1.2-gepub-0.7 \
      gir1.2-gnomeautoar-0.1 \
      gnome-epub-thumbnailer \
      jq \
      lld \
      kodi \
      kodi-vfs-libarchive \
      libarchive-tools \
      libextractor-plugin-archive \
      pacman-package-manager \
      python3 \
      python3-gi \
      python3-libarchive-c \
      python3-pil \
      rustc \
      xauth \
      xdotool \
      xvfb \
      zathura \
      zathura-cb \
 && apt-get build-dep -y --no-install-recommends libarchive \
 && rm -rf /var/lib/apt/lists/*
DOCKERFILE

docker run --rm -i \
  --device /dev/fuse \
  --cap-add SYS_ADMIN \
  --security-opt apparmor:unconfined \
  -e "LIBARCHIVE_BUILD_TARGET=$TARGET" \
  -e "LIBARCHIVE_TEST_ONLY=$ONLY" \
  -v "$ROOT:/work:ro" \
  "$IMAGE_TAG" \
  bash -s <<'CONTAINER'
set -euo pipefail

export LANG=C.UTF-8
export LC_ALL=C.UTF-8
export DEBIAN_FRONTEND=noninteractive
export XDG_RUNTIME_DIR=/tmp/runtime-root

ROOT=/work
BUILD_TARGET="${LIBARCHIVE_BUILD_TARGET:-safe}"
ONLY_FILTER="${LIBARCHIVE_TEST_ONLY:-}"
TEST_ROOT=/tmp/libarchive-dependent-tests
BUILD_ROOT=/tmp/libarchive-local
RUNTIME_EXTRACT_ROOT=/tmp/libarchive-runtime-pkg
ACTIVE_LIBARCHIVE=""
LIBARCHIVE_RUNTIME_DEB=""
LIBARCHIVE_DEV_DEB=""
LIBARCHIVE_TOOLS_DEB=""
LIBARCHIVE_MULTIARCH=""

declare -a EXPECTED_PACKAGES=(
  "file-roller"
  "ark"
  "archivemount"
  "zathura-cb"
  "kodi-vfs-libarchive"
  "gnome-epub-thumbnailer"
  "libgepub-0.7-0"
  "libgnome-autoar-0-0"
  "fwupd"
  "pacman-package-manager"
  "libextractor-plugin-archive"
  "python3-libarchive-c"
)

log_step() {
  printf '\n==> %s\n' "$1"
}

die() {
  echo "error: $*" >&2
  exit 1
}

require_nonempty_file() {
  local path="$1"

  [[ -s "$path" ]] || die "expected non-empty file: $path"
}

require_contains() {
  local path="$1"
  local needle="$2"

  if ! grep -F -- "$needle" "$path" >/dev/null 2>&1; then
    printf 'missing expected text in %s: %s\n' "$path" "$needle" >&2
    printf -- '--- %s ---\n' "$path" >&2
    cat "$path" >&2
    exit 1
  fi
}

assert_status_in() {
  local actual="$1"
  shift
  local allowed

  for allowed in "$@"; do
    if [[ "$actual" == "$allowed" ]]; then
      return 0
    fi
  done

  die "unexpected exit status: $actual (expected one of: $*)"
}

should_run() {
  local package="$1"

  [[ -z "$ONLY_FILTER" || "$package" == "$ONLY_FILTER" ]]
}

reset_test_dir() {
  local name="$1"
  local dir="$TEST_ROOT/$name"

  rm -rf "$dir"
  mkdir -p "$dir"
  printf '%s\n' "$dir"
}

assert_links_to_active_libarchive() {
  local target="$1"
  local resolved

  resolved="$(ldd "$target" | awk '$1 == "libarchive.so.13" { print $3; exit }')"
  [[ -n "$resolved" ]] || die "ldd did not report libarchive.so.13 for $target"
  resolved="$(readlink -f "$resolved")"
  [[ "$resolved" == "$ACTIVE_LIBARCHIVE" ]] || {
    printf 'expected %s to resolve libarchive.so.13 from %s, got %s\n' "$target" "$ACTIVE_LIBARCHIVE" "$resolved" >&2
    ldd "$target" >&2
    exit 1
  }
}

make_png() {
  local path="$1"
  local red="$2"
  local green="$3"
  local blue="$4"

  python3 - "$path" "$red" "$green" "$blue" <<'PY'
import sys
from PIL import Image

path, r, g, b = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4])
Image.new("RGB", (64, 64), color=(r, g, b)).save(path)
PY
}

make_cbz() {
  local archive_path="$1"
  local image_path="$2"

  python3 - "$archive_path" "$image_path" <<'PY'
import sys
import zipfile

archive_path, image_path = sys.argv[1:3]
with zipfile.ZipFile(archive_path, "w") as zf:
    zf.write(image_path, "page1.png")
PY
}

make_epub() {
  local epub_path="$1"
  local cover_path="$2"

  python3 - "$epub_path" "$cover_path" <<'PY'
import sys
import zipfile

epub_path, cover_path = sys.argv[1:3]

files = {
    "mimetype": b"application/epub+zip",
    "META-INF/container.xml": (
        b'<?xml version="1.0"?>\n'
        b'<container version="1.0" '
        b'xmlns="urn:oasis:names:tc:opendocument:xmlns:container">'
        b"<rootfiles>"
        b'<rootfile full-path="OEBPS/content.opf" '
        b'media-type="application/oebps-package+xml"/>'
        b"</rootfiles></container>\n"
    ),
    "OEBPS/content.opf": (
        b'<?xml version="1.0" encoding="UTF-8"?>\n'
        b'<package xmlns="http://www.idpf.org/2007/opf" version="2.0" '
        b'unique-identifier="BookId">'
        b'<metadata xmlns:dc="http://purl.org/dc/elements/1.1/">'
        b"<dc:title>Test EPUB</dc:title>"
        b'<dc:identifier id="BookId">urn:uuid:test-epub</dc:identifier>'
        b'<meta name="cover" content="cover-image"/>'
        b"</metadata>"
        b"<manifest>"
        b'<item id="cover" href="cover.xhtml" media-type="application/xhtml+xml"/>'
        b'<item id="cover-image" href="cover.png" media-type="image/png"/>'
        b"</manifest>"
        b"<spine><itemref idref=\"cover\"/></spine>"
        b"</package>\n"
    ),
    "OEBPS/cover.xhtml": (
        b'<?xml version="1.0" encoding="UTF-8"?>\n'
        b'<html xmlns="http://www.w3.org/1999/xhtml">'
        b"<head><title>Cover</title></head>"
        b"<body><p>Hello EPUB</p><img src=\"cover.png\" alt=\"cover\"/></body>"
        b"</html>\n"
    ),
}

with zipfile.ZipFile(epub_path, "w") as zf:
    zf.writestr("mimetype", files["mimetype"], compress_type=zipfile.ZIP_STORED)
    for name, data in files.items():
        if name == "mimetype":
            continue
        zf.writestr(name, data, compress_type=zipfile.ZIP_DEFLATED)
    zf.write(cover_path, "OEBPS/cover.png", compress_type=zipfile.ZIP_DEFLATED)
PY
}

make_fwupd_cabinet() {
  local dir="$1"
  local archive_path="$2"

  cat >"$dir/firmware.metainfo.xml" <<'XML'
<?xml version="1.0" encoding="UTF-8"?>
<component type="firmware">
  <id>org.example.guid12345678</id>
  <name>Example Test Firmware</name>
  <summary>Firmware for a test device</summary>
  <description>
    <p>Updates the test device firmware.</p>
  </description>
  <provides>
    <firmware type="flashed">12345678-1234-1234-1234-123456789012</firmware>
  </provides>
  <url type="homepage">https://example.invalid/</url>
  <metadata_license>CC0-1.0</metadata_license>
  <project_license>proprietary</project_license>
  <developer_name>ExampleVendor</developer_name>
  <releases>
    <release version="1.2.3" date="2026-04-05">
      <description>
        <p>Initial test release.</p>
      </description>
    </release>
  </releases>
  <custom>
    <value key="LVFS::VersionFormat">triplet</value>
    <value key="LVFS::UpdateProtocol">org.example.test</value>
  </custom>
</component>
XML

  printf 'firmware-bytes' >"$dir/firmware.bin"
  fwupdtool build-cabinet "$archive_path" "$dir/firmware.bin" "$dir/firmware.metainfo.xml" >/dev/null
}

make_pacman_package() {
  local dir="$1"
  local archive_path="$2"
  local tar_path

  mkdir -p "$dir/pkg/usr/share/demo"
  tar_path="${archive_path%.gz}"

  cat >"$dir/pkg/.PKGINFO" <<'PKG'
pkgname = demo-pkg
pkgbase = demo-pkg
pkgver = 1.0-1
pkgdesc = Demo package
url = https://example.invalid/
builddate = 1712280000
packager = Test Harness <test@example.invalid>
size = 15
arch = any
license = MIT
PKG

  printf 'hello pacman\n' >"$dir/pkg/usr/share/demo/hello.txt"
  bsdtar -cf "$tar_path" -C "$dir/pkg" .PKGINFO usr
  gzip -n "$tar_path"
}

validate_dependents() {
  local expected_count actual_count
  local expected_sorted actual_sorted

  expected_count="${#EXPECTED_PACKAGES[@]}"
  actual_count="$(jq -r '.dependents | length' "$ROOT/dependents.json")"
  [[ "$actual_count" == "$expected_count" ]] || {
    printf 'dependents.json count mismatch: expected %s, found %s\n' "$expected_count" "$actual_count" >&2
    exit 1
  }

  expected_sorted="$(printf '%s\n' "${EXPECTED_PACKAGES[@]}" | sort)"
  actual_sorted="$(jq -r '.dependents[].binary_package' "$ROOT/dependents.json" | sort)"
  [[ "$actual_sorted" == "$expected_sorted" ]] || die "dependents.json package set mismatch inside container"
}

build_and_install_local_libarchive() {
  log_step "Building local libarchive Debian packages"

  rm -rf "$BUILD_ROOT" "$RUNTIME_EXTRACT_ROOT"
  mkdir -p "$BUILD_ROOT"
  if [[ "$BUILD_TARGET" == "safe" ]]; then
    tar --exclude='safe/target' -C "$ROOT" -cf - safe | tar -C "$BUILD_ROOT" -xf -
    (
      cd "$BUILD_ROOT/safe"
      cat >"$BUILD_ROOT/link-with-lld.sh" <<'EOF'
#!/usr/bin/env bash
exec cc -fuse-ld=lld "$@"
EOF
      chmod +x "$BUILD_ROOT/link-with-lld.sh"
      export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="$BUILD_ROOT/link-with-lld.sh"
      export DEB_BUILD_OPTIONS="${DEB_BUILD_OPTIONS:+$DEB_BUILD_OPTIONS }nostrip noautodbgsym"
      dpkg-buildpackage -b -uc -us
    )
    LIBARCHIVE_RUNTIME_DEB="$(find "$BUILD_ROOT" -maxdepth 1 -type f -name 'libarchive13t64_*.deb' | head -n1)"
    LIBARCHIVE_DEV_DEB="$(find "$BUILD_ROOT" -maxdepth 1 -type f -name 'libarchive-dev_*.deb' | head -n1)"
    LIBARCHIVE_TOOLS_DEB="$(find "$BUILD_ROOT" -maxdepth 1 -type f -name 'libarchive-tools_*.deb' | head -n1)"
  else
    cp -a "$ROOT/original" "$BUILD_ROOT/"
    (
      cd "$BUILD_ROOT/original/libarchive-3.7.2"
      dpkg-buildpackage -b -uc -us
    )
    LIBARCHIVE_RUNTIME_DEB="$(find "$BUILD_ROOT/original" -maxdepth 1 -type f -name 'libarchive13t64_*.deb' | head -n1)"
    LIBARCHIVE_DEV_DEB="$(find "$BUILD_ROOT/original" -maxdepth 1 -type f -name 'libarchive-dev_*.deb' | head -n1)"
    LIBARCHIVE_TOOLS_DEB="$(find "$BUILD_ROOT/original" -maxdepth 1 -type f -name 'libarchive-tools_*.deb' | head -n1)"
  fi

  [[ -n "$LIBARCHIVE_RUNTIME_DEB" ]] || die "failed to locate built libarchive13t64 .deb"
  [[ -n "$LIBARCHIVE_DEV_DEB" ]] || die "failed to locate built libarchive-dev .deb"
  [[ -n "$LIBARCHIVE_TOOLS_DEB" ]] || die "failed to locate built libarchive-tools .deb"

  dpkg -i "$LIBARCHIVE_RUNTIME_DEB" "$LIBARCHIVE_DEV_DEB" "$LIBARCHIVE_TOOLS_DEB"
  ldconfig

  ACTIVE_LIBARCHIVE="$(ldconfig -p | awk '/libarchive\.so\.13 / && /x86-64/ { print $NF; exit }')"
  [[ -n "$ACTIVE_LIBARCHIVE" ]] || die "ldconfig did not report an active libarchive.so.13"
  ACTIVE_LIBARCHIVE="$(readlink -f "$ACTIVE_LIBARCHIVE")"

  dpkg-deb -x "$LIBARCHIVE_RUNTIME_DEB" "$RUNTIME_EXTRACT_ROOT"
  LIBARCHIVE_MULTIARCH="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
  cmp -s \
    "$ACTIVE_LIBARCHIVE" \
    "$(readlink -f "$RUNTIME_EXTRACT_ROOT/usr/lib/$LIBARCHIVE_MULTIARCH/$(basename "$ACTIVE_LIBARCHIVE")")" || {
      printf 'installed libarchive does not match the locally built runtime package\n' >&2
      exit 1
    }

  log_step "Verified local libarchive install at $ACTIVE_LIBARCHIVE"
}

test_file_roller() {
  local dir

  should_run "file-roller" || return 0
  log_step "file-roller"
  assert_links_to_active_libarchive /usr/bin/file-roller

  dir="$(reset_test_dir "file-roller")"
  printf 'hello from file-roller\n' >"$dir/hello.txt"

  timeout 60 dbus-run-session -- xvfb-run -a \
    file-roller --add-to="$dir/sample.cpio" "$dir/hello.txt"

  rm -f "$dir/hello.txt"
  mkdir -p "$dir/out"

  timeout 60 dbus-run-session -- xvfb-run -a \
    file-roller --extract-to="$dir/out" "$dir/sample.cpio"

  require_contains "$dir/out/hello.txt" "hello from file-roller"
}

test_ark() {
  local dir

  should_run "ark" || return 0
  log_step "ark"
  assert_links_to_active_libarchive /usr/lib/x86_64-linux-gnu/qt5/plugins/kerfuffle/kerfuffle_libarchive.so
  assert_links_to_active_libarchive /usr/lib/x86_64-linux-gnu/qt5/plugins/kerfuffle/kerfuffle_libarchive_readonly.so

  dir="$(reset_test_dir "ark")"
  printf 'hello from ark\n' >"$dir/hello.txt"

  timeout 90 dbus-run-session -- xvfb-run -a \
    ark --batch --add-to "$dir/sample.tar" "$dir/hello.txt"

  bsdtar -tf "$dir/sample.tar" >"$dir/listing.txt"
  require_contains "$dir/listing.txt" "hello.txt"
}

test_archivemount() {
  local dir
  local mounted=0

  should_run "archivemount" || return 0
  log_step "archivemount"
  assert_links_to_active_libarchive /usr/bin/archivemount

  dir="$(reset_test_dir "archivemount")"
  printf 'hello via archivemount\n' >"$dir/hello.txt"
  bsdtar --format cpio -cf "$dir/sample.cpio" -C "$dir" hello.txt
  mkdir -p "$dir/mnt"

  cleanup_archivemount() {
    if (( ${mounted:-0} )); then
      fusermount -u "$dir/mnt" >/dev/null 2>&1 || true
      mounted=0
    fi
  }
  trap cleanup_archivemount RETURN

  archivemount -o readonly "$dir/sample.cpio" "$dir/mnt"
  mounted=1
  require_contains "$dir/mnt/hello.txt" "hello via archivemount"
  cleanup_archivemount
  trap - RETURN
}

test_zathura_cb() {
  local dir
  local status=0

  should_run "zathura-cb" || return 0
  log_step "zathura-cb"
  assert_links_to_active_libarchive /usr/lib/x86_64-linux-gnu/zathura/libcb.so

  dir="$(reset_test_dir "zathura-cb")"
  make_png "$dir/page1.png" 200 40 20
  make_cbz "$dir/comic.cbz" "$dir/page1.png"

  set +e
  timeout 10 xvfb-run -a zathura -l debug "$dir/comic.cbz" >"$dir/zathura.log" 2>&1
  status=$?
  set -e

  assert_status_in "$status" 0 124
  require_contains "$dir/zathura.log" "plugin cb: version"
  require_contains "$dir/zathura.log" "Successfully loaded plugin from '/usr/lib/x86_64-linux-gnu/zathura/libcb.so'."
  require_contains "$dir/zathura.log" "render_job(): Rendering page 1"
}

test_kodi_vfs_libarchive() {
  local dir
  local kodi_home
  local addon_dir
  local db_path
  local result_path
  local archive_path
  local status=0

  should_run "kodi-vfs-libarchive" || return 0
  log_step "kodi-vfs-libarchive"

  dir="$(reset_test_dir "kodi-vfs-libarchive")"
  assert_links_to_active_libarchive /usr/lib/x86_64-linux-gnu/kodi/addons/vfs.libarchive/vfs.libarchive.so.20.3.0
  require_contains /usr/share/kodi/addons/vfs.libarchive/addon.xml 'protocols="archive"'
  require_contains /usr/share/kodi/addons/vfs.libarchive/addon.xml 'extensions=".7z|.tar.gz|.tar.bz2|.tar.xz|.zip|.tgz|.tbz2|.gz|.bz2|.xz|.tar"'

  kodi_home="$dir/kodi-home"
  addon_dir="$kodi_home/.kodi/addons/service.libarchiveprobe"
  db_path="$kodi_home/.kodi/userdata/Database/Addons33.db"
  result_path="$dir/kodi-result.json"
  archive_path="$dir/comic.tar"

  mkdir -p "$addon_dir"
  printf 'hello from kodi archive probe\n' >"$dir/frame.txt"
  bsdtar -cf "$archive_path" -C "$dir" frame.txt

  cat >"$addon_dir/addon.xml" <<'XML'
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<addon id="service.libarchiveprobe" name="Libarchive Probe" version="1.0.0" provider-name="test-harness">
  <requires>
    <import addon="xbmc.python" version="3.0.0"/>
  </requires>
  <extension point="xbmc.service" library="probe.py" start="startup"/>
  <extension point="xbmc.addon.metadata">
    <summary lang="en_GB">Kodi libarchive probe</summary>
    <description lang="en_GB">Kodi libarchive probe</description>
    <platform>all</platform>
  </extension>
</addon>
XML

  cat >"$addon_dir/probe.py" <<'PY'
import json
import os
import urllib.parse

import xbmc
import xbmcvfs

archive = os.environ["KODI_ARCHIVE"]
result_path = os.environ["KODI_RESULT_PATH"]
encoded_archive = urllib.parse.quote(archive, safe="")
url = f"archive://{encoded_archive}/"

xbmc.log("AUTOEXEC START", xbmc.LOGINFO)
xbmc.log("AUTOEXEC URL=" + url, xbmc.LOGINFO)

payload = {"url": url}
try:
    dirs, files = xbmcvfs.listdir(url)
    payload["dirs"] = dirs
    payload["files"] = files
    xbmc.log("AUTOEXEC DIRS=" + repr(dirs), xbmc.LOGINFO)
    xbmc.log("AUTOEXEC FILES=" + repr(files), xbmc.LOGINFO)
except Exception as exc:
    payload["listdir_error"] = repr(exc)
    xbmc.log("AUTOEXEC LISTDIR_ERROR=" + repr(exc), xbmc.LOGERROR)

entry_url = f"archive://{encoded_archive}/frame.txt"
payload["entry_url"] = entry_url
xbmc.log("AUTOEXEC ENTRY_URL=" + entry_url, xbmc.LOGINFO)
try:
    payload["entry_exists"] = xbmcvfs.exists(entry_url)
except Exception as exc:
    payload["entry_exists_error"] = repr(exc)
try:
    handle = xbmcvfs.File(entry_url)
    try:
        payload["entry_content"] = handle.read()
    finally:
        handle.close()
except Exception as exc:
    payload["entry_read_error"] = repr(exc)
xbmc.log("AUTOEXEC ENTRY_RESULT=" + repr({
    "exists": payload.get("entry_exists"),
    "content": payload.get("entry_content"),
}), xbmc.LOGINFO)

with open(result_path, "w", encoding="utf-8") as handle:
    json.dump(payload, handle, sort_keys=True)
PY

  # First boot seeds Kodi's addon database and records the local service addon.
  set +e
  env HOME="$kodi_home" \
    KODI_ARCHIVE="$archive_path" \
    KODI_RESULT_PATH="$result_path" \
    timeout 15 xvfb-run -a kodi --logging=console >"$dir/kodi-bootstrap.log" 2>&1
  status=$?
  set -e

  assert_status_in "$status" 0 124
  [[ -f "$db_path" ]] || die "Kodi did not create $db_path"

  python3 - "$db_path" <<'PY'
import sqlite3
import sys

db_path = sys.argv[1]
conn = sqlite3.connect(db_path)
cur = conn.cursor()
for addon_id in ("service.libarchiveprobe", "vfs.libarchive"):
    row = cur.execute(
        "select enabled from installed where addonID = ?",
        (addon_id,),
    ).fetchone()
    if row is None:
        raise SystemExit(f"Kodi did not register {addon_id}")
    cur.execute(
        "update installed set enabled = 1, disabledReason = 0 where addonID = ?",
        (addon_id,),
    )
conn.commit()
PY

  rm -f "$result_path"

  # Second boot must execute the enabled service addon, list the archive root,
  # and persist the result for this harness to inspect.
  set +e
  env HOME="$kodi_home" \
    KODI_ARCHIVE="$archive_path" \
    KODI_RESULT_PATH="$result_path" \
    timeout 20 xvfb-run -a kodi --logging=console >"$dir/kodi.log" 2>&1
  status=$?
  set -e

  assert_status_in "$status" 0 124
  require_contains "$dir/kodi.log" "CAddonMgr::FindAddons: vfs.libarchive v20.3.0 installed"
  require_contains "$dir/kodi.log" "AUTOEXEC START"
  require_contains "$dir/kodi.log" "AUTOEXEC URL=archive://"
  require_contains "$dir/kodi.log" "AUTOEXEC ENTRY_URL=archive://"
  [[ -f "$result_path" ]] || die "Kodi archive probe did not write $result_path"
  if grep -F 'unsupported protocol(archive)' "$dir/kodi.log" >/dev/null 2>&1; then
    die "Kodi log reported unsupported archive:// protocol"
  fi
  if grep -F 'error opening [archive://' "$dir/kodi.log" >/dev/null 2>&1; then
    die "Kodi log reported an archive:// open failure"
  fi

  python3 - "$result_path" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)

if payload.get("files") != ["frame.txt"]:
    raise SystemExit(f"Kodi archive probe did not list frame.txt at archive root: {payload!r}")

if payload.get("entry_exists") is not True or payload.get("entry_content") != "hello from kodi archive probe\n":
    raise SystemExit(
        "Kodi archive probe did not successfully read frame.txt via the archive:// VFS URL"
    )
PY
}

test_gnome_epub_thumbnailer() {
  local dir

  should_run "gnome-epub-thumbnailer" || return 0
  log_step "gnome-epub-thumbnailer"
  assert_links_to_active_libarchive /usr/bin/gnome-epub-thumbnailer

  dir="$(reset_test_dir "gnome-epub-thumbnailer")"
  make_png "$dir/cover.png" 12 140 90
  make_epub "$dir/book.epub" "$dir/cover.png"

  gnome-epub-thumbnailer -s 128 "$dir/book.epub" "$dir/thumb.png"
  require_nonempty_file "$dir/thumb.png"
}

test_libgepub() {
  local dir

  should_run "libgepub-0.7-0" || return 0
  log_step "libgepub-0.7-0"
  assert_links_to_active_libarchive /usr/lib/x86_64-linux-gnu/libgepub-0.7.so.0

  dir="$(reset_test_dir "libgepub-0.7-0")"
  make_png "$dir/cover.png" 12 140 90
  make_epub "$dir/book.epub" "$dir/cover.png"

  python3 - "$dir/book.epub" <<'PY'
import gi
import sys

gi.require_version("Gepub", "0.7")
from gi.repository import Gepub

doc = Gepub.Doc.new(sys.argv[1])
assert doc.get_metadata("title") == "Test EPUB"
assert doc.get_n_chapters() == 1
assert len(doc.get_text()) == 1
PY
}

test_libgnome_autoar() {
  local dir

  should_run "libgnome-autoar-0-0" || return 0
  log_step "libgnome-autoar-0-0"
  assert_links_to_active_libarchive /usr/lib/x86_64-linux-gnu/libgnome-autoar-0.so.0

  dir="$(reset_test_dir "libgnome-autoar-0-0")"
  mkdir -p "$dir/src" "$dir/out" "$dir/extracted"
  printf 'hello autoar\n' >"$dir/src/hello.txt"

  python3 - "$dir" <<'PY'
import gi
import sys

gi.require_version("GnomeAutoar", "0.1")
from gi.repository import GnomeAutoar, Gio

root = sys.argv[1]
src = Gio.File.new_for_path(f"{root}/src/hello.txt")
outdir = Gio.File.new_for_path(f"{root}/out")
archive = Gio.File.new_for_path(f"{root}/out/hello.tar.gz")
extractdir = Gio.File.new_for_path(f"{root}/extracted")

compressor = GnomeAutoar.Compressor.new([src], outdir, GnomeAutoar.Format.TAR, GnomeAutoar.Filter.GZIP, False)
compressor.start(None)

extractor = GnomeAutoar.Extractor.new(archive, extractdir)
extractor.start(None)
PY

  require_contains "$dir/extracted/hello.txt" "hello autoar"
}

test_fwupd() {
  local dir

  should_run "fwupd" || return 0
  log_step "fwupd"

  dir="$(reset_test_dir "fwupd")"
  make_fwupd_cabinet "$dir" "$dir/test.cab"
  timeout 90 fwupdtool get-details "$dir/test.cab" >"$dir/fwupd.log" 2>&1

  require_contains "$dir/fwupd.log" "Example Test Firmware Update"
  require_contains "$dir/fwupd.log" "Firmware for a test device"
  require_contains "$dir/fwupd.log" "12345678-1234-1234-1234-123456789012"
  require_contains "$dir/fwupd.log" "1.2.3"
}

test_pacman_package_manager() {
  local dir

  should_run "pacman-package-manager" || return 0
  log_step "pacman-package-manager"

  dir="$(reset_test_dir "pacman-package-manager")"
  make_pacman_package "$dir" "$dir/demo-pkg-1.0-1-any.pkg.tar.gz"

  pacman -Qip "$dir/demo-pkg-1.0-1-any.pkg.tar.gz" >"$dir/query-info.txt"
  pacman -Qlp "$dir/demo-pkg-1.0-1-any.pkg.tar.gz" >"$dir/query-files.txt"

  require_contains "$dir/query-info.txt" "Name            : demo-pkg"
  require_contains "$dir/query-info.txt" "Version         : 1.0-1"
  require_contains "$dir/query-files.txt" "/usr/share/demo/hello.txt"
}

test_libextractor_archive() {
  local dir

  should_run "libextractor-plugin-archive" || return 0
  log_step "libextractor-plugin-archive"

  dir="$(reset_test_dir "libextractor-plugin-archive")"
  printf 'meta test\n' >"$dir/hello.txt"
  bsdtar --format cpio -cf "$dir/sample.cpio" -C "$dir" hello.txt

  extract "$dir/sample.cpio" >"$dir/extract.log"
  require_contains "$dir/extract.log" "embedded filename - hello.txt"
  require_contains "$dir/extract.log" "format - POSIX octet-oriented cpio"
}

test_python3_libarchive_c() {
  local dir

  should_run "python3-libarchive-c" || return 0
  log_step "python3-libarchive-c"

  dir="$(reset_test_dir "python3-libarchive-c")"

  python3 - "$dir" <<'PY'
import os
import sys
from pathlib import Path

import libarchive

root = Path(sys.argv[1])
source = root / "hello.txt"
archive_path = root / "sample.tar"
out = root / "out"

source.write_text("hello python binding\n")
out.mkdir(exist_ok=True)

with libarchive.file_writer(str(archive_path), "ustar") as archive:
    archive.add_files(str(source))

entries = []
with libarchive.file_reader(str(archive_path)) as archive:
    for entry in archive:
        entries.append(entry.pathname)

assert entries == [str(source).lstrip("/")]
os.chdir(out)
libarchive.extract_file(str(archive_path))

extracted = out / str(source).lstrip("/")
assert extracted.read_text() == "hello python binding\n"
PY
}

run_tests() {
  test_file_roller
  test_ark
  test_archivemount
  test_zathura_cb
  test_kodi_vfs_libarchive
  test_gnome_epub_thumbnailer
  test_libgepub
  test_libgnome_autoar
  test_fwupd
  test_pacman_package_manager
  test_libextractor_archive
  test_python3_libarchive_c
}

mkdir -p "$XDG_RUNTIME_DIR" "$TEST_ROOT"
chmod 700 "$XDG_RUNTIME_DIR"

validate_dependents
build_and_install_local_libarchive
run_tests

log_step "All dependent checks passed"
CONTAINER
