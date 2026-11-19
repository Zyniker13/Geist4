# Migrating to Geist4

Geist4 is a CommonMark 0.31.2 engine with loadable extensions. Plain
CommonMark renders byte-for-byte like cmark 0.31.2; the built-in
extensions follow the reference implementations their decisions pinned
(cmark-gfm for the GFM constructs, comrak for footnotes, description
lists and front matter, GitHub's slugger for heading identifiers). This
guide maps the option and extension names of the engines projects
commonly migrate from onto Geist4's surface, and lists the output
differences to expect. Engine APIs evolve; check each engine's current
documentation when a mapping matters to you.

## The surface you are migrating to

- **Rust**: `geist4_core::parse_document`/`parse_with` and
  `geist4_core::basic_html::render`/`render_with`; the tree is
  `geist4_core::parser::Document` (arena, spans, borrowed text).
- **Tree exports**: `geist4-mdast` (mdast/unist shape, the compatibility
  form) and `geist4-json` (the versioned `geist4/tree@1` format, with a
  published JSON Schema at `docs/schema/geist4-tree-v1.json`).
- **JavaScript**: the `geist4` npm-style package built from
  `geist4-wasm` (`Document`, `Options`, `renderHtml`).
- **Python**: the `geist4` wheel built from `geist4-python`
  (`render_html`, `Document`, `Options`).
- **Swift**: the `Geist4` package (`bindings/swift`).
- **C and everything else**: the C ABI in `geist4-ffi`
  (`geist4_parse`, `geist4_render_html`, options handles), which every
  binding above shares.
- **CLI**: `geist4 render --format html|json|mdast`, `geist4 ast`,
  `geist4 check`, `geist4 format` (M8), `geist4 ext check`.

Built-in extensions, enabled by name everywhere (`Options.enable_builtin`
in the bindings, `Options::extension` in Rust): `strikethrough`,
`autolink`, `tables`, `tasklist`, `tagfilter`, `footnotes`,
`front_matter`, `heading_identifiers`, `description_lists`.

## From markdown-it

| markdown-it | Geist4 |
| --- | --- |
| `html: true` (default) | Raw HTML always renders, as in cmark; nothing to enable. GFM's disallowed-raw-HTML filter is the `tagfilter` extension, not a global switch. |
| `linkify: true` | The `autolink` extension (cmark-gfm's www/email/scheme rules, not markdown-it's linkify-it defaults). |
| `typographer: true` | No equivalent. Smart quotes and dashes change text and are out of scope. |
| `breaks: true` | No equivalent. Geist is CommonMark: a single newline is a soft break. Render soft breaks yourself from the tree if you need the behavior. |
| `highlight` | No built-in equivalent; syntax highlighting is not the renderer's job. |
| `xhtmlOut` | No equivalent; output is cmark-shaped. |
| plugins: `markdown-it-footnote`, `markdown-it-task-lists`, `markdown-it-deflist`, `markdown-it-anchor`, `markdown-it-table-of-contents` | The built-ins `footnotes`, `tasklist`, `description_lists`, `heading_identifiers` cover the common core; their output follows the GFM/comrak reference shapes, not the plugins'. |

Output differences: markdown-it is not CommonMark-conformant in a few
places it inherited from its CommonMark-preview-era roots; expect
differences around lists, emphasis flanking and entities. Geist's
output ends with a newline after the last block. mdast users: the
`geist4-mdast` export matches `mdast-util-from-markdown`'s shape.

## From marked

| marked | Geist4 |
| --- | --- |
| `gfm: true` (default) | The GFM set: `tables`, `strikethrough`, `tasklist`, `autolink`, and `tagfilter` if you want the disallowed-raw-HTML filter marked's sanitizer-adjacent behavior approximated. |
| `breaks: true` | No equivalent (see markdown-it above). |
| `pedantic: true` | No equivalent; Geist is CommonMark-only by design. |
| `headerIds: true` + `headerPrefix` | The `heading_identifiers` extension, following GitHub's DOM-text slug semantics (see the pinned divergences below). |
| `mangle: true` | No equivalent; email obfuscation is out of scope. |
| `sanitize: true` | Removed from marked long ago and not here either: sanitization is the caller's job (see `docs/security-model.md`). |

Output differences: marked historically deviated from CommonMark in
emphasis, lists and links; the byte-for-byte cmark guarantee covers all
of it. Heading identifiers differ from `marked-gfm-heading-id` in four
pinned cases (named entities, comment tails, processing instructions,
untrimmed strips) — the frozen oracle battery
(`tests/fixtures/heading_oracle.json`) pins both sides.

## From micromark / remark

| micromark/remark | Geist4 |
| --- | --- |
| `micromark-extension-gfm` | The GFM set: `tables`, `strikethrough`, `tasklist`, `autolink`, `tagfilter`. |
| `micromark-extension-frontmatter` | The `front_matter` extension (comrak's split rules; YAML is not parsed, only captured raw in the `source` attribute). |
| `remark-math` / `micromark-extension-math` | No built-in equivalent yet. |
| `mdast-util-from-markdown` output | `geist4-mdast` matches the shape (decision 0037), including UTF-16 positions; extension nodes export transparently as their children in v1. |
| remark's `dangerouslyAllowHTML` | Raw HTML is always allowed, as in cmark; the tree carries `html` nodes either way. |

Output differences: the remark ecosystem's `mdast` is the compatibility
target — expect the same tree, with Geist's own tree (and its
extension nodes) available through `geist4-json` when you need spans
and extension structure the mdast shape does not carry.

## From python-markdown

python-markdown predates CommonMark and is not conformant; migrating
means behavioral review, not just option mapping.

| python-markdown | Geist4 |
| --- | --- |
| `extensions=['extra']` (tables, fenced_code, footnotes, attr_list, def_list, ...) | `tables` and `description_lists` map directly; `footnotes` exists but its numbering and output shape follow comrak, not python-markdown's; `attr_list` has no equivalent; fenced code is core. |
| `extensions=['toc']` | The `heading_identifiers` extension — different slugger (GitHub's, not slugify), different id placement. |
| `extensions=['nl2br']` | No equivalent (soft breaks stay soft). |
| `extensions=['smarty']` | No equivalent. |
| `extensions=['meta']`/`pymdownx.frontmatter` | The `front_matter` extension captures the raw block; parsing YAML is your step. |

Output differences: list handling, inline HTML, entities and emphasis
all differ from python-markdown; the spec suite (652/652 examples) is
the contract Geist gives you instead.

## From league/commonmark

| league/commonmark (PHP) | Geist4 |
| --- | --- |
| `GithubFlavoredMarkdownExtension` | The GFM set: `tables`, `strikethrough`, `tasklist`, `autolink`, `tagfilter`. |
| `AutolinkExtension`, `StrikethroughExtension`, `TableExtension`, `TaskListListItemExtension` | The same four as part of the GFM set above. |
| `FootnoteExtension` (community) | The `footnotes` built-in, comrak-shaped. |
| `AttributesExtension` (`{#id}`) | No equivalent. |
| `DefaultAttributesExtension`, `HeadingPermalinkExtension`, `SmartPunctExtension` | `heading_identifiers` covers the permalink case (GitHub slug semantics); smart punctuation has no equivalent. |
| custom renderers / event listeners | The extension API (`docs/extensions/api.md`) covers render overrides per node kind and host hooks in C, Swift, Python and wasm. |

## From cmark / cmark-gfm

The closest relative: plain CommonMark output is byte-identical
(spec 652/652, plus the reference's own test suite ported as
regressions).

| cmark option / extension | Geist4 |
| --- | --- |
| `CMARK_OPT_DEFAULT` | `parse_document` + `render` — byte-identical output. |
| `CMARK_OPT_SOURCEPOS` | Positions are always in the tree (spans) and in both JSON exports; the HTML renderer never emits `data-sourcepos`. |
| `CMARK_OPT_HARDBREAKS`, `CMARK_OPT_NOBREAKS` | No equivalent; break semantics are CommonMark's. |
| `CMARK_OPT_SAFE` | Not equivalent: Geist renders raw HTML and all link schemes per CommonMark (the URL filter rule only, `docs/security-model.md`); sanitization stays the caller's job. |
| `CMARK_OPT_GITHUB_TAGFILTER` | The `tagfilter` extension. |
| `--extension strikethrough/table/autolink/tasklist` | The same four built-ins by name. |
| cmark-gfm's `--extension footnote` | The `footnotes` built-in (comrak 0.51.0 is the oracle; cmark-gfm's fork semantics match where they exist). |
| `CMARK_OPT_VALIDATE_UTF8`, `CMARK_OPT_LIBHTML_HTML` | No equivalent; input is Rust `&str` (always UTF-8), and the full HTML document shell is not part of the engine. |

## Known output differences, in one list

1. **Trailing newline**: the HTML output ends with a newline after the
   last block, as cmark, cmark-gfm and comrak do.
2. **Tasklist required space**: the box's required space follows the
   reference scanner's whitespace class, which — unlike cmark 0.31.2's
   — does not include VT and FF (decision 0016, documented in
   `spec/tasklist.md`).
3. **Heading identifiers** follow GitHub's DOM-text semantics
   (decision 0021); the four marked-gfm-heading-id divergences are
   pinned in the battery.
4. **Footnotes and description lists** follow comrak 0.51.0
   (decisions 0018 and 0022).
5. **Front matter** follows comrak's split rules while keeping spans
   true to the source (decision 0020); the block renders as nothing.
6. **No typographer, no `breaks`/`nl2br`, no syntax highlighting, no
   sanitizer, no email mangling** — each is either outside a Markdown
   engine's correctness contract or is the caller's security decision
   (`docs/security-model.md`).
7. **Extensions are namespaced opt-ins**: plain `parse_document`/
   `render` never load them, so a document that rendered one way
   without an extension renders the same way with it, except where the
   extension claims constructs (GFM tables, footnotes, and so on).
