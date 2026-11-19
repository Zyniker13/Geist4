"""The cross-binding suite's Python runner: renders the case list
through the installed wheel and writes one JSON line per case, the same
shape the Rust reference (`cross_reference`) and the other bindings'
runners write. The driver (`tools/cross_binding.py`) requires every
binding's html for every case to match the reference's byte for byte.

Usage: python cross_binding.py <cases.jsonl> <results.jsonl>

Runs against the wheel pip installs (see TESTING.md), so it tests what
a pip user gets.
"""

import json
import sys

import geist4

options_cache: dict[tuple, object] = {}


def options_for(names):
    key = tuple(names)
    options = options_cache.get(key)
    if options is None:
        options = geist4.Options()
        for name in names:
            options.enable_builtin(name)
        options_cache[key] = options
    return options


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: python cross_binding.py <cases.jsonl> <results.jsonl>", file=sys.stderr)
        return 2
    cases_path, results_path = sys.argv[1], sys.argv[2]

    failures: list[str] = []
    lines = open(cases_path, encoding="utf-8").read().split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    with open(results_path, "w", encoding="utf-8", newline="\n") as out:
        for line in lines:
            if not line:
                continue
            case = json.loads(line)
            try:
                html = geist4.render_html(case["source"], options_for(case.get("options", [])))
            except Exception as error:
                failures.append(f"{case['name']}: {error}")
                html = None
            out.write(json.dumps({"name": case["name"], "html": html}, ensure_ascii=False) + "\n")

    if failures:
        for failure in failures:
            print(f"cross_binding.py: {failure}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
