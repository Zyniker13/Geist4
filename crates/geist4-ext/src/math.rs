//! Built-in inline math: `$…$` renders as an inline math span and
//! `$$…$$` as a display math span, with the parsed inline content as
//! the node's children.
//!
//! The semantics are the declarative tier's `[[inline_span]]` rules,
//! applied to the dollar delimiters, plus the shape decision 0047
//! records: a run of dollars pushes one delimiter tagged with the kind
//! its length selects — `math_inline` for a single dollar,
//! `math_display` for two or more — and a pair matches only when both
//! runs are the same byte and the same kind. That keeps `$` and `$$`
//! apart by construction: `$$$x$$$` is a display pair, `$x$` an inline
//! pair, and neither half of one can close the other.
//!
//! Inline `$…$` follows the tier's border guards exactly (`$ x$`,
//! `$5$`, `5$x$` and `$x $` stay literal; interior spaces are fine).
//! Display `$$` runs carry no border guards — the reference scanner
//! has none for two or more dollars — but the ordinary flanking rules
//! still gate them, so `$$ $$` stays literal. Spans may cross soft
//! breaks inside one paragraph (the `multiline` default), which is
//! what lets display math hold a multi-line body.

use geist4_core::extension::{ExtNode, Extension, InlineTriggerCtx, KindDecl, RenderedChild, Run};

/// The built-in math extension.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Math;

impl Math {
    /// The inline math kind's name.
    pub const INLINE: &'static str = "math_inline";
    /// The display math kind's name.
    pub const DISPLAY: &'static str = "math_display";

    /// Math as a shared extension value.
    #[must_use]
    pub fn extension() -> std::sync::Arc<dyn Extension> {
        std::sync::Arc::new(Math)
    }
}

impl Extension for Math {
    fn name(&self) -> &str {
        "math"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![
            KindDecl {
                kind: Self::INLINE.to_owned(),
                container: false,
                inline_text: true,
                attrs: Vec::new(),
            },
            KindDecl {
                kind: Self::DISPLAY.to_owned(),
                container: false,
                inline_text: true,
                attrs: Vec::new(),
            },
        ]
    }

    fn inline_triggers(&self) -> &[u8] {
        b"$"
    }

    fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
        let mut run: Run = ctx.scan_run();
        // One dollar is the inline kind; two or more are display, per
        // the shape decision 0047 records. The stack cell's kind is
        // what a matching run must agree on, so the pair never mixes.
        let kind = if run.length == 1 {
            Self::INLINE
        } else {
            Self::DISPLAY
        };
        if run.length == 1 {
            // The tier's border guards, exactly: an opener needs a
            // non-space, non-digit character right after the run and
            // no digit right before it; a closer needs a non-space
            // character right before it. They narrow the flanking
            // verdicts, and a narrowed run must not match as the side
            // the guard declined.
            if run.can_open && !math_open_allowed(ctx, run.length) {
                run.can_open = false;
            }
            if run.can_close && !math_close_allowed(ctx, run.length) {
                run.can_close = false;
            }
        }
        if run.can_open || run.can_close {
            ctx.push_delim(&run, kind);
        }
    }

    fn render(&self, node: &ExtNode, children: &[RenderedChild<'_>], out: &mut String) {
        match node.kind.as_str() {
            Self::INLINE => {
                out.push_str("<span data-math-style=\"inline\">");
                for child in children {
                    out.push_str(&child.html);
                }
                out.push_str("</span>");
            }
            Self::DISPLAY => {
                out.push_str("<span data-math-style=\"display\">");
                for child in children {
                    out.push_str(&child.html);
                }
                out.push_str("</span>");
            }
            _ => {}
        }
    }
}

/// The inline math rule for an opening delimiter: the character right
/// after the run must exist and be neither whitespace nor a digit, and
/// the character right before the run must not be a digit — `$ x$`,
/// `$5$` and `5$x$` stay literal. That is the border form of the rule
/// that math content has no leading space and never follows a digit.
fn math_open_allowed(ctx: &InlineTriggerCtx<'_, '_, '_, '_>, run_len: usize) -> bool {
    let after = ctx.rest_of_line();
    if border_is_space(after) {
        return false;
    }
    if after.first().is_some_and(|byte| byte.is_ascii_digit()) {
        return false;
    }
    let before = ctx.line_prefix();
    match before.len().checked_sub(run_len + 1) {
        Some(at) => !before[at].is_ascii_digit(),
        None => true,
    }
}

/// The inline math rule for a closing delimiter: the character right
/// before the run must exist and not be whitespace — `$x $` stays
/// literal. The line end counts as whitespace; a run at a line's start
/// cannot close anyway because the flanking walk sees the newline
/// stand-in there.
fn math_close_allowed(ctx: &InlineTriggerCtx<'_, '_, '_, '_>, run_len: usize) -> bool {
    let before = ctx.line_prefix();
    match before.len().checked_sub(run_len + 1) {
        Some(at) => !border_is_space(&before[at..]),
        None => false,
    }
}

/// Whether the character at the start of `bytes` is whitespace, or
/// there is no character because the line ends: both border guards
/// treat the line end as whitespace.
fn border_is_space(bytes: &[u8]) -> bool {
    let Some(&first) = bytes.first() else {
        return true;
    };
    if first.is_ascii_whitespace() {
        return true;
    }
    if first >= 0x80 {
        return decoded_char(bytes).is_some_and(geist4_core::unicode::is_space);
    }
    false
}

/// The character at the start of `bytes`, decoded as UTF-8 so
/// multi-byte characters test whole, mirroring the flanking walk.
fn decoded_char(bytes: &[u8]) -> Option<char> {
    let len = match bytes.first()? {
        first if *first < 0x80 => 1,
        first if first & 0xE0 == 0xC0 => 2,
        first if first & 0xF0 == 0xE0 => 3,
        first if first & 0xF8 == 0xF0 => 4,
        _ => return None,
    };
    std::str::from_utf8(bytes.get(..len)?).ok()?.chars().next()
}
