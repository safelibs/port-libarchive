#!/usr/bin/env python3
from __future__ import annotations

import json
from collections import Counter, defaultdict
from pathlib import Path


SAFE_ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = SAFE_ROOT / "generated" / "test_manifest.json"
RUST_MANIFEST_PATH = SAFE_ROOT / "generated" / "rust_test_manifest.json"
FIXTURE_MANIFEST_PATH = SAFE_ROOT / "tests" / "fixtures-manifest.toml"
SUITE_CASE_FILES = {
    "libarchive": SAFE_ROOT / "tests" / "libarchive" / "ported_cases.rs",
    "tar": SAFE_ROOT / "tests" / "tar" / "ported_cases.rs",
    "cpio": SAFE_ROOT / "tests" / "cpio" / "ported_cases.rs",
    "cat": SAFE_ROOT / "tests" / "cat" / "ported_cases.rs",
    "unzip": SAFE_ROOT / "tests" / "unzip" / "ported_cases.rs",
}
SUITE_ROOTS = {
    "libarchive": "original/libarchive-3.7.2/libarchive/test",
    "tar": "original/libarchive-3.7.2/tar/test",
    "cpio": "original/libarchive-3.7.2/cpio/test",
    "cat": "original/libarchive-3.7.2/cat/test",
    "unzip": "original/libarchive-3.7.2/unzip/test",
}
FRONTEND_BINARIES = {
    "tar": "bsdtar",
    "cpio": "bsdcpio",
    "cat": "bsdcat",
    "unzip": "bsdunzip",
}


def json_string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def render_case_list(rows: list[dict[str, object]]) -> str:
    body = "\n".join(f"    {row['define_test']}," for row in rows)
    return f"define_ported_tests!(\n{body}\n);\n"


def render_fixture_manifest(rows_by_suite: dict[str, list[dict[str, object]]]) -> str:
    lines: list[str] = [
        "# Generated from safe/generated/test_manifest.json. Do not edit by hand.",
        "schema_version = 1",
        "",
    ]
    for suite, rows in rows_by_suite.items():
        lines.append("[[suite]]")
        lines.append(f'name = {json_string(suite)}')
        lines.append(f'root = {json_string(SUITE_ROOTS[suite])}')
        frontend = FRONTEND_BINARIES.get(suite)
        if frontend is not None:
            lines.append(f'frontend_binary = {json_string(frontend)}')
        lines.append("")
        for row in rows:
            lines.append("  [[suite.case]]")
            lines.append(f'  define_test = {json_string(row["define_test"])}')
            lines.append(f'  source_file = {json_string(row["source_file"])}')
            lines.append(
                f'  phase_group = {json_string(str(row.get("phase_group", "all")))}'
            )
            fixtures = ", ".join(
                json_string(str(fixture)) for fixture in row.get("fixture_refs", [])
            )
            lines.append(f"  fixture_refs = [{fixtures}]")
            lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def main() -> None:
    manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    rows = manifest["rows"]

    rows_by_suite: dict[str, list[dict[str, object]]] = defaultdict(list)
    rust_rows: list[dict[str, object]] = []
    rust_pairs: set[tuple[str, str]] = set()

    for row in rows:
        suite = row["suite"]
        define_test = row["define_test"]
        rust_pair = (suite, define_test)
        if rust_pair in rust_pairs:
            raise SystemExit(f"duplicate Rust test mapping for {suite}:{define_test}")
        rust_pairs.add(rust_pair)

        rows_by_suite[suite].append(row)
        rust_rows.append(
            {
                "suite": suite,
                "define_test": define_test,
                "source_file": row["source_file"],
                "phase_group": row["phase_group"],
                "fixture_refs": row["fixture_refs"],
                "rust_test_target": suite,
                "rust_test_name": define_test,
                "driver_kind": "upstream_c_suite",
                "driver_suite": suite,
                "fixture_manifest_suite": suite,
                "suite_fixture_root": SUITE_ROOTS[suite],
                "frontend_binary": FRONTEND_BINARIES.get(suite),
            }
        )

    rust_manifest = {
        "schema_version": 1,
        "counts": {
            "suite_rows": dict(sorted(Counter(row["suite"] for row in rust_rows).items())),
            "rust_test_targets": dict(
                sorted(Counter(row["rust_test_target"] for row in rust_rows).items())
            ),
            "total_rows": len(rust_rows),
        },
        "rows": rust_rows,
    }

    RUST_MANIFEST_PATH.write_text(
        json.dumps(rust_manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    FIXTURE_MANIFEST_PATH.write_text(
        render_fixture_manifest(rows_by_suite),
        encoding="utf-8",
    )

    for suite, output_path in SUITE_CASE_FILES.items():
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(
            render_case_list(rows_by_suite[suite]),
            encoding="utf-8",
        )


if __name__ == "__main__":
    main()
