#!/usr/bin/env python3
"""Generate crates/geist4-ext/src/slug_table.rs from the vendored github-slugger regex.

Reads tools/github-slugger-regex.txt (github-slugger 2.0.0, MIT), parses the
character-removal regex into Unicode code-point ranges, cross-checks the table
exhaustively against the original JavaScript regex (every code point 0..0x10FFFF
except lone surrogates), and emits the Rust table.

Usage: python3 tools/gen_slug_table.py [node_modules_dir]
Exits nonzero if the cross-check fails or the regex contains anything the
parser does not understand.
"""

import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import NoReturn

REPO = Path(__file__).resolve().parent.parent
VENDORED = REPO / "tools" / "github-slugger-regex.txt"
OUT = REPO / "crates" / "geist4-ext" / "src" / "slug_table.rs"

# Characters embedded as spot-check tests in the generated file. Expectations
# are derived from the parsed table AFTER the exhaustive JS cross-check passes.
SPOT_CHARS = [
    ".", "_", "-", " ", "\t", "\n", "©", " ", "­", "​",
    "é", "中", "😀", "ﬁ", "Σ", "0", "a", "!", "~",
]


def fail(msg) -> "NoReturn":
    print(f"error: {msg}", file=sys.stderr)
    sys.exit(1)


def parse_class(body, clamp=True):
    """Parse a JS character-class body into a sorted list of (lo, hi) ranges.

    `clamp` drops lone-surrogate code points (they are not Rust chars);
    it must be False for the low-surrogate class of an astral alternative.
    """
    ranges = []
    i, n = 0, len(body)
    items = []  # list of (kind, value, ranges?) items in order

    def read_item(i):
        if i >= n:
            fail("class ended while reading an item")
        c = body[i]
        if c == "\\":
            return read_escape(i)
        return (ord(c), i + 1)

    def read_escape(i):
        # body[i] == "\\"
        if i + 1 >= n:
            fail("escape at end of class")
        c = body[i + 1]
        if c == "u":
            if i + 6 > n:
                fail("truncated \\u escape")
            hexs = body[i + 2 : i + 6]
            try:
                return (int(hexs, 16), i + 6)
            except ValueError:
                fail(f"bad \\u escape: {hexs}")
        if c == "x":
            if i + 4 > n:
                fail("truncated \\x escape")
            try:
                return (int(body[i + 2 : i + 4], 16), i + 4)
            except ValueError:
                fail(f"bad \\x escape")
        simple = {
            "0": 0, "n": 10, "r": 13, "t": 9, "f": 12, "v": 11, "b": 8,
        }
        if c in simple:
            return (simple[c], i + 2)
        # any other escaped char is the literal char
        return (ord(c), i + 2)

    while i < n:
        # read one item (single char or range)
        (cp, i) = read_item(i)
        if i < n and body[i] == "-":
            # range separator: the next item is the range end
            (cp2, i2) = read_item(i + 1)
            if cp > cp2:
                fail(f"reversed range {cp:#x}-{cp2:#x}")
            ranges.append((cp, cp2))
            i = i2
        else:
            ranges.append((cp, cp))

    # Clamp out lone surrogates: they are not Rust chars and cannot occur.
    out = []
    for lo, hi in ranges:
        if not clamp:
            out.append((lo, hi))
            continue
        pieces = []
        # split [lo, hi] around the surrogate block D800..DFFF
        if lo < 0xD800:
            pieces.append((lo, min(hi, 0xD7FF)))
        if hi > 0xDFFF:
            pieces.append((max(lo, 0xE000), hi))
        out.extend(pieces)
    return sorted(out)


def split_top_level_alts(src):
    """Split the regex source on top-level '|' (classes contain no '|')."""
    alts, depth, cur = [], 0, []
    i = 0
    while i < len(src):
        c = src[i]
        if c == "\\":
            # escaped char (e.g. \\[, \\.): neither it nor the escape is structural
            cur.append(c)
            if i + 1 < len(src):
                cur.append(src[i + 1])
            i += 2
            continue
        if c == "[":
            depth += 1
        elif c == "]":
            depth -= 1
            if depth < 0:
                fail("unbalanced class")
        if c == "|" and depth == 0:
            alts.append("".join(cur))
            cur = []
        else:
            cur.append(c)
        i += 1
    if depth != 0:
        fail("unbalanced class at end")
    alts.append("".join(cur))
    return alts


