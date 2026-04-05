# Phase 3: Write And Disk

## Phase Name
Write Pipeline, Disk Semantics, and Early Round-Trip Reader Subset

## Implement Phase ID
`impl_write_and_disk`

## Preexisting Inputs
- `safe/Cargo.toml`
- `safe/build.rs`
- `safe/src/lib.rs`
- `safe/src/common/**`
- `safe/src/entry/**`
- `safe/src/match/**`
- `safe/src/util/**`
- `safe/src/ffi/mod.rs`
- `safe/src/ffi/bootstrap.rs`
- `safe/src/ffi/archive_common.rs`
- `safe/src/ffi/archive_entry.rs`
- `safe/src/ffi/archive_match.rs`
- `safe/include/archive.h`
- `safe/include/archive_entry.h`
- `safe/generated/api_inventory.json`
- `safe/generated/original_build_contract.json`
- `safe/generated/original_c_build/**`
- `safe/generated/test_manifest.json`
- `safe/config/libarchive_test_phase_groups.json`
- `safe/scripts/run-upstream-c-tests.sh`
- `safe/tests/support/**`
- `safe/tests/libarchive/foundation/**`
- `relevant_cves.json`
- `original/libarchive-3.7.2/libarchive/archive_read.c`
- `original/libarchive-3.7.2/libarchive/archive_read_private.h`
- `original/libarchive-3.7.2/libarchive/archive_read_open_*.c`
- `original/libarchive-3.7.2/libarchive/archive_read_set_format.c`
- `original/libarchive-3.7.2/libarchive/archive_read_support_filter_*.c`
- `original/libarchive-3.7.2/libarchive/archive_read_support_format_*.c`
- `original/libarchive-3.7.2/libarchive/archive_write.c`
- `original/libarchive-3.7.2/libarchive/archive_write_private.h`
- `original/libarchive-3.7.2/libarchive/archive_write_open_*.c`
- `original/libarchive-3.7.2/libarchive/archive_write_set_options.c`
- `original/libarchive-3.7.2/libarchive/archive_write_set_passphrase.c`
- `original/libarchive-3.7.2/libarchive/archive_write_add_filter*.c`
- `original/libarchive-3.7.2/libarchive/archive_write_set_format*.c`
- `original/libarchive-3.7.2/libarchive/archive_read_disk_*.c`
- `original/libarchive-3.7.2/libarchive/archive_read_extract*.c`
- `original/libarchive-3.7.2/libarchive/archive_write_disk_posix.c`
- `original/libarchive-3.7.2/libarchive/archive_write_disk_private.h`
- `original/libarchive-3.7.2/libarchive/test/`

## New Outputs
- `safe/src/read/**`
- `safe/src/write/**`
- `safe/src/disk/**`
- `safe/src/ffi/archive_read.rs`
- `safe/src/ffi/archive_write.rs`
- `safe/src/ffi/archive_write_disk.rs`
- `safe/src/ffi/archive_read_disk.rs`
- `safe/tests/libarchive/write_disk/**`

## File Changes
- Add the Rust writer core, the minimal reader round-trip subset needed by the phase-3 oracle group, the write formats needed for the early toolchain, and the read-disk/write-disk implementations.
- Extend the support/test layer with filesystem helpers equivalent to the existing `test_common.h` behavior.
- Add FFI exports for `archive_write_*`, `archive_write_disk_*`, `archive_read_disk_*`, and the `archive_read_*` subset required by the phase-3 round-trip tests.

