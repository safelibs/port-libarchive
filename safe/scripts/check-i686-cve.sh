#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
MATRIX="$ROOT/generated/cve_matrix.json"
TARGET="i686-unknown-linux-gnu"

python3 - "$MATRIX" <<'PY'
import json
import sys
from pathlib import Path

matrix = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
rows = {row["cve_id"]: row for row in matrix["rows"]}
for cve in ("CVE-2026-5121", "CVE-2026-4426"):
    row = rows.get(cve)
    if row is None:
        raise SystemExit(f"missing {cve} in cve_matrix.json")
    if row["verification"] != "./scripts/check-i686-cve.sh":
        raise SystemExit(f"{cve} must point to ./scripts/check-i686-cve.sh")
PY

if ! rustup target list --installed | grep -qx "$TARGET"; then
    rustup target add "$TARGET"
fi

I686_LIBS="$(mktemp -d)"
trap 'rm -rf "$I686_LIBS"' EXIT

ln -sf /lib/i386-linux-gnu/libbz2.so.1 "$I686_LIBS/libbz2.so"
ln -sf /lib/i386-linux-gnu/libz.so.1 "$I686_LIBS/libz.so"
ln -sf /lib/i386-linux-gnu/liblzma.so.5 "$I686_LIBS/liblzma.so"
ln -sf /lib/i386-linux-gnu/libzstd.so.1 "$I686_LIBS/libzstd.so"
ln -sf /lib/i386-linux-gnu/liblz4.so.1 "$I686_LIBS/liblz4.so"
ln -sf /lib/i386-linux-gnu/libnettle.so.8 "$I686_LIBS/libnettle.so"
ln -sf /lib/i386-linux-gnu/libxml2.so.2 "$I686_LIBS/libxml2.so"

cat > "$I686_LIBS/libacl_stub.c" <<'EOF'
#include <sys/types.h>

#define STUB_PTR(name, args) void *name args { return 0; }
#define STUB_INT(name, args) int name args { return -1; }

STUB_PTR(acl_get_fd, (int fd))
STUB_PTR(acl_get_file, (const char *path_p, int type_))
STUB_INT(acl_get_entry, (void *acl, int entry_id, void **entry_p))
STUB_INT(acl_get_tag_type, (void *entry_d, int *tag_type_p))
STUB_PTR(acl_get_qualifier, (void *entry_d))
STUB_INT(acl_get_permset, (void *entry_d, void **permset_p))
STUB_INT(acl_get_perm, (void *permset_d, int perm))
STUB_PTR(acl_init, (int count))
STUB_INT(acl_create_entry, (void **acl_p, void **entry_p))
STUB_INT(acl_set_tag_type, (void *entry_d, int tag_type))
STUB_INT(acl_set_qualifier, (void *entry_d, const void *tag_qualifier_p))
STUB_INT(acl_clear_perms, (void *permset_d))
STUB_INT(acl_add_perm, (void *permset_d, int perm))
STUB_INT(acl_set_fd, (int fd, void *acl))
STUB_INT(acl_set_file, (const char *path_p, int type_, void *acl))
STUB_INT(acl_equiv_mode, (void *acl, mode_t *mode_p))
STUB_INT(acl_free, (void *obj_p))
EOF
cc -m32 -shared "$I686_LIBS/libacl_stub.c" -o "$I686_LIBS/libacl.so"

export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-L native=$I686_LIBS"
export LD_LIBRARY_PATH="$I686_LIBS:/lib/i386-linux-gnu:/usr/lib/i386-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

cd "$ROOT"
cargo test --target "$TARGET" --test cve_regressions -- --exact i686_zisofs_pointer_table_overflow_is_rejected
cargo test --target "$TARGET" --test cve_regressions -- --exact i686_zisofs_block_shift_is_validated
cargo test --target "$TARGET" --test cve_regressions -- --exact i686_zstd_long_window_matches_ubuntu_patch_context
