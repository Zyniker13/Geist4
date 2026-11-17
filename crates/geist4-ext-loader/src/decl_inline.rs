//! The declarative inline tier of the extension format (version 1):
//! `[[inline_span]]` and `[[wiki_link]]` entries, compiled onto the
//! core extension API.
//!
//! The tier is one [`Extension`]. The span entries share the delimiter
//! machinery the emphasis parser drives
//! ([`InlineTriggerCtx::scan_run`] plus [`InlineTriggerCtx::push_delim`]),
//! so pairing, flanking, the rule of three and backslash escapes come
//! from the engine; the tier decides, per run, which entry owns the
//! trigger byte and whether the run may open or close. Wiki entries
//! claim bracket groups the link parser did not match. Entries with
//! `multiline = false` unwrap to literal delimiter text in
//! post-processing when their content crossed a soft break. Rendering
//! substitutes the entry's `html` template: `{children}` for the
//! span's inline children (or the wiki's label), `{target}` for the
//! wiki's percent-encoded target, and one `{name}` slot per declared
//! attribute the node carries — absent attributes render empty.

use geist4_core::arena::{Arena, NodeId};
use geist4_core::extension::{
    AttrValue, ExtNode, Extension, InlineBracketCtx, InlineTriggerCtx, KindDecl, RenderedChild,
    Run, TreeCtx,
};
use geist4_core::node::Node;
use geist4_core::span::Span;
use geist4_core::unicode::is_space;

use crate::types::{Behavior, Manifest};

/// One compiled `[[inline_span]]` entry.
#[derive(Debug, Clone)]
struct SpanRule {
    /// The node kind the span wraps its content into.
    kind: String,
    /// The opening delimiter, one to sixteen bytes.
    open: String,
    /// The closing delimiter, one to sixteen bytes.
    close: String,
    /// The render template.
    html: String,
    /// The opening delimiter's byte length; a shorter delimiter run
    /// never opens or closes this span.
    open_len: usize,
    /// Whether an opener-flanking run may open the span.
    can_open: bool,
    /// Whether a closer-flanking run may close the span.
    can_close: bool,
    /// Whether the span may cross soft breaks.
    multiline: bool,
    /// Whether the span's delimiters follow the inline math border
    /// rules instead of the general delimiter rules.
    math: bool,
}

/// One compiled `[[wiki_link]]` entry.
#[derive(Debug, Clone)]
struct WikiRule {
    /// The node kind the link becomes.
    kind: String,
    /// The bracket prefix, `[[` by default. The group's own `[`
    /// consumes the prefix's first byte, so the group's content must
    /// start with the prefix's remainder.
    prefix: String,
    /// The bracket suffix, `]]` by default. The group's `]` is the
    /// suffix's last byte, so the group's content must end with the
    /// suffix's remainder.
    suffix: String,
    /// The render template.
    html: String,
}

/// The declarative inline extension: every `[[inline_span]]` and
/// `[[wiki_link]]` entry of a manifest, compiled into one extension.
///
/// Create it with [`InlineTier::from_manifest`] and register it on the
/// core [`geist4_core::extension::Options`] like any other extension.
#[derive(Debug, Clone)]
pub struct InlineTier {
    /// The extension's name, taken from the manifest.
    name: String,
    /// The node kinds the tier declares, in manifest declaration order.
    kinds: Vec<KindDecl>,
    /// The compiled span entries, in declaration order.
    spans: Vec<SpanRule>,
    /// The compiled wiki entries, in declaration order.
    wikis: Vec<WikiRule>,
    /// The span entries' trigger bytes, deduplicated in first-seen
    /// order.
    triggers: Vec<u8>,
}

