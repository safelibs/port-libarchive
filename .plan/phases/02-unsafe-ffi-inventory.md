# Phase 2: Unsafe and FFI Inventory

## Phase Name
Unsafe Rust Census and Provider-Keyed Foreign Interface Analysis

## Implement Phase ID
`impl_port_doc_unsafe_ffi`

## Preexisting Inputs
- `safe/PORT.md`
- `safe/src/**`
- `safe/src/ffi/**`
- `safe/tests/**`
- `safe/build.rs`
- `safe/c_shims/archive_set_error.c`
- `safe/generated/api_inventory.json`
- `safe/abi/exported_symbols.txt`
- `safe/abi/libarchive.map`
- `safe/target/release/libarchive.so` from phase 1

## New Outputs
- Completed section 2.
- Completed section 3.

## File Changes
- Update `safe/PORT.md` in place.

## Implementation Details
- Start from `rg -n '\bunsafe\b' safe`, `rg -l '\bunsafe\b' ...`, and `rg -l 'extern "C"' ...` so the final document reconciles with the same repository-wide search named in the goal. Split the results into executable or declarative Rust unsafe sites, comments or strings that explain non-site grep hits, and vendored C hits under `safe/c_src/**` that are not Rust unsafe sites but may still matter to section 4 caveats.
- Produce a section-2 table with one row per executable or declarative unsafe site. A single wrapper such as `pub unsafe extern "C" fn ... { ... unsafe { ... } }` contributes multiple rows because the function declaration and the nested block are separate syntactic unsafe sites.
- Each section-2 row must include exact `file:line`, syntactic form, enclosing symbol, purpose bucket, and a one-sentence justification for why the unsafe is needed.
- Do not stop at the handful of files named elsewhere in the plan. Cover every checked-in file returned by the unsafe search, including `safe/src/common/helpers.rs`, `safe/src/disk/mod.rs`, `safe/src/entry/api.rs`, `safe/src/entry/internal.rs`, `safe/src/entry/mod.rs`, `safe/src/match/api.rs`, `safe/src/match/mod.rs`, `safe/src/util/mod.rs`, `safe/src/write/mod.rs`, `safe/src/read/mod.rs`, and every unsafe-bearing integration test under `safe/tests/**`.
- Bucket the unsafe rows at minimum into public ABI entrypoints and callback shims, raw-pointer casting and ownership conversion, backend bridge calls into vendored C, allocator and buffer and `CStr` interop, libc and syscall and descriptor interaction, and test-only FFI setup and helper code.
- Explicitly call out unsafe that is not required by the public libarchive ABI boundary. The inventory must name at least `safe/src/common/backend.rs:46-426` and `safe/src/common/backend.rs:420-421`, `safe/src/common/helpers.rs:59-75`, `safe/src/common/state.rs:426-450`, `safe/src/common/state.rs:558-598`, `safe/src/common/state.rs:634-756`, `safe/src/common/api.rs:18-42`, `safe/src/disk/mod.rs`, `safe/src/match/internal.rs:19-21`, `safe/src/disk/native.rs:93-110` and the syscall-heavy operations that follow, plus unsafe in `safe/src/entry/internal.rs`, `safe/src/util/mod.rs`, `safe/src/write/mod.rs`, and `safe/src/read/mod.rs` that bridges internal state to backend callbacks rather than exposing the public ABI directly.
- Generated `backend_linked.rs` lives under `OUT_DIR` rather than the checked-in tree. When section 2 or 3 needs to discuss those declarations, cite `safe/build.rs`, `safe/src/common/backend.rs`, and built-symbol evidence rather than inventing checked-in file and line references.
- Section 3 must inventory every FFI surface beyond the intended libarchive public ABI boundary, grouped by provider: vendored backend C functions reached through generated `backend_*` declarations from `build.rs` and `common/backend.rs`; `__archive_get_date` from the vendored backend; the static `archive_variadic_shim_*` and `archive_set_error` bridge from `safe/c_shims/archive_set_error.c`; `libacl` symbols from `safe/src/disk/native.rs:93-110`; libc allocator, passwd/group lookup, descriptor, path, time, and filesystem calls reached from Rust code in `safe/src/common/helpers.rs`, `safe/src/disk/mod.rs`, `safe/src/disk/native.rs`, `safe/src/entry/internal.rs`, and any other Rust module that directly calls into libc; and the transitive runtime libraries visible in the built ELF undefined-symbol table, including the compression, hash, XML, and iconv providers actually referenced by the vendored backend rather than only the subset named in `safe/build.rs`.
- For each non-public-boundary FFI provider, section 3 must record exact symbol names, which crate or system library provides them, why the port currently needs them, and a plausible safe-Rust replacement path or an explicit statement that the surface is unavoidable while preserving the C ABI and packaging contract.
- Build the provider inventory by combining source lookups with the live undefined-symbol table from `readelf --dyn-syms --wide safe/target/release/libarchive.so`. Do not stop at a vague library-name list from `safe/build.rs`; map concrete symbols to providers.
- Explicitly exclude the intended public libarchive ABI declarations in `safe/src/ffi/*.rs` from section 3, and say they are the original compatibility boundary rather than remaining FFI beyond the boundary.
- Explain the known grep trap at `safe/build.rs:147` and `safe/build.rs:155`: those hits contain `unsafe extern "C" fn` in build-script string parsing logic and are not executable Rust unsafe sites.
- If `cargo-geiger` is absent, record that in section 6 and do not fabricate its output.

