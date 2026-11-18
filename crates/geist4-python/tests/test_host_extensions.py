"""The host-extension surface's contract, mirroring the Rust battery in
`geist4-ffi/tests/host_extension.rs`: Python callbacks run through a
real parse and render exactly as the Rust closures do, every failure
path declines instead of raising through the engine, and the declared
data is validated when the extension is enabled, as a manifest would
be. It runs against the wheel the gate builds and installs, so it
tests exactly what a pip user gets.
"""

import unittest

import geist4


class FencedBlock(unittest.TestCase):
    def test_starts_continues_and_renders(self):
        def block_start(line, indented, column, container):
            if line.strip("::: \t") == "note":
                return {
                    "consume": "whole_line",
                    "kind": "note",
                    "attrs": {"class": "tip"},
                }
            return None

        def block_continue(line, indented, column, blank):
            if line.strip() == ":::":
                return {"consume": "close_line"}
            return {"consume": "rest"}

        def render(kind, attrs, children):
            self.assertEqual(kind, "note")
            self.assertEqual(attrs, {"class": "tip"})
            return f'<aside class="tip">{children}</aside>\n'

        ext = geist4.HostExtension(
            "notes",
            {"note": {"container": True, "attrs": ["class"]}},
            block_start=block_start,
            block_continue=block_continue,
            render=render,
        )
        options = geist4.Options()
        options.enable_host(ext)
        html = geist4.render_html(":::note\nhello *world*\n:::\n", options)
        self.assertEqual(
            html,
            '<aside class="tip"><p>hello <em>world</em></p>\n</aside>\n',
            "the render hook owns the block's output with its children inside",
        )


class BlockClose(unittest.TestCase):
    def test_close_html_replaces_the_children(self):
        def block_start(line, indented, column, container):
            if line.startswith("%%%"):
                return {"consume": "whole_line", "kind": "sign"}
            return None

        def block_continue(line, indented, column, blank):
            if line.startswith("%%%"):
                return None
            return {"consume": "rest"}

        ext = geist4.HostExtension(
            "signs",
            {"sign": {"container": True}},
            block_start=block_start,
            block_continue=block_continue,
            block_close=lambda: '<hr class="sign">\n',
        )
        options = geist4.Options()
        options.enable_host(ext)
        html = geist4.render_html("%%%\nhidden\n%%%\n", options)
        self.assertEqual(
            html,
            '<hr class="sign">\n',
            "the close hook's HTML is the whole output; the children render nowhere",
        )


class Claim(unittest.TestCase):
    def test_marks_the_container_and_the_override_renders(self):
        def block_start(line, indented, column, container):
            if line.startswith("TODO: "):
                return {
                    "consume": ("claim", 6),
                    "kind": "",
                    "attrs": {"data-todo": True},
                }
            return None

        def render_override(kind, attrs, children):
            self.assertEqual(attrs, {"data-todo": True})
            return f'<div class="todo">{"".join(children)}</div>'

        ext = geist4.HostExtension(
            "todos", block_start=block_start, render_override=render_override
        )
        options = geist4.Options()
        options.enable_host(ext)
        html = geist4.render_html("- TODO: buy milk\n", options)
        self.assertEqual(
            html,
            '<ul>\n<div class="todo">buy milk</div></ul>\n',
            "the claimed item renders only through the override; a tight "
            "list's paragraphs render bare",
        )


class InlineTrigger(unittest.TestCase):
    def test_tags_a_run_and_renders(self):
        ext = geist4.HostExtension(
            "marks",
            {"mark": []},
            ["="],
            inline_trigger=lambda text, column: "mark",
            render=lambda kind, attrs, children: (
                f"<mark>{children}</mark>" if kind == "mark" else None
            ),
        )
        options = geist4.Options()
        options.enable_host(ext)
        html = geist4.render_html("a ==bold== b\n", options)
        self.assertEqual(
            html,
            "<p>a <mark>bold</mark> b</p>\n",
            "the paired runs wrapped their content through the hook",
        )
        html = geist4.render_html("a ==x b\n", options)
        self.assertEqual(html, "<p>a ==x b</p>\n", "an unclosed run stays literal text")