impl InlineTier {
    /// Compiles the manifest's inline entries into one extension.
    ///
    /// Entries are matched in declaration order everywhere: the first
    /// entry sharing a trigger byte owns each run it accepts, and the
    /// first wiki entry whose brackets fit a group claims it.
    ///
    /// A span entry whose `open` is empty contributes no trigger byte
    /// and can never fire; the manifest validator rejects empty
    /// delimiters first. A span or wiki entry whose kind the manifest's
    /// `[[kinds]]` table omits still renders — the tier synthesizes an
    /// inline-text declaration for it.
    #[must_use]
    pub fn from_manifest(manifest: &Manifest) -> Self {
        let spans: Vec<SpanRule> = manifest
            .behavior
            .iter()
            .filter_map(|behavior| match behavior {
                Behavior::InlineSpan(span) => Some(SpanRule {
                    kind: span.kind.clone(),
                    open: span.open.clone(),
                    close: span.close.clone(),
                    html: span.html.clone(),
                    open_len: span.open.len(),
                    can_open: span.can_open,
                    can_close: span.can_close,
                    multiline: span.multiline,
                    math: span.math,
                }),
                _ => None,
            })
            .collect();
        let wikis: Vec<WikiRule> = manifest
            .behavior
            .iter()
            .filter_map(|behavior| match behavior {
                Behavior::WikiLink(wiki) => Some(WikiRule {
                    kind: wiki.kind.clone(),
                    prefix: wiki.prefix.clone(),
                    suffix: wiki.suffix.clone(),
                    html: wiki.html.clone(),
                }),
                _ => None,
            })
            .collect();
        let mut triggers: Vec<u8> = Vec::new();
        for span in &spans {
            if let Some(byte) = span.open.as_bytes().first()
                && !triggers.contains(byte)
            {
                triggers.push(*byte);
            }
        }
        Self {
            name: manifest.extension.name.clone(),
            kinds: declared_kinds(manifest, &spans, &wikis),
            spans,
            wikis,
            triggers,
        }
    }
}

/// The kinds the tier's entries use, in manifest declaration order.
///
/// Kinds the manifest declares keep their entry as-is (the tier only
/// declares the ones its entries actually use); kinds the `[[kinds]]`
/// table omits get a synthesized inline-text declaration so their span
/// children render inline.
fn declared_kinds(manifest: &Manifest, spans: &[SpanRule], wikis: &[WikiRule]) -> Vec<KindDecl> {
    let used = |kind: &str| {
        spans.iter().any(|span| span.kind == kind) || wikis.iter().any(|wiki| wiki.kind == kind)
    };
    let mut kinds: Vec<KindDecl> = Vec::new();
    for entry in &manifest.kinds {
        if !used(&entry.kind) || kinds.iter().any(|decl| decl.kind == entry.kind) {
            continue;
        }
        kinds.push(KindDecl {
            kind: entry.kind.clone(),
            container: entry.container,
            inline_text: entry.inline_text,
            attrs: entry.attrs.clone(),
        });
    }
    for span in spans {
        if !kinds.iter().any(|decl| decl.kind == span.kind) {
            kinds.push(KindDecl {
                kind: span.kind.clone(),
                container: false,
                inline_text: true,
                attrs: Vec::new(),
            });
        }
    }
    for wiki in wikis {
        if !kinds.iter().any(|decl| decl.kind == wiki.kind) {
            kinds.push(KindDecl {
                kind: wiki.kind.clone(),
                container: false,
                inline_text: true,
                attrs: Vec::new(),
            });
        }
    }
    kinds
}

impl Extension for InlineTier {
    fn name(&self) -> &str {
        &self.name
    }

    fn kinds(&self) -> Vec<KindDecl> {
        self.kinds.clone()
    }

    fn inline_triggers(&self) -> &[u8] {
        &self.triggers
    }

    fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
        let byte = ctx.byte();
        let mut run: Run = ctx.scan_run();
        for span in &self.spans {
            if span.open.as_bytes().first() != Some(&byte) {
                continue;
            }
            // A run shorter than the delimiter is not an occurrence of
            // it: a two-byte delimiter needs at least two of its byte,
            // so `=x=` stays literal under an `==` rule.
            if run.length < span.open_len {
                continue;
            }
            let mut can_open = run.can_open && span.can_open;
            let mut can_close = run.can_close && span.can_close;
            if span.math {
                if can_open {
                    can_open = math_open_allowed(ctx, run.length);
                }
                if can_close {
                    can_close = math_close_allowed(ctx, run.length);
                }
            }
            if can_open || can_close {
                // The stack cell carries the refined flags, not the raw
                // flanking verdicts: the math guards narrow them, and a
                // narrower run must not match as an opener or closer the
                // entry declined.
                run.can_open = can_open;
                run.can_close = can_close;
                ctx.push_delim(&run, &span.kind);
            }
            // The first entry in declaration order that accepts the
            // run's shape owns it; entries later in the list sharing
            // the byte only see the shapes this one rejected outright.
            break;
        }
    }

    fn inline_bracket(&self, ctx: &mut InlineBracketCtx<'_, '_, '_, '_>) {
        for wiki in &self.wikis {
            if wiki.prefix.is_empty() {
                continue;
            }
            let head = &wiki.prefix[1..];
            // The group's own `[` consumed the prefix's first byte, so
            // the content must start with the prefix's remainder.
            if !ctx.content_starts_with(head.as_bytes()) {
                continue;
            }
            let Some(content) = ctx.content() else {
                continue;
            };
            // The group's `]` is the suffix's last byte, so the content
            // must end with the suffix's remainder.
            let tail = &wiki.suffix[..wiki.suffix.len().saturating_sub(1)];
            let Some(inner) = content
                .strip_prefix(head)
                .and_then(|rest| rest.strip_suffix(tail))
            else {
                continue;
            };
            // The first `|` splits the target from the rendered
            // children; without one the target is also the label.
            let (target, label) = match inner.split_once('|') {
                Some((target, label)) => (target.to_owned(), label.to_owned()),
                None => (inner.to_owned(), inner.to_owned()),
            };
            // The content borrow ends here; the claim drops every node
            // the group holds, including any delimiters pushed inside
            // it, so plain text spans and wiki links inside brackets
            // stay plain text.
            ctx.claim(
                wiki.kind.clone(),
                vec![
                    ("target".to_owned(), AttrValue::Str(target)),
                    ("children".to_owned(), AttrValue::Str(label)),
                ],
            );
            // The first matching entry claims the group.
            break;
        }
    }

    fn post_process(&self, ctx: &mut TreeCtx<'_, '_, '_>) {
        // Only entries with `multiline = false` need the pass.
        if self.spans.iter().all(|span| span.multiline) {
            return;
        }
        let mut violations: Vec<(NodeId, String, String)> = Vec::new();
        collect_violations(ctx.arena, ctx.root, &self.spans, &mut violations);
        for (id, open, close) in violations {
            unwrap_span(ctx.arena, id, &open, &close);
        }
    }

    fn render(&self, node: &ExtNode, children: &[RenderedChild<'_>], out: &mut String) {
        let declared = self.kinds.iter().find(|decl| decl.kind == node.kind);
        if let Some(span) = self.spans.iter().find(|span| span.kind == node.kind) {
            substitute(
                &span.html,
                |name| {
                    if name == "children" {
                        return Some(
                            children
                                .iter()
                                .map(|child| child.html.as_str())
                                .collect::<String>(),
                        );
                    }
                    attr_substitution(declared, node, name)
                },
                out,
            );
        } else if let Some(wiki) = self.wikis.iter().find(|wiki| wiki.kind == node.kind) {
            substitute(
                &wiki.html,
                |name| {
                    if name == "target" {
                        return Some(
                            attr_value(node, "target")
                                .map(|value| encode_target(&value))
                                .unwrap_or_default(),
                        );
                    }
                    if name == "children" {
                        // The claimed node has no inline children; the label
                        // the group carried rides in the `children`
                        // attribute, escaped like inline text.
                        return Some(match attr_value(node, "children") {
                            Some(value) => escape_html(&value),
                            None => children
                                .iter()
                                .map(|child| child.html.as_str())
                                .collect::<String>(),
                        });
                    }
                    attr_substitution(declared, node, name)
                },
                out,
            );
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
        return decoded_char(bytes).is_some_and(is_space);
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

/// Collects the multiline-off span nodes whose content crossed a soft
/// break, paired with the delimiter texts to restore, walking the tree
/// iteratively so deep trees cannot overflow the stack.
fn collect_violations(
    arena: &Arena<'_>,
    root: NodeId,
    spans: &[SpanRule],
    out: &mut Vec<(NodeId, String, String)>,
) {
    let mut work: Vec<NodeId> = vec![root];
    while let Some(id) = work.pop() {
        for child in arena.children(id) {
            if let Node::Extension(ext) = arena.kind(*child) {
                if let Some(span) = spans
                    .iter()
                    .find(|span| !span.multiline && span.kind == ext.kind)
                    && subtree_has_softbreak(arena, *child)
                {
                    out.push((*child, span.open.clone(), span.close.clone()));
                }
            }
            work.push(*child);
        }
    }
}

/// Whether the inline subtree under `id` holds a soft break.
fn subtree_has_softbreak(arena: &Arena<'_>, id: NodeId) -> bool {
    let mut work: Vec<NodeId> = arena.children(id).to_vec();
    while let Some(current) = work.pop() {
        match arena.kind(current) {
            Node::SoftBreak => return true,
            Node::Text(_) | Node::CodeSpan(_) | Node::InlineHtml(_) | Node::HardBreak => {}
            _ => work.extend(arena.children(current).iter().copied()),
        }
    }
    false
}

/// Replaces one span node with its delimiters as literal text around
/// the node's children: the opener and closer the machinery consumed
/// reappear as text, the span's inline content stays.
fn unwrap_span(arena: &mut Arena<'_>, id: NodeId, open: &str, close: &str) {
    let Some(parent) = arena.parent(id) else {
        return;
    };
    let Some(index) = arena.children(parent).iter().position(|child| *child == id) else {
        return;
    };
    let children: Vec<NodeId> = arena.children(id).to_vec();
    arena.remove_child(parent, index);
    let open_id = arena.alloc(Node::Text(open.to_owned().into()), Span::empty(0));
    arena.insert_child(parent, index, open_id);
    for (offset, child) in children.iter().enumerate() {
        arena.insert_child(parent, index + 1 + offset, *child);
    }
    let close_id = arena.alloc(Node::Text(close.to_owned().into()), Span::empty(0));
    arena.insert_child(parent, index + 1 + children.len(), close_id);
}

/// The substitution for one declared attribute's `{name}` slot: the
/// node's value, escaped for HTML. A declared attribute the node does
/// not carry renders empty; an undeclared name is not a substitution
/// and stays literal.
fn attr_substitution(declared: Option<&KindDecl>, node: &ExtNode, name: &str) -> Option<String> {
    let declared = declared.is_some_and(|decl| decl.attrs.iter().any(|attr| attr == name));
    if !declared {
        return None;
    }
    Some(
        attr_value(node, name)
            .map(|value| escape_html(&value))
            .unwrap_or_default(),
    )
}

/// The node's value for `name`, as display text.
fn attr_value(node: &ExtNode, name: &str) -> Option<String> {
    node.attrs
        .iter()
        .find(|(attr, _)| attr == name)
        .map(|(_, value)| match value {
            AttrValue::Str(text) => text.clone(),
            AttrValue::Int(number) => number.to_string(),
            AttrValue::Bool(flag) => flag.to_string(),
        })
}

/// The wiki target for an `href`: the core's href escaping, with `#`
/// and `?` encoded past it so a page name stays one page name.
fn encode_target(target: &str) -> String {
    geist4_core::basic_html::escape_href(target)
        .replace('#', "%23")
        .replace('?', "%3F")
}

/// Escapes text for HTML output the way the core's renderer does:
/// `&`, `<`, `>` and `"` become entities; `'` and `/` stay raw.
fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(character),
        }
    }
    out
}

/// Writes `template` to `out`, replacing each `{name}` the resolver
/// knows with its value and emitting every other `{...}` span
/// literally, as the format requires.
fn substitute(template: &str, resolve: impl Fn(&str) -> Option<String>, out: &mut String) {
    let mut rest = 0usize;
    while rest < template.len() {
        let Some(at) = template[rest..].find('{') else {
            out.push_str(&template[rest..]);
            break;
        };
        let open = rest + at;
        out.push_str(&template[rest..open]);
        let Some(relative) = template[open + 1..].find('}') else {
            out.push_str(&template[open..]);
            break;
        };
        let close = open + 1 + relative;
        match resolve(&template[open + 1..close]) {
            Some(value) => out.push_str(&value),
            None => out.push_str(&template[open..=close]),
        }
        rest = close + 1;
    }
}
