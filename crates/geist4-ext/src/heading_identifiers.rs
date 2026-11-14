//! GFM heading identifiers: every ATX or setext heading carries an
//! `id` attribute generated from its inline content, and a claimed
//! heading renders as `<hN id="...">...</hN>` instead of the plain
//! built-in `<hN>` form.
//!
//! The identifier algorithm follows github-slugger 2.0.0 exactly — the
//! implementation the GFM appendix "Generating IDs" describes and that
//! GitHub anchors use: lowercase, remove every character the reference's
//! removal expression matches, turn spaces into hyphens, and dedupe
//! repeats with `-1`, `-2`, ... suffixes. Nothing here is transcribed by
//! hand: the removal table in [`crate::slug_table`] is generated from the
//! vendored reference expression (`tools/github-slugger-regex.txt`), and
//! the port's behavior is frozen against the machine-generated fixture
//! `tests/fixtures/slugger_oracle.json` (`tools/slugger_probe.mjs`).
//!
//! The text an identifier is generated from is the heading's decoded
//! inline text content, in document order — what GitHub's DOM pipeline
//! would see as the heading's text content: text and code spans
//! contribute their text (entities are decoded at parse time, so text
//! nodes already carry the decoded characters), emphasis, strong, links
//! and inline extension nodes contribute their descendants' text, soft
//! and hard breaks contribute a newline each, and raw HTML nodes and
//! images contribute nothing, exactly as a comment, processing
//! instruction or `img` element contributes nothing to an element's
//! text content. Decoded angle brackets need no stripping: the removal
//! table drops `<` and `>` itself (`&lt;tag&gt;` slugs to `tag`, the
//! value the reference pipeline produces).
//!
//! Four quirks of the reference pipeline (marked-gfm-heading-id) are
//! deliberately not reproduced; each follows from marked keeping named
//! entities raw in its rendered HTML — which is not
//! CommonMark-conformant — and stripping tags with a lazy regex, and
//! every case was confirmed against the real pipeline
//! (`tools/heading_oracle_probe.mjs`):
//!
//! - `# caf&eacute;`: the reference eats the named entity (`caf`);
//!   decoded text keeps the accent (`café`).
//! - `# <!-- a > b --> title`: the reference's regex stops at the
//!   first `>` and leaks ` b -->` into the slug (`-b----title`); a
//!   comment contributes nothing here (`title`).
//! - `# <?php ?> code`: the reference's regex never matches a
//!   processing instruction (`php--code`); one contributes nothing
//!   here (`code`).
//! - `# x <b> `: the reference strips after unescaping and keeps the
//!   inner space as a trailing hyphen (`x-`); trimming the extracted
//!   text gives `x` here.
//!
//! Identifiers are assigned in document order, so two headings with the
//! same text get `text` and `text-1`; the state lives on the stack of a
//! single pass, and the extension keeps none between parses.

use geist4_core::arena::NodeId;
use geist4_core::extension::{AttrValue, CoreNode, Extension, OverrideChildren, TreeCtx};
use geist4_core::node::Node;

/// The GFM heading identifiers extension.
///
/// The extension claims every heading in the document during
/// `post_process`, sets the generated `id` (and the heading's level, for
/// the override) as attributes, and renders claimed headings itself.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HeadingIdentifiers;

impl HeadingIdentifiers {
    /// The `id` attribute the extension sets on claimed headings.
    pub const ID: &'static str = "id";

    /// The heading level attribute the extension sets on claimed
    /// headings, for its rendering override to read.
    pub const LEVEL: &'static str = "level";
}

impl Extension for HeadingIdentifiers {
    fn name(&self) -> &str {
        "heading_identifiers"
    }