def parse_alt(alt):
    """Parse one alternative into code-point ranges.

    Shapes in this regex: a single BMP class, or \\uHHHH[lo-class]
    (one high surrogate), or [hi-class][lo-class] (several high
    surrogates sharing a lo class).
    """
    tokens = []  # ("class", ranges) | ("char", cp)
    i, n = 0, len(alt)
    while i < n:
        c = alt[i]
        if c == "\\":
            if i + 1 >= n:
                fail("trailing backslash")
            if alt[i + 1] == "u":
                cp = int(alt[i + 2 : i + 6], 16)
                tokens.append(("char", cp))
                i += 6
            else:
                fail(f"unexpected escape outside class: {alt[i:i+2]!r}")
        elif c == "[":
            end = alt.index("]", i)
            tokens.append(("class", parse_class(alt[i + 1 : end], clamp=False)))
            i = end + 1
        else:
            fail(f"unexpected literal outside class: {c!r}")

    if len(tokens) == 1 and tokens[0][0] == "class":
        return clamp_surrogates(list(tokens[0][1]))
    if len(tokens) == 2 and tokens[0][0] in ("char", "class") and tokens[1][0] == "class":
        his = (
            [tokens[0][1]]
            if tokens[0][0] == "char"
            else [cp for (lo, hi) in tokens[0][1] for cp in range(lo, hi + 1)]
        )
        out = []
        for h in his:
            if not 0xD800 <= h <= 0xDBFF:
                fail(f"astral alternative lead is not a high surrogate: {h:#x}")
            base = 0x10000 + ((h - 0xD800) << 10)
            for lo, hi in tokens[1][1]:
                if lo < 0xDC00 or hi > 0xDFFF:
                    fail(f"astral lo range outside DC00-DFFF: {lo:#x}-{hi:#x}")
                out.append((base + (lo - 0xDC00), base + (hi - 0xDC00)))
        return sorted(out)
    fail(f"unexpected alternative shape with {len(tokens)} tokens: {alt[:60]!r}")


def clamp_surrogates(ranges):
    """Drop lone-surrogate code points from BMP ranges (not Rust chars)."""
    out = []
    for lo, hi in ranges:
        if lo < 0xD800:
            out.append((lo, min(hi, 0xD7FF)))
        if hi > 0xDFFF:
            out.append((max(lo, 0xE000), hi))
    return sorted(out)


def merge(ranges):
    ranges = sorted(ranges)
    out = [list(ranges[0])]
    for lo, hi in ranges[1:]:
        if lo <= out[-1][1] + 1:
            out[-1][1] = max(out[-1][1], hi)
        else:
            out.append([lo, hi])
    return [(lo, hi) for lo, hi in out]


