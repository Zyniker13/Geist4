//! Tests for the declarative block tier: `[[block_container]]`,
//! `[[replace]]` and `[[attribute_list]]` semantics, end to end through
//! the core parser and renderer.

use std::collections::BTreeMap;
use std::sync::Arc;

use geist4_core::arena::NodeId;
use geist4_core::basic_html::render_with;
use geist4_core::extension::{
    AttrValue, BlockContinueCtx, BlockStartCtx, CloseCtx, ContinueOutcome, ExtNode, Extension,
    InlineBracketCtx, InlineTriggerCtx, KindDecl, LookCtx, Options, RenderedChild, TreeCtx,
};
use geist4_core::parser::{Document, parse_with};
use geist4_ext_loader::decl_block::{BlockTier, PostTier};
use geist4_ext_loader::decl_inline::InlineTier;
use geist4_ext_loader::types::{
    AttributeList, Behavior, BlockContainer, Extension as ManifestExtension, InlineSpan, KindEntry,
    Manifest, Replace,
};

/// A manifest's entry names, as the tests address them.
const NOTE: &str = "note_container";
const WARN: &str = "warn_container";
const QUOTE: &str = "quote_container";

/// A manifest compiled under test: the inline tier, the block tier and
/// the post-process tier forwarded from one [`Extension`], the way the
/// host's compiler composes the three.
struct Compiled {
    /// The extension's name.
    name: String,
    /// The manifest's kinds, as declarations.
    kinds: Vec<KindDecl>,
    /// The inline tier.
    inline: InlineTier,
    /// The block tier.
    block: BlockTier,
    /// The post-process tier.
    post: PostTier,
    /// The kinds the block tier renders.
    block_kinds: Vec<String>,
}

impl Compiled {
    /// Compiles a manifest.
    fn new(manifest: &Manifest) -> Self {
        let block = BlockTier::from_manifest(manifest);
        let block_kinds: Vec<String> = manifest
            .behavior
            .iter()
            .filter_map(|behavior| match behavior {
                Behavior::BlockContainer(entry) => Some(entry.kind.clone()),
                _ => None,
            })
            .collect();
        Self {
            name: manifest.extension.name.clone(),
            kinds: manifest
                .kinds
                .iter()
                .map(|entry| KindDecl {
                    kind: entry.kind.clone(),
                    container: entry.container,
                    inline_text: entry.inline_text,
                    attrs: entry.attrs.clone(),
                })
                .collect(),
            inline: InlineTier::from_manifest(manifest),
            block_kinds,
            block,
            post: PostTier::from_manifest(manifest),
        }
    }
}

impl Extension for Compiled {
    fn name(&self) -> &str {
        &self.name
    }

    fn kinds(&self) -> Vec<KindDecl> {
        self.kinds.clone()
    }

    fn inline_triggers(&self) -> &[u8] {
        self.inline.inline_triggers()
    }

    fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
        self.inline.inline_trigger(ctx);
    }

    fn inline_bracket(&self, ctx: &mut InlineBracketCtx<'_, '_, '_, '_>) {
        self.inline.inline_bracket(ctx);
    }

    fn contains_blocks(&self) -> bool {
        self.block.contains_blocks()
    }

    fn block_start_looks(&self, ctx: &LookCtx<'_, '_>) -> bool {
        self.block.block_start_looks(ctx)
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        self.block.try_start(ctx);
    }

    fn try_continue(&self, ctx: &mut BlockContinueCtx<'_, '_, '_>) -> ContinueOutcome {
        self.block.try_continue(ctx)
    }

    fn close(&self, state: &mut dyn std::any::Any, ctx: &mut CloseCtx<'_, '_>) {
        self.block.close(state, ctx);
    }

    fn render(&self, node: &ExtNode, children: &[RenderedChild<'_>], out: &mut String) {
        if self.block_kinds.contains(&node.kind) {
            self.block.render(node, children, out);
        } else {
            self.inline.render(node, children, out);
        }
    }

    fn post_process(&self, ctx: &mut TreeCtx<'_, '_, '_, '_>) {
        self.inline.post_process(ctx);
        self.post.post_process(ctx);
    }
}

