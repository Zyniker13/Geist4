#!/usr/bin/env python3
"""Generates the conformance kit's manifest cases.

Each case is a directory conformance/manifest/<NN-name>/ holding an
extension.toml (authored here, deliberately mutated from a valid base)
and an expected.txt (captured from the real loader, never invented).
The corpus is checked in; this script documents how it was produced
and regenerates it after an intentional change to the loader's
messages. It needs the CLI built first:

    cargo build -p geist4-cli --features geist4-ext-loader/sandbox
"""
import pathlib, subprocess, sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUT = ROOT / "conformance" / "manifest"

# Every case ships the same minimal spec.md: a package is
# extension.toml + spec.md by definition, and the loader requires both.
SPEC = """\
# sample

    geist4 ext manifest .

## Examples

```````````````````````````````` example
This is ==highlighted== text.
.
<p><sample>highlighted</sample> text.</p>
````````````````````````````````
"""

BASE = """\
[extension]
name = "sample"
version = "1.0.0"
format = "1.0"
description = "A sample span"
license = "MIT"
hooks = ["inline_span"]

[[kinds]]
kind = "sample"
inline_text = true

[[inline_span]]
kind = "sample"
open = "=="
close = "=="
html = "<sample>{children}</sample>"
"""

# name -> (manifest text, stable reason token or None for ok)
CASES = {
    "01-minimal-ok": (BASE, None),
    "02-format-major": (BASE.replace('format = "1.0"', 'format = "2.0"'), "format-unsupported"),
    "03-format-minor": (BASE.replace('format = "1.0"', 'format = "1.1"'), "format-unsupported"),
    "04-format-patch-ok": (BASE.replace('format = "1.0"', 'format = "1.0.2"'), None),
    "05-reserved-name": (BASE.replace('name = "sample"', 'name = "tables"'), "reserved-name"),
    "06-bad-name": (BASE.replace('name = "sample"', 'name = "Bad_Name"'), "bad-name"),
    "07-unknown-hook": (BASE.replace('hooks = ["inline_span"]', 'hooks = ["frobnicate"]'), "unknown-hook"),
    "08-duplicate-kind": (
        BASE + '\n[[kinds]]\nkind = "sample"\ninline_text = true\n', "duplicate-kind"),
    "09-unreferenced-kind": (
        BASE.replace('[[inline_span]]', '[[kinds]]\nkind = "extra"\n\n[[inline_span]]'), "unreferenced-kind"),
    "10-unknown-kind-ref": (BASE.replace('kind = "sample"\nopen', 'kind = "ghost"\nopen'), "unknown-kind"),
    "11-delimiter-too-long": (BASE.replace('open = "=="', 'open = "================="'), "bad-delimiter"),
    "12-container-kind-mismatch": (
        BASE.replace('[[inline_span]]', '[[block_container]]')
            .replace('open = "=="\nclose = "=="\nhtml = "<sample>{children}</sample>"',
                     'marker = "::: sample"\ntemplate = "<sample>\\n{children}</sample>\\n"'),
        "kind-not-container"),
    "13-unknown-substitution": (BASE.replace('{children}', '{bogus}'), "unknown-substitution"),
    "14-wasm-no-path": (BASE.replace('[[inline_span]]', '[wasm]\n\n'), "wasm-path-missing"),
    "15-wasm-missing-file": (BASE.replace('[[inline_span]]', '[wasm]\npath = "component.wasm"\n\n'), "wasm-file-missing"),
    "16-inline-trigger-nonprintable": (
        BASE.replace('[[inline_span]]', '[wasm]\npath = "component.wasm"\ninline_triggers = ["\\t"]\n\n'),
        "bad-inline-trigger"),
    "17-two-tiers": (BASE + '\n[wasm]\npath = "component.wasm"\n', "tier-conflict"),
    "18-option-default-type": (
        BASE + '\n[options.lang]\ntype = "number"\ndefault = "en"\ndescription = "language"\n',
        "option-default-type"),
    "19-replace-empty-from": (BASE.replace('[[inline_span]]', '[[replace]]')
        .replace('open = "=="\nclose = "=="\nhtml = "<sample>{children}</sample>"',
                 'from = ""\nto = "x"'), "bad-replacement"),
    "20-container-marker-too-short": (
        BASE.replace('[[inline_span]]', '[[kinds]]\nkind = "boxed"\ncontainer = true\n\n[[block_container]]')
            .replace('kind = "sample"\nopen = "=="\nclose = "=="\nhtml = "<sample>{children}</sample>"',
                     'kind = "boxed"\nmarker = "::"\ntemplate = "<boxed>\\n{children}</boxed>\\n"'),
        "bad-marker"),
}

def main():
    binpath = ROOT / "target" / "debug" / "geist4"
    if not binpath.exists():
        sys.exit("build the CLI first: cargo build -p geist4-cli")
    for name, (text, token) in sorted(CASES.items()):
        d = OUT / name
        d.mkdir(parents=True, exist_ok=True)
        (d / "extension.toml").write_text(text)
        (d / "spec.md").write_text(SPEC)
        proc = subprocess.run([str(binpath), "ext", "manifest", str(d)],
                              capture_output=True, text=True)
        ok = proc.returncode == 0
        if token is None and not ok:
            sys.exit(f"{name}: expected ok, got exit {proc.returncode}: {proc.stderr.strip()}")
        if token is not None and ok:
            sys.exit(f"{name}: expected an error, got exit 0")
        if ok:
            (d / "expected.txt").write_text("ok\n")
        else:
            # Strip the "<path>: " prefix the CLI adds before the loader's
            # one-line message.
            message = proc.stderr.strip()
            prefix = f"{d}: "
            if message.startswith(prefix):
                message = message[len(prefix):]
            (d / "expected.txt").write_text(f"error\nreason: {token}\n{message}\n")
        print(f"{name}: {'ok' if ok else 'error'} -> {message if not ok else ''}")

if __name__ == "__main__":
    main()
