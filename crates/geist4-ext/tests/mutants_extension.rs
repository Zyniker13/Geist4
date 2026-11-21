//! Kill tests for `crates/geist4-core/src/extension.rs` mutants: the
//! default hook implementations, the block-start and block-continue
//! context plumbing, the render-override child puller, and the
//! close-phase text-map handoff.
//!
//! Each test asserts the correct behavior first; the `kills:` comments
//! name the mutant positions (`path:line:col` as reported by
//! cargo-mutants) the test is designed to catch.

use geist4_core::arena::NodeId;
use geist4_core::basic_html::render_with;
use geist4_core::extension::{
    BlockContinueCtx, BlockStartCtx, ContinueOutcome, CoreNode, ExtNode, Extension, KindDecl,
    Options, OverrideChildren, RenderedChild, TreeCtx,
};
use geist4_core::node::Node;
use geist4_core::parser::parse_with;
use geist4_ext::{deflists, footnotes};
use std::sync::Arc;

/// Renders a document and strips the final newline, so expectations read
/// like the spec harness's.
fn html(src: &str, options: &Options) -> String {
    let doc = parse_with(src, options);
    let out = render_with(&doc, options);
    out.strip_suffix('\n').unwrap_or(&out).to_owned()
}

/// An extension that overrides nothing but its name: every hook keeps
/// its default implementation.
struct Named;

impl Extension for Named {
    fn name(&self) -> &str {
        "named"
    }
}

#[test]
fn default_front_matter_delimiter_claims_nothing() {
    // kills: crates/geist4-core/src/extension.rs:192:11: replace
    // Extension::front_matter_delimiter -> Option<&str> with Some("")
    let mut options = Options::new();
    options.extension(Arc::new(Named));
    assert_eq!(
        html("\npara\n", &options),
        "<p>para</p>",
        "an extension without a delimiter must not claim the document's opening blank line"
    );
}

#[test]
fn default_front_matter_delimiter_ignores_lookalike_lines() {
    // kills: crates/geist4-core/src/extension.rs:192:11: replace
    // Extension::front_matter_delimiter -> Option<&str> with Some("xyzzy")
    let mut options = Options::new();
    options.extension(Arc::new(Named));
    assert_eq!(
        html("xyzzy\nA\nxyzzy\nB\n", &options),
        "<p>xyzzy\nA\nxyzzy\nB</p>",
        "a line that merely looks like a delimiter must stay a paragraph when no delimiter is declared"
    );
}

#[test]
fn default_inline_triggers_are_empty() {
    // kills: crates/geist4-core/src/extension.rs:205:11: replace
    // Extension::inline_triggers -> &[u8] with Vec::leak(vec![0]) / vec![1]
    assert!(
        Named.inline_triggers().is_empty(),
        "an extension that overrides nothing must declare no inline triggers"
    );
}

#[test]
fn default_take_error_is_none() {
    // kills: crates/geist4-core/src/extension.rs:247:11: replace
    // Extension::take_error -> Option<String> with Some("xyzzy".into()) / Some(String::new())
    assert!(
        Named.take_error().is_none(),
        "a fresh extension must report no latched error"
    );
}

/// An extension that denies raw HTML literals starting `<x` through its
/// filter hook, without declaring itself a filtering extension: the
/// hook must stay inert until `filters_html` says otherwise.
struct DenyX;

impl Extension for DenyX {
    fn name(&self) -> &str {
        "deny-x"
    }

    fn html_filter(&self, literal: &[u8]) -> bool {
        !literal.starts_with(b"<x")
    }
}

#[test]
fn a_non_filtering_extension_leaves_raw_html_alone() {
    // kills: crates/geist4-core/src/extension.rs:304:11: replace
    // Extension::filters_html -> bool with true
    let mut options = Options::new();
    options.extension(Arc::new(DenyX));
    assert_eq!(
        html("<x>\n", &options),
        "<x>",
        "without filters_html the filter hook must not run, so the tag passes through"
    );
}

/// An extension that declares itself a filtering extension but keeps
/// the default allow-everything filter hook.
struct DeclaredFilter;

