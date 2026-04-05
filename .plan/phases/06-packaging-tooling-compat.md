# Phase 6: Packaging Tooling Compat

## Phase Name
Frontends, Debian Packaging, and Compatibility Harnesses

## Implement Phase ID
`impl_packaging_tooling_compat`

## Preexisting Inputs
- `safe/Cargo.toml`
- `safe/build.rs`
- `safe/include/archive.h`
- `safe/include/archive_entry.h`
- `safe/pkgconfig/libarchive.pc.in`
- `safe/generated/original_pkgconfig/libarchive.pc`
- `safe/generated/original_package_metadata.json`
- `safe/generated/original_build_contract.json`
- `safe/generated/original_c_build/**`
- `safe/generated/original_link_objects/**`
- `safe/generated/link_compat_manifest.json`
- `safe/generated/test_manifest.json`
- `safe/generated/cve_matrix.json`
- `safe/generated/api_inventory.json`
- `safe/generated/pkgconfig/libarchive.pc`
- `safe/scripts/render-pkg-config.sh`
- `safe/scripts/check-source-compat.sh`
- `safe/scripts/run-upstream-c-tests.sh`
- `safe/scripts/check-i686-cve.sh`
- `safe/src/**`
- `original/libarchive-3.7.2/debian/**`
- `original/libarchive-3.7.2/libarchive_fe/**`
- `original/libarchive-3.7.2/tar/**`
- `original/libarchive-3.7.2/cpio/**`
- `original/libarchive-3.7.2/cat/**`
- `original/libarchive-3.7.2/unzip/**`
- `original/libarchive-3.7.2/examples/**`
- `original/libarchive-3.7.2/doc/man/**`
- `test-original.sh`
- `dependents.json`

## New Outputs
- `safe/c_src/libarchive_fe/**`
- `safe/c_src/tar/**`
- `safe/c_src/cpio/**`
- `safe/c_src/cat/**`
- `safe/c_src/unzip/**`
- `safe/examples/**`
- `safe/doc/man/**`
- `safe/debian/**`
- `safe/scripts/run-debian-minitar.sh`
- `safe/scripts/check-link-compat.sh`
- Updated `test-original.sh`

## File Changes
- Vendor the existing frontend, helper, example, and manpage sources into `safe/` so the package becomes self-contained.
- Adapt the Debian packaging from the original source tree to build the Rust library plus the retained C frontends from `safe/`.
- Update the shared top-level dependent-package harness so it builds and installs the safe package by default while still allowing explicit baseline runs against the original source package.

## Implementation Details
- Keep `libarchive_fe`, `bsdtar`, `bsdcpio`, `bsdcat`, and `bsdunzip` in C for this port. Compile them unchanged or with only minimal build-path/include adjustments against the safe headers and safe library.
- Vendor those sources from the checked-in `original/` tree into `safe/`. The result must allow `dpkg-buildpackage` inside `safe/` to complete without reading sibling files outside `safe/`.
- Preserve the Debian build contract from the current package rules, including the effective configure choices from `debian/rules`: `--without-openssl --with-nettle --enable-bsdtar=shared --enable-bsdcpio=shared --enable-bsdcat=shared --enable-bsdunzip=shared`.
- Preserve package names and install behavior:
  - `libarchive13t64`
  - `libarchive-dev`
  - `libarchive-tools`
