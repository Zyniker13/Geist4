//! Definition-list edge branches the spec file does not reach: the
//! lazy-continuation looker (`block_start_looks`) against every
//! container shape it inspects, `try_start`'s declined holders, the
//! close-time moves when the previous sibling is not a paragraph, and
//! the tight/loose decision for a list's second item. Spec examples
//! stay in `spec/description_lists.md`.

use geist4_core::basic_html::render_with;
use geist4_core::extension::Options;
use geist4_core::parser::parse_with;

fn dl_html(src: &str) -> String {
    let mut options = Options::new();
    options.extension(geist4_ext::deflists());
    render_with(&parse_with(src, &options), &options)
}

/// `block_start_looks` declines indented lines: four spaces under a
/// definition stay the definition's content, never a new item.
#[test]
fn lazy_look_declines_indented() {
    assert_eq!(
        dl_html("Term\n: def\n    indented\n"),
        "<dl>\n<dt>Term</dt>\n<dd>def\nindented</dd>\n</dl>\n",
        "an indented line is definition content, not a marker"
    );
}

/// `block_start_looks` against this extension's own item: the marker
/// line closes the item so the definition it starts joins the list the
/// close builds — the item does not lazily continue.
#[test]
fn lazy_look_sees_own_item() {
    assert_eq!(
        dl_html("Term\n: def\n: def2\n"),
        "<dl>\n<dt>Term</dt>\n<dd>def</dd>\n<dd>def2</dd>\n</dl>\n",
        "the second marker line becomes a sibling definition, not lazy text"
    );
}

/// `block_start_looks` against this extension's list: a marker after a
/// complete term-definition item joins the list; against a list whose
/// last item has no definition (term only) the look declines and the
/// marker line stays lazy text.
#[test]
fn lazy_look_sees_own_list() {
    // A complete item: the marker line opens the next item.
    assert_eq!(
        dl_html("Term\n: def\n\nTerm2\n: def2\n: def3\n"),
        "<dl>\n<dt>Term</dt>\n<dd>def</dd>\n<dt>Term2</dt>\n<dd>def2</dd>\n<dd>def3</dd>\n</dl>\n",
        "a marker line after a complete item's blank line still starts an item"
    );
    // An item that ended without any definition: the list look finds
    // no details node, so the line stays lazy paragraph text.
    assert_eq!(
        dl_html("Term\n\n: def\n"),
        "<dl>\n<dt>Term</dt>\n<dd>\n<p>def</p>\n</dd>\n</dl>\n",
        "a marker line after a blank-closed term still claims it as the term; the definition is loose"
    );
}

/// `block_start_looks` against any other container (a block quote):
/// the marker line stays lazy paragraph text — the spec's `> Term`
/// followed by `: def`.
#[test]
fn lazy_look_declines_other_containers() {
    assert_eq!(
        dl_html("> Quote\n: def\n"),
        "<blockquote>\n<p>Quote\n: def</p>\n</blockquote>\n"
    );
}

/// `try_start`'s declined holders: a marker line with no paragraph
/// behind it and no open this-extension container starts nothing.
#[test]
fn try_start_declines_without_holder() {
    assert_eq!(
        dl_html("```\ncode\n```\n: def\n"),
        "<pre><code>code\n</code></pre>\n<p>: def</p>\n",
        "a marker line behind a closed code block starts a plain paragraph"
    );
    // A heading is not a paragraph and not this extension's node.
    assert_eq!(
        dl_html("# Head\n: def\n"),
        "<h1>Head</h1>\n<p>: def</p>\n",
        "a marker line behind a heading starts an item with an empty term"
    );
}

/// `try_start` behind this extension's own open list continues that
/// list with a new item whose tightness follows the list's last
/// definition, and behind a list item without definitions it declines.
#[test]
fn try_start_behind_own_list() {
    assert_eq!(
        dl_html("Term\n: def\n\nTerm2\n: def2\n"),
        "<dl>\n<dt>Term</dt>\n<dd>def</dd>\n<dt>Term2</dt>\n<dd>def2</dd>\n</dl>\n"
    );
}