/// One declared kind.
fn kind(name: &str, container: bool, inline_text: bool) -> KindEntry {
    KindEntry {
        kind: name.to_owned(),
        container,
        inline_text,
        attrs: Vec::new(),
    }
}

/// A `[[block_container]]` entry with the given kind, marker, name,
/// info template and EOF rule.
fn container(
    entry_kind: &str,
    marker: &str,
    name: &str,
    info_html: &str,
    unclosed_ok: bool,
) -> BlockContainer {
    let html = match entry_kind {
        NOTE => "<div class=\"note\">{children}</div>",
        WARN => "<aside class=\"warn\">{children}</aside>",
        _ => "<blockquote class=\"quote\">{info}{children}</blockquote>",
    };
    BlockContainer {
        kind: entry_kind.to_owned(),
        marker: marker.to_owned(),
        name: name.to_owned(),
        info_html: info_html.to_owned(),
        html: html.to_owned(),
        unclosed_ok,
    }
}

/// The default `:::note` entry.
fn note() -> BlockContainer {
    container(NOTE, ":::", "note", "", true)
}

/// A `[[replace]]` entry.
fn replace(from: &str, to: &str, once: bool) -> Replace {
    Replace {
        from: from.to_owned(),
        to: to.to_owned(),
        once,
    }
}

/// The `{:…}` attribute-list entry.
fn attr_list() -> AttributeList {
    AttributeList {
        kind: "attr_target".to_owned(),
        prefix: "{:".to_owned(),
        suffix: "}".to_owned(),
    }
}

/// The `==…==` inline span entry, for the inline attribute list.
fn highlight() -> InlineSpan {
    InlineSpan {
        kind: "hl".to_owned(),
        open: "==".to_owned(),
        close: "==".to_owned(),
        html: "<mark>{children}</mark>".to_owned(),
        can_open: true,
        can_close: true,
        multiline: true,
        math: false,
    }
}

/// A manifest with the given behavior entries over the test kinds.
fn manifest(behavior: Vec<Behavior>) -> Manifest {
    Manifest {
        extension: ManifestExtension {
            name: "test".to_owned(),
            version: "1.0.0".to_owned(),
            format: "1.0".to_owned(),
            description: String::new(),
            license: "MIT".to_owned(),
            homepage: None,
            hooks: Vec::new(),
            after: Vec::new(),
            conflicts: Vec::new(),
        },
        kinds: vec![
            kind(NOTE, true, false),
            kind(WARN, true, false),
            kind(QUOTE, true, false),
            kind("attr_target", false, false),
            kind("hl", false, true),
        ],
        options: BTreeMap::new(),
        behavior,
        wasm: None,
        host: false,
    }
}

/// Parses and renders `src` under the manifest.
fn html(src: &str, manifest: &Manifest) -> String {
    let mut options = Options::new();
    options.extension(Arc::new(Compiled::new(manifest)));
    let document = parse_with(src, &options);
    render_with(&document, &options)
}

/// Parses `src` under the manifest, for tree-level assertions.
fn parsed<'a>(src: &'a str, manifest: &Manifest) -> Document<'a> {
    let mut options = Options::new();
    options.extension(Arc::new(Compiled::new(manifest)));
    parse_with(src, &options)
}

/// Renders a document already parsed under `manifest`.
fn rendered(document: &Document<'_>, manifest: &Manifest) -> String {
    let mut options = Options::new();
    options.extension(Arc::new(Compiled::new(manifest)));
    render_with(document, &options)
}

/// The first extension node of `kind` in the document.
fn find_extension<'a>(document: &Document<'a>, kind: &str) -> Option<NodeId> {
    let mut work = vec![document.root];
    while let Some(id) = work.pop() {
        if let geist4_core::node::Node::Extension(ext) = document.arena.kind(id) {
            if ext.kind == kind {
                return Some(id);
            }
        }
        work.extend(document.arena.children(id).iter().copied());
    }
    None
}

