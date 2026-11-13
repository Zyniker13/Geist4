# Decision 0013: profile-guided optimization scope for M4

Date: 2026-11-13
Status: accepted

## Context

M4's done-when asks for corpus throughput at least equal to
`pulldown-cmark`'s, or for the gap and its reasons to be recorded. The
first recorded comparison (`docs/benchmarks.md`, at `543f61d3`) put
geist4 at 131.5 MiB/s against pulldown-cmark's 517.3 — a 3.9x gap —
with comrak at 174.6.

Profiling (`sample` over a parse+render driver, per-document hot-spot
counts) showed the cost was not scanning but allocation churn: the
node arena grew while filling, link-definition paragraphs were
rescanned, every line was converted through `String::from_utf8_lossy`,
the inline phase rebuilt its cell pool, delimiter list, bracket stack
and backtick memo per leaf block, HTML escaping copied byte by byte,
and inline nodes took a detour through an intermediate node tree with
a `HashMap` and a second copy of every string.

## Decision

- Optimize profile-first, in small rounds, each gated by the full
  suite (spec 652/652, the seeds differential byte-for-byte, the
  property and regression tests, fuzz): arena pre-reserve from the
  measured node density, a definition-scan guard for paragraphs that
  cannot hold one, inline-phase buffer reuse across a document with a
  generation-tagged backtick memo, byte-run-batched HTML escaping,
  borrowed source slices with owned fallbacks for tab-expanded and
  NUL-rebuilt lines, direct pool-to-arena attachment (removing the
  intermediate tree), a reusable label-normalization buffer, a
  child-vector reserve, and a reusable paragraph join buffer.
- Keep the hand-written byte dispatch instead of adding `memchr`. The
  reference scanners' semantics (whitespace sets, NUL handling, the
  code-span memo) are already mirrored there, the profiles showed
  allocation — not scan speed — as the cost, and the scanner code
  stays dependency-free.
- Record the remaining gap instead of chasing pulldown-cmark: after
  the rounds, geist4 measures 210.4 MiB/s against pulldown-cmark's
  540.8 (2.6x), ahead of comrak's 192.7. The gap is architectural:
  pulldown-cmark streams events over the source without building a
  tree, while geist4 builds a full arena tree with spans for the
  extension API, source maps and render overrides, and copies text
  into owned nodes so documents can outlive their input. The tree and
  its allocations are the product of the API the roadmap requires,
  not overhead to be optimized away.

## Consequences

- The comparison table gains a 1.60x improved geist4 row and the
  instruction-count gate (`bench/iai-baseline.txt`) is refreshed to
  the improved counts, locking the gains in.
- Wall-clock numbers stay local (shared runners are noisy); CI gating
  remains the instruction-count benchmark.
- A future milestone that wants the remaining gap closed should start
  from node storage (arena entries are fat: kind + owned strings +
  child vectors per node) and the per-line paragraph joining, and
  should re-profile first; the profile shape changed with each round
  in this set.
