//! The extension API's plumbing, proven end to end with a test
//! extension: its node kinds are declared, its post-processing pass
//! rewrites the tree, its renderer is called for its nodes, and plain
//! parsing without options is untouched.

use geist4_core::arena::NodeId;
use geist4_core::basic_html::{render, render_with};
use geist4_core::extension::{
    AttrValue, ExtNode, Extension, KindDecl, Options, RenderedChild, TreeCtx,
};
use geist4_core::node::Node;
use geist4_core::parser::parse_with;
use std::sync::Arc;

/// Wraps one paragraph's text in an extension node.
struct Marker;

impl Extension for Marker {
    fn name(&self) -> &str {
        "marker"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "marked".to_owned(),
            container: true,
            inline_text: true,
            attrs: vec!["label".to_owned()],
        }]
    }

    fn post_process(&self, ctx: &mut TreeCtx<'_, '_, '_, '_>) {
        let mut stack: Vec<NodeId> = ctx.arena.children(ctx.root).to_vec();
        while let Some(id) = stack.pop() {
            let text = match ctx.arena.kind(id) {
                Node::Paragraph => ctx
                    .arena
                    .children(id)
                    .iter()
                    .filter_map(|child| match ctx.arena.kind(*child) {
                        Node::Text(text) => Some(text.to_string()),
                        _ => None,
                    })
                    .collect::<String>(),
                _ => {
                    stack.extend(ctx.arena.children(id).iter().copied());
                    continue;
                }
            };
            let text_id = ctx
                .arena
                .alloc(Node::Text(text.into()), geist4_core::span::Span::empty(0));
            let ext_id = ctx.arena.alloc(
                Node::Extension(ExtNode {
                    kind: "marked".to_owned(),
                    attrs: vec![("label".to_owned(), AttrValue::Str("seen".to_owned()))],
                }),
                geist4_core::span::Span::empty(0),
            );
            ctx.arena.entry_mut(id).children.clear();
            ctx.arena.push_child(id, ext_id);
            ctx.arena.push_child(ext_id, text_id);
        }
    }

    fn render(&self, node: &ExtNode, children: &[RenderedChild<'_>], out: &mut String) {
        if node.kind != "marked" {
            return;
        }
        let label = match &node.attrs[0].1 {
            AttrValue::Str(value) => value.clone(),
            _ => String::new(),
        };
        out.push_str(&format!("<marked label=\"{label}\">"));
        for child in children {
            out.push_str(&child.html);
        }
        out.push_str("</marked>");
    }
}

#[test]
fn post_process_and_render_run() {
    let mut options = Options::new();
    options.extension(Arc::new(Marker));
    let doc = parse_with("hello\n", &options);
    let para = doc.arena.children(doc.root)[0];
    assert!(matches!(doc.arena.kind(para), Node::Paragraph));
    assert!(matches!(
        doc.arena.kind(doc.arena.children(para)[0]),
        Node::Extension(_)
    ));
    assert_eq!(
        render_with(&doc, &options),
        "<p><marked label=\"seen\">hello</marked></p>\n"
    );
}

#[test]
fn plain_parsing_is_untouched() {
    let mut options = Options::new();
    options.extension(Arc::new(Marker));
    assert!(!options.is_empty());
    assert_eq!(options.by_name("marker").unwrap().name(), "marker");
    // The same source parses as plain CommonMark without the extension.
    assert_eq!(
        render(&parse_with("hello\n", &Options::new())),
        "<p>hello</p>\n"
    );
    assert_eq!(
        render(&geist4_core::parse_document("hello\n")),
        "<p>hello</p>\n"
    );
}

#[test]
fn unregistered_kinds_render_nothing() {
    struct Other;
    impl Extension for Other {
        fn name(&self) -> &str {
            "other"
        }
    }
    let mut options = Options::new();
    options.extension(Arc::new(Other));
    let doc = parse_with("hello\n", &options);
    // No post-processing ran, so no extension node exists to render;
    // the output is the plain paragraph, ending with the newline the
    // renderer puts after the last block.
    assert_eq!(render_with(&doc, &options), "<p>hello</p>\n");
}

/// A hostile block start: every non-indented line matches, and it
/// never consumes anything — the same text is offered back to it, so
/// the parse only ends because the parser caps extension opens per
/// line.
struct Greedy;

impl Extension for Greedy {
    fn name(&self) -> &str {
        "greedy"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: "greedy".to_owned(),
            container: true,
            inline_text: true,
            attrs: vec![],
        }]
    }

    fn try_start(&self, ctx: &mut geist4_core::extension::BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.indented() {
            return;
        }
        ctx.seek_content(0);
        ctx.open_extension_keep_rest("greedy".to_owned(), Box::new(()));
    }
}

#[test]
fn a_block_start_that_never_advances_is_capped() {
    let mut options = Options::new();
    options.extension(Arc::new(Greedy));
    // The parse terminates even though every line re-matches: the
    // per-line cap bounds the extension's opens.
    let doc = parse_with("text\n", &options);
    let mut extensions = 0usize;
    let mut stack = vec![doc.root];
    while let Some(id) = stack.pop() {
        if matches!(doc.arena.kind(id), Node::Extension(_)) {
            extensions += 1;
        }
        stack.extend(doc.arena.children(id).iter().copied());
    }
    assert_eq!(extensions, 16);
    // The document still renders without panicking.
    let _ = render_with(&doc, &options);
}
