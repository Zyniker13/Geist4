//! Definition-list look/try-start arms that need a list behind an
//! open paragraph, a marker at document start, and indented markers.
//! Spec examples stay in `spec/description_lists.md`.

use geist4_core::basic_html::render_with;
use geist4_core::extension::Extension;
use geist4_core::extension::Options;
use geist4_core::parser::parse_with;

fn dl_html(src: &str) -> String {
    let mut options = Options::new();
    options.extension(geist4_ext::deflists());
    render_with(&parse_with(src, &options), &options)
}

/// The look's list arm: an open paragraph after a closed definition
/// list, then a marker line — the line would lazily continue the
/// paragraph, but the look approves (the list's last item ended in a
/// definition), so the marker steals the paragraph as the new term.
#[test]
fn look_approves_open_paragraph_after_list() {
    assert_eq!(
        dl_html("Term\n: def\n\nnext\n: def2\n"),
        "<dl>\n<dt>Term</dt>\n<dd>def</dd>\n<dt>next</dt>\n<dd>def2</dd>\n</dl>\n",
        "the open paragraph becomes the new term; the marker starts its definition"
    );
    assert_eq!(
        dl_html("Term\n: def\n\nnext\n  : def2\n"),
        "<dl>\n<dt>Term</dt>\n<dd>def</dd>\n<dt>next</dt>\n<dd>def2</dd>\n</dl>\n",
        "an indented marker line reaches the same decision"
    );
}

/// `block_start_looks` with an empty container: a marker line as the
/// very first block has no last child to inspect, so the look
/// declines and `try_start` declines with it — the marker line stays
/// a plain paragraph.
#[test]
fn marker_at_document_start_declines() {
    assert_eq!(dl_html(": def\n"), "<p>: def</p>\n");
    assert_eq!(
        dl_html("  : def\n"),
        "<p>: def</p>\n",
        "an indented marker at document start is a plain paragraph, unindented"
    );
}

/// The nested marker lines inside an open definition: each indented
/// marker opens a further nested list inside the definition.
#[test]
fn nested_markers_nest_repeatedly() {
    assert_eq!(
        dl_html("Term\n: def\n  : def2\n  : def3\n"),
        "<dl>\n<dt>Term</dt>\n<dd>\n<dl>\n<dt>def</dt>\n<dd>def2</dd>\n<dd>def3</dd>\n</dl>\n</dd>\n</dl>\n",
        "the third marker joins the inner list as a sibling definition"
    );
}

/// The extension trait surface: the registered name is `deflists`.
#[test]
fn trait_surface() {
    let extension = geist4_ext::deflists();
    assert_eq!(Extension::name(extension.as_ref()), "description_lists");
}
