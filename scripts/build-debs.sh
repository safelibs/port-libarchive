#!/usr/bin/env bash
# libarchive: drop the explicit dynamic-symbol export (its single
# call-site already supplies linkage) and switch the cargo build to
# clang+lld for the resulting .so. Then run the standard safe-debian
# build via the shared helper.
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=/dev/null
. "$repo_root/scripts/lib/build-deb-common.sh"

prepare_rust_env
prepare_dist_dir "$repo_root"

export DEB_BUILD_OPTIONS="${DEB_BUILD_OPTIONS:+$DEB_BUILD_OPTIONS }nostrip noautodbgsym"

python3 - <<'PY'
from pathlib import Path

build_rs = Path("safe/build.rs")
needle = '        println!("cargo:rustc-cdylib-link-arg=-Wl,--export-dynamic-symbol=archive_set_error");\n'
text = build_rs.read_text()
if needle in text:
    build_rs.write_text(text.replace(needle, ""))

rules = Path("safe/debian/rules")
cargo_build = "cargo build --release"
cargo_rustc = "cargo rustc --release -- -Clinker=clang -Clink-arg=-fuse-ld=lld"
rules_text = rules.read_text()
if cargo_build in rules_text and cargo_rustc not in rules_text:
    rules.write_text(rules_text.replace(cargo_build, cargo_rustc, 1))
PY

cd "$repo_root/safe"
stamp_safelibs_changelog "$repo_root"
build_with_dpkg_buildpackage "$repo_root"
