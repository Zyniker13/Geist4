// Probe github-slugger 2.0.0 exhaustively: plain slug() and Slugger dedup.
// Output: JSON on stdout, consumed to freeze crates/geist4-ext test fixtures.
// Usage: node tools/slugger_probe.mjs [path-to-node_modules-dir]
import { pathToFileURL } from 'node:url'
import process from 'node:process'

const dir = process.argv[2] ?? 'node_modules'
const { regex } = await import(pathToFileURL(dir + '/github-slugger/regex.js'))
const { default: Slugger, slug } = await import(
  pathToFileURL(dir + '/github-slugger/index.js')
)

// Strings probed one at a time with a FRESH Slugger each (pure slug semantics).
const strings = [
  // ASCII words, spaces, case
  'Hello World', 'hello world', 'HELLO WORLD', 'Hello, World!',
  'A', 'a', 'A B', 'a b', 'A  B',
  // punctuation soup
  'foo (bar) baz', 'foo: bar', 'foo-bar', 'foo_bar', 'foo.bar', 'foo/bar',
  'foo\\bar', "foo'bar", 'foo"bar', 'foo[bar]', 'foo{bar}', 'foo<bar>',
  'foo=bar', 'foo&bar', 'foo*bar', 'foo+bar', 'foo%bar', 'foo$bar',
  'foo#bar', 'foo@bar', 'foo!bar', 'foo?bar', 'foo|bar', 'foo;bar',
  'foo,bar', 'foo~bar', 'foo^bar', 'foo`bar', "foo'bar",
  // dashes and whitespace edge cases
  'a - b', 'a  -  b', ' - ', '-', '---', '-a-', 'a-', '-a',
  '   ', '   leading', 'trailing   ', ' ', '', '\t', '\n', 'a\tb',
  'a\nb', 'a\rb', 'a\tb\nc', 'a b', 'a b', 'a b',
  'a​b', 'a­b', 'a‍b', 'a b',
  // numbers and the classic GitHub example
  '1. Some text & 2. Some code', '123', '0 0', 'Version 1.2.3',
  '§1 Definitions', '#hashtag', '%100', '@user',
  // case-mapping probes (JS toLowerCase vs Rust)
  'ΟΔΟΣ', 'ΟΣ', 'ΟΔΟΣ ΣΟΣ', 'İstanbul', 'ıI', 'ß', 'ẞ STRASSE',
  'ǅ Dz Ǆ', 'ΣΣΣ', 'ΑΒΓ', 'αβγ', 'Tür', 'TÜRK', 'Straße',
  'ＡＢＣ', 'ʼnʼ', 'Kʼ', 'Ｒｕｓｔ',
  // scripts and blocks
  '中文字符', '中文 标题', '日本語テスト', '한국어 제목', 'Ελληνικά',
  'Русский текст', 'العربية', 'עברית', 'हिन्दी', 'ไทย',
  'Éléments', 'coïncidence', 'Naïve', 'Ελληνική Γλώσσα',
  // emoji, symbols, astral
  '😀', '😀 🎉', '👨‍👩‍👧‍👦 family', '👍🏽 thumb', '🇺🇸 flag',
  'x²', 'x³', 'ﬁ ligature', '𝐀 bold', '𠀀 ideograph', '🐶🐱',
  '✓ check', '★ star', '℃ degree', 'Ⅻ roman', '½ half',
  '− minus sign', '⁄ fraction slash',
  // things with marks and joins
  'é', 'é', 'ñ', 'g̈', 'אָלף', 'ᜀ ᜁ',
  // github-slugger README cases and misc
  'This & That', 'foo.bar.baz', 'foo bar baz', 'Foo: Bar?', '5 + 5 = 10',
  'C++', 'C#', 'F#', 'node.js', 'Obj-C', '.net', 'FAQ 2.0',
  'What’s New?', '“quoted”', '‘single’', '«guillemet»',
  // U+00A0 and friends (kept per table)
  'anbspb', 'a­b', 'x y',
]

