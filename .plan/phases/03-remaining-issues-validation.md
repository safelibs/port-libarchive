# Phase 3: Remaining Issues and Validation

## Phase Name
Remaining-Issues and Validation-Matrix Reconciliation

## Implement Phase ID
`impl_port_doc_remaining_issues`

## Preexisting Inputs
- `safe/PORT.md`
- `safe/tests/**`
- `safe/tests/fixtures-manifest.toml`
- `safe/config/libarchive_test_phase_groups.json`
- `safe/generated/test_manifest.json`
- `safe/generated/rust_test_manifest.json`
- `safe/generated/cve_matrix.json`
- `safe/generated/api_inventory.json`
- `safe/scripts/check-rust-test-coverage.sh`
- `safe/scripts/check-source-compat.sh`
- `safe/scripts/check-abi.sh`
- `safe/scripts/check-link-compat.sh`
- `safe/scripts/check-i686-cve.sh`
- `safe/scripts/run-upstream-c-tests.sh`
- `safe/debian/control`
- `safe/debian/rules`
- `safe/debian/tests/control`
- `safe/debian/tests/minitar`
- `safe/debian/README.Debian`
- `safe/debian/libarchive13t64.symbols`
- `safe/debian/libarchive13t64.lintian-overrides`
- `safe/debian/*.install`
- `safe/debian/*.docs`
- `safe/src/read/mod.rs`
- `safe/src/read/format/mod.rs`
- `safe/src/write/mod.rs`
- `safe/src/write/format/mod.rs`
- `safe/src/disk/native.rs`
- `safe/tests/cve_regressions.rs`
- `dependents.json`
- `relevant_cves.json`
- `test-original.sh`
- `original/libarchive-3.7.2/libarchive/test/test_write_format_zip_file.c`
- `original/libarchive-3.7.2/libarchive/test/test_write_format_zip_file_zip64.c`
- `original/libarchive-3.7.2/libarchive/test/test_write_format_iso9660_zisofs.c`

## New Outputs
- Completed section 4.
- Completed section 6 command log with the real verification commands that were run or skipped.

## File Changes
- Update `safe/PORT.md` in place.

## Implementation Details
- Use `safe/generated/test_manifest.json` and `safe/generated/rust_test_manifest.json` as the authoritative mapping between preserved upstream tests and Rust tests. Do not infer coverage from filenames alone.
- Explicitly audit checked-in byte-for-byte and exact-output evidence: confirm that `test_write_format_zip_file` and `test_write_format_zip_file_zip64` exist in both manifests and in `safe/tests/libarchive/ported_cases.rs`, search for additional exact-output-sensitive upstream tests, and record which such cases are covered by current Rust tests and which broader output classes still lack evidence of universal bit-for-bit equivalence.
- Section 4 must explicitly state that the document cannot claim global bit-for-bit equivalence unless the checked-in evidence supports that exact claim.
- Use `safe/src/read/mod.rs:181-193` and `safe/src/read/mod.rs:368-497` to document placeholder and deferred format support plus ISO9660 option-emulation caveats.
- Use `safe/src/read/format/mod.rs:10-123`, `safe/src/write/format/mod.rs:15-30`, and `safe/tests/cve_regressions.rs:214-279`, `safe/tests/cve_regressions.rs:282-343`, `safe/tests/cve_regressions.rs:488-557` to connect current source and test coverage to the CVE classes in `relevant_cves.json` and `safe/generated/cve_matrix.json`.
- Review `safe/src/disk/native.rs:1166-1176`, `safe/src/disk/native.rs:1396-1490`, and `safe/tests/cve_regressions.rs:156-212` for the current handling of the historical `umask` and secure-extraction bug class. Section 4 must report what the code and tests actually prove.
- Run a two-part TODO and FIXME inventory: one search over `safe/src`, `safe/tests`, `safe/scripts`, `safe/debian`, `safe/build.rs`, and `safe/c_shims`, and one separate search over vendored `safe/c_src`. Section 4 must explicitly say that the non-vendored Rust and packaging tree has no live TODO or FIXME markers if that remains true, and separately describe whether vendored backend TODO or FIXME markers are relevant imported caveats. Do not count vendored `TODO_*` macro or bitmask identifiers as open issue markers.
- Run a dedicated performance-evidence search. If no checked-in benchmark harness or performance report comparing `safe` to `original` exists, section 4 must say so plainly and avoid any performance-parity claim.
- Review packaging caveats from `safe/debian/README.Debian`, `safe/debian/tests/control`, `safe/debian/tests/minitar`, `safe/debian/rules:65-68`, `safe/debian/rules:40-60`, `safe/debian/libarchive13t64.symbols`, and `safe/debian/libarchive13t64.lintian-overrides`, including the i386 `_FILE_OFFSET_BITS 64` requirement, minitar-only autopkgtest scope, conditional `override_dh_auto_test`, manual install and symlink assembly, live `#MISSING:` entries, and the intentional package-name and SONAME mismatch override.
- Use `dependents.json` and `test-original.sh` to describe downstream coverage scope. If the Docker and FUSE harness is not rerun, the document must say that section 4 is reporting intended dependent coverage and prerequisites rather than fresh runtime results.
- If `./scripts/check-source-compat.sh` refreshes `safe/generated/pkgconfig/libarchive.pc` as part of proving the source-compat contract, keep that in-place artifact update and treat it as an evidence refresh, not as an unrelated side effect.

