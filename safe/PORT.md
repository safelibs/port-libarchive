# PORT.md

## 1. High-level architecture

`cargo metadata --format-version 1 --manifest-path safe/Cargo.toml --no-deps` reports a single Cargo package, `archive v3.7.2`, whose manifest at `safe/Cargo.toml` is also the one-member workspace root. There are no additional workspace members, no checked-in Cargo binaries, and no Cargo `[features]` table at all: `safe/Cargo.toml:1-20` defines only one library target plus build and dev dependencies, and the metadata `features` map is empty.

The Cargo target matrix is correspondingly small. There is one library target, `archive`, from `safe/src/lib.rs`, built as `cdylib`, `staticlib`, and `rlib`. The concrete integration-test targets reported by `cargo metadata` are `advanced_formats`, `cat`, `cpio`, `cve_regressions`, `libarchive`, `libarchive_entry_api`, `libarchive_foundation_support`, `libarchive_match_api`, `libarchive_read_core`, `libarchive_read_disk`, `libarchive_read_mainstream`, `libarchive_write_core`, `libarchive_write_disk`, `tar`, and `unzip`. The command-line tools shipped in Debian are not Cargo targets; they are vendored C frontends built separately by `safe/scripts/build-c-frontends.sh`.

### Module structure and ABI boundary

`safe/src/lib.rs:3-29` exposes the crate as a small set of top-level domains: `common`, `algorithms`, `disk`, `entry`, `ffi`, `match`, `read`, `util`, and `write`, plus a generated `version.rs` include that is re-exported as the libarchive version and SONAME constants. The public C-facing type layer starts in `safe/src/ffi/mod.rs:1-32`, which declares opaque `#[repr(C)]` carrier types for `archive`, `archive_entry`, `archive_acl`, and `archive_entry_linkresolver`, then wires in the checked-in `archive_*` declaration modules.

Those checked-in `archive_*.rs` files under `safe/src/ffi/` are the Rust declaration mirror of the libarchive C ABI and API, not the internal implementation state. They define public constants, callback typedefs, opaque handle references, and `extern "C"` declarations corresponding to the installed headers in `safe/include/archive.h` and `safe/include/archive_entry.h`. `safe/src/ffi/archive_common.rs` also carries the variadic declaration for `archive_set_error(...)`, which matters because the Rust port exports that symbol for C callers while translating the actual variadic handling through a static shim.

`safe/src/ffi/bootstrap.rs:1-23` is intentionally narrow: it defines the four `archive_*_new` constructors and delegates the real allocation work to `common::state::alloc_archive`. Everything behind those opaque pointers lives elsewhere. Internal state and ownership live in `safe/src/common/state.rs`; the vendored-backend function table and generated symbol linkage live in `safe/src/common/backend.rs`; the cross-cutting C API helpers, version helpers, and error-shim bootstrap live in `safe/src/common/api.rs`; the read/write API implementations live in `safe/src/read/mod.rs` and `safe/src/write/mod.rs`; and the native disk-path implementation lives in `safe/src/disk/native.rs`.

### Handle boundary and data flow

The raw `*mut archive` values returned to C callers are really boxed Rust handle structs chosen by `safe/src/common/state.rs:466-555`. `alloc_archive` creates a different layout for `Read`, `Write`, `ReadDisk`, `WriteDisk`, and `Match`, but every handle embeds an `ArchiveCore` plus mode-specific callback, option, entry, and traversal state. Recovery is magic-number based: helper functions such as `read_from_archive`, `write_from_archive`, `read_disk_from_archive`, `write_disk_from_archive`, and `backend_archive` (`safe/src/common/state.rs:430-464`) cast the opaque public pointer back to the right concrete Rust handle after consulting the stored archive magic.