/// The string value of `name` on `id`, from the node or the arena.
fn attr<'a>(document: &'a Document<'_>, id: NodeId, name: &str) -> Option<&'a str> {
    match document.arena.kind(id) {
        geist4_core::node::Node::Extension(ext) => ext
            .attrs
            .iter()
            .find(|(attr_name, _)| attr_name == name)
            .and_then(|(_, value)| match value {
                AttrValue::Str(text) => Some(text.as_str()),
                _ => None,
            }),
        _ => match document.arena.attr(id, name) {
            Some(AttrValue::Str(text)) => Some(text.as_str()),
            _ => None,
        },
    }
}

/// The default block container manifest: `:::note`, `===warn` and the
/// info-carrying `+++quote`.
fn block_manifest() -> Manifest {
    manifest(vec![
        Behavior::BlockContainer(note()),
        Behavior::BlockContainer(container(WARN, "===", "warn", "", true)),
        Behavior::BlockContainer(container(
            QUOTE,
            "+++",
            "quote",
            "<span class=\"info\">{children}</span>",
            true,
        )),
    ])
}

#[test]
fn container_round_trip() {
    let out = html(":::note\nhello\n\nworld\n:::\n", &block_manifest());
    assert_eq!(
        out,
        "<div class=\"note\"><p>hello</p>\n<p>world</p>\n</div>"
    );
}

#[test]
fn close_fence_is_consumed() {
    let out = html(":::note\nhi\n:::\n\nafter\n", &block_manifest());
    assert_eq!(out, "<div class=\"note\"><p>hi</p>\n</div>\n<p>after</p>\n");
    assert!(!out.contains(":::"));
}

#[test]
fn nested_same_name_containers() {
    let out = html(
        ":::note\nouter\n:::note\ninner\n:::\nafter inner\n:::\ntrailing\n",
        &block_manifest(),
    );
    assert_eq!(
        out,
        "<div class=\"note\"><p>outer</p>\n<div class=\"note\"><p>inner</p>\n</div><p>after inner</p>\n</div>\n<p>trailing</p>\n"
    );
}

#[test]
fn shorter_run_does_not_close() {
    let out = html(":::note\nhi\n::\nstill in\n:::\n", &block_manifest());
    assert_eq!(out, "<div class=\"note\"><p>hi\n::\nstill in</p>\n</div>");
}

#[test]
fn longer_run_does_not_close() {
    let out = html(":::note\nhi\n::::\nstill in\n:::\n", &block_manifest());
    assert_eq!(out, "<div class=\"note\"><p>hi\n::::\nstill in</p>\n</div>");
}

#[test]
fn three_space_indent_opens_and_closes() {
    let out = html("   :::note\nhi\n   :::\nafter\n", &block_manifest());
    assert_eq!(out, "<div class=\"note\"><p>hi</p>\n</div>\n<p>after</p>\n");
}

#[test]
fn four_space_indent_does_not_open() {
    let out = html("    :::note\nplain\n", &block_manifest());
    assert_eq!(out, "<pre><code>:::note\n</code></pre>\n<p>plain</p>\n");
}

#[test]
fn marker_interrupts_paragraph() {
    let out = html("text\n:::note\nmore\n:::\n", &block_manifest());
    assert_eq!(out, "<p>text</p>\n<div class=\"note\"><p>more</p>\n</div>");
}

#[test]
fn lazy_line_joins_paragraph_inside_container() {
    let out = html("> :::note\n> text\nlazy\n", &block_manifest());
    assert_eq!(
        out,
        "<blockquote>\n<div class=\"note\"><p>text\nlazy</p>\n</div>\n</blockquote>\n"
    );
}

#[test]
fn eof_closes_unclosed_container() {
    let out = html(":::note\nhi\n", &block_manifest());
    assert_eq!(out, "<div class=\"note\"><p>hi</p>\n</div>");
}

#[test]
fn eof_without_unclosed_ok_records_problem() {
    let strict = manifest(vec![Behavior::BlockContainer(container(
        NOTE, ":::", "note", "", false,
    ))]);
    let document = parsed(":::note\nhi\n", &strict);
    let id = find_extension(&document, NOTE).expect("the container opens");
    assert_eq!(
        attr(&document, id, "parse_problem"),
        Some("unclosed block container")
    );
    // The span reaches back to the open fence line.
    assert_eq!(document.arena.span(id).start, 0);
    assert_eq!(
        rendered(&document, &strict),
        "<div class=\"note\"><p>hi</p>\n</div>"
    );
}

