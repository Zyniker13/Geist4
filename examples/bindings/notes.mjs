// A host extension from JavaScript (the wasm binding): a fenced `note`
// block rendered through hooks, plus a render override that claims
// `TODO:` list items.
//
// Run with Node against the pkg/ glue wasm-bindgen emits (this example
// is repo-relative; a package user imports "geist4" instead):

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const pkgDir = join(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "crates",
  "geist4-wasm",
  "pkg",
);
const glue = await import(pathToFileURL(join(pkgDir, "geist4.js")).href);
const init = glue.default;
await init({
  module_or_path: readFileSync(join(pkgDir, "geist4_bg.wasm")),
});

const { Document, Options, HostExtension } = glue;

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
      kind === "note" ? `<aside class="tip">${children}</aside>\n` : null,
  },
});

const todos = new HostExtension({
  name: "todos",
  hooks: {
    blockStartLook: () => true,
    blockStart: (line) =>
      line.startsWith("TODO: ")
        ? {
            consume: ["claim", 6],
            kind: "",
            attrs: { "data-todo": true },
          }
        : null,
    renderOverride: (kind, attrs, children) => {
      if (kind !== "item" || attrs["data-todo"] !== true) return null;
      return `<div class="todo">${children.join("")}</div>`;
    },
  },
});

const options = new Options();
options.enableHost(notes);
options.enableHost(todos);

const document = Document.parseWith(
  ":::note\nhello *world*\n:::\n\n- TODO: buy milk\n",
  options,
);
console.log(document.html());
