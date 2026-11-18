# 0031 — Rebasing the iai instruction gate after the problems channel

## Context

The iai gate (`tools/check_iai.py`) has held instruction counts to within 10% of
`bench/iai-baseline.txt` since M5. m6-5 (source spans and the problems channel)
moved the counted path for the first time, and CI began failing the gate:

- `spec_parse_to_html`: 22,350,528 → 24,924,844 (+11.5%)
- `corpus_parse_to_html`: 10,953,331 → 11,826,964 (+8.0%)

Both increases come from real work the spans/problems milestone added on the
parse path: span tracking threaded through the inline parser, problem records
allocated and stored with their spans, and the document-level problem list.
Nothing is per-character; the cost is proportional to constructs, not text
length. The corpus document (mostly prose) grew 8%, well inside the gate; the
spec document (dense constructs) grew 11.5%, just past the 1.10 limit.

## Decision

Rebase the baseline to the counts measured on the current tree, keeping the
1.10 threshold armed exactly as before:

- `spec_parse_to_html`: 24,924,844
- `corpus_parse_to_html`: 11,826,964

The counts were taken from CI's iai job on the current tree (Linux), which is
the environment the gate compares.

## Consequences

The gate now watches the post-spans cost: any further regression of more than
10% fails CI, so the two features' price is paid once and the gate is armed
again from the new floor.

A future performance milestone can reclaim part of the increase by optimizing
the spans/problem bookkeeping against this new baseline; the 11.5% is the
upper bound of what such work could win back.

The counts are CI-only measurements (iai needs callgrind and runs on Linux);
local development cannot re-baseline the gate. When a deliberate change moves
the counted path, take the fresh counts from CI's failing log and commit them
with the change, as here.
