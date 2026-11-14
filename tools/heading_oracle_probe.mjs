// Probe the heading-identifier reference pipeline: marked with
// marked-gfm-heading-id over a battery of Markdown sources.
// Output: JSON on stdout, consumed to freeze
// crates/geist4-ext/tests/fixtures/heading_oracle.json and to generate
// crates/geist4-ext/spec/heading_identifiers.md. Each case carries the
// reference's rendered HTML and the heading ids extracted from it, in
// document order.
// Usage: node tools/heading_oracle_probe.mjs [path-to-node_modules-dir]
import { pathToFileURL } from 'node:url'
import process from 'node:process'

const dir = process.argv[2] ?? 'node_modules'
const { Marked } = await import(pathToFileURL(dir + '/marked/lib/marked.esm.js'))
const { gfmHeadingId, resetHeadings } = await import(
  pathToFileURL(dir + '/marked-gfm-heading-id/src/index.js')
)

// Each case: [name, markdown]. Names are stable fixture keys.
const cases = [
  // basic ATX and setext, one heading per document
  ['atx-basic', '# Hello\n'],
  ['atx-level2', '## Hello\n'],
  ['atx-empty', '## ##\n'],
  ['atx-hashes-only', '# ##\n'],
  ['atx-closing-seq', '## foo ##\n'],
  ['setext-basic', 'Title\n===\n'],
  ['setext-underline-h2', 'Title\n---\n'],
  ['setext-two-lines', 'a\nb\n===\n'],
  // dedup across the document, in document order
  ['dedup-3', '# a\n\n# a\n\n# a a\n'],
  ['dedup-case', '# word\n\ntext\n\n# word\n\ntext\n\n# WORD\n'],
  ['dedup-levels', '# Hello\n\n## Hello\n\n### Hello\n\n#### Hello\n'],
  // inline markup inside the heading
  ['code-span', '# A `great` worry\n'],
  ['emphasis', '# *em*phasis\n'],
  ['strong', '# **bold** claim\n'],
  ['link', '# [docs](https://example.com)\n'],
  ['image', '# a ![alt](/img.png) b\n'],
  ['autolink', '# <https://example.com>\n'],
  ['entity-text', '# caf&eacute;\n'],
  ['entity-tag', '# &lt;tag&gt;\n'],
  // raw HTML stripping semantics
  ['html-comment', '# <!-- a > b --> title\n'],
  ['html-processing', '# <?php ?> code\n'],
  ['html-open-tag', '# a <b> c\n'],
  ['html-close-tag', '# a </b> c\n'],
  ['html-unmatched', '# a < b\n'],
  ['html-spans-newline', '# a <b\n c> d\n'],
  // whitespace handling
  ['inner-spaces', '#   spaced   out  \n'],
  ['trailing-space-tag', '# x <b> \n'],
  // non-ASCII
  ['greek-final-sigma', '# ΟΔΟΣ\n'],
  ['cjk', '# 中文 标题\n'],
  ['emoji', '# emoji 😀\n'],
  // headings inside containers
  ['in-blockquote', '> # Quoted\n'],
  ['in-list', '- # In list\n'],
  ['in-list-loose', '- # In list\n\ntext\n'],
  // document structure around the headings
  ['interleaved', 'text\n\n# Tail\n\ntext\n\n## Tail\n'],
  ['leading-text', 'text\n\n# Tail\n'],
  ['trailing-paragraph', '# Head\n\nmore text\n'],
]

const marked = new Marked()
marked.use(gfmHeadingId())

const results = {}
for (const [name, markdown] of cases) {
  // Each document gets a fresh slugger: the package keeps module state.
  resetHeadings()
  const html = marked.parse(markdown)
  // The ids the reference generated, in document order.
  const ids = [...html.matchAll(/<h[1-6](?:\s[^>]*)?\sid="([^"]*)"/g)].map(
    (match) => match[1],
  )
  results[name] = { markdown, html, ids }
}

process.stdout.write(JSON.stringify(results, null, 1) + '\n')
