# Phase 1: Architecture Baseline

## Phase Name
Architecture, Build, Packaging, and Dependency Baseline

## Implement Phase ID
`impl_port_doc_architecture`

## Preexisting Inputs
- `safe/Cargo.toml`
- `safe/Cargo.lock`
- `safe/build.rs`
- `safe/src/lib.rs`
- `safe/src/ffi/**`
- `safe/src/common/api.rs`
- `safe/src/common/backend.rs`
- `safe/src/common/state.rs`
- `safe/src/read/mod.rs`
- `safe/src/write/mod.rs`
- `safe/src/disk/native.rs`
- `safe/include/archive.h`
- `safe/include/archive_entry.h`
- `safe/c_shims/archive_set_error.c`
- `safe/scripts/build-c-frontends.sh`
- `safe/scripts/render-pkg-config.sh`
- `safe/pkgconfig/libarchive.pc.in`
- `safe/debian/control`
- `safe/debian/rules`
- `safe/debian/README.Debian`
- `safe/debian/tests/control`
- `safe/debian/tests/minitar`
- `safe/debian/libarchive13t64.symbols`
- `safe/debian/libarchive13t64.lintian-overrides`
- `safe/debian/*.install`
- `safe/debian/*.docs`
- `safe/generated/api_inventory.json`
- `safe/generated/link_compat_manifest.json`
- `safe/generated/original_build_contract.json`
- `safe/generated/original_package_metadata.json`
- `safe/generated/original_pkgconfig/libarchive.pc`
- `safe/generated/pkgconfig/libarchive.pc`
- `safe/generated/original_c_build/config.h`
- `.plan/workflow-structure.yaml`
- `.plan/phases/*.md`

## New Outputs
- New `safe/PORT.md` scaffold with all required headings in final order.
- Completed section 1.
- First complete draft of section 5 and an initial command/file log for section 6.

## File Changes
- Create `safe/PORT.md`.
- Do not change any other file unless `./scripts/render-pkg-config.sh --check` proves that `safe/generated/pkgconfig/libarchive.pc` is stale, in which case refresh that tracked artifact in place.

## Implementation Details
- Describe the crate topology exactly as reported by `cargo metadata`: one Cargo package whose manifest also acts as the one-member workspace root, no additional workspace members, no Cargo `[features]` table, one library target, and the concrete integration-test targets.
- Explain the Rust module structure using `safe/src/lib.rs:3-29`, `safe/src/ffi/mod.rs:1-32`, `safe/src/ffi/bootstrap.rs:1-23`, and the checked-in `safe/src/ffi/archive_*.rs` modules, including which files define the public C ABI and API declarations versus the internal implementation state.
- Explain the handle and data-flow boundary using `safe/src/common/state.rs:466-555` for archive-handle allocation, `safe/src/common/state.rs:634-720` for close and free behavior, `safe/src/common/backend.rs:46-426` for the backend vtable, and `safe/src/common/api.rs:18-42` for the variadic error shim initialization.
- Explain the build pipeline using `safe/build.rs:377-565`: generated `backend_symbol_prefix.h`, generated `backend_linked.rs`, generated `version.rs`, vendored backend C compilation from `safe/c_src/libarchive`, static variadic shim compilation from `safe/c_shims/archive_set_error.c`, and Linux `cdylib` link arguments for `--export-dynamic-symbol=archive_set_error`, `--version-script`, and SONAME.
- Explain the packaging and build glue using `safe/debian/rules:11-73`, `safe/scripts/build-c-frontends.sh`, and `safe/scripts/render-pkg-config.sh`, including the preserved `safe/generated/original_c_build/config.h` contract, C frontend builds for `bsdcat`, `bsdcpio`, `bsdtar`, and `bsdunzip`, staged pkg-config rendering, and manual shared-library install and symlink creation under `debian/tmp/usr/lib/*`.
- State explicitly that there is no checked-in `cbindgen.toml`, no checked-in bindgen workflow, and no Cargo feature matrix to describe. Record those absences rather than leaving them implicit.
- Build section 5 as four explicit subsections: direct Cargo dependencies from `safe/Cargo.toml` with exact resolved versions from `cargo tree`; linked system libraries from `safe/build.rs`; build-time tools and scripts required by the port and its verification harnesses; and Debian build, runtime, and autopkgtest dependencies from `safe/debian/control` and `safe/debian/tests/control`.
- For direct Cargo dependencies, inspect the local crate sources under `${CARGO_HOME:-$HOME/.cargo}/registry/src/...` and classify each as `unsafe-heavy`, `contains some unsafe`, or `forbids unsafe`, with a one-line acceptability justification. Derive exact versions from the live `cargo tree`; at this commit those are `libc v0.2.184`, `cc v1.2.59`, `serde v1.0.228`, `serde_json v1.0.149`, and `toml v0.8.23`.
- If `./scripts/render-pkg-config.sh --check` reports that `safe/generated/pkgconfig/libarchive.pc` is stale because the checkout path changed, refresh that exact file in place and describe it as a path-sensitive build-tree artifact rather than a packaging-semantics regression.
- Include a short directory map covering `src/`, `include/`, `c_shims/`, `c_src/`, `generated/`, `abi/`, `scripts/`, `tests/`, `pkgconfig/`, `debian/`, `examples/`, `doc/man/`, `config/`, and `tools/`.

