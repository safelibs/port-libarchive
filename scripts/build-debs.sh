#!/usr/bin/env bash
# Build the safe port via dpkg-buildpackage rooted in safe/.
# Stamps the changelog with `+safelibs<commit-epoch>` so the produced
# .deb files have a deterministic version that wins over Ubuntu's copy
# under the apt pin in safelibs/apt.
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
dist_dir="$repo_root/dist"

# shellcheck source=/dev/null
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

if [[ -d "$HOME/.cargo/bin" ]]; then
  case ":$PATH:" in
    *":$HOME/.cargo/bin:"*) ;;
    *) export PATH="$HOME/.cargo/bin:$PATH" ;;
  esac
fi

rm -rf -- "$dist_dir"
mkdir -p -- "$dist_dir"

# libarchive-specific setup: drop the explicit dynamic-symbol export
# (its single-callsite already supplies linkage) and switch the cargo
# build to clang+lld for the resulting .so.
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

upstream_version="$(dpkg-parsechangelog -S Version | sed -E 's/\+safelibs[0-9]+$//')"
package_name="$(dpkg-parsechangelog -S Source)"
distribution="$(dpkg-parsechangelog -S Distribution)"

if [[ -n "${SAFELIBS_COMMIT_SHA:-}" ]] \
   && command -v git >/dev/null 2>&1 \
   && git -C "$repo_root" cat-file -e "$SAFELIBS_COMMIT_SHA^{commit}" 2>/dev/null; then
  commit_epoch="$(git -C "$repo_root" log -1 --format=%ct "$SAFELIBS_COMMIT_SHA")"
elif command -v git >/dev/null 2>&1 && git -C "$repo_root" rev-parse HEAD >/dev/null 2>&1; then
  commit_epoch="$(git -C "$repo_root" log -1 --format=%ct HEAD)"
else
  commit_epoch="$(date -u +%s)"
fi

new_version="${upstream_version}+safelibs${commit_epoch}"
release_date="$(date -u -R -d "@${commit_epoch}")"

{
  printf '%s (%s) %s; urgency=medium\n\n  * Automated SafeLibs rebuild.\n\n -- SafeLibs CI <ci@safelibs.org>  %s\n\n' \
    "$package_name" "$new_version" "$distribution" "$release_date"
  cat debian/changelog
} > debian/changelog.new
mv debian/changelog.new debian/changelog

sudo mk-build-deps -i -r -t "apt-get -y --no-install-recommends" debian/control
dpkg-buildpackage -us -uc -b

cp -v ../*.deb "$dist_dir"/