- Preserve the installed payload expected by those packages: `libarchive.so.13*`, `libarchive.so`, `libarchive.a`, the public headers, `libarchive.pc`, the manpages, the four frontend binaries, and the documentation payload defined by `debian/libarchive-dev.docs`, including `NEWS` and the `examples/` tree under `/usr/share/doc/libarchive-dev/`. Keep the vendored `safe/examples/**` and `safe/debian/tests/minitar` source-tree copies as the oracle used by the autopkgtest-style checker.
- Generate the installed `libarchive.pc` from `safe/pkgconfig/libarchive.pc.in`; do not introduce a second independent pkg-config template under `safe/debian/` or hard-code package-facing compiler flags in the packaging rules.
- Update `safe/debian/libarchive13t64.symbols` from the original symbols file while preserving Debian comment/directive syntax. The compatibility contract and build-fail checks must consider only live symbol entries: exclude `#MISSING:` historical comments and the `* Build-Depends-Package` directive from required-export comparisons, and make the build fail if the produced shared object drifts from the recorded live ABI without an intentional symbols update.
- Implement `./scripts/run-debian-minitar.sh` so it unpacks the built `safe/` `.deb` artifacts into a temporary sysroot, compares the staged sysroot's installed `libarchive.pc` against `safe/generated/original_pkgconfig/libarchive.pc` using the same path-normalized comparison rules from phase 4, creates a temporary source-style working tree that contains `./debian/tests/minitar` and `./examples/` from the vendored `safe/` source tree at the exact relative paths expected by the original script, and runs the existing `debian/tests/minitar` scenario from that working tree while `PKG_CONFIG_SYSROOT_DIR` points at the staged sysroot and `PKG_CONFIG_LIBDIR` searches the sysroot's `libarchive.pc` first before the normal Ubuntu 24.04 dependency-metadata directories. `LD_LIBRARY_PATH`, `PATH`, and any needed multiarch variables must likewise resolve against the staged package sysroot. The script must prove that `pkg-config`, headers, examples, and runtime behavior work together without assuming that an unpacked `.deb` provides an `examples/` tree at its root.
- Implement `./scripts/check-link-compat.sh` so it compares the built safe library's exported names and version info against the phase-1 oracle and then consumes `safe/generated/link_compat_manifest.json`, `safe/generated/original_link_objects/**`, and `safe/generated/original_package_metadata.json` verbatim. For every manifest target, it must relink the exact preserved phase-1 object files in recorded target-local link order against the safe library. For `libarchive_test`, those preserved phase-1 object files must be only the consumer-side objects recorded in the manifest; the original libarchive implementation must be supplied solely by the safe shared library plus any separately recorded non-libarchive extra libraries. For every target whose manifest record contains a `run_contract`, the script must then execute that contract exactly as recorded: materialize the required fresh temporary working directory, perform only the recorded setup actions, resolve any repo-relative fixture roots, apply the recorded environment overrides, run the exact recorded argv in order, and enforce the recorded exit statuses plus post-run assertions. If the manifest requests `LANG=en_US.UTF-8` and `LC_ALL=en_US.UTF-8`, the script must bootstrap a temporary locale root when that locale is unavailable, matching the Debian test-build practice instead of silently changing locales. It may not recompile original sources, regenerate objects, replace preserved original-built objects with new ones, or invent runtime arguments, fixture paths, or smoke scenarios not already encoded in the manifest. The required manifest coverage is:
  - every preserved consumer object file needed to reconstruct `libarchive_test` from `original/libarchive-3.7.2/test_utils/*.c` and `original/libarchive-3.7.2/libarchive/test/{read_open_memory.c,test_*.c}`, with any non-libarchive extra libraries recorded separately
  - the preserved original-built object for `examples/minitar/minitar.c`
  - the preserved original-built object for `examples/untar.c`
- Modify `test-original.sh` in place to add an explicit `--target safe|original` switch. `safe` must become the default. `original` must preserve the current baseline behavior for comparison. The script must keep the existing `--only` behavior and must continue verifying that the active `libarchive.so.13` is the locally built package that was just installed.

## Verification Phases

### `check_packaging_and_dependents`
- Phase ID: `check_packaging_and_dependents`
- Type: `check`
- Bounce Target: `impl_packaging_tooling_compat`
- Purpose: prove that the `safe/` source package builds the Ubuntu packages, that the Debian minitar source-compat test passes against the built package, and that the downstream runtime harness succeeds with the safe package installed.
- Commands:
```bash
cd safe
dpkg-buildpackage -b -uc -us
./scripts/run-debian-minitar.sh
cd ..
./test-original.sh --target safe
```

### `check_link_compat_and_frontends`
- Phase ID: `check_link_compat_and_frontends`
- Type: `check`
- Bounce Target: `impl_packaging_tooling_compat`
- Purpose: prove link compatibility and validate the original front-end binaries and their upstream test suites against the Rust library.
- Commands:
```bash
cd safe
./scripts/check-link-compat.sh
./scripts/run-upstream-c-tests.sh tar all
./scripts/run-upstream-c-tests.sh cpio all
./scripts/run-upstream-c-tests.sh cat all
./scripts/run-upstream-c-tests.sh unzip all
```

## Success Criteria
- `dpkg-buildpackage -b -uc -us` succeeds inside `safe/`, producing self-contained `libarchive13t64`, `libarchive-dev`, and `libarchive-tools` packages.
- `./scripts/run-debian-minitar.sh` succeeds from a source-style temporary tree while all compile-time and runtime resolution comes from the staged safe-package sysroot and the staged `libarchive.pc` matches the phase-1 original `pkg-config` oracle after path normalization.
- `./scripts/check-link-compat.sh` relinks every manifest-listed preserved consumer object file without recompilation and the recorded runnable-target contracts for the relinked `libarchive_test`, `minitar`, and `untar` executables all execute successfully against the safe library.
- The frontend-suite checks consume the phase-1 original build contract and generated-header snapshot, and the link-compat check consumes the phase-1 preserved object snapshot and manifest; none of these verifiers rerun autotools, regenerate suite `list.h`, or relink a freshly recompiled original object set.
- The frontends still pass their original suite tests while dynamically linked against the safe library, and `./test-original.sh --target safe` validates the dependent-package matrix with the safe package installed as the active `libarchive.so.13`.

## Git Commit Requirement
The implementer must commit all phase changes to git before yielding. The commit message must begin with `impl_packaging_tooling_compat:`.
