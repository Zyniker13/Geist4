"""Geist4: a CommonMark-conformant Markdown engine, native Python binding.

The surface mirrors the engine's other bindings: `version`, `render_html`,
`Options` with `enable_builtin` and `enable_host`, `HostExtension` for
host extensions with Python callbacks, and `Document` with `html` and
`problems`. Errors are one type, `Geist4Error`.
"""

from ._geist4 import (
    Document,
    Geist4Error,
    HostExtension,
    Options,
    Problem,
    render_html,
    version,
)

__all__ = [
    "Document",
    "Geist4Error",
    "HostExtension",
    "Options",
    "Problem",
    "render_html",
    "version",
]
