//! Syntax highlighting for Geist4 fenced code blocks.
//!
//! The crate turns a fenced code block's literal plus its info string
//! into class-based HTML tokens: `<span class="…">` fragments whose
//! class names come from syntect scope names (a scope like
//! `storage.type.rust` becomes the class `type`). No theme, no inline
//! color: the colors live in the host's stylesheet.
//!
//! The engine is syntect with the fancy-regex backend (decision 0043),
//! which builds for `wasm32-unknown-unknown`; the default syntax pack
//! loads from the dumps syntect itself ships. An unknown info string
//! yields `None` from [`highlight`], and the caller renders plain
//! escaped code — highlighting never errors a render.
//!
//! The API is deliberately independent of `geist4-core` types: the
//! caller (the HTML renderer's highlighting mode) passes the code text
//! and the language token and gets back an owned, escaped HTML string.

#![forbid(unsafe_code)]

use geist4_core::basic_html::escape_html;
use std::sync::OnceLock;

/// The shared syntax set: syntect's defaults, loaded once and reused
/// across every render (the extension object is shared, so the pack is
/// too). `ParseState` borrows from it, so highlighting takes the set
/// from here rather than constructing it per call.
static SYNTAX_SET: OnceLock<syntect::parsing::SyntaxSet> = OnceLock::new();

fn syntax_set() -> &'static syntect::parsing::SyntaxSet {
    SYNTAX_SET.get_or_init(syntect::parsing::SyntaxSet::load_defaults_newlines)
}

/// The scope components we name classes after. A scope stack's last
/// pushed scope decides the class; its atoms are matched tail-first, so
/// `constant.numeric.integer.decimal.rust` yields `numeric` first,
/// `constant` only if no more specific atom matches. The list follows
/// the TextMate conventional atom order the themes themselves use.
const CLASS_ATOMS: &[&str] = &[
    "comment",
    "string",
    "constant",
    "numeric",
    "character",
    "keyword",
    "operator",
    "punctuation",
    "entity",
    "meta",
    "storage",
    "variable",
    "support",
    "markup",
    "invalid",
    "source",
    "text",
    "embedded",
];

/// Extracts the class name for a scope: the first conventional atom
/// found scanning the scope's atoms from the last to the first, mapped
/// through [`CLASS_ATOMS`]. A scope that matches no atom falls back to
/// the `source`/`text` root kind, so every span gets a class.
fn class_for_scope(scope: syntect::parsing::Scope) -> String {
    let text = scope.to_string();
    for atom in text.split('.').rev() {
        if CLASS_ATOMS.contains(&atom) {
            return atom.to_string();
        }
    }
    "source".to_string()
}

/// The ready-to-register highlighter: implements core's
/// [`CodeHighlighter`](geist4_core::extension::CodeHighlighter) so an
/// options value can enable highlighting with one call.
///
/// The struct is a shared, stateless token mapper; the syntax pack it
/// reads is the process-wide syntax set, loaded once. Every method
/// is therefore safe to call from any thread, and registering the same
/// instance on several options values shares the pack.
#[derive(Debug, Default, Clone, Copy)]
pub struct SyntaxHighlighter;

impl geist4_core::extension::CodeHighlighter for SyntaxHighlighter {
    fn highlight(&self, code: &str, language: &str) -> Option<String> {
        highlight(code, language)
    }
}

/// Applies one parser operation to the scope stack. `ScopeStack::apply`
/// handles every op kind — pushes, pops and clears — with syntect's own
/// semantics; no hand-rolled matcher to drift.
fn apply_op(stack: &mut syntect::parsing::ScopeStack, op: &syntect::parsing::ScopeStackOp) {
    let _ = stack.apply(op);
}

/// Highlights `code` as `language`, returning the inner HTML of the
/// `<code>` element: escaped text wrapped in `<span class="…">`
/// fragments. Returns `None` when the language is unknown — the caller
/// renders plain code instead, byte-identical to the non-highlighting
/// renderer.
///
/// The spans are derived from the parser's own byte-offset ops: each
/// token runs between consecutive op offsets, its class taken from the
/// topmost scope after the ops at or before its start applied. A span
/// opens only when its class differs from the previous token's, so
/// adjacent same-class tokens merge into one span.
pub fn highlight(code: &str, language: &str) -> Option<String> {
    let set = syntax_set();
    let syntax = set.find_syntax_by_token(language)?;
    let mut state = syntect::parsing::ParseState::new(syntax);
    let mut stack = syntect::parsing::ScopeStack::new();
    let mut out = String::with_capacity(code.len() + 32);
    let mut open_class: Option<String> = None;

    for line in syntect::util::LinesWithEndings::from(code) {
        let ops = state.parse_line(line, set).ok()?;
        let mut pos = 0usize;
        let mut next_op = 0usize;
        while pos < line.len() {
            // Apply every op at or before the current position; the
            // token then runs to the next op offset or the line end.
            while next_op < ops.len() && ops[next_op].0 <= pos {
                apply_op(&mut stack, &ops[next_op].1);
                next_op += 1;
            }
            let end = if next_op < ops.len() && ops[next_op].0 > pos {
                ops[next_op].0.min(line.len())
            } else {
                line.len()
            };
            if end > pos {
                let text = &line[pos..end];
                let class = stack
                    .as_slice()
                    .last()
                    .map(|s| class_for_scope(*s))
                    .unwrap_or_else(|| "source".to_string());
                if open_class.as_deref() != Some(class.as_str()) {
                    if open_class.take().is_some() {
                        out.push_str("</span>");
                    }
                    out.push_str("<span class=\"");
                    out.push_str(&class);
                    out.push_str("\">");
                    open_class = Some(class);
                }
                out.push_str(&escape_html(text));
                pos = end;
            } else {
                // Ops beyond the line end cannot advance the slice; emit
                // the remaining bytes verbatim (escaped) to guarantee
                // termination and completeness.
                out.push_str(&escape_html(&line[pos..]));
                break;
            }
        }
        // Consume any remaining ops inside this line's byte range so the
        // stack state carries correctly across lines.
        while next_op < ops.len() && ops[next_op].0 <= line.len() {
            apply_op(&mut stack, &ops[next_op].1);
            next_op += 1;
        }
    }
    if open_class.take().is_some() {
        out.push_str("</span>");
    }
    Some(out)
}
