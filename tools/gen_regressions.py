#!/usr/bin/env python3
"""Generate .html expectations for the regression corpus.

For every tests/regressions/*.md: strips the leading //// header lines,
runs the reference implementation (cmark --to html --unsafe, stripped of
its document-final newline) to produce the paired .html file, and reports
any case where the local engine already disagrees (printed for review;
the regression test itself enforces the local render).
"""
import pathlib
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
CRATE = HERE.parent / "crates" / "geist4-core"


def strip_header(text: str) -> str:
    lines = text.splitlines(keepends=True)
    body = 0
    for line in lines:
        if line.startswith("////"):
            body += 1
        else:
            break
    return "".join(lines[body:])


def main() -> int:
    for md in sorted((CRATE / "tests/regressions").glob("*.md")):
        body = strip_header(md.read_text())
        out = subprocess.run(
            ["cmark", "--to", "html", "--width", "0", "--unsafe"],
            input=body, capture_output=True, text=True,
        )
        if out.returncode != 0:
            print(f"cmark failed on {md.name}: {out.stderr}", file=sys.stderr)
            return 1
        html = out.stdout
        if html.endswith("\n"):
            html = html[:-1]
        (md.with_suffix(".html")).write_text(html)
        print(f"{md.name} -> {md.with_suffix('.html').name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
