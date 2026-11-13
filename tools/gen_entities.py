"""Generates the HTML5 entity table module for Geist4.

Reads tools/data/entities.json (WHATWG full entity list, see README) and
writes crates/geist4-core/src/inline/entity_table.rs: the named references
with names stored without the leading ampersand and trailing semicolon,
sorted for binary search, with their replacement code points.

Run from the repository root: python3 tools/gen_entities.py
"""

import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HERE, "data", "entities.json")
OUT = os.path.join(
    HERE, "..", "crates", "geist4-core", "src", "inline", "entity_table.rs"
)

with open(DATA) as f:
    data = json.load(f)

table = {}
for key, value in data.items():
    name = key[1:]
    if name.endswith(";"):
        name = name[:-1]
    table[name] = value["codepoints"]

names = sorted(table)

lines = [
    "//! The HTML5 named character references, generated from the vendored",
    "//! WHATWG `entities.json` (see `tools/README.md`). Regenerate with",
    "//! `python3 tools/gen_entities.py`; do not edit by hand.",
    "",
    "/// The entity names, without the leading ampersand and trailing",
    "/// semicolon, sorted for binary search.",
    f"pub(crate) static ENTITY_NAMES: [&str; {len(names)}] = [",
]
for name in names:
    lines.append(f'    "{name}",')
lines.append("];")
lines.append("")
lines.append("/// Replacement code points for `ENTITY_NAMES`, same order.")
lines.append(f"pub(crate) static ENTITY_CODEPOINTS: [&[u32]; {len(names)}] = [")
for name in names:
    cps = table[name]
    parts = ", ".join(f"0x{cp:X}" for cp in cps)
    lines.append(f"    &[{parts}],")
lines.append("];")

with open(OUT, "w") as f:
    f.write("\n".join(lines) + "\n")

print(f"{len(names)} entities")