The internal/backend boundary is explicit. `safe/src/common/backend.rs:46-426` defines a single `Api` vtable struct whose fields are typed function pointers for every vendored backend symbol the Rust code may call. `build.rs` generates `backend_linked.rs`, and `common/backend.rs:423-426` includes it to populate a static `linked_api()` table that points at `backend_archive_*` symbols. That prefixing step is important: the vendored C backend is compiled with a generated `backend_symbol_prefix.h` so its public-looking `archive_*` names are renamed to `backend_archive_*`, which keeps the port’s exported `archive_*` ABI from colliding with the bundled implementation copy.

Error propagation across the Rust/C seam is also centralized. `safe/src/common/api.rs:18-42` links a static library named `archive_variadic_shim`, registers `archive_set_error_bridge` exactly once through a `Once`, and teaches the C shim to forward `archive_set_error(...)` calls into `set_error_option` on the Rust `ArchiveCore`. That lets vendored C code keep calling the original variadic entrypoint while the Rust side retains ownership of the authoritative per-handle error state.

At runtime, public entrypoints validate the handle and state with `archive_magic` and `archive_check_magic`, dispatch either into the vendored backend through `backend_api()` or into native Rust disk helpers, then mirror backend state back into `ArchiveCore` with `sync_backend_core`. Teardown follows the same split. `safe/src/common/state.rs:634-692` frees per-kind resources by dropping Rust boxes, freeing temporary entry objects, invoking backend `archive_*_free` hooks, and running lookup cleanups. `safe/src/common/state.rs:694-756` performs close semantics first, calling either backend `archive_read_close` or `archive_write_close` or the native disk close helpers before marking the handle closed and clearing `backend_opened`.

### Build pipeline

The build is driven entirely by a handwritten `safe/build.rs`; there is no checked-in `cbindgen.toml`, no checked-in bindgen workflow, and no generated-header pipeline beyond what `build.rs` itself emits into `OUT_DIR`. The script first reads `safe/generated/original_build_contract.json`, `safe/generated/original_package_metadata.json`, and `safe/generated/original_c_build/config.h` to assert that the preserved upstream-oracle build contract still matches the Cargo package version and expected SONAME (`safe/build.rs:442-520`).

From there the script generates three key artifacts:

- `backend_symbol_prefix.h` by scanning the vendored upstream headers and prefixing every public `archive_*` declaration with `backend_` (`safe/build.rs:67-110`, `377-393`).
- `backend_linked.rs` by parsing `safe/src/common/backend.rs` and emitting `extern "C"` declarations plus the static `linked_api()` table (`safe/build.rs:112-252`, `377-393`).
- `version.rs` by deriving `LIBARCHIVE_VERSION_NUMBER`, `LIBARCHIVE_VERSION_STRING`, `LIBARCHIVE_SONAME`, and related constants from the preserved package metadata (`safe/build.rs:455-545`), then re-exporting them through `safe/src/lib.rs:21-29`.

Compilation is mixed Rust and C. `safe/build.rs:547-555` compiles `safe/c_shims/archive_set_error.c` into the static `archive_variadic_shim` helper, then `safe/build.rs:377-432` compiles the vendored backend from `safe/c_src/libarchive` with `cc::Build`, `HAVE_CONFIG_H`, `LIBARCHIVE_STATIC`, the preserved `safe/generated/original_c_build/config.h`, `/usr/include/libxml2`, and the generated prefix header forced via `-include`. The build script then links the resulting Rust `cdylib`/`staticlib` against `acl`, `bz2`, `z`, `lzma`, `zstd`, `lz4`, `nettle`, and `xml2`.

Linux `cdylib` linkage is tightened to match the packaged shared object contract. `safe/build.rs:557-565` adds `-Wl,--export-dynamic-symbol=archive_set_error` so the variadic shim target remains visible, `-Wl,--version-script=safe/abi/libarchive.map` so the exported symbol set matches the recorded ABI map, and `-Wl,-soname,libarchive.so.13` so `safe/target/release/libarchive.so` has the expected runtime SONAME.