#[test]
fn closed_by_fence_records_no_problem() {
    let strict = manifest(vec![Behavior::BlockContainer(container(
        NOTE, ":::", "note", "", false,
    ))]);
    let document = parsed(":::note\nhi\n:::\n", &strict);
    let id = find_extension(&document, NOTE).expect("the container opens");
    assert!(attr(&document, id, "parse_problem").is_none());
}

#[test]
fn info_string_is_rendered() {
    let out = html("+++quote See *this*\nbody\n+++\n", &block_manifest());
    assert_eq!(
        out,
        "<blockquote class=\"quote\"><span class=\"info\">See <em>this</em></span><p>body</p>\n</blockquote>"
    );
}

#[test]
fn info_without_template_is_ignored() {
    let out = html(":::note custom info\nhi\n:::\n", &block_manifest());
    assert_eq!(out, "<div class=\"note\"><p>hi</p>\n</div>");
}

#[test]
fn empty_info_leaves_empty_info_slot() {
    let out = html("+++quote\nbody\n+++\n", &block_manifest());
    assert_eq!(
        out,
        "<blockquote class=\"quote\"><p>body</p>\n</blockquote>"
    );
}

#[test]
fn entries_dispatch_in_declaration_order() {
    let out = html(":::note\na\n:::\n\n===warn\nb\n===\n", &block_manifest());
    assert_eq!(
        out,
        "<div class=\"note\"><p>a</p>\n</div>\n<aside class=\"warn\"><p>b</p>\n</aside>"
    );
}

#[test]
fn close_fence_may_trail_spaces() {
    let out = html(":::note\nhi\n:::   \n", &block_manifest());
    assert_eq!(out, "<div class=\"note\"><p>hi</p>\n</div>");
}

#[test]
fn bare_close_fence_stays_text() {
    let out = html(":::\n", &block_manifest());
    assert_eq!(out, "<p>:::</p>\n");
}

#[test]
fn marker_mid_line_stays_text() {
    let out = html("see :::note here\n", &block_manifest());
    assert_eq!(out, "<p>see :::note here</p>\n");
}

#[test]
fn lists_open_inside_container() {
    let out = html(":::note\n- a\n- b\n:::\n", &block_manifest());
    assert_eq!(
        out,
        "<div class=\"note\"><ul>\n<li>a</li>\n<li>b</li>\n</ul>\n</div>"
    );
}

/// The replacement manifest: the given `[[replace]]` entries.
fn replace_manifest(entries: Vec<Replace>) -> Manifest {
    manifest(entries.into_iter().map(Behavior::Replace).collect())
}

#[test]
fn replace_applies_to_every_occurrence_without_once() {
    let out = html(
        "a (c) b (c)\n",
        &replace_manifest(vec![replace("(c)", "©", false)]),
    );
    assert_eq!(out, "<p>a © b ©</p>\n");
}

#[test]
fn replace_once_limits_to_one_per_node() {
    let out = html(
        "a (c) b (c)\n",
        &replace_manifest(vec![replace("(c)", "©", true)]),
    );
    assert_eq!(out, "<p>a © b (c)</p>\n");
}

#[test]
fn replaces_chain_in_declaration_order() {
    let out = html(
        "ab ab\n",
        &replace_manifest(vec![replace("ab", "X", false), replace("X", "Y", false)]),
    );
    assert_eq!(out, "<p>Y Y</p>\n");
}

#[test]
fn replace_skips_code_spans() {
    let out = html("`a` a\n", &replace_manifest(vec![replace("a", "b", false)]));
    assert_eq!(out, "<p><code>a</code> b</p>\n");
}

#[test]
fn replace_skips_code_blocks() {
    let out = html(
        "```\na\n```\n\na\n",
        &replace_manifest(vec![replace("a", "b", false)]),
    );
    assert_eq!(out, "<pre><code>a\n</code></pre>\n<p>b</p>\n");
}

