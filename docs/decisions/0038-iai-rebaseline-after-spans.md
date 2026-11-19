# 0038: The instruction-count baseline after the span work

## Context

The instruction-count gate (`tools/check_iai.py`, decision 0012) compares
`cargo bench -p geist4-bench --bench iai` against `bench/iai-baseline.txt`,
failing above 110%. Decision 0031's baseline (spec 24,924,844 /
corpus 11,826,964 instructions) predates the block-side TextMap
(`4aa2c441`, decision 0036) and the reference-correct span ends
(`33b03bea`), which put the counted work above the threshold: the runner
measured spec 28,628,698 (+14.9%) and corpus 18,168,655 (+53.6%).

Spans are a roadmap requirement (M7's mdast export, source spans in
problems, render overrides), not an optimization mistake, so the gate
needs a new baseline — but only after the added work was measured and
pruned of accidental cost, so the rebaseline records a justified number.

## Decision

Before rebaselining, the increase was investigated on a quiet machine
with worktree A/Bs against `4aa2c441` (best-of-N minima, parse split
from render) and piece-by-piece disable measurements, which found and
fixed two real defects (committed with the fix that introduced them):

1. `TextMap::map_byte` located the anchor by binary search but then
   looked the anchor's line length up by scanning the whole anchor list
   on every call — quadratic for a paragraph of many lines with many
   inline nodes. The length is index arithmetic now, and `shift`'s
   lengths are computed by index instead of per-anchor scans. A new
   pathological family (`multiline_paragraph_span_mapping_scales_linearly`)
   fails on the old code and passes on the new.
2. The per-line TextMap record hashed a `HashMap<NodeId, _>` on every
   appended line; the maps live in a dense `Vec<Option<TextMap>>` side
   table indexed by the arena slot, with take/put-back at close instead
   of a clone per paragraph.

The fixes took the runner's counts from spec 28,628,698 /
corpus 18,168,655 to spec 27,540,069 / corpus 17,292,643. The remaining
increase is the honest cost of the feature itself: one anchor recorded
per leaf-block line, one span store per closed frame, and one mapped
span per inline node. On the benchmark corpus that measures ~+21% parse
wall-clock and ~+16% end-to-end against `4aa2c441` (the renderer is
unchanged); the spec corpus, whose examples are shorter, costs less.

The baseline is re-armed from the runner's own measured counts of the
commit that carries the fixes above (read from the temporary annotation
step, then the step is removed with this commit): spec 27,540,069 /
corpus 17,292,643. The threshold stays 110%.

## Consequences

- The gate now guards the span-feature cost as the floor: a future
  change that spends instructions beyond 110% of the new numbers fails
  CI regardless of its motive.
- The corpus bench carries more headroom consumption than the spec
  bench; future span-related work should target the per-line anchor
  recording (the largest remaining share) if another round is needed.
- The annotation-based readout was a temporary diagnostic (the same
  pattern as decision 0031's commit-status diagnostic); it is removed
  here, and job logs remain the durable record on the runner.