### Packaging and build glue

Debian packaging is intentionally manual around the library and the C frontends. `safe/debian/rules:22-31` leaves configure as a no-op because the port consumes the preserved `safe/generated/original_c_build/config.h` snapshot instead of rerunning autotools, then `override_dh_auto_build` performs three explicit stages: `cargo build --release`, `./scripts/build-c-frontends.sh --suite all`, and `./scripts/render-pkg-config.sh --mode staged-sysroot --prefix /usr`.

`safe/scripts/build-c-frontends.sh` is the bridge for the non-Cargo tools. It requires the preserved `config.h`, the release `libarchive.so`, `safe/generated/original_build_contract.json`, and `safe/generated/original_package_metadata.json`; extracts the extra link libraries and recorded runtime library basenames with `python3`; creates temporary symlinks in `target/release`; and then compiles vendored C frontends for `bsdcat`, `bsdcpio`, `bsdtar`, and `bsdunzip` against the just-built safe library.

`safe/scripts/render-pkg-config.sh` is the pkg-config contract renderer. In `build-tree` mode it bakes the local checkout path into `prefix=`, `libdir=${exec_prefix}/target/release`, and `includedir=${prefix}/include`; in `staged-sysroot` mode it renders a package-installable `libarchive.pc` rooted at `/usr`. In both modes it preserves the non-path fields from `safe/generated/original_pkgconfig/libarchive.pc`. Because of that design, `safe/generated/pkgconfig/libarchive.pc` is path-sensitive: if the checkout path changes, the tracked build-tree artifact legitimately changes only in `prefix=`-family fields.

Installation is also explicit. `safe/debian/rules:40-60` creates `debian/tmp/usr/lib/$(DEB_HOST_MULTIARCH)`, installs `safe/target/release/libarchive.so` as `libarchive.so.13.7.2`, creates the `libarchive.so.13` and `libarchive.so` symlinks by hand, copies `libarchive.a`, installs the checked-in headers, drops the staged pkg-config file into `pkgconfig/`, installs the four C frontends, and copies the manpages. The package split is then finished by `safe/debian/libarchive13t64.install`, `safe/debian/libarchive-dev.install`, and `safe/debian/libarchive-tools.install`, which carve the staged tree into the runtime library, development package, and tool package.

### Explicit absences

- No additional workspace members: `cargo metadata` reports only `archive v3.7.2`.
- No Cargo feature matrix: `safe/Cargo.toml` has no `[features]` table, and metadata reports `features: {}`.
- No checked-in `cbindgen.toml`.
- No checked-in bindgen script, workflow, or generated Rust-from-header pipeline.

### Short directory map

- `safe/src/`: Rust library sources, including public ABI shims and the internal read/write/disk/common implementation.
- `safe/include/`: checked-in `archive.h` and `archive_entry.h` installed for C consumers.
- `safe/c_shims/`: small C shim layer for variadic `archive_set_error`.
- `safe/c_src/`: vendored upstream C backend, frontend programs, and shared frontend helpers.
- `safe/generated/`: preserved or rendered oracle artifacts, manifests, captured `config.h`, and pkg-config outputs.
- `safe/abi/`: exported-symbol inventories and the linker version script.
- `safe/scripts/`: build, compatibility, ABI, and test harness shell scripts.
- `safe/tests/`: Rust integration tests plus fixture and upstream-manifest helpers.
- `safe/pkgconfig/`: `libarchive.pc` template used for build-tree and staged rendering.
- `safe/debian/`: Debian control files, rules, package split manifests, symbols, overrides, and autopkgtests.
- `safe/examples/`: example consumers used by the source-compat harness.
- `safe/doc/man/`: installed manual pages.
- `safe/config/`: checked-in test phase grouping data.
- `safe/tools/`: local generators for API, link-compat, and test-manifest artifacts.

