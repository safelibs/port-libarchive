# Phase 1: Safe Scaffold

## Phase Name
Safe Package Scaffold, Oracle ABI Snapshot, Test Manifest, and Build-Tree Contracts

## Implement Phase ID
`impl_safe_scaffold`

## Preexisting Inputs
- `original/libarchive-3.7.2/libarchive/archive.h`
- `original/libarchive-3.7.2/libarchive/archive_entry.h`
- `original/libarchive-3.7.2/configure.ac`
- `original/libarchive-3.7.2/config.h.in`
- `original/libarchive-3.7.2/build/autogen.sh`
- `original/libarchive-3.7.2/build/version`
- `original/libarchive-3.7.2/CMakeLists.txt`
- `original/libarchive-3.7.2/Makefile.am`
- `original/libarchive-3.7.2/build/pkgconfig/libarchive.pc.in`
- `original/libarchive-3.7.2/libarchive/CMakeLists.txt`
- `original/libarchive-3.7.2/libarchive/test/CMakeLists.txt`
- `original/libarchive-3.7.2/tar/test/CMakeLists.txt`
- `original/libarchive-3.7.2/cpio/test/CMakeLists.txt`
- `original/libarchive-3.7.2/cat/test/CMakeLists.txt`
- `original/libarchive-3.7.2/unzip/test/CMakeLists.txt`
- `original/libarchive-3.7.2/test_utils/test_common.h`
- `original/libarchive-3.7.2/test_utils/test_main.c`
- `original/libarchive-3.7.2/debian/control`
- `original/libarchive-3.7.2/debian/rules`
- `original/libarchive-3.7.2/debian/libarchive-dev.docs`
- `original/libarchive-3.7.2/debian/libarchive13t64.symbols`
- `original/libarchive-3.7.2/debian/tests/control`
- `original/libarchive-3.7.2/debian/tests/minitar`
- `original/libarchive-3.7.2/debian/patches/series`
- `original/libarchive-3.7.2/examples/minitar/minitar.c`
- `original/libarchive-3.7.2/examples/untar.c`
- `original/libarchive-3.7.2/libarchive/test/`
- `original/libarchive-3.7.2/tar/test/`
- `original/libarchive-3.7.2/cpio/test/`
- `original/libarchive-3.7.2/cat/test/`
- `original/libarchive-3.7.2/unzip/test/`
- `test-original.sh`
- `dependents.json`
- `relevant_cves.json`

## New Outputs
- `safe/Cargo.toml`
- `safe/build.rs`
- `safe/src/lib.rs`
- `safe/src/common/error.rs`
- `safe/src/common/state.rs`
- `safe/src/common/panic_boundary.rs`
- `safe/src/ffi/mod.rs`
- `safe/src/ffi/bootstrap.rs`
- `safe/include/archive.h`
- `safe/include/archive_entry.h`
- `safe/pkgconfig/libarchive.pc.in`
- `safe/config/libarchive_test_phase_groups.json`
- `safe/abi/libarchive.map`
- `safe/abi/exported_symbols.txt`
- `safe/abi/original_exported_symbols.txt`
- `safe/abi/original_version_info.txt`
- `safe/generated/api_inventory.json`
- `safe/generated/link_compat_manifest.json`
- `safe/generated/original_build_contract.json`
- `safe/generated/original_link_objects/**`
- `safe/generated/original_c_build/config.h`
- `safe/generated/original_c_build/libarchive/test/list.h`
- `safe/generated/original_c_build/tar/test/list.h`
- `safe/generated/original_c_build/cpio/test/list.h`
- `safe/generated/original_c_build/cat/test/list.h`
- `safe/generated/original_c_build/unzip/test/list.h`
- `safe/generated/original_pkgconfig/libarchive.pc`
- `safe/generated/pkgconfig/libarchive.pc`
- `safe/generated/test_manifest.json`
- `safe/generated/original_package_metadata.json`
- `safe/scripts/build-original-oracle.sh`
- `safe/scripts/check-abi.sh`
- `safe/scripts/check-source-compat.sh`
- `safe/scripts/render-pkg-config.sh`
- `safe/scripts/run-upstream-c-tests.sh`
- `safe/tools/gen_api_inventory.py`
- `safe/tools/gen_link_compat_manifest.py`
- `safe/tools/gen_test_manifest.py`

