# Phase 4: Read Core Mainstream

## Phase Name
Reader Completion and Mainstream Format/Filter Coverage

## Implement Phase ID
`impl_read_core_mainstream`

## Preexisting Inputs
- `safe/Cargo.toml`
- `safe/build.rs`
- `safe/src/lib.rs`
- `safe/src/common/**`
- `safe/src/entry/**`
- `safe/src/match/**`
- `safe/src/util/**`
- `safe/src/read/**`
- `safe/src/write/**`
- `safe/src/disk/**`
- `safe/src/ffi/archive_read.rs`
- `safe/src/ffi/archive_read_disk.rs`
- `safe/src/ffi/archive_write.rs`
- `safe/src/ffi/archive_write_disk.rs`
- `safe/include/archive.h`
- `safe/include/archive_entry.h`
- `safe/pkgconfig/libarchive.pc.in`
- `safe/generated/api_inventory.json`
- `safe/generated/original_build_contract.json`
- `safe/generated/original_c_build/**`
- `safe/generated/original_pkgconfig/libarchive.pc`
- `safe/generated/pkgconfig/libarchive.pc`
- `safe/generated/test_manifest.json`
- `safe/scripts/check-source-compat.sh`
- `safe/scripts/render-pkg-config.sh`
- `safe/scripts/run-upstream-c-tests.sh`
- `safe/tests/support/**`
- `safe/tests/libarchive/write_disk/**`
- `original/libarchive-3.7.2/libarchive/archive_read.c`
- `original/libarchive-3.7.2/libarchive/archive_read_private.h`
- `original/libarchive-3.7.2/libarchive/archive_read_open_*.c`
- `original/libarchive-3.7.2/libarchive/archive_read_set_format.c`
- `original/libarchive-3.7.2/libarchive/archive_read_set_options.c`
- `original/libarchive-3.7.2/libarchive/archive_read_support_filter_*.c`
- `original/libarchive-3.7.2/libarchive/archive_read_support_format_*.c`
- `original/libarchive-3.7.2/examples/minitar/minitar.c`
- `original/libarchive-3.7.2/examples/untar.c`
- `original/libarchive-3.7.2/libarchive/test/`

## New Outputs
- `safe/src/read/**`
- `safe/src/ffi/archive_read.rs`
- `safe/tests/libarchive/read_mainstream/**`

## File Changes
- Complete the reader core that phase 3 introduced, including the standalone open/option APIs, callback plumbing, filter and format registration, and the mainstream format handlers.
- Extend source-compat tooling so unchanged C examples can compile and run against the safe library and the generated pkg-config metadata.

## Implementation Details
- Build on the phase-3 reader subset and finish the `archive_read` object model from `archive_read_private.h`, including client callback datasets, skip/seek/read trampolines, filter bidding, format bidding, sparse-gap handling, header-position tracking, passphrase queues, and the state/error behavior exercised by the dedicated reader lifecycle tests.
- Preserve the original registration order of `archive_read_support_filter_all()` and `archive_read_support_format_all()` so bid precedence, format selection, and error ordering remain unchanged.
- Complete the mainstream read formats required by unchanged examples, the early public API surface, and the later frontend phases: tar, gnutar wrapper behavior, pax metadata and filename-encoding paths, cpio, ar, raw, empty, and the common read-core/filter plumbing that those formats depend on. Defer CAB, ZIP and ZIP64 variants, ISO9660, LHA, mtree, RAR, RAR5, WARC, XAR, and 7zip reader coverage to phase 5.
- Complete the mainstream filter set and wrappers required by the ABI and package behavior: gzip, bzip2, compress, grzip, lrzip, lz4, lzip, lzma, xz, lzop, zstd, uuencode, rpm, and program filters. Deprecated `archive_read_support_compression_*` symbols must remain exported as wrappers around the modern filter functions.
- Port the standalone open/read entry points and option wrappers that the regrouped `read_mainstream` oracle requires, including `archive_read_open_fd`, `archive_read_open_file`, `archive_read_open_filename`, `archive_read_open2`, `archive_read_open_memory2`, `archive_read_support_filter_by_code`, `archive_read_append_filter*`, `archive_read_set_format`, `archive_read_set_option`, `archive_read_set_filter_option`, `archive_read_set_format_option`, `archive_read_set_options`, `archive_read_add_passphrase`, `archive_read_data`, `archive_read_data_block`, `archive_read_data_skip`, and `archive_seek_data`.
- Make `./scripts/check-source-compat.sh` render the current build-tree `safe/generated/pkgconfig/libarchive.pc`, normalize it against `safe/generated/original_pkgconfig/libarchive.pc` by allowing differences only in `prefix`, `exec_prefix`, `libdir`, and `includedir`, and fail if any other field, token ordering, or dependency list drifts from the phase-1 original oracle. After that comparison passes, set `PKG_CONFIG_LIBDIR` to the generated directory first and then append the host's normal Ubuntu 24.04 pkg-config directories needed for dependency metadata resolution, clear any conflicting `PKG_CONFIG_PATH`, and then compile and run the unchanged example programs from `original/libarchive-3.7.2/examples/` against the safe headers, safe `pkg-config` metadata, and safe shared library without source changes.
- Keep error strings, close-twice behavior, open-failure behavior, and state transitions aligned with the upstream `read_mainstream` group.
- Consume the phase-1 safe-side `pkg-config` artifact, the phase-1 original `pkg-config` oracle, and the phase-1 build contract in place. Do not substitute manual compiler or linker flags, invent a new `pkg-config` contract, or regenerate the original build-support files.

## Verification Phases

### `check_read_core_mainstream`
- Phase ID: `check_read_core_mainstream`
- Type: `check`
- Bounce Target: `impl_read_core_mainstream`
- Purpose: validate the completed mainstream reader stack, the standalone reader APIs deferred from phase 3, and unchanged C example builds.
- Commands:
```bash
cd safe
cargo test --test libarchive_read_core
./scripts/run-upstream-c-tests.sh libarchive read_mainstream
./scripts/check-source-compat.sh
```

## Success Criteria
- `./scripts/run-upstream-c-tests.sh libarchive read_mainstream` passes against the safe library while consuming the phase-1 manifest and generated-build contract in place.
- This is the first phase where unchanged external C example code compiles and runs through the safe package surface.
- The source-compat check consumes the phase-1 safe-side `pkg-config` artifact and the phase-1 original `pkg-config` oracle. It does not substitute manual compiler or linker flags or invent a new `pkg-config` contract.
- The rendered build-tree `safe/generated/pkgconfig/libarchive.pc` matches `safe/generated/original_pkgconfig/libarchive.pc` after normalizing only `prefix`, `exec_prefix`, `libdir`, and `includedir`.
- The reader stack is now good enough that downstream package checks in phase 6 are testing real library behavior instead of stubs.

## Git Commit Requirement
The implementer must commit all phase changes to git before yielding. The commit message must begin with `impl_read_core_mainstream:`.
