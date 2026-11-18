"""A host extension from Python: a fenced ``note`` block rendered through
hooks, plus a render override that claims ``TODO:`` list items.

A host extension is behavior supplied from the embedding language at run
time, declared with the same manifest the loader validates for a tier-2
extension (see `examples/extensions/`). Run this against an installed
wheel:

    pip install geist4
    python examples/bindings/notes.py
"""

import geist4


def block_start(line, indented, column, container):
    if line.strip() != ":::note":
        return None
    return {
        "consume": "whole_line",
        "kind": "note",
        "attrs": {"class": "tip"},
    }


def block_continue(line, indented, column, blank):
    if line.strip() == ":::":
        return {"consume": "close_line"}
    return {"consume": "rest"}


def render(kind, attrs, children):
    if kind == "note":
        return f'<aside class="tip">{children}</aside>\n'
    return None


def render_override(kind, attrs, children):
    if kind != "item" or not attrs.get("data-todo"):
        return None
    return f'<div class="todo">{"".join(children)}</div>'


def claim_todo(line, indented, column, container):
    if line.startswith("TODO: "):
        return {
            "consume": ("claim", 6),
            "kind": "",
            "attrs": {"data-todo": True},
        }
    return None


notes = geist4.HostExtension(
    "notes",
    {"note": {"container": True, "attrs": ["class"]}},
    block_start=block_start,
    block_continue=block_continue,
    render=render,
)

todos = geist4.HostExtension(
    "todos",
    block_start_look=lambda *_: True,
    block_start=claim_todo,
    render_override=render_override,
)

options = geist4.Options()
options.enable_host(notes)
options.enable_host(todos)

print(geist4.render_html(
    ":::note\nhello *world*\n:::\n\n- TODO: buy milk\n", options))
