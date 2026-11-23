"""The Python binding's contract, mirroring the C contract test: the
same checks the C ABI pins against the shared library, written against
the native Python surface. It runs against the wheel the gate builds
and installs, so it tests exactly what a pip user gets.

A Python string is always valid Unicode, so the C ABI's null and
invalid-UTF-8 rejections are unreachable here, as in the Swift
binding; the NUL byte reaches the engine as ordinary content.
"""

import unittest

import geist4


class VersionContract(unittest.TestCase):
    def test_version_is_a_nonempty_string(self):
        self.assertIsInstance(geist4.version(), str)
        self.assertNotEqual(geist4.version(), "")


class RenderContract(unittest.TestCase):
    def test_heading_renders_byte_for_byte(self):
        self.assertEqual(geist4.render_html("# hi\n"), "<h1>hi</h1>\n")

    def test_empty_document_renders_empty(self):
        self.assertEqual(geist4.render_html(""), "")

    def test_nul_byte_is_ordinary_content(self):
        # A NUL reaches the engine as ordinary content: U+FFFD replaces
        # it, exactly as the C contract pins.
        self.assertEqual(geist4.render_html("a\x00b\n"), "<p>a\ufffdb</p>\n")

    def test_render_html_takes_options_as_a_keyword(self):
        options = geist4.Options()
        options.enable_builtin("gfm")
        self.assertEqual(
            geist4.render_html("~~gone~~\n", options=options),
            "<p><del>gone</del></p>\n",
        )


class OptionsContract(unittest.TestCase):
    def test_gfm_bundle_renders_strikethrough(self):
        options = geist4.Options()
        options.enable_builtin("gfm")
        self.assertEqual(
            geist4.render_html("~~gone~~\n", options), "<p><del>gone</del></p>\n"
        )

    def test_table_renders_byte_for_byte(self):
        options = geist4.Options()
        options.enable_builtin("gfm")
        source = "| a | b |\n| --- | --- |\n| 1 | 2 |\n"
        expected = (
            "<table>\n<thead>\n<tr>\n<th>a</th>\n<th>b</th>\n</tr>\n"
            "</thead>\n<tbody>\n<tr>\n<td>1</td>\n<td>2</td>\n</tr>\n"
            "</tbody>\n</table>\n"
        )
        self.assertEqual(geist4.render_html(source, options), expected)

    def test_unknown_name_lists_the_valid_names(self):
        options = geist4.Options()
        with self.assertRaises(geist4.Geist4Error) as caught:
            options.enable_builtin("nope")
        self.assertIn("valid names", str(caught.exception))

    def test_bundle_after_member_is_idempotent(self):
        options = geist4.Options()
        options.enable_builtin("tables")
        options.enable_builtin("gfm")  # includes tables again: a no-op.
        source = "| a | b |\n| --- | --- |\n| 1 | 2 |\n"
        self.assertIn("<table>", geist4.render_html(source, options))

    def test_bundle_and_single_name_compose(self):
        options = geist4.Options()
        options.enable_builtin("gfm")
        options.enable_builtin("strikethrough")
        self.assertEqual(
            geist4.render_html("~~gone~~\n", options), "<p><del>gone</del></p>\n"
        )


class DocumentContract(unittest.TestCase):
    def test_document_renders_and_renders_identically(self):
        options = geist4.Options()
        options.enable_builtin("strikethrough")
        source = "~~x~~ [text][missing]\n"
        document = geist4.Document(source, options=options)
        options = None  # The document keeps its own options copy.
        first = document.html()
        self.assertEqual(first, "<p><del>x</del> [text][missing]</p>\n")
        self.assertEqual(document.html(), first)

    def test_problems_name_the_missing_reference_within_the_span(self):
        source = "~~x~~ [text][missing]\n"
        document = geist4.Document(source)
        problems = document.problems()
        self.assertGreaterEqual(len(problems), 1)
        source_len = len(source.encode("utf-8"))
        seen_missing = False
        for problem in problems:
            self.assertGreaterEqual(problem.start, 0)
            self.assertLess(problem.start, source_len)
            self.assertLessEqual(problem.end, source_len)
            self.assertLess(problem.start, problem.end)
            if "missing" in problem.message:
                seen_missing = True
        self.assertTrue(seen_missing)


if __name__ == "__main__":
    unittest.main()

    def test_highlighting_off_renders_plain_code(self):
        options = geist4.Options()
        html = geist4.render_html("```rust\nlet x = 1;\n```\n", options=options)
        self.assertEqual(
            html,
            "<pre><code class=\"language-rust\">let x = 1;\n</code></pre>\n",
        )

    def test_highlighting_on_wraps_tokens_in_class_spans(self):
        options = geist4.Options()
        options.enable_highlighting()
        html = geist4.render_html("```rust\nlet x = 1;\n```\n", options=options)
        self.assertIn('<span class="storage">let</span>', html)
        self.assertIn('<span class="numeric">1</span>', html)
        self.assertTrue(html.startswith('<pre><code class="language-rust">'))
