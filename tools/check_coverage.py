#!/usr/bin/env python3
"""Enforce the roadmap's line-coverage gate: core and HTML crates over 95%.

Reads the JSON report produced by `cargo llvm-cov --json --output-path`,
aggregates line coverage per crate, and exits 1 unless geist4-core (and
geist4-html, if it has any instrumentable lines) reach 95%. The rest of
the workspace is reported for the record but not gated: the binding
crates are pinned by their byte-exact contract suites, and the ext
crates' residual gaps are tracked in TESTING.md.

Usage: check_coverage.py <coverage.json> [min-percent]
"""

import json
import sys


def main() -> int:
    if len(sys.argv) < 2:
        print("usage: check_coverage.py <coverage.json> [min-percent]", file=sys.stderr)
        return 2
    min_pct = float(sys.argv[2]) if len(sys.argv) > 2 else 95.0

    with open(sys.argv[1], "r", encoding="utf-8") as f:
        report = json.load(f)

    per_crate: dict[str, list[int]] = {}
    for data in report.get("data", []):
        for entry in data.get("files", []):
            name = entry.get("filename", "")
            if "/crates/" not in name:
                continue
            crate = name.split("/crates/", 1)[1].split("/", 1)[0]
            s = entry.get("summary", {}).get("lines", {})
            counts = per_crate.setdefault(crate, [0, 0])
            counts[0] += int(s.get("count", 0))
            counts[1] += int(s.get("covered", 0))

    if not per_crate:
        print("no crate files found in the coverage report", file=sys.stderr)
        return 2

    failed = False
    for crate in sorted(per_crate):
        total, covered = per_crate[crate]
        pct = 100.0 * covered / total if total else None
        shown = f"{pct:.2f}%" if pct is not None else "no instrumentable lines"
        print(f"{crate:24s} lines={total:6d} covered={covered:6d} {shown}")
        if pct is None:
            continue  # a documentation shell cannot fail a line gate
        gated = crate in ("geist4-core", "geist4-html")
        if gated and pct < min_pct:
            print(
                f"FAIL: {crate} line coverage {pct:.2f}% is below {min_pct:.0f}%",
                file=sys.stderr,
            )
            failed = True

    if failed:
        return 1
    print(f"coverage gate ok (core/html >= {min_pct:.0f}% lines)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
