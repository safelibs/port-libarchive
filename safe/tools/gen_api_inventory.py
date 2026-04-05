#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = ROOT.parent
OUTPUT = ROOT / "generated" / "api_inventory.json"
SYMBOLS_FILE = REPO_ROOT / "original" / "libarchive-3.7.2" / "debian" / "libarchive13t64.symbols"
EXPORTS_FILE = ROOT / "abi" / "original_exported_symbols.txt"
HEADER_PATHS = [
    ROOT / "include" / "archive.h",
    ROOT / "include" / "archive_entry.h",
]


def load_header_declarations() -> dict[str, dict[str, object]]:
    declarations: dict[str, dict[str, object]] = {}
    declaration_re = re.compile(r"__LA_DECL\b.*?\b([A-Za-z_]\w*)\s*\([^;]*\)\s*;", re.S)

    for header_path in HEADER_PATHS:
        current: list[str] = []
        for raw_line in header_path.read_text(encoding="utf-8").splitlines():
            line = raw_line.rstrip()
            if "__LA_DECL" in line or current:
                current.append(line)
                if ";" in line:
                    chunk = "\n".join(current)
                    current.clear()
                    match = declaration_re.search(chunk)
                    if not match:
                        continue
                    symbol = match.group(1)
                    entry = declarations.setdefault(
                        symbol,
                        {
                            "deprecated": False,
                            "headers": [],
                        },
                    )
                    entry["deprecated"] = bool(entry["deprecated"] or "__LA_DEPRECATED" in chunk)
                    entry["headers"].append(str(header_path.relative_to(REPO_ROOT).as_posix()))

    return declarations


def load_live_symbol_file_entries() -> list[str]:
    symbols: list[str] = []
    for raw_line in SYMBOLS_FILE.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line:
            continue
        if line.startswith("#MISSING:"):
            continue
        if line.startswith("* Build-Depends-Package"):
            continue
        if line.startswith("libarchive.so.13 "):
            continue
        if line.startswith("#"):
            continue
        symbol = line.split("@", 1)[0].strip()
        if symbol:
            symbols.append(symbol)
    return symbols


def load_original_exports() -> list[str]:
    return [
        line.strip()
        for line in EXPORTS_FILE.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]


def build_inventory() -> dict[str, object]:
    header_decls = load_header_declarations()
    live_symbols = load_live_symbol_file_entries()
    original_exports = load_original_exports()

    original_export_set = set(original_exports)
    filtered_live_symbols = sorted({symbol for symbol in live_symbols if symbol in original_export_set})
    if set(filtered_live_symbols) != original_export_set:
        missing = sorted(original_export_set.difference(filtered_live_symbols))
        extra = sorted(set(filtered_live_symbols).difference(original_export_set))
        raise SystemExit(
            "filtered debian symbol entries do not match the original export oracle\n"
            f"missing_from_symbol_file={missing}\nextra_in_symbol_file={extra}"
        )

    rows = []
    all_symbols = sorted(original_export_set.union(symbol for symbol in header_decls if symbol in original_export_set))
    for symbol in all_symbols:
        declared = header_decls.get(symbol)
        if declared:
            if declared["deprecated"]:
                classification = "deprecated compatibility alias"
            else:
                classification = "header-declared public API"
        else:
            classification = "live symbol-file-only export that still must remain link-compatible"

        rows.append(
            {
                "symbol": symbol,
                "classification": classification,
                "header_declared": bool(declared),
                "deprecated": bool(declared and declared["deprecated"]),
                "headers": sorted(declared["headers"]) if declared else [],
                "listed_in_debian_symbols": symbol in filtered_live_symbols,
                "present_in_original_exports": True,
            }
        )

    return {
        "schema_version": 1,
        "original_export_count": len(original_exports),
        "rows": rows,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="validate the checked-in inventory")
    args = parser.parse_args()

    data = build_inventory()
    rendered = json.dumps(data, indent=2, sort_keys=True) + "\n"

    if args.check:
        if not OUTPUT.exists():
            raise SystemExit(f"missing generated inventory: {OUTPUT}")
        existing = OUTPUT.read_text(encoding="utf-8")
        if existing != rendered:
            raise SystemExit("safe/generated/api_inventory.json is out of date")
        if data["original_export_count"] != 421:
            raise SystemExit(
                f"expected 421 original exports, found {data['original_export_count']}"
            )
    else:
        OUTPUT.parent.mkdir(parents=True, exist_ok=True)
        OUTPUT.write_text(rendered, encoding="utf-8")

    return 0


if __name__ == "__main__":
    sys.exit(main())