class InlineBracket(unittest.TestCase):
    def test_claims_an_unmatched_group(self):
        def inline_bracket(text):
            if text == "note":
                return {"kind": "note-ref", "end": 4294967295}
            return None

        ext = geist4.HostExtension(
            "refs",
            {"note-ref": []},
            inline_bracket=inline_bracket,
            render=lambda kind, attrs, children: (
                f'<a class="note-ref">{children}</a>' if kind == "note-ref" else None
            ),
        )
        options = geist4.Options()
        options.enable_host(ext)
        html = geist4.render_html("see [note] here\n", options)
        self.assertEqual(
            html,
            '<p>see <a class="note-ref">note</a> here</p>\n',
            "the unmatched bracket group rendered through the claim",
        )


class PostProcess(unittest.TestCase):
    def test_rewrites_text_only(self):
        ext = geist4.HostExtension(
            "zones", post_process=lambda text: text.replace("GMT", "UTC")
        )
        options = geist4.Options()
        options.enable_host(ext)
        html = geist4.render_html("Time GMT, see `GMT` code\n\n    GMT in code\n", options)
        self.assertEqual(
            html,
            "<p>Time UTC, see <code>GMT</code> code</p>\n"
            "<pre><code>GMT in code\n</code></pre>\n",
            "text nodes are offered; code is not",
        )


class FailureLatch(unittest.TestCase):
    def test_raises_and_surfaces_once(self):
        def post_process(text):
            raise RuntimeError("host callback exploded")

        ext = geist4.HostExtension("boom", post_process=post_process)
        options = geist4.Options()
        options.enable_host(ext)
        document = geist4.Document("hello GMT\n", options)
        html = document.html()
        self.assertEqual(html, "<p>hello GMT</p>\n", "the failed hook declined")
        problems = document.problems()
        self.assertEqual(len(problems), 1, "the latched message surfaces")
        self.assertEqual(
            problems[0].message,
            "extension 'boom': RuntimeError: host callback exploded",
            "a raised exception's type name prefixes its message",
        )
        self.assertEqual(problems[0].start, 0, "host errors carry no span")
        self.assertEqual(len(document.problems()), 1, "problems is a view")


class Validation(unittest.TestCase):
    def test_bad_name_is_rejected_when_enabled(self):
        options = geist4.Options()
        with self.assertRaises(geist4.Geist4Error) as caught:
            options.enable_host(geist4.HostExtension("Bad Name"))
        self.assertIn("host extension:", str(caught.exception))

    def test_builtin_name_is_rejected_when_enabled(self):
        options = geist4.Options()
        with self.assertRaises(geist4.Geist4Error) as caught:
            options.enable_host(geist4.HostExtension("tables"))
        self.assertIn("reserved for a built-in", str(caught.exception))

    def test_bad_kind_is_rejected_when_enabled(self):
        options = geist4.Options()
        with self.assertRaises(geist4.Geist4Error) as caught:
            options.enable_host(geist4.HostExtension("ok", {"Nope": []}))
        self.assertIn('host extension kind "Nope"', str(caught.exception))

    def test_bad_attr_is_rejected_when_enabled(self):
        options = geist4.Options()
        with self.assertRaises(geist4.Geist4Error) as caught:
            options.enable_host(geist4.HostExtension("ok", {"fine": ["bad name"]}))
        self.assertIn('attribute "bad name"', str(caught.exception))

    def test_too_many_triggers_is_rejected_when_enabled(self):
        options = geist4.Options()
        with self.assertRaises(geist4.Geist4Error) as caught:
            options.enable_host(geist4.HostExtension("ok", triggers=["a"] * 33))
        self.assertIn("host extension triggers:", str(caught.exception))

    def test_multibyte_trigger_is_rejected_at_construction(self):
        with self.assertRaises(geist4.Geist4Error) as caught:
            geist4.HostExtension("ok", triggers=["ab"])
        self.assertIn("must be one byte", str(caught.exception))

    def test_non_callable_callback_is_rejected_at_construction(self):
        with self.assertRaises(geist4.Geist4Error) as caught:
            geist4.HostExtension("ok", post_process=42)
        self.assertIn("is not callable", str(caught.exception))

    def test_bad_kind_shape_is_rejected_at_construction(self):
        with self.assertRaises(geist4.Geist4Error) as caught:
            geist4.HostExtension("ok", {"note": 3})
        self.assertIn("must be a list of attribute names or a mapping", str(caught.exception))


class Duplicates(unittest.TestCase):
    def test_a_second_registration_with_the_same_name_is_rejected(self):
        options = geist4.Options()
        options.enable_host(geist4.HostExtension("dup"))
        with self.assertRaises(geist4.Geist4Error) as caught:
            options.enable_host(geist4.HostExtension("dup"))
        self.assertIn('"dup" is already registered', str(caught.exception))


if __name__ == "__main__":
    unittest.main()
