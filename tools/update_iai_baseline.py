#!/usr/bin/env python3
"""Write bench/iai-baseline.txt from iai-callgrind output.

Usage: update_iai_baseline.py <iai-output-file> [baseline-file]

Run `cargo bench -p geist4-bench --bench iai` on Linux, save its output,
and pass the file here. The result is the checked-in baseline that
`tools/check_iai.py` compares against. Commit it after a deliberate,
reviewed performance change, with the reason in the commit message.
"""

import sys

from check_iai import parse


def main():
    if len(sys.argv) < 2:
        sys.exit("usage: update_iai_baseline.py <iai-output-file> [baseline-file]")
    with open(sys.argv[1], encoding="utf-8") as handle:
        counts = parse(handle.read())
    if not counts:
        sys.exit("no instruction counts found in the iai output")
    path = sys.argv[2] if len(sys.argv) > 2 else "bench/iai-baseline.txt"
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(
            "# Instruction counts from `cargo bench -p geist4-bench --bench iai`"
            " (iai-callgrind, Linux).\n"
            "# Update with tools/update_iai_baseline.py after a deliberate,"
            " reviewed performance change.\n"
        )
        for name, count in counts.items():
            handle.write(f"{name} {count}\n")
    print(f"wrote {len(counts)} entries to {path}")


if __name__ == "__main__":
    main()