impl Extension for DeclaredFilter {
    fn name(&self) -> &str {
        "declared-filter"
    }

    fn filters_html(&self) -> bool {
        true
    }
}

#[test]
fn the_default_filter_hook_allows_everything() {
    // kills: crates/geist4-core/src/extension.rs:313:11: replace
    // Extension::html_filter -> bool with false
    let mut options = Options::new();
    options.extension(Arc::new(DeclaredFilter));
    assert_eq!(
        html("<x>\n", &options),
        "<x>",
        "the default filter hook allows every literal, so the tag passes through"
    );
}

/// An extension that claims every heading and renders it with a child
/// census: for each child it writes the not-yet-rendered count, then
/// pulls the child through with `render_next`.
struct Counting {
    index: std::sync::OnceLock<usize>,
}

impl Counting {
    fn new() -> Self {
        Self {
            index: std::sync::OnceLock::new(),
        }
    }
}

impl Extension for Counting {
    fn name(&self) -> &str {
        "counting"
    }

    fn post_process(&self, ctx: &mut TreeCtx<'_, '_, '_, '_>) {
        let index = *self.index.get_or_init(|| {
            ctx.options
                .extensions()
                .iter()
                .position(|extension| {
                    Arc::ptr_eq(
                        extension,
                        ctx.options.by_name("counting").expect("registered"),
                    )
                })
                .expect("self in options")
        });
        collect_headings(ctx.arena, ctx.root, index);
    }

    fn render_override(
        &self,
        _node: &CoreNode<'_>,
        children: &mut OverrideChildren<'_, '_>,
        out: &mut String,
    ) {
        out.push_str("<h1>");
        loop {
            out.push_str(&children.remaining().to_string());
            if !children.render_next(out) {
                break;
            }
        }
        out.push_str("</h1>");
    }
}

fn collect_headings(arena: &mut geist4_core::arena::Arena<'_>, id: NodeId, index: usize) {
    if matches!(arena.kind(id), Node::Heading(_)) {
        arena.set_extension(id, index);
    }
    for child in arena.children(id).to_vec() {
        collect_headings(arena, child, index);
    }
}

#[test]
fn render_override_pulls_children_in_order_with_counts() {
    // kills: crates/geist4-core/src/extension.rs:135:16 (- with +, - with /)
    // and -> usize with 0 in OverrideChildren::remaining;
    // :141:15 (-> bool with false, -> bool with true, += with *=, += with -=)
    // in OverrideChildren::render_next
    let mut options = Options::new();
    options.extension(Arc::new(Counting::new()));
    assert_eq!(
        html("# a *b*\n", &options),
        "<h1>2a 1<em>b</em>0</h1>",
        "the override must see exact remaining counts and pull each child once, in order"
    );
}

/// An extension claiming `probe` lines, keeping the default
/// `block_start_looks` (false).
struct Looker;

impl Extension for Looker {
    fn name(&self) -> &str {
        "looker"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "probe".to_owned(),
            container: false,
            inline_text: false,
            attrs: Vec::new(),
        }]
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.line().starts_with(b"probe") {
            ctx.open_extension("probe".to_owned(), Box::new(()));
        }
    }

    fn render(&self, _node: &ExtNode, _children: &[RenderedChild<'_>], out: &mut String) {
        out.push_str("<div class=\"probe\"></div>");
    }
}

#[test]
fn an_extension_block_does_not_reach_past_its_container() {
    // kills: crates/geist4-core/src/extension.rs:273:11: replace
    // Extension::block_start_looks -> bool with true
    let mut options = Options::new();
    options.extension(Arc::new(Looker));
    assert_eq!(
        html("> probe x\n\nplain\n", &options),
        "<blockquote>\n<div class=\"probe\"></div>\n</blockquote>\n<p>plain</p>",
        "the blockquote holds only the probe block; 'plain' stays a top-level paragraph"
    );
}

/// An extension claiming `%%go` only when the line's content starts
/// past column zero — inside a container prefix.
struct Gate;