## File Changes
- Create the initial `safe/` Cargo package and build glue.
- Copy the public installed headers from the existing `original/` tree into `safe/include/`.
- Add bootstrap FFI entry points, ABI-oracle capture scripts, the shared upstream-test runner, the safe-side `pkg-config` template/rendering path, and the original installed `pkg-config` oracle snapshot.
- Add the phase-1 original configured-build snapshot, the preserved original-built link-object snapshot, the checked-in `libarchive/test` phase-group table, and the first generated inventories that later phases must consume instead of rediscovering symbols, tests, compile flags, or link-compat source sets.

## Implementation Details
- Create `safe/` as a standard Cargo package whose library name is `archive` and whose crate types include at least `cdylib` and `staticlib`, so it emits `libarchive.so` and `libarchive.a`.
- Derive version values from the existing oracle rather than hardcoding independent math. The phase must read `original/libarchive-3.7.2/build/version` and the SOVERSION logic in `original/libarchive-3.7.2/CMakeLists.txt`, then verify that the resulting values for this tree are `ARCHIVE_VERSION_NUMBER = 3007002`, package version `3.7.2`, and SONAME `libarchive.so.13`.
- Preserve the installed-header contract by copying `archive.h` and `archive_entry.h` verbatim into `safe/include/`. Later phases may not replace them with generated headers.
- Copy `original/libarchive-3.7.2/build/pkgconfig/libarchive.pc.in` into `safe/pkgconfig/libarchive.pc.in` and implement `./scripts/render-pkg-config.sh` so it renders `safe/generated/pkgconfig/libarchive.pc` for the current build tree. The rendered file must preserve the original `Name`, `Description`, `Version`, `Cflags`, `Cflags.private`, `Libs`, `Libs.private`, and `Requires.private` contract, with only `prefix`, `exec_prefix`, `libdir`, and `includedir` rebased to the safe build tree or staged sysroot. Its `--check` mode must be read-only and must compare the normalized non-path contract of the rendered safe file against `safe/generated/original_pkgconfig/libarchive.pc`.
- Implement `./scripts/build-original-oracle.sh` so it performs all phase-1 oracle captures in one deterministic write pass and a separate read-only `--check` pass:
  - build the local original Debian package in a temporary working directory, extract the produced `libarchive13t64_*.deb` and `libarchive-dev_*.deb`, and record:
  - the exact defined dynamic symbol list
  - the exact ELF version-info output
  - `safe/generated/original_package_metadata.json`, which must record at least the package version, Debian revision, multiarch triplet, the runtime shared-library install path, the development `pkg-config` install path, and the produced `.deb` filenames needed by later ABI, source-compat, and packaging checks
  - the installed original `pkg-config` contract as `safe/generated/original_pkgconfig/libarchive.pc`
  - create a separate out-of-tree Autotools build of `original/libarchive-3.7.2/` using the effective Debian configure contract from `debian/rules`: run `build/autogen.sh` or the equivalent autoreconf/bootstrap step, configure with `--without-openssl --with-nettle --enable-bsdtar=shared --enable-bsdcpio=shared --enable-bsdcat=shared --enable-bsdunzip=shared`, then run `make` for `libarchive/test/list.h`, `tar/test/list.h`, `cpio/test/list.h`, `cat/test/list.h`, and `unzip/test/list.h`, and copy the resulting `config.h` plus generated suite `list.h` files into `safe/generated/original_c_build/`.
  - build and preserve the original-built consumer object set required for later link compatibility:
  - derive the `libarchive_test` consumer-source list from the checked-in `original/libarchive-3.7.2/libarchive/test/CMakeLists.txt` target `libarchive_test_SOURCES`, and cross-check it against the Autotools topology by taking `original/libarchive-3.7.2/Makefile.am`'s `libarchive_test_SOURCES`, subtracting `$(libarchive_la_SOURCES)`, and ignoring `test.h`: the resulting consumer set must include `../../test_utils/test_utils.c`, `../../test_utils/test_main.c`, `read_open_memory.c`, and every `test_*.c` entry, while explicitly excluding any source that belongs to `libarchive_la_SOURCES`, `libarchive_SOURCES`, or `archive_static`
  - use the original out-of-tree build tree to build `libarchive_test`, locate the emitted object file for each source in that consumer-source list, and copy only those consumer objects into `safe/generated/original_link_objects/libarchive_test/`
  - record any non-libarchive link dependencies needed to reconstruct `libarchive_test` under `safe/generated/link_compat_manifest.json` as extra libraries, but do not preserve, copy, or later relink any original libarchive implementation object or `archive_static` member
  - compile `original/libarchive-3.7.2/examples/minitar/minitar.c` and `original/libarchive-3.7.2/examples/untar.c` to preserved original-built objects using the captured original headers and `safe/generated/original_pkgconfig/libarchive.pc`, and copy them into `safe/generated/original_link_objects/examples/`
  - write `safe/generated/original_build_contract.json` describing the reusable original C-suite compile contract, including the generated-header paths, suite-specific generated `list.h` paths, ordered include roots, and required preprocessor defines. That contract must require later C-oracle builds to search headers in this order: `safe/include` first, then `safe/generated/original_c_build`, then `safe/generated/original_c_build/<suite>/test`, and only then the original internal/test-harness source roots such as `original/libarchive-3.7.2/libarchive`, `original/libarchive-3.7.2/test_utils`, the suite test directory, and any frontend/helper source directories required by the original suite build files.
  - in `--check` mode, validate that every required output already exists, that the preserved export counts and test counts still match the fixed phase-1 expectations, that `safe/generated/original_pkgconfig/libarchive.pc` and `safe/generated/original_link_objects/**` are present, that the `libarchive_test` snapshot contains only preserved consumer objects from `test_utils/*.c` and `libarchive/test/{read_open_memory.c,test_*.c}`, and that no package build, autoreconf, configure, make, or object recompilation is performed
