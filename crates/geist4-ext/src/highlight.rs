//! Built-in highlight marks: `==…==` becomes a `mark` node, rendered
//! as `<mark>` with the parsed inline content as its children.
//!
//! The semantics are the declarative tier's `[[inline_span]]` rules,
//! exactly as `conformance/behavior/inline-spans` pins them: a run of
//! two or more `=` bytes is one delimiter unit, and runs pair only
//! when their lengths are equal, so `====x====` is one mark, `===x==`
//! stays literal, and a lone `=` is not an occurrence of the
//! delimiter at all (`=x=` stays literal). The ordinary flanking rules
//! gate both sides, the first close after the open wins, backslash
//! escapes work like emphasis's, and code spans are immune. The spans
//! nest with the core emphasis machinery from either side, and a span
//! may cross a soft break inside one paragraph (the `multiline`
//! default), matching the tier.

use geist4_core::extension::{ExtNode, Extension, InlineTriggerCtx, KindDecl, RenderedChild, Run};

/// The built-in highlight extension.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Highlight;

impl Highlight {
    /// The highlight kind's name.
    pub const KIND: &'static str = "mark";

    /// Highlight as a shared extension value.
    #[must_use]
    pub fn extension() -> std::sync::Arc<dyn Extension> {
        std::sync::Arc::new(Highlight)
    }
}

impl Extension for Highlight {
    fn name(&self) -> &str {
        "highlight"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: Self::KIND.to_owned(),
            container: false,
            inline_text: true,
            attrs: Vec::new(),
        }]
    }

    fn inline_triggers(&self) -> &[u8] {
        b"="
    }

    fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
        let run: Run = ctx.scan_run();
        // A run shorter than the delimiter is not an occurrence of it:
        // the two-byte delimiter needs at least two of its byte, so
        // `=x=` stays literal. Runs pair only on equal length, which
        // the core's extension matcher enforces.
        if run.length >= 2 && (run.can_open || run.can_close) {
            // No border guards apply; the stack cell carries the
            // flanking verdicts as scanned.
            ctx.push_delim(&run, Self::KIND);
        }
    }

    fn render(&self, node: &ExtNode, children: &[RenderedChild<'_>], out: &mut String) {
        if node.kind != Self::KIND {
            return;
        }
        out.push_str("<mark>");
        for child in children {
            out.push_str(&child.html);
        }
        out.push_str("</mark>");
    }
}
