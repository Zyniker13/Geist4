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
// The problem's span is the paragraph's: its last content byte, before
// the final newline — the reference rule core/tests/problems.rs pins.
check(
  "one broken reference is one problem",
  problems.length === 1 &&
    problems[0].message ===
      "no link reference definition for 'nope'" &&
    problems[0].start === 0 &&
    problems[0].end === "~~x~~ [a][nope]".length,
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

// ---------------------------------------------------------------------------
// Host extensions: JavaScript callbacks through the safe module — the same
// nine contract checks the Swift and Python bindings run, so the tier-3
// surface is one contract reached three ways.
// ---------------------------------------------------------------------------
{
  const { HostExtension } = glue;

  const notes = new HostExtension({
    name: "notes",
    kinds: { note: { container: true, attrs: ["class"] } },
    hooks: {
      blockStartLook: (line) => line.startsWith(":::"),
      blockStart: (line) =>
        line.slice(3).trim() === "note"
          ? { consume: "whole_line", kind: "note", attrs: { class: "tip" } }
          : null,
      blockContinue: (line) =>
        line.trim() === ":::" ? { consume: "close_line" } : { consume: ["columns", 0] },
      render: (kind, attrs, children) =>
        kind === "note" && attrs.class === "tip"
          ? `<aside class="tip">${children}</aside>\n`
          : null,
    },
  });
  const notesOptions = new Options();
  notesOptions.enableHost(notes);
  check(
    "host extension renders a fenced block through the hooks",
    renderHtmlWith(":::note\nhello *world*\n:::\n", notesOptions) ===
      '<aside class="tip"><p>hello <em>world</em></p>\n</aside>\n',
    JSON.stringify(renderHtmlWith(":::note\nhello *world*\n:::\n", notesOptions)),
  );

  const signs = new HostExtension({
    name: "signs",
    kinds: { sign: { container: true } },
    hooks: {
      blockStart: (line) =>
        line.startsWith("%%%") ? { consume: "whole_line", kind: "sign" } : null,
      blockContinue: (line) =>
        line.startsWith("%%%") ? { consume: "close_line" } : { consume: ["columns", 0] },
      blockClose: () => '<hr class="sign">\n',
    },
  });
  const signsOptions = new Options();
  signsOptions.enableHost(signs);
  check(
    "host block close owns the whole output",
    renderHtmlWith("%%%\nhidden\n%%%\n", signsOptions) === '<hr class="sign">\n',
    JSON.stringify(renderHtmlWith("%%%\nhidden\n%%%\n", signsOptions)),
  );

  const todos = new HostExtension({
    name: "todos",
    hooks: {
      blockStartLook: (line) => line.startsWith("TODO: "),
      blockStart: (line) =>
        line.startsWith("TODO: ")
          ? { consume: ["claim", 6], kind: "", attrs: { "data-todo": true } }
          : null,
      renderOverride: (kind, attrs, children) => {
        if (kind !== "item" || attrs["data-todo"] !== true) return null;
        let html = '<div class="todo">';
        for (const piece of children) html += piece;
        return `${html}</div>`;
      },
    },
  });
  const todosOptions = new Options();
  todosOptions.enableHost(todos);
  check(
    "host claim renders through the override",
    renderHtmlWith("- TODO: buy milk\n", todosOptions) ===
      '<ul>\n<div class="todo">buy milk</div></ul>\n',
    JSON.stringify(renderHtmlWith("- TODO: buy milk\n", todosOptions)),
  );

  const marks = new HostExtension({
    name: "marks",
    kinds: { mark: { inline_text: true } },
    triggers: ["="],
    hooks: {
      inlineTrigger: () => "mark",
      render: (kind, attrs, children) =>
        kind === "mark" ? `<mark>${children}</mark>` : null,
    },
  });
  const marksOptions = new Options();
  marksOptions.enableHost(marks);
  check(
    "host inline trigger tags and renders a run",
    renderHtmlWith("a ==bold== b\n", marksOptions) === "<p>a <mark>bold</mark> b</p>\n",
    JSON.stringify(renderHtmlWith("a ==bold== b\n", marksOptions)),
  );
  check(
    "an unmatched trigger run stays text",
    renderHtmlWith("a ==x b\n", marksOptions) === "<p>a ==x b</p>\n",
    JSON.stringify(renderHtmlWith("a ==x b\n", marksOptions)),
  );

  const refs = new HostExtension({
    name: "refs",
    kinds: { "note-ref": { inline_text: true } },
    hooks: {
      inlineBracket: (content) =>
        content === "note" ? { kind: "note-ref", end: 4294967295, attrs: {} } : null,
      render: (kind, attrs, children) =>
        kind === "note-ref" ? '<a class="note-ref">' + children + "</a>" : null,
    },
  });
  const refsOptions = new Options();
  refsOptions.enableHost(refs);
  check(
    "host bracket claims an unmatched group",
    renderHtmlWith("see [note] here\n", refsOptions) ===
      '<p>see <a class="note-ref">note</a> here</p>\n',
    JSON.stringify(renderHtmlWith("see [note] here\n", refsOptions)),
  );

  const zones = new HostExtension({
    name: "zones",
    hooks: { postProcess: (text) => text.replaceAll("GMT", "UTC") },
  });
  const zonesOptions = new Options();
  zonesOptions.enableHost(zones);
  const zonesSource = "Time GMT, see `GMT` code\n\n    GMT in code\n";
  check(
    "host post-process rewrites text but not code",
    renderHtmlWith(zonesSource, zonesOptions) ===
      "<p>Time UTC, see <code>GMT</code> code</p>\n<pre><code>GMT in code\n</code></pre>\n",
    JSON.stringify(renderHtmlWith(zonesSource, zonesOptions)),
  );

  const boom = new HostExtension({
    name: "boom",
    hooks: {
      postProcess: () => {
        throw new Error("host callback exploded");
      },
    },
  });
  const boomOptions = new Options();
  boomOptions.enableHost(boom);
  const boomDocument = Document.parseWith("hello GMT\n", boomOptions);
  check(
    "a failing host hook declines the parse",
    boomDocument.html() === "<p>hello GMT</p>\n",
    JSON.stringify(boomDocument.html()),
  );
  const boomProblems = boomDocument.problems();
  check(
    "a failing host hook latches one problem",
    boomProblems.length === 1,
    JSON.stringify(boomProblems),
  );
  check(
    "the latched message names the extension and the error",
    boomProblems[0].message === "extension 'boom': Error: host callback exploded",
    JSON.stringify(boomProblems[0].message),
  );
  check("host errors carry no span", boomProblems[0].start === 0);

  const rejects = (spec, within) => {
    const options = new Options();
    try {
      options.enableHost(new HostExtension(spec));
      return false;
    } catch (error) {
      return String(error.message).includes(within);
    }
  };
  const manyTriggers = [];
  for (let code = 33; manyTriggers.length < 33 && code < 127; code += 1) {
    const text = String.fromCharCode(code);
    if (!"\\`&<*_[]!".includes(text)) manyTriggers.push(text);
  }
  check(
    "host extension validation rejects bad declared data",
    rejects({ name: "Bad Name" }, "host extension:") &&
      rejects({ name: "tables" }, "reserved for a built-in") &&
      rejects({ name: "ok", kinds: { Nope: {} } }, 'host extension kind "Nope":') &&
      rejects(
        { name: "ok", kinds: { fine: { attrs: ["bad name"] } } },
        'attribute "bad name":',
      ) &&
      rejects({ name: "ok", triggers: manyTriggers }, "host extension triggers:"),
  );

  const dupOptions = new Options();
  dupOptions.enableHost(new HostExtension({ name: "dup" }));
  let dupRejected = false;
  try {
    dupOptions.enableHost(new HostExtension({ name: "dup" }));
  } catch (error) {
    dupRejected = String(error.message).includes("already registered");
  }
  check("a duplicate host extension name is rejected", dupRejected);
}

process.exit(failures.length);