def main():
    text = VENDORED.read_text()
    start = text.index("export const regex = /") + len("export const regex = /")
    end = text.index("/g", start)
    src = text[start:end]
    alts = split_top_level_alts(src)
    ranges = []
    for k, alt in enumerate(alts):
        ranges.extend(parse_alt(alt))
    table = merge(ranges)
    print(f"parsed {len(alts)} alternatives -> {len(table)} merged ranges")

    # Exhaustive cross-check against the real JavaScript regex.
    node_modules = Path(sys.argv[1]) if len(sys.argv) > 1 else None
    if node_modules is None:
        candidates = sorted(
            p for p in (REPO.parent / "cache" / "scratch").glob("*/node_modules/github-slugger")
        )
        if not candidates:
            fail("no github-slugger node_modules found; pass its path as argv[1]")
        node_modules = candidates[0]
    print(f"cross-checking against {node_modules}")

    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        (tmp / "ranges.json").write_text(json.dumps(table))
        checker = tmp / "check.mjs"
        checker.write_text(
            "import { pathToFileURL } from 'node:url';\n"
            "import { readFileSync } from 'node:fs';\n"
            "const { regex } = await import(pathToFileURL(%s));\n"
            "const table = JSON.parse(readFileSync(process.argv[2], 'utf8'));\n"
            "// fresh non-global copy: .test with /g keeps lastIndex\n"
            "const re = new RegExp(regex.source);\n"
            "function removed(cp) {\n"
            "  let lo = 0, hi = table.length;\n"
            "  while (lo < hi) { const mid = (lo + hi) >> 1;\n"
            "    if (table[mid][1] < cp) lo = mid + 1; else hi = mid; }\n"
            "  return lo < table.length && table[lo][0] <= cp;\n"
            "}\n"
            "let bad = 0;\n"
            "for (let cp = 0; cp <= 0x10FFFF; cp++) {\n"
            "  if (cp >= 0xD800 && cp <= 0xDFFF) continue;\n"
            "  const js = re.test(String.fromCodePoint(cp));\n"
            "  if (js !== removed(cp)) {\n"
            "    if (bad++ < 20) console.log('MISMATCH U+' + cp.toString(16).padStart(4,'0'), 'js', js, 'table', removed(cp));\n"
            "  }\n"
            "}\n"
            "console.log(bad === 0 ? 'EXHAUSTIVE CHECK PASSED' : 'FAILED ' + bad);\n"
            "process.exit(bad === 0 ? 0 : 1);\n" % json.dumps(str(node_modules / "regex.js"))
        )
        proc = subprocess.run(
            ["node", str(checker), str(tmp / "ranges.json")],
            capture_output=True, text=True,
        )
        print(proc.stdout.strip())
        if proc.returncode != 0:
            print(proc.stderr, file=sys.stderr)
            fail("exhaustive cross-check failed")

    # Emit the Rust table.
    lines = []
    lines.append("//! The character-removal table for heading identifiers, generated from")
    lines.append("//! github-slugger 2.0.0 (MIT). Source of truth:")
    lines.append("//! `tools/github-slugger-regex.txt`; regenerate with `python3 tools/gen_slug_table.py`.")
    lines.append("//! The table was cross-checked exhaustively against the original regex for")
    lines.append("//! every code point. Do not edit by hand.")
    lines.append("")
    lines.append(f"pub(crate) static SLUG_REMOVE_RANGES: [(u32, u32); {len(table)}] = [")
    for lo, hi in table:
        lines.append(f"    (0x{lo:04X}, 0x{hi:04X}),")
    lines.append("];")
    lines.append("")
    lines.append("/// Whether the heading-identifier algorithm removes this character from")
    lines.append("/// the slug (github-slugger 2.0.0's regex, as code-point ranges).")
    lines.append("pub(crate) fn slug_removed(c: char) -> bool {")
    lines.append("    let cp = c as u32;")
    lines.append("    let i = SLUG_REMOVE_RANGES.partition_point(|r| r.1 < cp);")
    lines.append("    i < SLUG_REMOVE_RANGES.len() && SLUG_REMOVE_RANGES[i].0 <= cp")
    lines.append("}")
    lines.append("")
    lines.append("#[cfg(test)]")
    lines.append("mod tests {")
    lines.append("    use super::slug_removed;")
    lines.append("")
    tests = []
    for ch in SPOT_CHARS:
        cp = ord(ch)
        removed = any(lo <= cp <= hi for lo, hi in table)
        name = f"U+{cp:04X}"
        tests.append(
            f'    assert_eq!(slug_removed(\'\\u{{{cp:X}}}\'), {str(removed).lower()}, "{name}");'
        )
    lines.extend(tests)
    lines.append("")
    lines.append("    #[test]")
    lines.append("    fn slug_table_boundaries() {")
    lines.append("        // range ends and their neighbours stay honest")
    lines.append("        for w in SLUG_REMOVE_RANGES.windows(2) {")
    lines.append('            assert!(w[0].1 < w[1].0, "overlapping or unsorted ranges");')
    lines.append("        }")
    lines.append("        for &(lo, hi) in SLUG_REMOVE_RANGES {")
    lines.append("            if (0xD800..=0xDFFF).contains(&lo) {")
    lines.append('                panic!("lone-surrogate range in table");')
    lines.append("            }")
    lines.append("        }")
    lines.append("    }")
    lines.append("}")
    OUT.write_text("\n".join(lines) + "\n")
    print(f"wrote {OUT} ({len(table)} ranges)")


if __name__ == "__main__":
    main()
