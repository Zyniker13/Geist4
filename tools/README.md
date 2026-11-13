# Generators

The tables under `crates/geist4-core/src/inline/` are generated, not
hand-written (decision 0009). Each generator writes its file directly; run
them from the repository root:

```sh
python3 tools/gen_unicode.py    # inline/unicode.rs — whitespace and
                                # punctuation/symbol code point ranges
python3 tools/gen_entities.py   # inline/entity_table.rs — HTML5 entities
python3 tools/gen_fold.py       # inline/fold.rs — case-fold overrides
```

## Data sources

- `tools/data/entities.json` — the WHATWG full entity list,
  <https://html.spec.whatwg.org/entities.json>, fetched 2026-11-12,
  SHA-256 `d741d877ac77c4194c4ad526b5b4a19aef8dfe411ab840a466891cdbb9f362e6`.
- The Unicode tables come from the Python standard library's
  `unicodedata` (general categories and default case folding), matching
  the Unicode version of the local Python build; record the versions in
  the commit message when regenerating.

## Notes

- `inline/unicode.rs` and `inline/fold.rs` are whole-file outputs.
- `inline/entity_table.rs` is a whole-file output; the handwritten
  scanning code beside it lives in `inline/entities.rs`.
- After regenerating, run `cargo fmt` and the test suite; the tables are
  checked in, so a regeneration is an ordinary reviewable commit.
