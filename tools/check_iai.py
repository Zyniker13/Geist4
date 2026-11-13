#!/usr/bin/env python3
"""Check iai-callgrind instruction counts against the checked-in baseline.

Usage: check_iai.py <iai-output-file> [baseline-file]

Reads the output of `cargo bench -p geist4-bench --bench iai`, extracts
each benchmark's instruction count, and compares it with
`bench/iai-baseline.txt`. A count above baseline * threshold (default
1.10, overridable with IAI_THRESHOLD) fails; a benchmark missing from
either side fails, so renames and removals cannot slip past the gate.

If the baseline file does not exist yet, the counts are printed and the
check passes in bootstrap mode: commit a baseline with
`tools/update_iai_baseline.py` to arm the gate.
"""

import os
import re
import sys

HEADER = re.compile(r"^\S")
INSTRUCTIONS = re.compile(
    r"^\s+Instructions:\s+([0-9,]+)\s*(?:\|[^|]*)?(?:\([^)]*\))?\s*$"
)


def parse(text):
    """Return {benchmark header: instruction count} from iai output."""
    counts = {}
    current = None
    for line in text.splitlines():
        if HEADER.match(line):
            current = line.strip()
        elif current is not None:
            found = INSTRUCTIONS.match(line)
            if found and current not in counts:
                counts[current] = int(found.group(1).replace(",", ""))
    return counts


def parse_baseline(path):
    """Return {benchmark header: count} from the baseline file."""
    counts = {}
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            line = line.rstrip("\n")
            if not line or line.lstrip().startswith("#"):
                continue
            name, _, number = line.rpartition(" ")
            counts[name] = int(number)
    return counts


def main():
    if len(sys.argv) < 2:
        sys.exit("usage: check_iai.py <iai-output-file> [baseline-file]")
    with open(sys.argv[1], encoding="utf-8") as handle:
        counts = parse(handle.read())
    if not counts:
        sys.exit("no instruction counts found in the iai output")

    baseline_path = sys.argv[2] if len(sys.argv) > 2 else "bench/iai-baseline.txt"
    if not os.path.exists(baseline_path):
        print("iai baseline does not exist yet; bootstrap mode, counts:")
        for name, count in counts.items():
            print(f"  {name}: {count}")
        print("arm the gate with tools/update_iai_baseline.py")
        return

    baseline = parse_baseline(baseline_path)
    threshold = float(os.environ.get("IAI_THRESHOLD", "1.10"))
    failed = False
    for name, count in baseline.items():
        if name not in counts:
            print(f"FAIL {name}: in the baseline but not in this run")
            failed = True
            continue
        limit = count * threshold
        status = "ok" if counts[name] <= limit else "FAIL"
        if status == "FAIL":
            failed = True
        print(
            f"{status} {name}: {counts[name]} instructions"
            f" (baseline {count}, limit {limit:.0f})"
        )
    for name in counts:
        if name not in baseline:
            print(f"FAIL {name}: {counts[name]} instructions, not in the baseline")
            failed = True
    if failed:
        sys.exit(1)


if __name__ == "__main__":
    main()
