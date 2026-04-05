# Phase 5: Advanced Formats And CVE

## Phase Name
Advanced Format Coverage, Option Wrappers, and CVE Hardening

## Implement Phase ID
`impl_advanced_formats_cve`

## Preexisting Inputs
- `safe/Cargo.toml`
- `safe/src/read/**`
- `safe/src/write/**`
- `safe/src/disk/**`
- `safe/src/ffi/archive_read.rs`
- `safe/src/ffi/archive_write.rs`
- `safe/generated/api_inventory.json`
- `safe/generated/original_build_contract.json`
- `safe/generated/original_c_build/**`
- `safe/generated/test_manifest.json`
- `safe/generated/original_pkgconfig/libarchive.pc`
- `safe/generated/pkgconfig/libarchive.pc`
- `safe/scripts/run-upstream-c-tests.sh`
- `safe/tests/libarchive/read_mainstream/**`
- `original/libarchive-3.7.2/libarchive/archive_read_support_format_*.c`
- `original/libarchive-3.7.2/libarchive/archive_write_set_format_*.c`
- `original/libarchive-3.7.2/libarchive/archive_write_set_format_by_name.c`
- `original/libarchive-3.7.2/libarchive/archive_write_set_format_filter_by_ext.c`
- `original/libarchive-3.7.2/libarchive/archive_write_set_passphrase.c`
- `original/libarchive-3.7.2/libarchive/archive_read_set_options.c`
- `original/libarchive-3.7.2/libarchive/archive_write_set_options.c`
- `original/libarchive-3.7.2/libarchive/archive_ppmd7.c`
- `original/libarchive-3.7.2/libarchive/archive_ppmd8.c`
- `original/libarchive-3.7.2/libarchive/archive_blake2*.c`
- `original/libarchive-3.7.2/libarchive/xxhash.c`
- `original/libarchive-3.7.2/debian/patches/series`
- `original/libarchive-3.7.2/libarchive/test/`
- `all_cves.json`
- `relevant_cves.json`

## New Outputs
- `safe/src/read/format/**`
- `safe/src/write/format/**`
- `safe/src/algorithms/**`
- `safe/src/ffi/archive_options.rs`
- `safe/generated/cve_matrix.json`
- `safe/scripts/check-i686-cve.sh`
- `safe/tests/libarchive/advanced/**`
- `safe/tests/libarchive/security/**`

## File Changes
- Add the remaining advanced format and algorithm implementations.
- Add the by-name, by-code, and option-wrapper exports that the CLI frontends and downstream programs rely on.
- Add a machine-readable CVE coverage matrix and the explicit regression tests/scripts that consume it.

## Implementation Details
- Finish the remaining high-complexity parsers and writers: CAB; ZIP, ZIP64, encryption, and secondary codec variants; ISO9660 with Rock Ridge, Joliet, and zisofs; mtree; WARC; XAR; LHA; RAR; RAR5; 7zip; and the advanced ZIP and ISO9660 write paths.
- Complete the advanced behavior now isolated in the regrouped `advanced_formats` oracle, including mtree digest materialization for `test_archive_digest.c`, ZIP string-conversion and `hdrcharset` behavior for `test_archive_string_conversion.c`, and the mtree/WARC/XAR writer round-trips that phase 3 intentionally deferred.
- Implement the option-wrapper and convenience exports that frontends depend on, including `archive_read_support_format_gnutar`, `archive_write_set_format_ar_bsd`, `archive_write_set_format_ar_svr4`, `archive_write_set_format_mtree_classic`, `archive_write_set_format_pax_restricted`, `archive_write_set_format_shar_dump`, `archive_write_set_format_by_name`, `archive_write_set_format_filter_by_ext`, `archive_read_set_option`, `archive_read_set_filter_option`, `archive_read_set_format_option`, `archive_write_set_option`, `archive_write_set_filter_option`, and `archive_write_set_format_option`.
- Use checked arithmetic everywhere attacker-controlled sizes, shifts, offsets, counts, and allocation requests appear. Arithmetic overflow must become a hard parse failure, never a panic or silent wrap.
- Add explicit forward-progress guards to decompression loops and continuation parsers, especially for RAR5, recursive filter chains, and Rock Ridge continuation parsing.
- Enforce extraction containment for hardlinks, symlinks, ACL application, absolute paths, and metadata writes in the exact areas highlighted by the `records[]` entries in `relevant_cves.json`.
- Consume the existing `all_cves.json` artifact in place as the broader CVE inventory context for the curated `relevant_cves.json` crosswalk. Do not regenerate or replace either CVE inventory during this phase.
- Implement `safe/generated/cve_matrix.json` as the authoritative security crosswalk. It must contain one entry per `records[].cve_id`, the targeted code area, the required controls taken from `records[].required_controls`, and the exact Rust test or script that verifies the control.
- Use `debian/patches/series` as additional behavioral documentation for Ubuntu-specific bug fixes and test deltas. If a Debian patch changes runtime behavior, validation strictness, or diagnostics, the Rust port must match the patched Ubuntu behavior, not the pristine upstream behavior.
- `./scripts/check-i686-cve.sh` must explicitly exercise overflow-sensitive 32-bit cases, including the zisofs-size cases documented in `relevant_cves.json` and the existing Debian `test-zstd-32bit.patch` context where relevant.

## Verification Phases

### `check_advanced_formats`
- Phase ID: `check_advanced_formats`
- Type: `check`
- Bounce Target: `impl_advanced_formats_cve`
- Purpose: validate the remaining high-complexity readers, writers, option wrappers, and long-tail compatibility suites.
- Commands:
```bash
cd safe
cargo test --test advanced_formats
./scripts/run-upstream-c-tests.sh libarchive advanced_formats
```

### `check_cve_regressions`
- Phase ID: `check_cve_regressions`
- Type: `check`
- Bounce Target: `impl_advanced_formats_cve`
- Purpose: validate that the Rust port closes the explicitly curated CVE classes, including cases not fully covered by upstream tests.
- Commands:
```bash
cd safe
cargo test --test cve_regressions
./scripts/check-i686-cve.sh
```

## Success Criteria
- Every CVE record in `relevant_cves.json` has a concrete row in `safe/generated/cve_matrix.json`, and every row points to a passing regression test or script.
- The phase consumes the checked-in `all_cves.json` and `relevant_cves.json` artifacts in place and does not regenerate a new CVE inventory.
- The `advanced_formats` C-oracle group passes with the remaining high-complexity readers, writers, option wrappers, and long-tail compatibility surface implemented.
- Overflow, forward-progress, containment, and Ubuntu-patch behavior called out by the CVE crosswalk are covered by executable regressions, including the 32-bit zisofs-sensitive cases.
- After this phase, the library surface needed by unchanged `bsdtar`, `bsdcpio`, `bsdcat`, and `bsdunzip` is present.

## Git Commit Requirement
The implementer must commit all phase changes to git before yielding. The commit message must begin with `impl_advanced_formats_cve:`.