## Verification Phases

### `check_port_doc_unsafe_ffi`
- `phase_id`: `check_port_doc_unsafe_ffi`
- `type`: `check`
- `bounce_target`: `impl_port_doc_unsafe_ffi`
- `purpose`: reconcile sections 2 and 3 line-for-line against the source tree, generated backend linkage, C shim, tests, and built shared object.
- `commands`:
```bash
rg -n '\bunsafe\b' safe
rg -l '\bunsafe\b' safe/src safe/tests safe/build.rs safe/c_shims | sort
rg -l 'extern "C"' safe/src safe/tests safe/build.rs safe/c_shims | sort
rg -n 'unsafe impl|unsafe fn|unsafe extern|extern "C"' safe/src safe/tests safe/build.rs safe/c_shims
readelf --dyn-syms --wide safe/target/release/libarchive.so | awk '$7=="UND" {print $8}' | sed 's/@.*//' | sort -u
if command -v cargo-geiger >/dev/null 2>&1; then
  cargo geiger --manifest-path safe/Cargo.toml
else
  echo 'cargo-geiger: not installed'
fi
nm -D --defined-only safe/target/release/libarchive.so | rg ' archive_| backend_'
```
- `review_checks`:
  - Verify that section 2 contains one row per executable or declarative unsafe site, each with `file:line`, syntactic form, enclosing symbol, grouping bucket, and one-sentence justification.
  - Verify that section 3 groups foreign interfaces by provider and names the exact symbol or symbols involved.
  - Verify that `safe/build.rs:147` and `safe/build.rs:155` are explained as string-literal grep hits, not executable unsafe sites.
  - Verify that any generated `backend_linked.rs` discussion is cited through `safe/build.rs` and built-symbol evidence rather than invented checked-in line references.

## Success Criteria
- The section-2 table reconciles with `rg -n '\bunsafe\b' safe` modulo comments, strings, and vendored C, with one row per syntactic unsafe site.
- Every symbol named in section 3 is discoverable with `rg`, `nm`, or `readelf`.
- Section 3 inventories every remaining non-public-boundary foreign interface by provider, with exact symbols, provider identity, purpose, and a plausible safe-Rust replacement path or an explicit statement that none exists.
- Section 3 does not stop at vague summaries such as "uses libc" or "links zlib"; it names concrete functions or symbol families and reconciles them with the built library's undefined-symbol table.

## Git Commit Requirement
The implementer must commit work to git before yielding.