## Verification Phases

### `check_port_doc_remaining_issues`
- `phase_id`: `check_port_doc_remaining_issues`
- `type`: `check`
- `bounce_target`: `impl_port_doc_remaining_issues`
- `purpose`: establish current-state evidence for section 4: tests, skipped checks, source-level issue markers, bit-for-bit coverage, packaging caveats, downstream coverage, and CVE mitigation scope.
- `commands`:
```bash
rg -n 'TODO|FIXME' safe/src safe/tests safe/scripts safe/debian safe/build.rs safe/c_shims
rg -n 'TODO|FIXME' safe/c_src
rg -n 'byte-for-byte|Detailed byte-for-byte|exact output|bit-for-bit' \
  safe/generated/rust_test_manifest.json safe/generated/test_manifest.json \
  safe/tests safe/tests/fixtures-manifest.toml original/libarchive-3.7.2/libarchive/test
rg -n 'performance regression|benchmark|throughput|latency|criterion|hyperfine|\bperf\b' \
  safe/scripts safe/tests safe/generated safe/debian original/libarchive-3.7.2 \
  -g '!safe/doc/**' -g '!safe/c_src/**'
cd safe && ./scripts/check-rust-test-coverage.sh
cd safe && cargo test --workspace --all-features
cd safe && ./scripts/check-source-compat.sh
cd safe && ./scripts/check-abi.sh --strict
cd safe && ./scripts/check-link-compat.sh
cd safe && ./scripts/check-i686-cve.sh
cd safe && ./scripts/run-upstream-c-tests.sh libarchive foundation
```
- `review_checks`:
  - If `docker`, `jq`, and `/dev/fuse` are available, also run `./test-original.sh --target safe --only python3-libarchive-c` from the repo root and record the real result.
  - If those prerequisites are unavailable, the document must explicitly say the downstream Docker and FUSE harness was not rerun and must fall back to `dependents.json` plus `test-original.sh` as evidence of intended scope.
  - Verify that section 4 distinguishes observed failures, observed passes, checks not rerun, source-derived caveats, packaging caveats, and explicit absence findings.

## Success Criteria
- Section 4 explicitly covers failing or skipped tests, TODO and FIXME inventory results, bit-for-bit evidence and gaps, performance-evidence results, packaging caveats, dependent coverage status, and CVE scope and gaps.
- Observed failures, observed passes, checks not rerun, source-derived caveats, packaging caveats, and explicit absence findings are separated clearly.
- Any failed command in this phase is named in section 4 or section 6 with the actual failing script or test name.
- The document does not overclaim global output equivalence or performance parity beyond the checked-in evidence.

## Git Commit Requirement
The implementer must commit work to git before yielding.