## 2. Where the unsafe Rust lives

This section is intentionally deferred to phase 2 of the documentation pass. It will be replaced in place with a file:line inventory of every executable or declarative `unsafe` site in `safe/`, grouped by purpose and cross-checked against `rg -n '\\bunsafe\\b' safe`.

## 3. Remaining unsafe FFI beyond the original ABI/API boundary

This section is intentionally deferred to phase 2 of the documentation pass. It will be replaced in place with a provider-keyed inventory of non-public-boundary FFI surfaces, concrete symbol names, why each is still needed, and plausible pure-Rust replacement paths where they exist.

## 4. Remaining issues

This section is intentionally deferred to phase `impl_port_doc_remaining_issues`. It will be replaced in place with the remaining-issues, coverage, packaging-caveat, dependent-coverage, and CVE-scope reconciliation grounded in the checked-in manifests and later verification runs.

## 5. Dependencies and other libraries used

This section is a phase-1 baseline. It inventories the current direct Cargo dependencies, the system libraries explicitly linked by `safe/build.rs`, the build/verification tools the port relies on, and the Debian packaging dependencies checked into `safe/debian/`.

### Direct Cargo dependencies from `safe/Cargo.toml`

`cargo tree --manifest-path safe/Cargo.toml -e normal,build,dev` shows exactly five direct dependencies at this commit:

| Crate | Kind | Resolved version | Current purpose in this tree | Unsafe posture from local crate source | Acceptability |
| --- | --- | --- | --- | --- | --- |
| `libc` | normal | `0.2.184` | Provides C ABI scalar/types such as `FILE`, `stat`, `wchar_t`, `mode_t`, `size_t`, and syscall-facing constants used throughout `safe/src/ffi/**`, `safe/src/common/backend.rs`, and `safe/src/disk/native.rs`. | `unsafe\-heavy` | This port preserves a C ABI and explicit libc/syscall interop, so raw libc bindings are unavoidable and confined to the ABI/OS boundary. |
| `cc` | build | `1.2.59` | Build-script glue used by `safe/build.rs` to compile the vendored backend C sources under `safe/c_src/libarchive` and the static variadic shim `safe/c_shims/archive_set_error.c`. | `contains some unsafe` | The unsafe is build-script internal (`OnceLock`, fd/jobserver helpers) and does not ship in the runtime library; the crate is the standard Cargo-side way to compile bundled C. |
| `serde` | dev | `1.0.228` | Derive support for the test-manifest structs in `safe/tests/support/fixtures.rs` and `safe/tests/support/upstream.rs`. | `contains some unsafe` | Dev-only parsing support, not linked into the shipped library or CLI frontends. |
| `serde_json` | dev | `1.0.149` | Parses generated JSON manifests and security/test metadata in `safe/tests/support/upstream.rs` and `safe/tests/libarchive/security/mod.rs`. | `contains some unsafe` | Dev-only JSON parsing, with no runtime impact on the packaged library. |
| `toml` | dev | `0.8.23` | Parses `safe/tests/fixtures-manifest.toml` in `safe/tests/support/fixtures.rs`. | `forbids unsafe` | Dev-only manifest parsing with `#![forbid(unsafe\_code)]`, so it does not widen the unsafe surface. |

Dependency-posture evidence came from the local registry sources under `${CARGO_HOME:-$HOME/.cargo}/registry/src/...`: `toml-0.8.23/src/lib.rs` explicitly declares `#![forbid(unsafe\_code)]`; `serde-1.0.228`, `serde_json-1.0.149`, and `cc-1.2.59` contain a small number of targeted `unsafe` sites; and `libc-0.2.184` is, by design, a raw FFI binding crate with extensive `unsafe` declarations.

### Linked system libraries from `safe/build.rs`

`safe/build.rs:405-432` and `safe/build.rs:547-565` hard-wire the following linked system libraries for the Rust `cdylib` and the vendored backend:

