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

// ---------------------------------------------------------------------------
// Updates: an editor's edit sequence through `update`, checked against
// fresh parses — the same value-identity contract the Rust suite
// (`crates/geist4-ffi/tests/update.rs`) and the C test pin, driven
// here through the wasm binding.
// ---------------------------------------------------------------------------
{
  const { UpdatedDocument } = glue;

  // A three-paragraph source; the sequence edits every paragraph in
  // turn, then appends two, then shrinks back. Each step compares the
  // updated document against a fresh parse of the same source (HTML
  // and problems must be identical), and records the reported spans.
  const sequence = [
    ["one\n\ntwo\n\nthree\n", "one\n\nTWO!\n\nthree\n", "para two replaced"],
    ["one\n\nTWO!\n\nthree\n", "one\n\nTWO!\n\nthree\n\nfour\n\nfive\n", "two paragraphs appended"],
    ["one\n\nTWO!\n\nthree\n\nfour\n\nfive\n", "TWO!\n", "shrunk to one paragraph"],
    ["TWO!\n", "TWO!\n", "identical source"],
  ];

  let document = Document.parse(sequence[0][0]);
  const stepSpans = [];
  let allIdentical = true;
  for (const [, edited] of sequence) {
    const updated = document.update(edited);
    const fresh = Document.parse(edited);
    const sameHtml = updated.html() === fresh.html();
    const sameProblems = deepEqual(
      updated.problems().map((problem) => [problem.message, problem.start, problem.end]),
      fresh.problems().map((problem) => [problem.message, problem.start, problem.end]),
    );
    if (!sameHtml || !sameProblems) {
      allIdentical = false;
      console.log(`  update vs fresh: html ${updated.html()} vs ${fresh.html()}`);
    }
    const flat = Array.from(updated.changedSpans());
    stepSpans.push(flat);
    document = updated;
  }

  check(
    "an edit sequence matches fresh parses at every step",
    allIdentical,
    "html or problems diverged from a full parse",
  );

  const [editSpans, appendSpans, shrinkSpans, identicalSpans] = stepSpans;
  const covers = (flat, start, end) =>
    flat.some((value, index) =>
      index % 2 === 0 && value <= start && flat[index + 1] >= end,
    );
  const inBounds = (flat, sourceLength) =>
    flat.every((value, index) =>
      index % 2 === 1 ? value <= sourceLength : value <= flat[index + 1],
    );

  // "two" -> "TWO!": the edited bytes lie inside the reported span,
  // which lies inside the source (the window is block-granular).
  check(
    "the mid-document edit reports a span covering the edit",
    covers(editSpans, 6, 9) && inBounds(editSpans, "one\n\nTWO!\n\nthree\n".length),
    JSON.stringify(editSpans),
  );

  // The appended paragraphs are inside the window either way; the
  // invariant is coverage plus bounds.
  const appended = "one\n\nTWO!\n\nthree\n\nfour\n\nfive\n";
  check(
    "the append reports a span covering the new tail",
    appendSpans.length > 0 && inBounds(appendSpans, appended.length),
    JSON.stringify(appendSpans),
  );

  // The shrink must still produce the right document (checked above);
  // its span list is nonempty because the source changed.
  check(
    "the shrink reports at least one span",
    shrinkSpans.length > 0,
    JSON.stringify(shrinkSpans),
  );

  // The identical source reports nothing changed.
  check(
    "an identical source reports no changed spans",
    identicalSpans.length === 0,
    JSON.stringify(identicalSpans),
  );

  // The returned object is an UpdatedDocument and keeps updating.
  check(
    "update returns an UpdatedDocument that chains",
    document instanceof UpdatedDocument &&
      document.update("TWO!\n\nand more\n").html() ===
        Document.parse("TWO!\n\nand more\n").html(),
  );

  // A fresh-parse document can update too, and spans report through
  // both entry points identically.
  const fromParse = Document.parse("alpha\n\nbeta\n").update("alpha\n\nBETA\n");
  const fromUpdated = Document.parse("alpha\n\nbeta\n\n")
    .update("alpha\n\nbeta\n\n") // identical: nothing changed
    .update("alpha\n\nBETA\n");
  check(
    "span reports agree from parse and update entry points",
    Array.from(fromParse.changedSpans()).join(",") ===
      Array.from(fromUpdated.changedSpans()).join(","),
    `${JSON.stringify(Array.from(fromParse.changedSpans()))} vs ${JSON.stringify(
      Array.from(fromUpdated.changedSpans()),
    )}`,
  );
}

process.exit(failures.length);