    fn post_process(&self, ctx: &mut TreeCtx<'_, '_, '_>) {
        // The hook has no index of its own; the registration order in
        // the options is the index the arena's claim slot stores.
        let Some(index) = ctx
            .options
            .extensions()
            .iter()
            .position(|ext| ext.name() == self.name())
        else {
            return;
        };
        // One slugger per parse, exactly like the reference pipeline's
        // per-document instance.
        let mut slugger = crate::slugger::Slugger::new();
        // Depth-first pre-order over the tree visits headings in
        // document order; children are pushed reversed so they pop in
        // order.
        let mut stack = vec![ctx.root];
        while let Some(node) = stack.pop() {
            let kind = ctx.arena.kind(node).clone();
            match &kind {
                Node::Heading(data) => {
                    let level = data.level;
                    let children = ctx.arena.children(node).to_vec();
                    let mut text = String::new();
                    collect_text(ctx.arena, &children, &mut text);
                    // The extracted text is trimmed before slugging; the
                    // slug itself never trims again.
                    let id = slugger.slug(text.trim(), false);
                    ctx.arena.set_extension(node, index);
                    ctx.arena.set_attr(node, Self::ID, AttrValue::Str(id));
                    ctx.arena
                        .set_attr(node, Self::LEVEL, AttrValue::Int(i64::from(level)));
                }
                // Block containers carry headings deeper in the tree.
                Node::Document | Node::BlockQuote | Node::List(_) | Node::Item(_) => {
                    stack.extend(ctx.arena.children(node).iter().rev().copied());
                }
                // Everything else holds no headings.
                _ => {}
            }
        }
    }

    fn render_override(
        &self,
        node: &CoreNode<'_>,
        children: &mut OverrideChildren<'_, '_>,
        out: &mut String,
    ) {
        if node.kind != "heading" {
            return;
        }
        let Some(level) = level_of(node.attrs) else {
            return;
        };
        let Some(AttrValue::Str(id)) = node
            .attrs
            .iter()
            .find(|(name, _)| name == Self::ID)
            .map(|(_, value)| value)
        else {
            return;
        };
        // A slug holds only letters, marks, numbers and connector
        // punctuation, so no HTML attribute escaping can apply.
        out.push_str("<h");
        out.push(char::from(b'0' + level));
        out.push_str(" id=\"");
        out.push_str(id);
        out.push_str("\">");
        while children.render_next(out) {}
        out.push_str("</h");
        out.push(char::from(b'0' + level));
        out.push_str(">\n");
    }
}

/// The heading level the extension set on a claimed heading, if it is a
/// plausible level of 1 to 6.
fn level_of(attrs: &[(String, AttrValue)]) -> Option<u8> {
    let value = attrs
        .iter()
        .find(|(name, _)| name == HeadingIdentifiers::LEVEL)
        .map(|(_, value)| value)?;
    let AttrValue::Int(level) = value else {
        return None;
    };
    let level = u8::try_from(*level).ok()?;
    (1..=6).contains(&level).then_some(level)
}

/// Collects the inline text a heading's identifier is generated from:
/// the heading's decoded text content in document order, with raw HTML
/// nodes and images contributing nothing and line breaks contributing
/// newlines.
fn collect_text(arena: &geist4_core::arena::Arena<'_>, children: &[NodeId], out: &mut String) {
    for &child in children {
        match arena.kind(child) {
            Node::Text(text) => out.push_str(text),
            Node::CodeSpan(text) => out.push_str(text),
            // Both break forms leave a newline in the reference's
            // output; the removal step drops it either way.
            Node::SoftBreak | Node::HardBreak => out.push('\n'),
            Node::Emphasis | Node::Strong => {
                let inner = arena.children(child).to_vec();
                collect_text(arena, &inner, out);
            }
            // A link contributes its text; an image contributes nothing.
            Node::Link(data) if !data.image => {
                let inner = arena.children(child).to_vec();
                collect_text(arena, &inner, out);
            }
            // Raw HTML — a tag, comment or processing instruction —
            // contributes nothing, like its DOM counterpart.
            Node::InlineHtml(_) => {}
            // An extension's inline node contributes whatever its
            // children's text amounts to.
            Node::Extension(_) => {
                let inner = arena.children(child).to_vec();
                collect_text(arena, &inner, out);
            }
            // Blocks cannot appear in a heading's inline content.
            _ => {}
        }
    }
}

/// The heading identifiers extension as a shared extension value.
#[must_use]
pub fn heading_identifiers() -> std::sync::Arc<dyn Extension> {
    std::sync::Arc::new(HeadingIdentifiers)
}
