// The cross-binding suite's JavaScript runner: renders the case list
// through the WebAssembly binding and writes one JSON line per case,
// the same shape the Rust reference (`cross_reference`) and the other
// bindings' runners write. The driver (`tools/cross_binding.py`)
// requires every binding's html for every case to match the reference's
// byte for byte.
//
// Usage: node cross.mjs <cases.jsonl> <results.jsonl>
//
// The glue is the checked-in `--target web` build, or the fresh one at
// GEIST4_PKG_DIR; Node has no `fetch` for a sibling file, so the test
// hands `init` the bytes directly, as `run.mjs` does.

import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const [casesPath, resultsPath] = process.argv.slice(2);
if (!casesPath || !resultsPath) {
  console.error("usage: node cross.mjs <cases.jsonl> <results.jsonl>");
  process.exit(2);
}

const pkgDir =
  process.env.GEIST4_PKG_DIR ??
  join(fileURLToPath(new URL(".", import.meta.url)), "..", "pkg");
const glue = await import(pathToFileURL(join(pkgDir, "geist4.js")).href);
await glue.default({
  module_or_path: readFileSync(join(pkgDir, "geist4_bg.wasm")),
});
const { Options, renderHtmlWith } = glue;

const optionsCache = new Map();

function optionsFor(names) {
  const key = names.join("\u0000");
  let options = optionsCache.get(key);
  if (!options) {
    options = new Options();
    for (const name of names) {
      options.enable_builtin(name);
    }
    optionsCache.set(key, options);
  }
  return options;
}

const failures = [];
const lines = readFileSync(casesPath, "utf8").split("\n");
if (lines.at(-1) === "") lines.pop();
const out = lines.map((line) => {
  if (!line) return line;
  const testCase = JSON.parse(line);
  let html;
  try {
    html = renderHtmlWith(testCase.source, optionsFor(testCase.options ?? []));
  } catch (error) {
    failures.push(`${testCase.name}: ${error.message ?? error}`);
    html = null;
  }
  return JSON.stringify({ name: testCase.name, html });
});

writeFileSync(resultsPath, out.join("\n") + "\n");
if (failures.length > 0) {
  for (const failure of failures) {
    console.error(`cross.mjs: ${failure}`);
  }
  process.exit(1);
}