- Implement `safe/generated/api_inventory.json` as the authoritative inventory for later phases. It must be derived from the copied public headers, the live entries in `debian/libarchive13t64.symbols`, and the captured original shared-object export list. The generator must discard `#MISSING:` historical lines, the `* Build-Depends-Package` directive, and any symbol name absent from the captured original export list. It must explicitly classify each remaining symbol as one of:
  - header-declared public API
  - deprecated compatibility alias
  - live symbol-file-only export that still must remain link-compatible
- Implement `safe/config/libarchive_test_phase_groups.json` as a checked-in deterministic classification table for the 604 source-defined `libarchive/test` entries present in `safe/generated/original_c_build/libarchive/test/list.h`. It must be derived only from the defining `source_file`; every `DEFINE_TEST(...)` row from the same source file must receive the same `phase_group`; no source file may be split across phases. Apply the following exact ordered rules:
  1. `foundation` when the defining source file basename is one of `test_acl_nfs4.c`, `test_acl_posix1e.c`, `test_acl_text.c`, `test_archive_api_feature.c`, `test_archive_clear_error.c`, `test_archive_getdate.c`, `test_archive_match_owner.c`, `test_archive_match_path.c`, `test_archive_pathmatch.c`, `test_archive_set_error.c`, `test_archive_string.c`, `test_entry.c`, `test_entry_strmode.c`, or `test_link_resolver.c`.
  2. `advanced_formats` when the defining source file basename is `test_archive_digest.c` or `test_archive_string_conversion.c`, or when it matches any of `^test_compat_.*\.c$`, `^test_fuzz\.c$`, `^test_archive_write_add_filter_by_name\.c$`, `^test_archive_write_set_filter_option\.c$`, `^test_archive_write_set_format_by_name\.c$`, `^test_archive_write_set_format_filter_by_ext\.c$`, `^test_archive_write_set_format_option\.c$`, `^test_archive_write_set_option\.c$`, `^test_archive_write_set_options\.c$`, `^test_archive_write_set_passphrase\.c$`, `^test_read_set_format\.c$`, `^test_read_format_(7zip|cab|iso|isojoliet|isorr|isozisofs|lha|mtree|rar|rar5|warc|xar|zip).*\.c$`, `^test_write_format_(7zip|iso9660|mtree|warc|xar|zip).*\.c$`, `^test_write_read_format_zip\.c$`, or `^test_zip_filename_encoding\.c$`.
  3. `write_disk` when the defining source file basename is one of `test_acl_pax.c`, `test_acl_platform_nfs4.c`, `test_acl_platform_posix1e.c`, `test_archive_cmdline.c`, `test_archive_match_time.c`, `test_empty_write.c`, `test_extattr_freebsd.c`, `test_pax_xattr_header.c`, `test_read_disk.c`, `test_read_disk_directory_traversals.c`, `test_read_disk_entry_from_file.c`, `test_read_extract.c`, `test_short_writes.c`, `test_warn_missing_hardlink_target.c`, or `test_xattr_platform.c`, or when it matches `^test_write_.*\.c$`.
  4. `read_mainstream` when the defining source file basename is `test_bad_fd.c` or `test_filter_count.c`, or when it matches any of `^test_archive_read_(add_passphrase|close_twice|close_twice_open_fd|close_twice_open_filename|multiple_data_objects|next_header_empty|next_header_raw|open2|set_filter_option|set_format_option|set_option|set_options|support)\.c$`, `^test_open_(failure|fd|file|filename)\.c$`, `^test_(gnutar|pax|ustar)_filename_encoding\.c$`, `^test_read_(data_large|file_nonexistent|large|pax_xattr_rht_security_selinux|pax_xattr_schily|pax_truncated|position|too_many_filters|truncated|truncated_filter)\.c$`, `^test_read_filter_(compress|grzip|lrzip|lzop|lzop_multiple_parts|program|program_signature|uudecode|uudecode_raw)\.c$`, `^test_read_format_(ar|cpio_.*|empty|gtar_.*|pax_bz2|raw|tar.*|tbz|tgz|tlz|txz|tz|ustar_.*)\.c$`, or `^test_(sparse_basic|tar_filenames|tar_large|ustar_filenames)\.c$`.
  5. `advanced_formats` for every remaining `libarchive/test` source file.