## Verification Phases

### `check_port_doc_architecture`
- `phase_id`: `check_port_doc_architecture`
- `type`: `check`
- `bounce_target`: `impl_port_doc_architecture`
- `purpose`: prove that section 1, the dependency inventory in section 5, and the command-log scaffold in section 6 match the live crate, build, generated-artifact, and packaging state.
- `commands`:
```bash
cargo metadata --format-version 1 --manifest-path safe/Cargo.toml --no-deps
cargo tree --manifest-path safe/Cargo.toml -e normal,build,dev
cargo build --manifest-path safe/Cargo.toml --release
cd safe && ./scripts/check-abi.sh --inventory-only
cd safe && ./scripts/render-pkg-config.sh --check
readelf --dyn-syms --wide safe/target/release/libarchive.so | rg ' archive_'
```
- `review_checks`:
  - Verify that `safe/PORT.md` exists after the implement phase and already contains the six required headings in order.
  - Verify that section 5 is not limited to Cargo crates and explicitly includes linked system libraries, build-time tools and scripts, and Debian packaging dependencies.
  - Verify that each direct Cargo dependency is listed with exact resolved version, dependency kind, one-line purpose, and unsafe posture.
  - Verify that section 1 explicitly states that `safe/Cargo.toml` has no `[features]` table and that the tree has no checked-in `cbindgen` or `bindgen` configuration.

## Success Criteria
- `safe/PORT.md` exists after this phase and already contains the six required headings in order.
- Section 1 accurately describes the one-package workspace layout, the single library target and concrete integration-test targets, the public ABI boundary, the internal backend boundary, the build pipeline, the packaging flow, and the explicit absences of Cargo features and checked-in `cbindgen` or `bindgen` configuration.
- Section 5 inventories direct Cargo dependencies, linked system libraries, build-time tools and scripts, and Debian build, runtime, and autopkgtest dependencies using current repository evidence.
- The dependency table in section 5 reconciles with live `cargo metadata` and `cargo tree` output.
- The build and packaging prose matches the built `safe/target/release/libarchive.so`, the Debian rules and install files, and the path-sensitive pkg-config contract.
- Section 5 explicitly enumerates Debian build dependencies from `safe/debian/control:5-23`, package dependencies from `safe/debian/control:34-44`, `safe/debian/control:88-89`, and `safe/debian/control:127-129`, plus autopkgtest dependencies from `safe/debian/tests/control:1-2`.

## Git Commit Requirement
The implementer must commit work to git before yielding.
