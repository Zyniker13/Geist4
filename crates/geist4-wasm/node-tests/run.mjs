// The WebAssembly binding's contract test, run by Node without any
// dependency: the same byte-exact HTML, problem reporting, and error
// behavior the Rust tests (`crates/geist4-ffi/tests/safe.rs`) and the C
// test (`crates/geist4-ffi/c-tests/`) pin, asserted through the
// JavaScript glue so the three bindings cannot quietly diverge.
//
// Output: one `ok - description` or `FAIL - description (why)` line per
// check; the exit status is the number of failures.
//
// The glue is the checked-in `--target web` build by default; Node has no
// `fetch` for a sibling file, so the test hands `init` the bytes directly —
// the same entry point a wasm-only environment uses. GEIST4_PKG_DIR points
// the harness at another glue directory (check.sh uses it to exercise the
// freshly built glue next to the checked-in one).

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const pkgDir =
  process.env.GEIST4_PKG_DIR ??
  join(dirname(fileURLToPath(import.meta.url)), "..", "pkg");
const glue = await import(pathToFileURL(join(pkgDir, "geist4.js")).href);
const init = glue.default;
const { Document, Options, renderHtml, renderHtmlWith, version } = glue;

const failures = [];
let checks = 0;

function check(description, condition, why) {
  checks += 1;
  if (condition) {
    console.log(`ok - ${description}`);
  } else {
    const reason = why === undefined ? "" : ` (${why})`;
    console.log(`FAIL - ${description}${reason}`);
    failures.push(description);
  }
}

function deepEqual(actual, expected) {
  return JSON.stringify(actual) === JSON.stringify(expected);
}

await init({
  module_or_path: readFileSync(join(pkgDir, "geist4_bg.wasm")),
});

check("the version reads", typeof version() === "string" && version().length > 0);

check(
  "a heading renders byte for byte",
  renderHtml("# hi\n") === "<h1>hi</h1>\n",
  JSON.stringify(renderHtml("# hi\n")),
);

check(
  "inline markup renders byte for byte",
  renderHtml("*a* **b** `c`\n") ===
    "<p><em>a</em> <strong>b</strong> <code>c</code></p>\n",
  JSON.stringify(renderHtml("*a* **b** `c`\n")),
);

check(
  "plain rendering leaves strikethrough literal",
  renderHtml("~~gone~~\n") === "<p>~~gone~~</p>\n",
  JSON.stringify(renderHtml("~~gone~~\n")),
);

const gfm = new Options();
check(
  "an unknown extension throws with the valid names",
  (() => {
    try {
      new Options().enable_builtin("nope");
      return false;
    } catch (error) {
      return (
        error instanceof Error &&
        error.message.startsWith("unknown extension `nope`")
      );
    }
  })(),
);

check(
  "enable_builtin accepts a valid name twice",
  (() => {
    gfm.enable_builtin("gfm");
    gfm.enable_builtin("gfm");
    return true;
  })(),
);

check(
  "the gfm bundle enables strikethrough",
  renderHtmlWith("~~gone~~\n", gfm) === "<p><del>gone</del></p>\n",
  JSON.stringify(renderHtmlWith("~~gone~~\n", gfm)),
);

check(
  "a table renders byte for byte",
  renderHtmlWith("| a | b |\n| --- | --- |\n| 1 | 2 |\n", gfm) ===
    "<table>\n<thead>\n<tr>\n<th>a</th>\n<th>b</th>\n</tr>\n</thead>\n<tbody>\n<tr>\n<td>1</td>\n<td>2</td>\n</tr>\n</tbody>\n</table>\n",
  JSON.stringify(renderHtmlWith("| a | b |\n| --- | --- |\n| 1 | 2 |\n", gfm)),
);

check(
  "a tasklist item renders the checkbox",
  renderHtmlWith("- [x] done\n", gfm) ===
    '<ul>\n<li><input type="checkbox" checked="" disabled="" /> done</li>\n</ul>\n',
  JSON.stringify(renderHtmlWith("- [x] done\n", gfm)),
);

const document = Document.parseWith("~~x~~ [a][nope]\n", gfm);
check(
  "the document keeps rendering the same bytes",
  document.html() === "<p><del>x</del> [a][nope]</p>\n" &&
    document.html() === document.html(),
  JSON.stringify(document.html()),
);

const problems = document.problems();
check(
  "one broken reference is one problem",
  problems.length === 1 &&
    problems[0].message ===
      "no link reference definition for 'nope'" &&
    problems[0].start === 0 &&
    problems[0].end === "~~x~~ [a][nope]\n".length,
  JSON.stringify(problems),
);

check(
  "a defined reference leaves no problems",
  Document.parse("[text][ref]\n\n[ref]: https://example.com/x\n", null).problems()
    .length === 0,
);

check(
  "a chain of broken groups reports each distinct label",
  deepEqual(
    Document.parse("[a][b][c]\n", null)
      .problems()
      .map((problem) => problem.message),
    [
      "no link reference definition for 'b'",
      "no link reference definition for 'c'",
    ],
  ),
);

check(
  "a NUL byte renders as the replacement character",
  renderHtml("a\u0000b\n") === "<p>a\uFFFDb</p>\n",
  JSON.stringify(renderHtml("a\u0000b\n")),
);

check(
  "an empty document parses and renders empty",
  Document.parse("", null).html() === "",
);

process.exit(failures.length);