- The resulting `safe/config/libarchive_test_phase_groups.json` table must contain exactly 604 rows with fixed `DEFINE_TEST(...)` counts `foundation = 16`, `write_disk = 73`, `read_mainstream = 147`, and `advanced_formats = 368`. These counts are deliberate: phase 2 contains only file-free object and utility tests, phase 3 contains the disk/extract and mainstream writer round-trip files, and all mtree/ZIP/WARC/XAR/7zip/ISO9660-dependent files remain deferred to phase 5.
- Implement `safe/generated/test_manifest.json` as the authoritative test inventory for later phases. It must be generated from `safe/generated/original_c_build/*/test/list.h` as the primary ordered inventory and use source-file scanning only to map each `DEFINE_TEST(...)` name back to its defining source file and fixture references. Each manifest row must record at least `suite`, `define_test`, `source_file`, `suite_order`, `fixture_refs`, and `phase_group`.
- `safe/tools/gen_test_manifest.py` must fail if any generated suite `list.h` entry is missing from the manifest, if any `libarchive/test` entry is missing from `safe/config/libarchive_test_phase_groups.json`, if the phase-group table contains an unknown or duplicate name, if any `DEFINE_TEST(...)` entry found by source scanning is absent from the generated `list.h` snapshot, or if the fixed `foundation = 16`, `write_disk = 73`, `read_mainstream = 147`, `advanced_formats = 368` counts do not hold. For `tar/test`, `cpio/test`, `cat/test`, and `unzip/test`, the generated `phase_group` value must be the literal `all`.
- Implement `safe/generated/link_compat_manifest.json` as the authoritative object-link inventory for later phases. It must contain explicit target records for `libarchive_test`, `minitar`, and `untar`, plus the preserved original-built object membership for those targets. For every preserved object file under `safe/generated/original_link_objects/` that participates in one of those executables, record at least the original source path, the preserved object path, the owning target name, and the target-local link order. For every target record, record at least the final link target name, the ordered object list, any additional non-libarchive libraries needed at link time, whether the linked executable must be run as part of link-compat verification, and a `run_contract` object when runnable. The `libarchive_test` target record must contain only preserved consumer objects built from `original/libarchive-3.7.2/test_utils/*.c` and `original/libarchive-3.7.2/libarchive/test/{read_open_memory.c,test_*.c}`; it must not record or relink any object produced from `libarchive_la_SOURCES`, `libarchive_SOURCES`, or `archive_static`. Later phases may not replace those preserved objects with freshly compiled equivalents.
- The `run_contract` schema must be specific enough that `./scripts/check-link-compat.sh` can execute it without guessing. It must record the working-directory policy (`fresh_tempdir` for all runnable targets in this tree), any setup actions, the exact argv to run, any required repo-relative fixture roots, environment overrides, the expected exit status for each run, and any required post-run assertions such as byte-for-byte file comparisons or MIME checks. When argv needs a fixture path, the manifest must record the fixture root as its own named field and the argv position must reference that named fixture so the checker can substitute the resolved absolute path mechanically instead of inferring it.
- The fixed runnable-target contracts that phase 1 must encode are:
  - `libarchive_test`: run the relinked executable exactly once from a fresh temporary working directory with argv `["-r", "{reference_dir}"]`, named fixture root `reference_dir = original/libarchive-3.7.2/libarchive/test`, environment overrides `LANG=en_US.UTF-8` and `LC_ALL=en_US.UTF-8`, and expected exit status `0`. This target's contract is the full upstream consumer-side test executable reconstructed from preserved original test/support objects plus the safe library, not a hand-picked smoke subset.
  - `minitar`: in a fresh temporary working directory, create a file `foo` whose exact contents are `Deadbeaf\n`; run the relinked executable with `["-cf", "foo.tar", "foo"]`, `["-czf", "foo.tar.gz", "foo"]`, and `["-cyf", "foo.tar.bz2", "foo"]`; assert MIME types `application/x-tar`, `application/gzip`, and `application/x-bzip2` for those three archives; copy `foo` to `foo.orig`; remove `foo`; then run `["-xf", "foo.tar"]`, `["-xf", "foo.tar.gz"]`, and `["-xf", "foo.tar.bz2"]`, removing the extracted `foo` between runs and asserting after each extraction that `foo` is byte-for-byte identical to `foo.orig`. Every relinked-target invocation in this scenario must exit `0`.
  - `untar`: in a fresh temporary working directory, create a file `foo` whose exact contents are `Deadbeaf\n`, copy it to `foo.orig`, create `foo.tar` via the recorded host-side setup action `tar -cf foo.tar foo`, remove `foo`, then run the relinked executable with `["-xf", "foo.tar"]`, expect exit status `0`, and assert that the extracted `foo` is byte-for-byte identical to `foo.orig`.
