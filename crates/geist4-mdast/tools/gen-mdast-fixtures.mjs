// Generates the mdast export fixtures: for every CommonMark spec example and
// every probe input, the expected tree from the pinned reference
// implementation (mdast-util-from-markdown, see package.json).
//
// The spec example parsing mirrors crates/geist4-spec-harness/src/example.rs
// exactly (32-backtick fences, ` example` tag, `.` separator, tabs written
// as U+2192 arrows).
//
// Output: tests/fixtures/<number>.json for spec examples and
// tests/fixtures/probe-<name>.json for the files under tools/probes/.
// The fixtures are checked in; this script is dev-only and node_modules
// is never committed.

import { fromMarkdown } from 'mdast-util-from-markdown';
import { mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = new URL('.', import.meta.url).pathname;
const crateRoot = join(here, '..');
const repoRoot = join(crateRoot, '..', '..');
const outDir = join(crateRoot, 'tests', 'fixtures');

const FENCE = '`'.repeat(32);

/** Parses spec.txt into [{number, section, markdown}], like the harness. */
function parseSpecExamples(text) {
  const lines = text.split('\n');
  const examples = [];
  let section = null;
  let number = 0;
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (line.startsWith('#')) {
      section = line.replace(/^#+\s*/, '').trim();
      i += 1;
      continue;
    }
    if (line.trimEnd() === FENCE + ' example') {
      const content = [];
      i += 1;
      while (i < lines.length && lines[i].trimEnd() !== FENCE) {
        content.push(lines[i]);
        i += 1;
      }
      const sep = content.findIndex((l) => l.trimEnd() === '.');
      number += 1;
      const markdown = expandTabs(content.slice(0, sep).join('\n'));
      examples.push({ number, section, markdown });
      i += 1;
      continue;
    }
    i += 1;
  }
  return examples;
}

function expandTabs(text) {
  return text.replace(/\t/g, '\u2192').split('\n').map(expandLine).join('\n');
}

function expandLine(line) {
  let out = '';
  for (const ch of line) {
    if (ch === '\u2192') {
      const width = 4 - (out.length % 4);
      out += ' '.repeat(width);
    } else {
      out += ch;
    }
  }
  return out;
}

rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });

const spec = readFileSync(join(repoRoot, 'spec', 'spec.txt'), 'utf8');
const examples = parseSpecExamples(spec);
let count = 0;
for (const { number, section, markdown } of examples) {
  const expected = fromMarkdown(markdown);
  writeFileSync(
    join(outDir, `${number}.json`),
    JSON.stringify({ number, section, input: markdown, expected }),
  );
  count += 1;
}

const probeDir = join(here, 'probes');
let probes;
try {
  probes = readdirSync(probeDir).filter((f) => f.endsWith('.md')).sort();
} catch {
  probes = [];
}
for (const name of probes) {
  const input = readFileSync(join(probeDir, name), 'utf8');
  const expected = fromMarkdown(input);
  writeFileSync(
    join(outDir, `probe-${name.slice(0, -3)}.json`),
    JSON.stringify({ name: name.slice(0, -3), input, expected }),
  );
  count += 1;
}

console.log(`wrote ${count} fixtures to ${outDir}`);
