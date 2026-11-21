//! Kill tests for `crates/geist4-core/src/basic_html.rs` renderer
//! mutants whose behavior only appears when an HTML-filtering
//! extension is registered (the `push_filtered` path).
//!
//! Each test asserts the correct behavior first; the `kills:` comment
//! names the mutants that this test catches.

use geist4_core::basic_html::render_with;
use geist4_core::extension::Options;
use geist4_core::parser::parse_with;
use geist4_ext::tagfilter;

#[test]
fn tagfilter_escapes_disallowed_tag_starts() {
    // kills: crates/geist4-core/src/basic_html.rs:116:21 (< with ==),
    // :121:28 (+ with -, + with *), :128:24 (+ with -) in push_filtered
    let mut options = Options::new();
    options.extension(tagfilter());
    let doc = parse_with("a <xmp>b</xmp>\n", &options);
    assert_eq!(
        render_with(&doc, &options),
        "<p>a &lt;xmp>b&lt;/xmp></p>\n",
        "each disallowed tag start's '<' must become &lt;, in order, once"
    );
}

#[test]
fn dangerous_link_urls_are_suppressed() {
    // kills: crates/geist4-core/src/basic_html.rs:553:9 (is_dangerous_url -> false),
    // the scheme-list || -> && mutants, the == -> != and && -> || in has_prefix,
    // and the !is_empty && !is_dangerous_url && -> || pair in inline_child
    let options = Options::new();
    assert_eq!(
        render_with(
            &parse_with("[a](javascript:alert(1))\n", &options),
            &options
        ),
        "<p><a href=\"\">a</a></p>\n",
        "a javascript: destination must leave an empty href"
    );
}

#[test]
fn safe_data_image_urls_are_kept() {
    // kills: crates/geist4-core/src/basic_html.rs:557:29: replace >= with < in is_dangerous_url
    let options = Options::new();
    assert_eq!(
        render_with(
            &parse_with("![i](data:image/png;base64,AAAA)\n", &options),
            &options
        ),
        "<p><img src=\"data:image/png;base64,AAAA\" alt=\"i\" /></p>\n",
        "the allow-listed image data schemes must keep their src"
    );
}

#[test]
fn short_and_plain_urls_render_normally() {
    // kills: crates/geist4-core/src/basic_html.rs:554:66: replace == with != in is_dangerous_url
    // and the && -> || in has_prefix (a short url must not panic).
    let options = Options::new();
    assert_eq!(
        render_with(&parse_with("[a](x)\n", &options), &options),
        "<p><a href=\"x\">a</a></p>\n"
    );
    assert_eq!(
        render_with(
            &parse_with("[a](https://example.com)\n", &options),
            &options
        ),
        "<p><a href=\"https://example.com\">a</a></p>\n"
    );
}

#[test]
fn image_alt_text_includes_code_spans() {
    // kills: crates/geist4-core/src/basic_html.rs:538:17: delete match arm Node::CodeSpan(text) in plain_text
    let options = Options::new();
    assert_eq!(
        render_with(&parse_with("![a `b` c](x)\n", &options), &options),
        "<p><img src=\"x\" alt=\"a b c\" /></p>\n",
        "the alt text must include the code span's text"
    );
}

#[test]
fn every_dangerous_scheme_is_suppressed_in_links_and_images() {
    // kills: crates/geist4-core/src/basic_html.rs:504:45 and :504:48 (the
    // image url gate in inline_child), plus any scheme-list flips in
    // is_dangerous_url. Exact renders, because escape_href percent-encodes
    // the raw url and a substring check would not see it.
    let options = Options::new();
    for (kind, template, empty) in [
        ("link", "[a]({url})\n", "<p><a href=\"\">a</a></p>\n"),
        (
            "image",
            "![i]({url})\n",
            "<p><img src=\"\" alt=\"i\" /></p>\n",
        ),
    ] {
        for url in [
            "javascript:alert(1)",
            "vbscript:run",
            "file:///etc/hosts",
            "data:text/html,hi",
        ] {
            let src = template.replace("{url}", url);
            assert_eq!(
                render_with(&parse_with(&src, &options), &options),
                empty,
                "{kind} destination {url} must be suppressed"
            );
        }
    }
    assert_eq!(
        render_with(
            &parse_with("![i](data:image/gif;base64,AA)\n", &options),
            &options
        ),
        "<p><img src=\"data:image/gif;base64,AA\" alt=\"i\" /></p>\n",
        "the allow-listed image data schemes must keep their src"
    );

    assert_eq!(
        render_with(
            &parse_with("[a](https://example.com/docs/page)\n", &options),
            &options
        ),
        "<p><a href=\"https://example.com/docs/page\">a</a></p>\n",
        "a safe url keeps its href"
    );
}
