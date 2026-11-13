//! Built-in Geist4 extensions: GitHub Flavored Markdown (tables,
//! strikethrough, extended autolinks, task list items, and the disallowed
//! raw HTML filter), footnotes, front matter, heading identifiers, and
//! definition lists.
//!
//! Built-in extensions use only the public extension API of the core, exactly
//! like loaded extensions, so the API stays honest. See the repository
//! roadmap.

use geist4_core::extension::{ExtNode, Extension, InlineTriggerCtx, KindDecl, RenderedChild, Run};

/// GFM strikethrough: text wrapped in a matching pair of one or two tildes
/// becomes a `strikethrough` node, rendered as `<del>`.
///
/// The semantics follow the reference implementation: a run of one or two
/// tildes with at least one flanking side joins the delimiter stack; runs
/// of three or more tildes, and runs with no flanking side, are literal
/// text. An opener and a closer pair only when their runs have the same
/// length, so `~~a~` never opens a node.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Strikethrough;

impl Strikethrough {
    /// The strikethrough extension's node kind name.
    pub const KIND: &'static str = "strikethrough";
}

impl Extension for Strikethrough {
    fn name(&self) -> &str {
        "strikethrough"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: Self::KIND.to_owned(),
            container: true,
            inline_text: true,
            attrs: Vec::new(),
        }]
    }

    fn inline_triggers(&self) -> &[u8] {
        b"~"
    }

    fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
        let run: Run = ctx.scan_run();
        // The reference implementation pushes runs of one or two tildes
        // that have at least one flanking side; longer runs are literal.
        if (run.can_open || run.can_close) && run.length <= 2 {
            ctx.push_delim(&run, Self::KIND);
        }
    }

    fn render(&self, node: &ExtNode, children: &[RenderedChild<'_>], out: &mut String) {
        if node.kind != Self::KIND {
            return;
        }
        out.push_str("<del>");
        for child in children {
            out.push_str(&child.html);
        }
        out.push_str("</del>");
    }
}

/// The strikethrough extension as a shared extension value.
#[must_use]
pub fn strikethrough() -> std::sync::Arc<dyn Extension> {
    std::sync::Arc::new(Strikethrough)
}

/// The autolink extension as a shared extension value.
#[must_use]
pub fn autolink() -> std::sync::Arc<dyn Extension> {
    std::sync::Arc::new(autolink::Autolink)
}

/// The tables extension as a shared extension value.
#[must_use]
pub fn tables() -> std::sync::Arc<dyn Extension> {
    std::sync::Arc::new(tables::Tables)
}

/// The task list item extension as a shared extension value.
#[must_use]
pub fn tasklist() -> std::sync::Arc<dyn Extension> {
    tasklist::tasklist()
}

/// The disallowed raw HTML filter extension as a shared extension value.
#[must_use]
pub fn tagfilter() -> std::sync::Arc<dyn Extension> {
    tagfilter::tagfilter()
}

/// The footnotes extension as a shared extension value.
#[must_use]
pub fn footnotes() -> std::sync::Arc<dyn Extension> {
    footnotes::footnotes()
}

pub mod autolink;
pub mod footnotes;
pub mod tables;
pub mod tagfilter;
pub mod tasklist;