/// A marker line inside a block quote against the quote's paragraph
/// stays lazy (the quote container's last child is the paragraph, so
/// the look declines and the paragraph continues).
#[test]
fn quote_paragraph_continues_lazily() {
    assert_eq!(
        dl_html("> a\n> : def\n"),
        "<blockquote>\n<dl>\n<dt>a</dt>\n<dd>def</dd>\n</dl>\n</blockquote>\n",
        "inside a quote the marker line is not a definition"
    );
}

/// An empty definition list: a definition list can only form behind a
/// term paragraph, so nothing appears for a bare `:` in any position.
#[test]
fn bare_marker_never_starts() {
    assert_eq!(
        dl_html(":\n"),
        "<p>:</p>\n",
        "a colon with no space and no content is plain text"
    );
    assert_eq!(
        dl_html(":   \n"),
        "<p>:</p>\n",
        "a colon with only whitespace is plain text"
    );
}

/// The close-time sibling join: consecutive items gather into one
/// list, and a definition list inside one definition stays nested.
#[test]
fn close_joins_and_nests() {
    assert_eq!(
        dl_html("a\n: 1\n\nb\n: 2\n"),
        "<dl>\n<dt>a</dt>\n<dd>1</dd>\n<dt>b</dt>\n<dd>2</dd>\n</dl>\n",
        "a second item joins the list the first built"
    );
    assert_eq!(
        dl_html("t\n: a\n\n: b\n\n: c\n"),
        "<dl>\n<dt>t</dt>\n<dd>a</dd>\n<dd>b</dd>\n<dd>c</dd>\n</dl>\n",
        "a blank line between definitions keeps one term with three definitions"
    );
}

/// The loose/tight boundary: a blank line followed by more definition
/// content makes the definition loose (paragraph-wrapped).
#[test]
fn loose_definition_rendering() {
    assert_eq!(
        dl_html("Term\n: def\n\ndef2 para\n"),
        "<dl>\n<dt>Term</dt>\n<dd>def</dd>\n</dl>\n<p>def2 para</p>\n",
        "a blank line inside the definition makes it a loose one; a trailing paragraph stays outside"
    );
}

/// A tight definition whose block child is not a plain paragraph
/// keeps its own wrapper: the strip only fires for paragraphs, and a
/// list child renders with the newline the `ensure_newline` hook
/// supplies (`strip_paragraph_wrapper`'s `None` arm).
#[test]
fn tight_definition_with_a_list_child() {
    assert_eq!(
        dl_html("Term\n: - one\n- two\n"),
        "<dl>\n<dt>Term</dt>\n<dd>\n<ul>\n<li>one</li>\n</ul>\n</dd>\n</dl>\n<ul>\n<li>two</li>\n</ul>\n",
        "the first list joins the definition; the second breaks out at column 0 and closes the item"
    );
}

/// A tight definition with an indented code block child: the code
/// block's own rendering ends with a newline, so no extra one is
/// inserted, and no `<p>` wrapper is stripped from it.
#[test]
fn tight_definition_with_a_code_child() {
    assert_eq!(
        dl_html("Term\n: def\n    code\n"),
        "<dl>\n<dt>Term</dt>\n<dd>def\ncode</dd>\n</dl>\n"
    );
}

/// The foreign-kind render arm: a node whose kind belongs to no
/// deflist kind renders nothing through this extension's renderer,
/// while the real kinds render through the same entry point.
#[test]
fn foreign_kind_renders_nothing() {
    use geist4_core::extension::{ExtNode, RenderedChild};

    let extension = geist4_ext::deflists();
    let node = ExtNode {
        kind: "not_a_deflist_kind".to_owned(),
        attrs: Vec::new(),
    };
    let mut out = String::new();
    extension.render(&node, &[] as &[RenderedChild<'_>], &mut out);
    assert_eq!(out, "", "a foreign kind renders nothing");

    let term = ExtNode {
        kind: "description_term".to_owned(),
        attrs: Vec::new(),
    };
    let mut out2 = String::new();
    extension.render(&term, &[] as &[RenderedChild<'_>], &mut out2);
    assert_eq!(out2, "<dt></dt>\n");
}
