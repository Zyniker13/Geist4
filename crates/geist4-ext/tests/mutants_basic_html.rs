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