impl Extension for Gate {
    fn name(&self) -> &str {
        "gate"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "gate".to_owned(),
            container: false,
            inline_text: false,
            attrs: Vec::new(),
        }]
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.line() == b"%%go" && ctx.column() > 0 {
            ctx.open_extension("gate".to_owned(), Box::new(()));
        }
    }

    fn render(&self, _node: &ExtNode, _children: &[RenderedChild<'_>], out: &mut String) {
        out.push_str("<div class=\"gate\"></div>");
    }
}

#[test]
fn block_start_column_counts_the_container_prefixes() {
    // kills: crates/geist4-core/src/extension.rs:563:35: replace
    // BlockStartCtx::column -> usize with 0
    let mut options = Options::new();
    options.extension(Arc::new(Gate));
    assert_eq!(
        html("> %%go\n", &options),
        "<blockquote>\n<div class=\"gate\"></div>\n</blockquote>",
        "inside the quote the content starts at column 2, so the gate opens"
    );
    assert_eq!(
        html("%%go\n", &options),
        "<p>%%go</p>",
        "at the document's column 0 the gate must decline"
    );
}

/// An extension claiming `foo` only at column zero.
struct Zero;

impl Extension for Zero {
    fn name(&self) -> &str {
        "zero"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "zero".to_owned(),
            container: false,
            inline_text: false,
            attrs: Vec::new(),
        }]
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.line() == b"foo" && ctx.column() == 0 {
            ctx.open_extension("zero".to_owned(), Box::new(()));
        }
    }

    fn render(&self, _node: &ExtNode, _children: &[RenderedChild<'_>], out: &mut String) {
        out.push_str("<div class=\"zero\"></div>");
    }
}

#[test]
fn block_start_sees_column_zero_at_the_document() {
    // kills: crates/geist4-core/src/extension.rs:563:35: replace
    // BlockStartCtx::column -> usize with 1
    let mut options = Options::new();
    options.extension(Arc::new(Zero));
    assert_eq!(
        html("foo\n", &options),
        "<div class=\"zero\"></div>",
        "at the document level the content starts at column 0, so the block opens"
    );
}

/// An extension whose blocks contain blocks: a repeated marker inside
/// an open block opens a nested block only when the open block's state
/// is visible through `state`.
struct Sated;

impl Extension for Sated {
    fn name(&self) -> &str {
        "sated"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![
            KindDecl {
                kind: "sat".to_owned(),
                container: true,
                inline_text: false,
                attrs: Vec::new(),
            },
            KindDecl {
                kind: "satsub".to_owned(),
                container: true,
                inline_text: false,
                attrs: Vec::new(),
            },
        ]
    }

    fn contains_blocks(&self) -> bool {
        true
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.line() != b"%%s" {
            return;
        }
        if ctx.state::<u32>().is_some() {
            ctx.open_extension("satsub".to_owned(), Box::new(0u32));
        } else {
            ctx.open_extension("sat".to_owned(), Box::new(0u32));
        }
    }

    fn try_continue(&self, _ctx: &mut BlockContinueCtx<'_, '_, '_>) -> ContinueOutcome {
        ContinueOutcome::Continue
    }

    fn render(&self, node: &ExtNode, children: &[RenderedChild<'_>], out: &mut String) {
        let class = if node.kind == "sat" { "sat" } else { "satsub" };
        out.push_str(&format!("<div class=\"{class}\">"));
        for child in children {
            out.push_str(&child.html);
        }
        out.push_str("</div>");
    }
}

#[test]
fn state_is_visible_to_nested_block_starts() {
    // kills: crates/geist4-core/src/extension.rs:569:35: replace
    // BlockStartCtx::state -> Option<&mut T> with None
    let mut options = Options::new();
    options.extension(Arc::new(Sated));
    assert_eq!(
        html("%%s\n%%s\n", &options),
        "<div class=\"sat\"><div class=\"satsub\"></div></div>",
        "the second marker must open a nested block because the open block's state is Some"
    );
}

// ---------------------------------------------------------------------------
// Definition lists: the close phase must hand the term paragraph's
// text map to the term node, keeping source-true spans.
// ---------------------------------------------------------------------------

/// The text node holding `needle`, with its span sliced from `src`.
fn text_span<'a>(
    src: &'a str,
    doc: &geist4_core::Document<'_>,
    id: NodeId,
    needle: &str,
) -> Option<&'a str> {
    let mut found = None;
    walk_for_text(doc, id, needle, &mut found);
    found.map(|text_id| {
        let span = doc.arena.span(text_id);
        &src[span.start..span.end]
    })
}

