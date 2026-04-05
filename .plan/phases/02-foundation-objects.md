# Phase 2: Foundation Objects

## Phase Name
Foundational Objects, Error Semantics, and Pure-Logic APIs

## Implement Phase ID
`impl_foundation_objects`

## Preexisting Inputs
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
- `safe/abi/libarchive.map`
- `safe/abi/original_exported_symbols.txt`
- `safe/generated/api_inventory.json`
- `safe/generated/original_build_contract.json`
- `safe/generated/original_c_build/**`
- `safe/generated/test_manifest.json`
- `safe/config/libarchive_test_phase_groups.json`
- `safe/scripts/run-upstream-c-tests.sh`
- `original/libarchive-3.7.2/libarchive/archive_private.h`
- `original/libarchive-3.7.2/libarchive/archive_entry_private.h`
- `original/libarchive-3.7.2/libarchive/archive_entry.c`
- `original/libarchive-3.7.2/libarchive/archive_entry_sparse.c`
- `original/libarchive-3.7.2/libarchive/archive_entry_stat.c`
- `original/libarchive-3.7.2/libarchive/archive_entry_xattr.c`
- `original/libarchive-3.7.2/libarchive/archive_entry_link_resolver.c`
- `original/libarchive-3.7.2/libarchive/archive_acl.c`
- `original/libarchive-3.7.2/libarchive/archive_match.c`
- `original/libarchive-3.7.2/libarchive/archive_pathmatch.c`
- `original/libarchive-3.7.2/libarchive/archive_getdate.c`
- `original/libarchive-3.7.2/libarchive/archive_options.c`
- `original/libarchive-3.7.2/libarchive/archive_pack_dev.c`
- `original/libarchive-3.7.2/libarchive/archive_string.c`
- `original/libarchive-3.7.2/libarchive/archive_string_sprintf.c`
- `original/libarchive-3.7.2/libarchive/test/`

## New Outputs
- `safe/src/common/**`
- `safe/src/entry/**`
- `safe/src/match/**`
- `safe/src/util/**`
- `safe/src/ffi/archive_entry.rs`
- `safe/src/ffi/archive_match.rs`
- `safe/src/ffi/archive_common.rs`
- `safe/tests/support/**`
- `safe/tests/libarchive/foundation/**`

## File Changes
- Add the safe Rust object model for `struct archive`, `struct archive_entry`, ACLs, xattrs, sparse maps, link resolution, and `archive_match`.
- Add FFI exports for the foundational public APIs, including the live `archive_entry_acl_*` entry-ACL functions and any other foundation-side symbols present in the phase-1 ABI inventory.
- Add Rust-native tests for the phase-2 surface and wire the C-oracle group through the shared runner.

## Implementation Details
- Consume the phase-1 generated artifacts in place. Do not regenerate `config.h`, suite `list.h`, the test manifest, or the API inventory in this phase.
- Port the `struct archive` magic/state model from `archive_private.h`, including `ARCHIVE_*_MAGIC`, `ARCHIVE_STATE_*`, error number handling, error strings, and `archive_check_magic` semantics. Callers that use an object in the wrong state must get the same return code class and error behavior expected by the original implementation.
- Port `struct archive_entry` from `archive_entry_private.h` into a safe Rust model with explicit set/unset flags, ACL/xattr/sparse collections, digest slots, symlink type, encryption flags, and lazy `stat` materialization.
- Preserve public pointer-return semantics by caching C strings and wide-string views inside the object. Functions such as `archive_entry_pathname()`, `archive_entry_pathname_w()`, `archive_entry_gname_utf8()`, and related getters must remain valid until the next mutation, matching the original object lifetime behavior.
- Port `archive_match` with pathname pattern logic, time filters, owner filters, unmatched iteration order, and the same error-message precedence used by the original C implementation.
- Port the pure utility logic from `archive_pathmatch`, `archive_getdate`, `archive_options`, `archive_pack_dev`, and the shared string/error helpers that underpin option parsing and path/date behavior.
- Implement the foundational exported surface identified by the phase-1 ABI inventory for the entry/matcher/string subsystem, including the live `archive_entry_acl_*` family and any deprecated compatibility aliases that still appear in the captured original export list. Do not add historical `#MISSING` families such as `archive_acl_*`, `Ppmd8_*`, or any other names absent from `safe/abi/original_exported_symbols.txt`.
- Keep unsafe limited to FFI pointer conversion, `libc::stat` interop, and narrowly scoped raw buffer handling needed to match the C ABI.

## Verification Phases

### `check_foundation_rust_and_c_api`
- Phase ID: `check_foundation_rust_and_c_api`
- Type: `check`
- Bounce Target: `impl_foundation_objects`
- Purpose: validate the file-free foundational object model, error/state logic, `archive_entry`, `archive_match`, the live `archive_entry_acl_*` APIs, and any phase-1-inventoried foundation-side compatibility exports through Rust tests and the upstream C oracle group.
- Commands:
```bash
cd safe
cargo test --test libarchive_entry_api
cargo test --test libarchive_match_api
cargo test --test libarchive_foundation_support
./scripts/run-upstream-c-tests.sh libarchive foundation
```

## Success Criteria
- Unchanged C code can create, mutate, query, and free `archive_entry` and `archive_match` objects through the safe library.
- The C-oracle group `foundation` passes using the `safe/generated/test_manifest.json` inventory produced in phase 1.
- The shared C-oracle runner resolves `config.h`, generated `list.h`, include order, and required defines from `safe/generated/original_build_contract.json` and `safe/generated/original_c_build/**`; it does not regenerate those artifacts.
- The foundational exported surface matches the phase-1 ABI inventory, including the live `archive_entry_acl_*` family and required compatibility aliases, without reintroducing historical `#MISSING` exports.

## Git Commit Requirement
The implementer must commit all phase changes to git before yielding. The commit message must begin with `impl_foundation_objects:`.
