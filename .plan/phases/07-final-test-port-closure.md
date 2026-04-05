# Phase 7: Final Test Port Closure

## Phase Name
Remaining Rust Test Port and Final Compatibility Closure

## Implement Phase ID
`impl_final_test_port_closure`

## Preexisting Inputs
- `safe/abi/original_exported_symbols.txt`
- `safe/abi/original_version_info.txt`
- `safe/generated/test_manifest.json`
- `safe/generated/cve_matrix.json`
- `safe/generated/original_pkgconfig/libarchive.pc`
- `safe/generated/original_package_metadata.json`
- `safe/generated/original_build_contract.json`
- `safe/generated/original_c_build/**`
- `safe/generated/original_link_objects/**`
- `safe/generated/link_compat_manifest.json`
- `safe/scripts/check-abi.sh`
- `safe/scripts/check-link-compat.sh`
- `safe/scripts/check-i686-cve.sh`
- `safe/scripts/run-debian-minitar.sh`
- `safe/scripts/run-upstream-c-tests.sh`
- `safe/tests/libarchive/**`
- `safe/c_src/**`
- `safe/debian/**`
- `safe/examples/**`
- `safe/src/**`
- `original/libarchive-3.7.2/libarchive/test/`
- `original/libarchive-3.7.2/tar/test/`
- `original/libarchive-3.7.2/cpio/test/`
- `original/libarchive-3.7.2/cat/test/`
- `original/libarchive-3.7.2/unzip/test/`
- `dependents.json`
- `test-original.sh`

## New Outputs
- Completed `safe/tests/libarchive/**`
- Completed `safe/tests/tar/**`
- Completed `safe/tests/cpio/**`
- Completed `safe/tests/cat/**`
- Completed `safe/tests/unzip/**`
- `safe/tests/fixtures-manifest.toml`
- `safe/generated/rust_test_manifest.json`
- `safe/scripts/check-rust-test-coverage.sh`
- Any final bug fixes across `safe/src/**`
- Any final compatibility fixes across `safe/c_src/**`
- Any final packaging fixes across `safe/debian/**`
- Any final shared-harness fixes in `test-original.sh`

## File Changes
- Finish any remaining one-for-one Rust test ports for original `DEFINE_TEST(...)` entries.
- Add the final fixture-manifest and helper layer used by the Rust-native test tree.
- Fix the remaining compatibility defects revealed by the full matrix.

## Implementation Details
- Finish porting every original `DEFINE_TEST(...)` entry enumerated in `safe/generated/test_manifest.json` into the Rust package layout under the matching suite directory. Multiple original test cases may live in one Rust module or integration-test crate, and Rust file basenames do not need to match the original C basename when the upstream file contains multiple test cases or the upstream test name already diverges from the basename.
- Implement `safe/generated/rust_test_manifest.json` as the final Rust-port coverage map. It must contain exactly one row for every entry in `safe/generated/test_manifest.json`, no two rows may share the same `(rust_test_target, rust_test_name)` pair, and each row must record at least `suite`, `define_test`, `source_file`, `rust_test_target`, `rust_test_name`, and any required frontend-binary driver metadata.
- Implement `./scripts/check-rust-test-coverage.sh` so it consumes `safe/generated/test_manifest.json` and `safe/generated/rust_test_manifest.json`, runs `cargo test --workspace --all-features -- --list`, and fails if any original `DEFINE_TEST(...)` name is missing, duplicated, mapped to more than one Rust test, shares a `(rust_test_target, rust_test_name)` pair with another original entry, or maps to a Rust test name that is absent from the cargo test listing.
- Preserve the existing fixture corpus by consuming the files under `original/.../test/` through a shared manifest/helper layer. Do not regenerate or refetch reference archives. Duplicate fixtures into `safe/` only when an explicit packaging or test-isolation need makes the copy necessary.
- Use the final phase to reconcile any remaining ABI drift, symbol-version mismatches, error-string mismatches, path-edge cases, performance regressions in hot loops, or packaging inconsistencies exposed by the full matrix.
- Keep the final internal unsafe surface narrowly limited to FFI, raw descriptor handling, and unavoidable low-level algorithm interop.

## Verification Phases

### `check_full_rust_test_corpus`
- Phase ID: `check_full_rust_test_corpus`
- Type: `check`
- Bounce Target: `impl_final_test_port_closure`
- Purpose: prove that the original upstream tests have been ported into the Rust package structure rather than surviving only as an external C oracle.
- Commands:
```bash
cd safe
./scripts/check-rust-test-coverage.sh
cargo test --workspace --all-features
```

### `check_full_matrix_final`
- Phase ID: `check_full_matrix_final`
- Type: `check`
- Bounce Target: `impl_final_test_port_closure`
- Purpose: rerun the final end-to-end matrix over formatting, linting, ABI, link compatibility, upstream suites, package build, Debian source-compat, CVE checks, and downstream runtime compatibility.
- Commands:
```bash
cd safe
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
./scripts/check-rust-test-coverage.sh
cargo test --workspace --all-features
dpkg-buildpackage -b -uc -us
./scripts/run-debian-minitar.sh
./scripts/check-abi.sh --strict
./scripts/check-link-compat.sh
./scripts/run-upstream-c-tests.sh libarchive all
./scripts/run-upstream-c-tests.sh tar all
./scripts/run-upstream-c-tests.sh cpio all
./scripts/run-upstream-c-tests.sh cat all
./scripts/run-upstream-c-tests.sh unzip all
./scripts/check-i686-cve.sh
cd ..
./test-original.sh --target safe
```

## Success Criteria
- `safe/generated/rust_test_manifest.json` covers all 762 original `DEFINE_TEST(...)` entries from `safe/generated/test_manifest.json`.
- `./scripts/check-rust-test-coverage.sh` passes and the mapped Rust test names are present in `cargo test --workspace --all-features -- --list`.
- The Rust-native corpus, the upstream C oracle suites, the ABI checks, the link-compat checks, the Debian package build, the Debian minitar source-compat check, and the downstream runtime harness all pass in one linear workflow.
- Remaining compatibility defects across `safe/src/**`, `safe/c_src/**`, `safe/debian/**`, and `test-original.sh` are closed without regenerating or refetching the original fixture corpus.

## Git Commit Requirement
The implementer must commit all phase changes to git before yielding. The commit message must begin with `impl_final_test_port_closure:`.