| Library | Evidence | Why the port links it |
| --- | --- | --- |
| `acl` | `safe/build.rs:429-431`; `safe/src/disk/native.rs:93-110` | POSIX ACL read/write support for the read-disk and write-disk paths. |
| `bz2` | `safe/build.rs:429-431` | bzip2 compression filter support preserved from upstream libarchive. |
| `z` | `safe/build.rs:429-431` | zlib-backed gzip/zip support preserved from upstream libarchive. |
| `lzma` | `safe/build.rs:429-431` | xz/lzma filter support preserved from upstream libarchive. |
| `zstd` | `safe/build.rs:429-431` | zstd filter support preserved from upstream libarchive. |
| `lz4` | `safe/build.rs:429-431` | lz4 filter support preserved from upstream libarchive. |
| `nettle` | `safe/build.rs:429-431` | upstream-compatible crypto/hash support expected by the vendored backend build contract. |
| `xml2` | `safe/build.rs:412-413`, `429-431` | XML-dependent backend functionality; the build also requires `/usr/include/libxml2`. |

The build script also asserts that the preserved upstream build contract still contains the expected original private link set (`-lnettle`, `-lacl`, `-llzma`, `-lzstd`, `-llz4`, `-lbz2`, `-lz`) before compiling anything. Linux shared-library linkage is then constrained further with the version script, SONAME, and explicit `archive_set_error` export so the resulting `safe/target/release/libarchive.so` matches the packaging contract.

### Build-time tools and scripts required by the port and its verification harnesses

Core toolchain and helpers:

- `cargo` and `rustc`: build the one `archive` package and run the handwritten `safe/build.rs`.
- Host C compiler `${CC:-cc}` plus the system linker: compile the vendored backend and the four C frontends.
- `python3`: required by `safe/scripts/build-c-frontends.sh`, `safe/scripts/render-pkg-config.sh`, `safe/scripts/check-abi.sh`, and `safe/scripts/check-source-compat.sh` for JSON parsing and pkg-config/ABI rendering or comparison.
- `pkg-config` or `pkgconf`: used by `safe/scripts/check-source-compat.sh` and listed in both Debian build dependencies and autopkgtest dependencies.
- `readelf`: used by `safe/scripts/check-abi.sh` and by the architecture-phase verifier to inspect exported symbols.
- `cmp` and `diff`: required by `safe/scripts/render-pkg-config.sh --check` to detect path-only drift versus semantic contract drift.
- Standard POSIX shell utilities such as `install`, `ln`, `find`, `grep`, `mktemp`, and `readlink`: used by `safe/debian/rules` and the helper scripts.

Repository scripts in this workflow:

- `safe/scripts/build-c-frontends.sh`: builds `bsdcat`, `bsdcpio`, `bsdtar`, and `bsdunzip` against the release build tree.
- `safe/scripts/render-pkg-config.sh`: renders `safe/generated/pkgconfig/libarchive.pc` for the build tree or a staged sysroot.
- `safe/scripts/check-abi.sh`: validates the recorded ABI inventory and, in stricter modes, the live shared object.
- `safe/scripts/check-source-compat.sh`: rebuilds the build-tree pkg-config file, compiles the example consumers, and exercises `minitar`/`untar`.
- `safe/scripts/check-link-compat.sh`, `safe/scripts/check-rust-test-coverage.sh`, `safe/scripts/check-i686-cve.sh`, `safe/scripts/run-upstream-c-tests.sh`, and `safe/scripts/run-debian-minitar.sh`: later-phase verification harnesses for link compatibility, test coverage, CVE regression scope, upstream C tests, and Debian autopkgtest behavior.

### Debian build, runtime, and autopkgtest dependencies

Checked-in Debian source build dependencies from `safe/debian/control:5-23`:

