# Phase 4: Final Reconciliation

## Phase Name
Final PORT.md Reconciliation and Commit

## Implement Phase ID
`impl_port_doc_finalize`

## Preexisting Inputs
- `safe/PORT.md` from phases 1-3
- `safe/Cargo.toml`
- `safe/Cargo.lock`
- `safe/build.rs`
- `safe/src/**`
- `safe/tests/**`
- `safe/include/archive.h`
- `safe/include/archive_entry.h`
- `safe/c_shims/archive_set_error.c`
- `safe/scripts/render-pkg-config.sh`
- `safe/generated/api_inventory.json`
- `safe/generated/cve_matrix.json`
- `safe/generated/link_compat_manifest.json`
- `safe/generated/original_build_contract.json`
- `safe/generated/original_package_metadata.json`
- `safe/generated/original_pkgconfig/libarchive.pc`
- `safe/generated/pkgconfig/libarchive.pc`
- `safe/generated/original_c_build/config.h`
- `safe/generated/rust_test_manifest.json`
- `safe/generated/test_manifest.json`
- `safe/abi/exported_symbols.txt`
- `safe/abi/libarchive.map`
- `safe/abi/original_exported_symbols.txt`
- `safe/abi/original_version_info.txt`
- `safe/debian/control`
- `safe/debian/rules`
- `safe/debian/tests/control`
- `safe/debian/README.Debian`
- `safe/target/release/libarchive.so`
- `dependents.json`
- `relevant_cves.json`
- `test-original.sh`

## New Outputs
- Final `safe/PORT.md`.
- One git commit containing the documentation pass and only any evidence-driven incidental fixes required to keep the document truthful.

## File Changes
- Final polish of `safe/PORT.md`.
- Optional in-place fixes to stale evidence artifacts, especially `safe/generated/pkgconfig/libarchive.pc`, only if a checker proves they drifted and the document would otherwise be false.

## Implementation Details
- Reconcile repeated facts across sections so that linked libraries, dependency versions, unsafe counts, test-manifest counts, and package names do not drift.
- Section 6 must list both files consulted and commands executed, including commands that were skipped or unavailable.
- Verify that every cited path exists, every cited symbol is discoverable with `rg`, `nm`, or `readelf`, every dependency named in section 5 appears in `safe/Cargo.toml` or `safe/debian/control`, and section 2 still accounts for the full `rg -n '\bunsafe\b' safe` result modulo comments, strings, and vendored C.
- If a final check exposes stale checked-in evidence, update the existing artifact in place and mention the correction in the commit message.
- Do not stage, delete, or otherwise touch the unrelated untracked `safe/tools/__pycache__/` directory.
- Commit the final result in one commit whose message summarizes the documentation pass, for example `docs: add authoritative PORT.md for safe libarchive`.

## Verification Phases

### `check_port_doc_finalize`
- `phase_id`: `check_port_doc_finalize`
- `type`: `check`
- `bounce_target`: `impl_port_doc_finalize`
- `purpose`: prove that the finished `safe/PORT.md` is internally consistent, path-valid, symbol-valid, dependency-valid, and committed in one documentation commit.
- `commands`:
```bash
cargo metadata --format-version 1 --manifest-path safe/Cargo.toml --no-deps
cargo tree --manifest-path safe/Cargo.toml -e normal,build,dev
cd safe && ./scripts/render-pkg-config.sh --check
python3 - <<'PY'
from pathlib import Path
import re

text = Path("safe/PORT.md").read_text(encoding="utf-8")
candidates = sorted(set(re.findall(r'(?:safe|original|dependents\.json|relevant_cves\.json|test-original\.sh)[A-Za-z0-9_./-]*', text)))
missing = [path for path in candidates if not Path(path).exists()]
if missing:
    raise SystemExit("missing referenced paths: " + ", ".join(missing))
print(f"validated {len(candidates)} referenced repo paths")
PY
rg -n '\bunsafe\b' safe
git diff --check
git status --short
```
- `review_checks`:
  - Verify manually that the six required sections appear in the exact requested order.
  - Verify manually that every dependency named in section 5 appears in `safe/Cargo.toml` or `safe/debian/control`.
  - Verify manually that section 2 accounts for every executable or declarative `rg -n '\bunsafe\b' safe` hit.
  - Verify manually that section 3 names providers, symbols, purposes, and plausible safe-Rust replacement paths.
  - Verify manually that section 4 explicitly covers bit-for-bit evidence and gaps, not just generic compatibility.
  - Verify manually that section 6 lists the real commands and notes unavailable tools such as `cargo-geiger` if absent.

## Success Criteria
- The finished `safe/PORT.md` is internally consistent, cites only extant paths, and keeps the six required sections in the exact requested order.
- Every named dependency in section 5 is traceable to `safe/Cargo.toml` or `safe/debian/control`.
- Every claimed provider or symbol in section 3 is discoverable through checked-in sources or the built library.
- The final state is captured in one git commit, with only evidence-driven incidental artifact refreshes included alongside the documentation update.

## Git Commit Requirement
The implementer must commit work to git before yielding, and the final state must be captured in one documentation commit.