#[test]
fn replace_skips_raw_html_tags() {
    let out = html(
        "<div>a</div>\n\nq <span data-x=\"a\">a</span>\n",
        &replace_manifest(vec![replace("a", "b", false)]),
    );
    // The block's raw text and the inline tags keep their `a`; the
    // text between the inline tags is ordinary text and does not.
    assert_eq!(out, "<div>a</div>\n<p>q <span data-x=\"a\">b</span></p>\n");
}

#[test]
fn replace_never_crosses_a_node_boundary() {
    let out = html("x*b*\n", &replace_manifest(vec![replace("xb", "y", false)]));
    assert_eq!(out, "<p>x<em>b</em></p>\n");
}

/// The attribute-list manifest: the list rule plus a container and a
/// span to attach to.
fn attr_manifest() -> Manifest {
    manifest(vec![
        Behavior::BlockContainer(note()),
        Behavior::AttributeList(attr_list()),
        Behavior::InlineSpan(highlight()),
    ])
}

#[test]
fn attribute_list_attaches_to_preceding_paragraph() {
    let manifest = attr_manifest();
    let document = parsed("hello\n\n{: #intro .note k=v}\n", &manifest);
    let id = document.arena.children(document.root)[0];
    assert_eq!(attr(&document, id, "id"), Some("intro"));
    assert_eq!(attr(&document, id, "class"), Some("note"));
    assert_eq!(attr(&document, id, "k"), Some("v"));
    assert_eq!(rendered(&document, &manifest), "<p>hello</p>\n");
}

#[test]
fn attribute_list_final_line_attaches_to_paragraph() {
    let manifest = attr_manifest();
    let document = parsed("hello\n{: .note}\n", &manifest);
    let id = document.arena.children(document.root)[0];
    assert_eq!(attr(&document, id, "class"), Some("note"));
    assert_eq!(rendered(&document, &manifest), "<p>hello</p>\n");
}

#[test]
fn second_list_appends_class() {
    let document = parsed("hello\n\n{: .a}\n\n{: .b}\n", &attr_manifest());
    let id = document.arena.children(document.root)[0];
    assert_eq!(attr(&document, id, "class"), Some("a b"));
}

#[test]
fn malformed_list_stays_text() {
    let out = html("{: .}\n", &attr_manifest());
    assert_eq!(out, "<p>{: .}</p>\n");
}

#[test]
fn bare_key_and_quoted_value() {
    let document = parsed("hello\n\n{: title=\"a &amp; b\" draft}\n", &attr_manifest());
    let id = document.arena.children(document.root)[0];
    assert_eq!(attr(&document, id, "title"), Some("a & b"));
    assert_eq!(attr(&document, id, "draft"), Some(""));
}

#[test]
fn over_the_entry_cap_stays_text() {
    let items: Vec<String> = (0..33)
        .map(|n| {
            format!(
                "k-{}{}",
                (b'a' + (n / 26) as u8) as char,
                (b'a' + (n % 26) as u8) as char
            )
        })
        .collect();
    let list = format!("{{: {}}}", items.join(" "));
    let out = html(&list, &attr_manifest());
    assert_eq!(out, format!("<p>{list}</p>\n"));
}

#[test]
fn attribute_list_attaches_to_container_node() {
    let manifest = attr_manifest();
    let document = parsed(":::note\nx\n:::\n\n{: #special .big}\n", &manifest);
    let id = find_extension(&document, NOTE).expect("the container opens");
    assert_eq!(attr(&document, id, "id"), Some("special"));
    assert_eq!(attr(&document, id, "class"), Some("big"));
    assert_eq!(
        rendered(&document, &manifest),
        "<div class=\"note\"><p>x</p>\n</div>"
    );
}

#[test]
fn attribute_list_attaches_to_inline_span() {
    let manifest = attr_manifest();
    let document = parsed("a ==b=={:.c} d\n", &manifest);
    let id = find_extension(&document, "hl").expect("the span opens");
    assert_eq!(attr(&document, id, "class"), Some("c"));
    assert_eq!(
        rendered(&document, &manifest),
        "<p>a <mark>b</mark> d</p>\n"
    );
}

#[test]
fn list_without_target_stays_text() {
    let out = html("{: .x}\n", &attr_manifest());
    assert_eq!(out, "<p>{: .x}</p>\n");
}