- Add the minimal exported functions needed for smoke builds and ABI inventory checks: `archive_version_number`, `archive_version_string`, `archive_version_details`, `archive_read_new`, `archive_write_new`, `archive_read_disk_new`, `archive_write_disk_new`, `archive_entry_new`, `archive_entry_new2`, `archive_match_new`, `archive_free`, `archive_read_free`, `archive_write_free`, `archive_entry_free`, and `archive_match_free`. Stub behavior is acceptable in this phase as long as the ABI and panic boundary are correct.
- Add a panic boundary helper that wraps every exported `extern "C"` entry point with `catch_unwind` and converts unwinds into `ARCHIVE_FATAL`, `NULL`, or another ABI-appropriate failure result instead of unwinding through C.
- Consume the checked-in oracle artifacts in place and keep `original/` immutable. Later phases must consume the generated phase-1 outputs instead of rescanning the tree, regenerating new inventories, or rebuilding the original snapshots on demand.

## Verification Phases

### `check_safe_scaffold_build`
- Phase ID: `check_safe_scaffold_build`
- Type: `check`
- Bounce Target: `impl_safe_scaffold`
- Purpose: prove the initial Rust package, build scripts, and bootstrap FFI exports build successfully.
- Commands:
```bash
cd safe
cargo check
cargo build --release
```

