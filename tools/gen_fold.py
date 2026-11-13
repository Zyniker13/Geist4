"""Generates crates/geist4-core/src/inline/fold.rs.

The module carries the codepoints whose full case folding differs from
their simple lowercase mapping (for example the sharp s folds to "ss");
everything else folds with the ordinary lowercase algorithm at runtime.
The folding comes from the Python standard library's str.casefold, which
implements the Unicode default case folding.

Run from the repository root: python3 tools/gen_fold.py
"""

OUT = "crates/geist4-core/src/inline/fold.rs"

HEADER = """//! Unicode case-fold overrides: codepoints whose full case folding
//! differs from their lowercase mapping. Generated from the Unicode
//! character database; sorted by codepoint for binary search.

/// `(codepoint, folded string)` pairs, ascending by codepoint.
pub(crate) static FOLD_OVERRIDES: &[(u32, &str)] = &[
"""

entries = []
for cp in range(0x110000):
    ch = chr(cp)
    fold = ch.casefold()
    if fold != ch.lower():
        entries.append((cp, fold))

lines = []
for cp, fold in entries:
    esc = fold.replace("\\", "\\\\").replace('"', '\\"')
    lines.append(f'    ({cp}, "{esc}"),')

with open(OUT, "w") as f:
    f.write(HEADER)
    f.write("\n".join(lines))
    f.write("\n];\n")

print(len(entries), "overrides")
