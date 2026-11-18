"""Geist4: a CommonMark-conformant Markdown engine, native Python binding.

The surface mirrors the engine's other bindings: `version`, `render_html`,
`Options` with `enable_builtin`, and `Document` with `html` and `problems`.
Errors are one type, `Geist4Error`.
"""

from ._geist4 import (
    Document,
    Geist4Error,
    Options,
    Problem,
    render_html,
    version,
)

__all__ = ["Document", "Geist4Error", "Options", "Problem", "render_html", "version"]