fn walk_for_text(
    doc: &geist4_core::Document<'_>,
    id: NodeId,
    needle: &str,
    found: &mut Option<NodeId>,
) {
    if found.is_some() {
        return;
    }
    if let Node::Text(text) = doc.arena.kind(id) {
        if text.as_ref() == needle {
            *found = Some(id);
            return;
        }
    }
    for child in doc.arena.children(id) {
        walk_for_text(doc, *child, needle, found);
    }
}

#[test]
fn definition_term_text_keeps_its_source_span() {
    // kills: crates/geist4-core/src/extension.rs:494:9 (take_text_map -> None),
    // :494:9 (take_text_map -> Some(Default::default())), :498:46 (<= with >),
    // :503:9 (set_text_map with ()) in CloseCtx
    let mut options = Options::new();
    options.extension(deflists());
    let src = "Term\n: def\n";
    let doc = parse_with(src, &options);
    assert_eq!(
        text_span(src, &doc, doc.root, "Term"),
        Some("Term"),
        "the term's text child must keep the term paragraph's source span, not a fallback"
    );
}

// ---------------------------------------------------------------------------
// Footnotes: the block-continue context plumbing.
// ---------------------------------------------------------------------------

fn footnote_options() -> Options {
    let mut options = Options::new();
    options.extension(footnotes());
    options
}

#[test]
fn footnote_definition_after_a_blank_line_continues() {
    // kills: crates/geist4-core/src/extension.rs:670:9: replace
    // BlockStartCtx::seek_content with ()
    assert_eq!(
        html("x[^1]\n\n[^1]: note\n", &footnote_options()),
        "<p>x<sup class=\"footnote-ref\"><a href=\"#fn-1\" id=\"fnref-1\" data-footnote-ref>1</a></sup></p>\n<section class=\"footnotes\" data-footnotes>\n<ol>\n<li id=\"fn-1\">\n<p>note <a href=\"#fnref-1\" class=\"footnote-backref\" data-footnote-backref data-footnote-backref-idx=\"1\" aria-label=\"Back to reference 1\">↩</a></p>\n</li>\n</ol>\n</section>",
        "the definition's marker must be consumed and its content placed after the cursor"
    );
}

#[test]
fn a_blank_line_inside_a_footnote_definition_keeps_it_open() {
    // kills: crates/geist4-core/src/extension.rs:687:35: replace
    // BlockContinueCtx::pure_blank -> bool with false
    assert_eq!(
        html("x[^1]\n\n[^1]: first\n\n    second\n", &footnote_options()),
        "<p>x<sup class=\"footnote-ref\"><a href=\"#fn-1\" id=\"fnref-1\" data-footnote-ref>1</a></sup></p>\n<section class=\"footnotes\" data-footnotes>\n<ol>\n<li id=\"fn-1\">\n<p>first</p>\n<p>second <a href=\"#fnref-1\" class=\"footnote-backref\" data-footnote-backref data-footnote-backref-idx=\"1\" aria-label=\"Back to reference 1\">↩</a></p>\n</li>\n</ol>\n</section>",
        "a blank line must not close the definition, so the four-space line after it joins as a second paragraph"
    );
}

#[test]
fn indented_continuation_columns_are_counted_exactly() {
    // kills: crates/geist4-core/src/extension.rs:694:35: replace
    // BlockContinueCtx::indent -> usize with 0 / with 1
    assert_eq!(
        html("x[^1]\n\n[^1]: note\n    indented\n", &footnote_options()),
        "<p>x<sup class=\"footnote-ref\"><a href=\"#fn-1\" id=\"fnref-1\" data-footnote-ref>1</a></sup></p>\n<section class=\"footnotes\" data-footnotes>\n<ol>\n<li id=\"fn-1\">\n<p>note\nindented <a href=\"#fnref-1\" class=\"footnote-backref\" data-footnote-backref data-footnote-backref-idx=\"1\" aria-label=\"Back to reference 1\">↩</a></p>\n</li>\n</ol>\n</section>",
        "the four-space continuation must be consumed as indentation, not left to form a code block"
    );
}

