#!/usr/bin/env python3
"""Generates the cross-binding case list from the checked-in spec files.

The cross-binding suite (TESTING.md, "Cross-binding") runs the same
documents through every binding and requires byte-identical HTML. The
documents are the CommonMark spec's 652 examples (rendered plain) and
every built-in extension's spec file (rendered with that one extension
enabled), matching how `crates/geist4-ext/tests/spec.rs` runs them.

Output: `tests/bindings/cases.jsonl`, one JSON object per line:

    {"name": "spec 123", "options": [], "source": "..."}

`options` lists the built-in extension names the case needs; the
bindings enable them by name (the same names `enable_builtin` takes).
Regenerate with `python3 tools/gen_cross_cases.py` after changing a
spec file; the output is checked in and must regenerate byte for byte
(`--check` verifies that without writing, which the gate runs).
"""

import io
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "tests" / "bindings" / "cases.jsonl"

# Each extension spec file runs with exactly its own built-in enabled,
# as the Rust spec test does.
EXTENSION_SPECS = [
    "strikethrough",
    "tables",
    "autolink",
    "tasklist",
    "tagfilter",
    "footnotes",
    "front_matter",
    "heading_identifiers",
    "description_lists",
]


def parse_examples(text: str) -> list[dict]:
    """Extracts the examples from a spec file in the CommonMark format.

    The format is the one `geist4-spec-harness` parses: examples are
    fenced by a line starting with exactly thirty-two backticks (the
    opening fence usually carries an ` example` suffix), the markdown
    runs to a `.` separator line, and the expected HTML runs to the
    bare closing fence. Tab arrows expand to real tabs in both fields,
    exactly as the harness does (`text.replace('→', "\\t")`).
    """
    fence = "`" * 32
    lines = text.splitlines()
    examples = []
    i = 0
    while i < len(lines):
        if not lines[i].startswith(fence):
            i += 1
            continue
        i += 1
        markdown: list[str] = []
        while i < len(lines) and lines[i] != ".":
            markdown.append(lines[i])
            i += 1
        if i >= len(lines):
            break
        i += 1
        html: list[str] = []
        while i < len(lines) and lines[i] != fence:
            html.append(lines[i])
            i += 1
        examples.append(
            {
                "markdown": "\n".join(markdown).replace("\u2192", "\t") + "\n",
                "html": "\n".join(html).replace("\u2192", "\t") + "\n",
            }
        )
        i += 1
    return examples


def build_cases() -> list[dict]:
    cases: list[dict] = []
    spec = (ROOT / "spec" / "spec.txt").read_text(encoding="utf-8")
    for index, example in enumerate(parse_examples(spec), start=1):
        cases.append({"name": f"spec {index}", "options": [], "source": example["markdown"]})
    for builtin in EXTENSION_SPECS:
        path = ROOT / "crates" / "geist4-ext" / "spec" / f"{builtin}.md"
        text = path.read_text(encoding="utf-8")
        for index, example in enumerate(parse_examples(text), start=1):
            cases.append(
                {
                    "name": f"{builtin} {index}",
                    "options": [builtin],
                    "source": example["markdown"],
                }
            )
    return cases


def write_cases(handle, cases: list[dict]) -> None:
    for case in cases:
        handle.write(json.dumps(case, ensure_ascii=False) + "\n")


def main() -> None:
    cases = build_cases()
    if "--check" in sys.argv:
        # Regenerate into memory and require the checked-in file to match
        # byte for byte, so a spec-file change cannot leave stale cases.
        buffer = io.StringIO()
        write_cases(buffer, cases)
        expected = buffer.getvalue()
        actual = OUT.read_text(encoding="utf-8") if OUT.exists() else ""
        if actual != expected:
            print(
                "tests/bindings/cases.jsonl is stale; regenerate it with: "
                "python3 tools/gen_cross_cases.py",
                file=sys.stderr,
            )
            raise SystemExit(1)
        print(f"cases.jsonl is current ({len(cases)} cases)")
        return

    OUT.parent.mkdir(parents=True, exist_ok=True)
    with OUT.open("w", encoding="utf-8", newline="\n") as handle:
        write_cases(handle, cases)
    print(f"wrote {len(cases)} cases to {OUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