- `debhelper-compat (= 13)`
- `cargo:native`
- `dpkg-build-api (= 1)`
- `dpkg-dev (>= 1.22.5)`
- `dh-package-notes`
- `pkgconf`
- `python3`
- `libbz2-dev`
- `liblz4-dev`
- `liblzma-dev`
- `libxml2-dev`
- `libzstd-dev`
- `zlib1g-dev`
- `libacl1-dev [!hurd-any]`
- `libext2fs-dev`
- `sharutils <!nocheck>`
- `rustc:native`
- `nettle-dev`
- `locales <!nocheck> | locales-all <!nocheck>`

Checked-in binary package dependencies from `safe/debian/control:34-44`, `88-89`, and `127-129`:

- `libarchive-dev`: `libarchive13t64 (= ${binary:Version})`, `libbz2-dev`, `liblz4-dev`, `liblzma-dev`, `libxml2-dev`, `libzstd-dev`, `zlib1g-dev`, `libacl1-dev [!hurd-any]`, `libext2fs-dev`, `nettle-dev`, `${misc:Depends}`.
- `libarchive13t64`: `${shlibs:Depends}`, `${misc:Depends}`; `Suggests: lrzip`.
- `libarchive-tools`: `libarchive13t64 (= ${binary:Version})`, `${shlibs:Depends}`, `${misc:Depends}`.

The `${shlibs:Depends}` placeholder is intentional repository evidence: the exact runtime expansion is produced by Debian tooling from the linked shared-library dependencies, not hard-coded in the tree. Package split manifests under `safe/debian/*.install` align with the manual install layout from `safe/debian/rules`.

Autopkgtest dependency from `safe/debian/tests/control:1-2`:

- Test stanza: `Tests: minitar`
- Runtime test dependencies: `build-essential`, `file`, `libarchive-dev`, `pkg-config`

## 6. How this document was produced

This is the phase-1 reproducibility log. Later phases will extend it with the full unsafe census, remaining-issues checks, and any additional unavailable-tool notes.

### Files consulted in this phase

- Planning inputs: `.plan/goal.md`, `.plan/plan.md`, `.plan/workflow-structure.yaml`, and `.plan/phases/01-architecture-baseline.md` through `.plan/phases/04-final-reconciliation.md`.
- Crate/build inputs: `safe/Cargo.toml`, `safe/Cargo.lock`, `safe/build.rs`.
- Rust architecture inputs: `safe/src/lib.rs`, `safe/src/ffi/mod.rs`, `safe/src/ffi/bootstrap.rs`, `safe/src/ffi/archive_common.rs`, `safe/src/ffi/archive_entry.rs`, `safe/src/ffi/archive_match.rs`, `safe/src/ffi/archive_options.rs`, `safe/src/ffi/archive_read.rs`, `safe/src/ffi/archive_read_disk.rs`, `safe/src/ffi/archive_write.rs`, `safe/src/ffi/archive_write_disk.rs`, `safe/src/common/api.rs`, `safe/src/common/backend.rs`, `safe/src/common/state.rs`, `safe/src/read/mod.rs`, `safe/src/write/mod.rs`, `safe/src/disk/native.rs`.
- Packaging/build glue inputs: `safe/include/archive.h`, `safe/include/archive_entry.h`, `safe/c_shims/archive_set_error.c`, `safe/scripts/build-c-frontends.sh`, `safe/scripts/render-pkg-config.sh`, `safe/scripts/check-abi.sh`, `safe/scripts/check-source-compat.sh`, `safe/pkgconfig/libarchive.pc.in`, `safe/debian/control`, `safe/debian/rules`, `safe/debian/README.Debian`, `safe/debian/tests/control`, `safe/debian/*.install`, `safe/debian/*.docs`.
- Captured-oracle inputs: `safe/generated/original_build_contract.json`, `safe/generated/original_package_metadata.json`, `safe/generated/original_pkgconfig/libarchive.pc`, `safe/generated/pkgconfig/libarchive.pc`, `safe/generated/original_c_build/config.h`.
- Test/dependency-purpose inputs: `safe/tests/support/fixtures.rs`, `safe/tests/support/upstream.rs`, `safe/tests/libarchive/security/mod.rs`, and `safe/tests/fixtures-manifest.toml`.
- Registry-source inputs for direct dependency classification: local crate sources for `libc-0.2.184`, `cc-1.2.59`, `serde-1.0.228`, `serde_json-1.0.149`, and `toml-0.8.23` under `${CARGO_HOME:-$HOME/.cargo}/registry/src/...`.