#[test]
fn a_shallow_quoted_continuation_does_not_join_the_definition() {
    // kills: crates/geist4-core/src/extension.rs:694:35: replace - with +
    // in BlockContinueCtx::indent
    assert_eq!(
        html(
            "> x[^1]\n>\n> [^1]: note\n>   indented\n",
            &footnote_options()
        ),
        "<blockquote>\n<p>x<sup class=\"footnote-ref\"><a href=\"#fn-1\" id=\"fnref-1\" data-footnote-ref>1</a></sup></p>\n</blockquote>\n<section class=\"footnotes\" data-footnotes>\n<ol>\n<li id=\"fn-1\">\n<p>note\nindented <a href=\"#fnref-1\" class=\"footnote-backref\" data-footnote-backref data-footnote-backref-idx=\"1\" aria-label=\"Back to reference 1\">↩</a></p>\n</li>\n</ol>\n</section>",
        "a shallow continuation lazily joins the definition's paragraph instead of closing it"
    );
}

#[test]
fn a_zero_indent_continuation_joins_the_definition() {
    // kills: crates/geist4-core/src/extension.rs:694:35: replace - with /
    // in BlockContinueCtx::indent
    assert_eq!(
        html("x[^1]\n\n[^1]: note\nsecond\n", &footnote_options()),
        "<p>x<sup class=\"footnote-ref\"><a href=\"#fn-1\" id=\"fnref-1\" data-footnote-ref>1</a></sup></p>\n<section class=\"footnotes\" data-footnotes>\n<ol>\n<li id=\"fn-1\">\n<p>note\nsecond <a href=\"#fnref-1\" class=\"footnote-backref\" data-footnote-backref data-footnote-backref-idx=\"1\" aria-label=\"Back to reference 1\">↩</a></p>\n</li>\n</ol>\n</section>",
        "a zero-indent line continues the definition, so it joins the note paragraph"
    );
}

#[test]
fn continuation_sees_the_definition_state() {
    // kills: crates/geist4-core/src/extension.rs:709:35: replace
    // BlockContinueCtx::state -> Option<&mut T> with None
    assert_eq!(
        html("x[^1]\n\n[^1]: first\nsecond\n", &footnote_options()),
        "<p>x<sup class=\"footnote-ref\"><a href=\"#fn-1\" id=\"fnref-1\" data-footnote-ref>1</a></sup></p>\n<section class=\"footnotes\" data-footnotes>\n<ol>\n<li id=\"fn-1\">\n<p>first\nsecond <a href=\"#fnref-1\" class=\"footnote-backref\" data-footnote-backref data-footnote-backref-idx=\"1\" aria-label=\"Back to reference 1\">↩</a></p>\n</li>\n</ol>\n</section>",
        "the state must be Some while the definition is open, so 'second' continues it"
    );
}

#[test]
fn consumed_columns_leave_the_rest_of_the_indent() {
    // kills: crates/geist4-core/src/extension.rs:718:9: replace
    // BlockContinueCtx::consume_cols with ()
    assert_eq!(
        html(
            "x[^1]\n\n[^1]: note\n      six-space\n",
            &footnote_options()
        ),
        "<p>x<sup class=\"footnote-ref\"><a href=\"#fn-1\" id=\"fnref-1\" data-footnote-ref>1</a></sup></p>\n<section class=\"footnotes\" data-footnotes>\n<ol>\n<li id=\"fn-1\">\n<p>note\nsix-space <a href=\"#fnref-1\" class=\"footnote-backref\" data-footnote-backref data-footnote-backref-idx=\"1\" aria-label=\"Back to reference 1\">↩</a></p>\n</li>\n</ol>\n</section>",
        "the six-space continuation joins the definition's paragraph with its remaining indent stripped"
    );
}
