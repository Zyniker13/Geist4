# 0021: Heading identifiers extract GitHub DOM text, not marked's strip

## Context

GFM's "Generating IDs" appendix is unobtainable (the official spec
copies and their archived versions predate it), so no normative text
pins how a heading's id is derived from its content, and comrak has no
heading ids at all. The pinned transform oracle is github-slugger 2.0.0
(decision 0009's generated-table policy; frozen fixture
`tests/fixtures/slugger_oracle.json`), but the slugger only defines the
transform applied to a string; the choice of string is a separate
policy.

The reference pipeline (marked + marked-gfm-heading-id, the only
real-world pipeline drivable end to end) feeds the slugger the heading's
rendered inline HTML after `unescape(...).trim()` and a whole-string
strip with `/<[!\/a-z].*?>/gi`. Probing the real pipeline
(`tools/heading_oracle_probe.mjs`) exposed its extraction artifacts:

- `# caf&eacute;` → id `caf`: marked keeps named entities raw in its
  rendered HTML (not CommonMark-conformant), and its unescape eats them.
- `# &lt;tag&gt;` → id `tag`: the unescape decodes the entities and the
  slugger drops the angle brackets — the earlier assumption that the
  strip expression removes them was wrong.
- `# <!-- a > b --> title` → id `-b----title`: the lazy regex stops at
  the first `>` and leaks the comment tail into the slug.
- `# <?php ?> code` → id `php--code`: the regex never matches a
  processing instruction, so the PI's inner text leaks.

Geist4 is CommonMark-conformant: entities are decoded at parse time
(`&eacute;` → `é`; spec.txt requires this), comments and processing
instructions parse as single raw-HTML nodes, and an unterminated `<`
stays text. Reproducing marked's exact ids would require deliberately
un-conformant extraction.

## Decision

The slug input is the heading's decoded inline text content in document
order — GitHub's DOM `textContent` semantics: text and code spans
contribute their text, emphasis, strong, links and inline extension
nodes contribute their descendants' text, soft and hard breaks
contribute a newline, and raw-HTML nodes and images contribute nothing.
The extracted text is trimmed, then `slugger.slug(text, false)`; the
slugger's own removal table drops `<`, `>` and `&`, so decoded tag text
and bare ampersands need no separate strip step.

Four marked-gfm-heading-id behaviors are deliberately not reproduced,
each documented in the module and the spec file with the reference's
value alongside ours: entity-eating (`caf` vs `café`), comment-tail
leak (`-b----title` vs `title`), processing-instruction leak
(`php--code` vs `code`), and the post-strip trailing hyphen (`x-` vs
`x`). github-slugger 2.0.0 remains the transform oracle; every
non-divergent case was verified equal against the probed pipeline,
including `<tag>`-style slugs, unmatched `<`, entity-encoded tags,
multi-line setext content, and code spans.

Ids are computed during `post_process` and stored as attributes, so the
renderer stays stateless and hooks stay pure; the rendering override
reads the `id` and level attributes it set.

## Consequences

- Extraction is a small tree walk with no string surgery: the
  lazy-regex and line-terminator edge cases cease to exist.
- Ids match GitHub's anchors for tag, entity-tag, comment and
  processing-instruction headings (GitHub drops non-text DOM nodes;
  where its exact trim behavior is unobservable offline, ours is
  documented).
- Ids differ from marked-gfm-heading-id on exactly the four documented
  classes — all cases where marked disagrees with GitHub.
- A marked-based consumer migrating to this engine sees id changes only
  on headings containing named entities, comments or processing
  instructions.
- Inline extension nodes contribute their descendants' text — a generic
  rule; core gains no `id`-specific knowledge.