### Commands executed in this phase

- Repository and plan discovery:
  - `git status --short`
  - `rg --files .plan safe | sort`
  - `rg -n 'PORT.md|impl_port_doc_architecture|check_port_doc_architecture|six required headings|required headings|Section 1|Section 5|Section 6' .plan safe -S`
  - `rg -n '^## ' .plan/plan.md .plan/plan-before-cleanup.md .plan/phases -S`
- Cargo topology and dependency inspection:
  - `cargo metadata --format-version 1 --manifest-path safe/Cargo.toml --no-deps`
  - `cargo tree --manifest-path safe/Cargo.toml -e normal,build,dev`
  - `cargo tree --manifest-path safe/Cargo.toml -i serde -e normal,build,dev`
  - `cargo tree --manifest-path safe/Cargo.toml -i serde_json -e normal,build,dev`
  - `cargo tree --manifest-path safe/Cargo.toml -i toml -e normal,build,dev`
  - `cargo tree --manifest-path safe/Cargo.toml -i libc -e normal,build,dev`
  - `cargo tree --manifest-path safe/Cargo.toml -i cc -e normal,build,dev`
- File and directory inspection:
  - `nl -ba <file> | sed -n ...` across the files listed above
  - `rg --files safe/src/ffi | sort`
  - `rg -n '\\bserde\\b|serde_json|toml::|Deserialize|Serialize' safe/src safe/tests safe/tools safe/scripts`
  - directory and packaging manifest listings under `safe/src`, `safe/include`, `safe/c_shims`, `safe/c_src`, `safe/generated`, `safe/abi`, `safe/scripts`, `safe/tests`, `safe/pkgconfig`, `safe/debian`, `safe/examples`, `safe/doc/man`, `safe/config`, and `safe/tools`
- Direct-dependency unsafe classification:
  - `find "${CARGO_HOME:-$HOME/.cargo}/registry/src" -name '<crate>-<version>'`
  - `rg -n 'forbid\\(unsafe\\_code\\)|deny\\(unsafe\\_code\\)|allow\\(unsafe\\_code\\)|\\bunsafe\\b' <crate-source>`
  - `nl -ba <crate-source>/src/lib.rs | sed -n ...`
- Pkg-config contract checks:
  - `cd safe && ./scripts/render-pkg-config.sh --check`
  - `cd safe && ./scripts/render-pkg-config.sh --mode build-tree`
  - `git diff -- safe/generated/pkgconfig/libarchive.pc`
- Verifier and sanity-check commands run in this phase:
  - `cargo build --manifest-path safe/Cargo.toml --release`
  - `cd safe && ./scripts/check-abi.sh --inventory-only`
  - `readelf --dyn-syms --wide safe/target/release/libarchive.so | rg ' archive_'`
  - `git diff --check`
  - `python3` snippets to confirm the six required headings are present in order and that every referenced repo path in `safe/PORT.md` exists

### Deferred or not-yet-run items

- The full repo-wide unsafe census and non-public-boundary FFI provider inventory are intentionally deferred to section 2 and section 3 work.
- The remaining-issues validation matrix is intentionally deferred to section 4 work.
- `safe/generated/pkgconfig/libarchive.pc` changed in this phase only because the checked-out repository path changed; the non-path pkg-config contract was preserved.