## Implementation Details
- Consume the phase-1 build contract, generated-header snapshot, and test manifest in place. Do not regenerate them in this phase.
- Port the `archive_write` core object from `archive_write_private.h`, including client callbacks, block-size handling, skip-file handling, filter chain setup, format callbacks, and passphrase plumbing.
- Implement the write formats required by the phase-3 compatibility path: tar family, pax with ACL/xattr metadata, cpio variants, ar, raw, and shar. Defer mtree, WARC, XAR, ZIP, ISO9660, and 7zip write paths to phase 5 because the regrouped `advanced_formats` oracle owns every source file that requires those writers.
- Implement the early write filters and wrapper exports required by the Ubuntu ABI and the downstream tool flows: none, gzip, bzip2, compress, grzip, lrzip, lz4, lzip, lzma, xz, lzop, zstd, b64encode, uuencode, and program-filter plumbing.
- Add the minimal `archive_read` substrate that the regrouped `write_disk` oracle now requires for writer round-trips and extraction flows: `archive_read_new/free/close`, `archive_read_open_memory`, `archive_read_open_filename`, `archive_read_support_filter_all`, the per-filter registrations needed to read the phase-3 write filters back, `archive_read_support_format_all`, the per-format registrations needed to read tar/gnutar/pax/cpio/ar/raw output back, `archive_read_next_header`, `archive_read_data`, `archive_read_extract`, and `archive_read_extract2`. Preserve the original filter and format registration order for the subset implemented in this phase so bid precedence and error ordering already match the C oracle.
- Port `archive_read_disk_*` and `archive_write_disk_*` with a safe descriptor-based extraction model. The Rust implementation must not reproduce the process-global `umask()` race described by `CVE-2023-30571`.
- Mirror the original deferred-fixup model from `archive_write_disk_posix.c`: queue chmod/chown/times/xattr/acl work until after filesystem object creation, but apply it via file descriptors or `*at` syscalls so metadata updates cannot escape the extraction root.
- Integrate the phase-2 entry, ACL, xattr, and match objects with the phase-3 pipelines so the regrouped source files `test_acl_pax.c`, `test_acl_platform_nfs4.c`, `test_acl_platform_posix1e.c`, `test_extattr_freebsd.c`, `test_archive_match_time.c`, and `test_archive_cmdline.c` are genuinely in scope. That includes pax ACL/xattr round-trips, `archive_read_disk_entry_from_file()` interoperability with `archive_match`, and the exact command-line tokenization semantics of `archive_write_add_filter_program()`.
- Preserve hardlink, symlink, absolute-path, `..`, no-overwrite, safe-writes, sparse-file, ACL, xattr, and timestamp semantics covered by the `write_disk` test group and by the filesystem-extraction entries in `relevant_cves.json`.
- Keep Linux-specific ownership, ACL, and xattr behavior configurable so Ubuntu 24.04 package builds behave like the current package rules.
- Stop the reader work at the round-trip subset consumed by the `write_disk` oracle. Standalone reader APIs such as `archive_read_open_fd`, the close-twice/open2 family, support-by-code registration, mtree/WARC/XAR readers, and the rest of the `read_mainstream` and `advanced_formats` reader surface remain deferred to phases 4 and 5.

## Verification Phases

### `check_write_and_disk`
- Phase ID: `check_write_and_disk`
- Type: `check`
- Bounce Target: `impl_write_and_disk`
- Purpose: validate writer behavior, the reader subset needed by the `write_disk` oracle group, extraction semantics, and filesystem traversal/security.
- Commands:
```bash
cd safe
cargo test --test libarchive_write_core
cargo test --test libarchive_write_disk
cargo test --test libarchive_read_disk
./scripts/run-upstream-c-tests.sh libarchive write_disk
```

## Success Criteria
- `./scripts/run-upstream-c-tests.sh libarchive write_disk` passes while consuming the phase-1 manifest and generated-build contract in place.
- The disk/extract path is secure enough that later dependent-package runtime checks are meaningful.
- The `write_disk` C-oracle group uses the phase-1 manifest rather than a fresh test list.
- Pax ACL/xattr round-trips, `archive_read_disk_entry_from_file()` plus `archive_match` interoperability, and `archive_write_add_filter_program()` tokenization semantics are implemented and verified in this phase.
- The phase-3 reader work stops at the round-trip subset consumed by the `write_disk` oracle. Standalone reader APIs such as `archive_read_open_fd`, the close-twice/open2 family, support-by-code registration, mtree/WARC/XAR readers, and the rest of the `read_mainstream` and `advanced_formats` reader surface remain deferred to phases 4 and 5.

## Git Commit Requirement
The implementer must commit all phase changes to git before yielding. The commit message must begin with `impl_write_and_disk:`.