// Every ASCII code point alone.
for (let cp = 0; cp < 128; cp++) strings.push(String.fromCodePoint(cp))
// Curated non-ASCII singles around removal boundaries and case edges.
const singles = [
  0x00AD, 0x00A0, 0x00A9, 0x00AB, 0x00B2, 0x00B5, 0x00B6, 0x00B9, 0x00BB,
  0x0130, 0x0131, 0x017F, 0x02BC, 0x0345, 0x03C2, 0x03C3, 0x05D0, 0x0640,
  0x1680, 0x180E, 0x2000, 0x200B, 0x200C, 0x200D, 0x2010, 0x2013, 0x2018,
  0x2019, 0x201C, 0x201D, 0x2028, 0x2029, 0x2032, 0x203B, 0x2060, 0x2192,
  0x2E3A, 0x3000, 0x3001, 0x3041, 0x30A1, 0x4E2D, 0x9FBF, 0x9FFC, 0xA000,
  0xAC00, 0xD7A3, 0xE000, 0xF8FF, 0xFB00, 0xFEFF, 0xFF01, 0xFF21, 0xFF41,
  0x1F600, 0x1F1E6, 0x1D400, 0x20000, 0x2FFFF, 0x30000, 0x10FFFF, 0xE01EF,
  0x1E900, 0x10D40, 0x11F00, 0x1AFF0, 0x1CF00, 0x1D2C0, 0x1E030,
]
for (const cp of singles) strings.push(String.fromCodePoint(cp))

const results = {}
{
  const s = new Slugger()
  for (const str of strings) {
    let key = str
    if (results[key] !== undefined) key = JSON.stringify(str) + results[key]++
    else results[key] = 0
    results['slug:' + key] = slug(str)
  }
}

// Dedup sequences: [label, [args...]...] where an arg is a string or
// [string, maintainCase].
const dedupScripts = [
  ['repeatA', ['A', 'A', 'A', 'a', 'A', 'a']],
  ['repeatAa', ['A a', 'A a', 'a A', 'a  a']],
  ['collide-dash1', ['a', 'a-1', 'a', 'a-2', 'a', 'a-1']],
  ['collide-dash2', ['a', 'a', 'a-1', 'a-2', 'a', 'a-1', 'a-3', 'a']],
  ['numSuffix', ['x', 'x', 'x', 'x-1', 'x', 'x-1', 'x-2', 'x']],
  ['empty', ['', '', '']],
  ['dash', ['-', '-', '-']],
  ['space', [' ', ' ', ' ']],
  ['case', ['Foo', 'foo', 'Foo', ['FOO', true], 'foo', ['Foo', true], ['FOO', true]]],
  ['maintain', [['A', true], 'a', ['A', true], 'A', ['a', true], ['a', false]]],
  ['unicode', ['É', 'é', 'É', 'Ε', 'ε', 'İ', 'i̇', 'İ']],
  ['emoji', ['😀', '😀', '😀 🎉', '😀', '🎉 😀', '😀 🎉']],
  ['mixed', ['§1 a', '§1 a', '§1 a', '1. a', '1. a']],
  ['collideNum', ['a-1', 'a', 'a', 'a', 'a-1', 'a-1']],
]

const dedup = {}
for (const [label, args] of dedupScripts) {
  const s = new Slugger()
  dedup[label] = args.map((a) =>
    Array.isArray(a) ? s.slug(a[0], a[1]) : s.slug(a)
  )
}

// Maintain-case one-offs (fresh slugger each).
const maintain = {}
for (const str of ['Hello World', 'HELLO', 'MiXeD CaSe', 'A', 'ΟΔΟΣ']) {
  const s = new Slugger()
  maintain[str] = s.slug(str, true)
}

process.stdout.write(
  JSON.stringify({ regexSource: regex.source, results, dedup, maintain }, null, 1)
)