### `check_safe_scaffold_abi_contract`
- Phase ID: `check_safe_scaffold_abi_contract`
- Type: `check`
- Bounce Target: `impl_safe_scaffold`
- Purpose: prove phase 1 captured the exact ABI/test oracles from existing local artifacts before deeper implementation begins.
- Commands:
```bash
cd safe
./scripts/build-original-oracle.sh --check
./scripts/render-pkg-config.sh --mode build-tree --check
python3 tools/gen_api_inventory.py --check
python3 tools/gen_test_manifest.py --check
python3 tools/gen_link_compat_manifest.py --check
./scripts/check-abi.sh --inventory-only
```

## Success Criteria
- `cargo check` and `cargo build --release` succeed for the initial `safe/` package.
- `safe/abi/original_exported_symbols.txt` matches the 421-name export surface captured from the original local package build.
- The live-entry subset of `debian/libarchive13t64.symbols` agrees with that same oracle after excluding `#MISSING:` lines and Debian control directives.
- `safe/generated/original_c_build/libarchive/test/list.h` contains 604 names and includes `test_read_format_rar_overflow`, `test_read_format_rar5_loop_bug`, and `test_read_format_warc_incomplete`.
- `safe/generated/original_pkgconfig/libarchive.pc` is the installed original `libarchive.pc` captured from the same local original package build that produced the ABI oracle, and `safe/generated/original_package_metadata.json` records the corresponding multiarch install locations that later verifiers consume.
- `safe/generated/test_manifest.json` contains all 762 original `DEFINE_TEST(...)` entries from the generated suite `list.h` snapshot: 604 in `libarchive`, 70 in `tar`, 48 in `cpio`, 18 in `cat`, and 22 in `unzip`.
- `safe/config/libarchive_test_phase_groups.json` classifies the 604 `libarchive/test` entries into exactly 16 `foundation`, 73 `write_disk`, 147 `read_mainstream`, and 368 `advanced_formats` rows.
- `safe/generated/original_link_objects/**` contains the preserved original-built consumer object set for `libarchive_test` plus the preserved original-built objects for `minitar` and `untar`, and `safe/generated/link_compat_manifest.json` points at those exact objects. The `libarchive_test` portion contains only objects built from `original/libarchive-3.7.2/test_utils/*.c` and `original/libarchive-3.7.2/libarchive/test/{read_open_memory.c,test_*.c}`.
- `safe/generated/link_compat_manifest.json` records the fixed runnable-target contracts above without leaving argv, temporary working directories, locale handling, setup actions, fixture roots, or expected outcomes implicit.
- `safe/generated/api_inventory.json`, `safe/generated/original_build_contract.json`, `safe/generated/original_c_build/**`, `safe/generated/original_link_objects/**`, `safe/generated/original_pkgconfig/libarchive.pc`, `safe/generated/original_package_metadata.json`, `safe/generated/test_manifest.json`, `safe/generated/link_compat_manifest.json`, and `safe/generated/pkgconfig/libarchive.pc` become mandatory inputs for later phases. Later phases must consume them instead of inventing new inventories, new generated headers, new pkg-config contracts, or new compiler/linker contracts.

## Git Commit Requirement
The implementer must commit all phase changes to git before yielding. The commit message must begin with `impl_safe_scaffold:`.
