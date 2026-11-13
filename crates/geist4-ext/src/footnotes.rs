//! Footnotes: GFM-style `[^label]` references and `[^label]: text`
//! definitions.
//!
//! The behavior follows comrak's implementation of the feature, which
//! is where GitHub's footnote rendering lives; cmark-gfm carries no
//! public footnotes branch. Definitions parse as ordinary containers:
//! their lines continue while indented four columns — exactly four are
//! consumed, so a six-space continuation is a two-indented paragraph
//! inside the definition — or blank. A post-processing pass then
//! numbers the references in document order, moves each referenced
//! definition to the end of the document in first-reference order,
//! restores references whose label matches no definition back to
//! literal text, and detaches definitions nobody referenced. A
//! definition whose label a later definition of the same normalized
//! label overwrote never enters the map: it stays where it was
//! written and renders without backrefs, as the reference's renderer
//! shows it.

use geist4_core::arena::{Arena, NodeId};
use geist4_core::basic_html::escape_href;
use geist4_core::extension::{
    AttrValue, BlockContinueCtx, BlockStartCtx, CloseCtx, ExtNode, Extension, InlineBracketCtx,
    KindDecl, RenderedChild, TreeCtx,
};
use geist4_core::node::Node;
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Returns the footnotes extension.
#[must_use]
pub fn footnotes() -> Arc<dyn Extension> {
    Arc::new(Footnotes {
        state: Mutex::new(RenderState::default()),
    })
}

/// The open definition's parse state: the label as written, kept for
/// the close hook to put on the node.
struct DefState {
    label: String,
}

/// One definition found in the tree.
struct DefEntry {
    node: NodeId,
    parent: NodeId,
    /// The label with case preserved, after normalization.
    preserve: String,
    /// The number of the definition's first reference, assigned in
    /// reference order; `None` when nothing referenced it.
    ix: Option<u32>,
    /// How many references point at it, in document order.
    references: u32,
}

struct Footnotes {
    /// Per-render counters, behind a lock because the trait requires
    /// `Sync` while a render is single-threaded. `begin_render`
    /// resets them.
    state: Mutex<RenderState>,
}

/// Per-render counters, shared because the extension object outlives
/// any single render. `begin_render` resets them.
#[derive(Default)]
struct RenderState {
    /// How many definitions the render has entered.
    ix: u32,
}

impl Extension for Footnotes {
    fn name(&self) -> &str {
        "footnotes"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![
            KindDecl {
                kind: "footnote_definition".to_string(),
                container: true,
                inline_text: false,
                attrs: vec!["label".to_string(), "total_references".to_string()],
            },
            KindDecl {
                kind: "footnote_reference".to_string(),
                container: false,
                inline_text: false,
                attrs: vec![
                    "label".to_string(),
                    "name".to_string(),
                    "ix".to_string(),
                    "ref_num".to_string(),
                ],
            },
        ]
    }

    fn contains_blocks(&self) -> bool {
        true
    }

    fn block_start_looks(&self, line: &[u8]) -> bool {
        scan_definition(line).is_some()
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.indented() {
            return;
        }
        let Some((marker_len, label_len)) = scan_definition(ctx.line()) else {
            return;
        };
        let label = String::from_utf8_lossy(&ctx.line()[2..2 + label_len]).into_owned();
        let state: Box<dyn Any + Send> = Box::new(DefState { label });
        ctx.open_extension_keep_rest("footnote_definition".to_string(), state);
        ctx.seek_content(marker_len);
    }

    fn try_continue(&self, ctx: &mut BlockContinueCtx<'_, '_, '_>) -> bool {
        if ctx.state::<DefState>().is_none() {
            return false;
        }
        // A pure blank line continues the definition; a line indented
        // four columns continues it with those columns consumed.
        // Anything else closes it and frees the line for other blocks.
        if ctx.pure_blank() {
            return true;
        }
        if ctx.indent() >= 4 {
            ctx.consume_cols(4);
            return true;
        }
        false
    }

    fn close(&self, state: &mut dyn Any, ctx: &mut CloseCtx<'_, '_>) {
        let Some(state) = state.downcast_ref::<DefState>() else {
            return;
        };
        payload_set(
            ctx.arena,
            ctx.node,
            "label",
            AttrValue::Str(state.label.clone()),
        );
    }

    fn inline_bracket(&self, ctx: &mut InlineBracketCtx<'_, '_, '_, '_>) {
        // The label must start with `^` and have at least one byte
        // after it. Checking the prefix first keeps bracket-heavy
        // input linear; building the content happens only for
        // footnote-shaped groups.
        if !ctx.content_starts_with(b"^") {
            return;
        }
        let Some(content) = ctx.content() else {
            return;
        };
        if content.len() < 2 {
            return;
        }
        let label = content[1..].to_string();
        ctx.claim(
            "footnote_reference".to_string(),
            vec![("label".to_string(), AttrValue::Str(label))],
        );
    }

    fn post_process(&self, ctx: &mut TreeCtx<'_, '_, '_>) {
        let arena = &mut *ctx.arena;
        let root = ctx.root;

        // The definitions, in document order. The walk does not
        // descend into definitions, matching the reference's
        // collection pass; a definition inside a definition belongs to
        // the outer one's content.
        let mut all_defs: Vec<(NodeId, NodeId)> = Vec::new();
        let mut stack: Vec<(NodeId, NodeId)> = vec![(root, root)];
        let mut order: Vec<NodeId> = Vec::new();
        while let Some((id, parent)) = stack.pop() {
            if is_definition(arena, id) {
                all_defs.push((id, parent));
                order.push(id);
                continue;
            }
            for child in arena.children(id).to_vec() {
                stack.push((child, id));
            }
        }
        // `stack.pop` walks children in reverse; recover document order.
        order.reverse();
        all_defs.reverse();

        let mut defs: HashMap<String, DefEntry> = HashMap::new();
        for (id, parent) in &all_defs {
            let label = payload_label(arena, *id);
            defs.insert(
                normalize_fold(&label),
                DefEntry {
                    node: *id,
                    parent: *parent,
                    preserve: normalize_preserve(&label),
                    ix: None,
                    references: 0,
                },
            );
        }

        // The references, in document order. The walk descends into
        // definitions too: a reference inside a definition counts.
        let mut next_ix = 0u32;
        let mut stack: Vec<(NodeId, NodeId)> = vec![(root, root)];
        let mut refs: Vec<(NodeId, NodeId)> = Vec::new();
        while let Some((id, parent)) = stack.pop() {
            if is_reference(arena, id) {
                refs.push((id, parent));
            }
            for child in arena.children(id).to_vec() {
                stack.push((child, id));
            }
        }
        refs.reverse();

        let mut restorations: Vec<(NodeId, NodeId, String)> = Vec::new();
        for (id, parent) in &refs {
            let label = payload_label(arena, *id);
            if let Some(entry) = defs.get_mut(&normalize_fold(&label)) {
                entry.references += 1;
                let ref_num = entry.references;
                let ix = *entry.ix.get_or_insert_with(|| {
                    next_ix += 1;
                    next_ix
                });
                let name = entry.preserve.clone();
                payload_set(arena, *id, "name", AttrValue::Str(name));
                payload_set(arena, *id, "ix", AttrValue::Int(i64::from(ix)));
                payload_set(arena, *id, "ref_num", AttrValue::Int(i64::from(ref_num)));
            } else {
                restorations.push((*id, *parent, label));
            }
        }
        // A reference whose label matches nothing goes back to being
        // the literal text it was written as.
        for (id, parent, label) in restorations {
            let Some(index) = arena.children(parent).iter().position(|c| *c == id) else {
                continue;
            };
            let span = arena.entry(id).span;
            let text = arena.alloc(Node::Text(format!("[^{label}]").into()), span);
            arena.remove_child(parent, index);
            arena.insert_child(parent, index, text);
        }

        // Referenced definitions move to the end of the document in
        // first-reference order; the rest detach and vanish. A
        // definition whose label a later one overwrote is in neither
        // group: it never entered the map, so it stays where it was
        // written and still renders, without backrefs.
        let mapped: Vec<NodeId> = defs.values().map(|entry| entry.node).collect();
        let mut moved: Vec<DefEntry> = defs
            .into_values()
            .filter(|entry| entry.ix.is_some())
            .collect();
        moved.sort_unstable_by_key(|entry| entry.ix);
        for entry in &moved {
            payload_set(
                arena,
                entry.node,
                "name",
                AttrValue::Str(entry.preserve.clone()),
            );
            payload_set(
                arena,
                entry.node,
                "total_references",
                AttrValue::Int(i64::from(entry.references)),
            );
            if let Some(index) = arena
                .children(entry.parent)
                .iter()
                .position(|c| *c == entry.node)
            {
                arena.remove_child(entry.parent, index);
                arena.push_child(root, entry.node);
            }
        }
        // The moved definitions are already gone from their old
        // parents; unreferenced ones vanish; orphans stay.
        let moved_ids: Vec<NodeId> = moved.iter().map(|entry| entry.node).collect();
        for (id, parent) in &all_defs {
            if moved_ids.contains(id) || !mapped.contains(id) {
                continue;
            }
            if let Some(index) = arena.children(*parent).iter().position(|c| *c == *id) {
                arena.remove_child(*parent, index);
            }
        }
    }

    fn begin_render(&self) {
        *self.render_state().lock().unwrap() = RenderState::default();
    }

    fn render(
        &self,
        node: &geist4_core::extension::ExtNode,
        children: &[RenderedChild<'_>],
        out: &mut String,
    ) {
        match node.kind.as_str() {
            "footnote_definition" => {
                // A referenced definition carries its normalized label
                // in `name`, set by the post-processing pass; an
                // orphaned one — its label overwritten by a later
                // definition — keeps the raw label it was written with.
                let name = match attr_str(node, "name") {
                    Some(name) => name,
                    None => attr_str(node, "label").unwrap_or(""),
                };
                let total_references =
                    match node.attrs.iter().find(|(key, _)| key == "total_references") {
                        Some((_, AttrValue::Int(n))) => *n,
                        _ => 0,
                    };
                let state = self.render_state();
                let mut state = state.lock().unwrap();
                if state.ix == 0 {
                    out.push_str("<section class=\"footnotes\" data-footnotes>\n<ol>\n");
                }
                state.ix += 1;
                let fnix = state.ix;
                out.push_str("<li id=\"fn-");
                out.push_str(&escape_href(name));
                out.push_str("\">");
                // A definition's first block starts on its own line,
                // exactly as any block after a mid-line tag does; with
                // no children the backrefs follow the tag directly.
                if !children.is_empty() {
                    out.push('\n');
                }
                // A definition whose last child is a paragraph carries
                // the backrefs inside that paragraph; after any other
                // tail they go bare on their own line.
                let last_para = children
                    .last()
                    .filter(|child| child.core_kind == Some("paragraph"))
                    .map(|child| child.html.as_str());
                match last_para {
                    Some(html) if html.ends_with("</p>\n") => {
                        for child in &children[..children.len() - 1] {
                            out.push_str(&child.html);
                        }
                        let body = &html[..html.len() - "</p>\n".len()];
                        out.push_str(body);
                        out.push(' ');
                        self.write_backrefs(out, name, total_references, fnix);
                        out.push_str("</p>\n");
                    }
                    _ => {
                        for child in children {
                            out.push_str(&child.html);
                        }
                        if self.write_backrefs(out, name, total_references, fnix) {
                            out.push('\n');
                        }
                    }
                }
                out.push_str("</li>\n");
            }
            "footnote_reference" => {
                let Some(name) = attr_str(node, "name") else {
                    return;
                };
                let Some(AttrValue::Int(ix)) = node
                    .attrs
                    .iter()
                    .find(|(key, _)| key == "ix")
                    .map(|(_, v)| v)
                else {
                    return;
                };
                let ref_num = match node
                    .attrs
                    .iter()
                    .find(|(key, _)| key == "ref_num")
                    .map(|(_, v)| v)
                {
                    Some(AttrValue::Int(n)) => *n,
                    _ => 1,
                };
                let mut ref_id = format!("fnref-{name}");
                if ref_num > 1 {
                    ref_id = format!("{ref_id}-{ref_num}");
                }
                out.push_str("<sup class=\"footnote-ref\"><a href=\"#fn-");
                out.push_str(&escape_href(name));
                out.push_str("\" id=\"");
                out.push_str(&escape_href(&ref_id));
                out.push_str("\" data-footnote-ref>");
                out.push_str(&ix.to_string());
                out.push_str("</a></sup>");
            }
            _ => {}
        }
    }

    fn document_suffix(&self, out: &mut String) {
        if self.render_state().lock().unwrap().ix > 0 {
            out.push_str("</ol>\n</section>\n");
        }
    }
}

impl Footnotes {
    fn render_state(&self) -> &Mutex<RenderState> {
        &self.state
    }

    /// Writes the definition's backrefs; returns whether any were
    /// written, mirroring the reference's put-footnote-backref.
    fn write_backrefs(
        &self,
        out: &mut String,
        name: &str,
        total_references: i64,
        fnix: u32,
    ) -> bool {
        // A definition in the output always has at least one
        // reference; the zero case stays unreachable but harmless.
        if total_references <= 0 {
            return true;
        }
        for ref_num in 1..=total_references {
            // The first backref is bare; each later one separates with
            // a space and carries its number as a suffix and a mark.
            let extra = if ref_num > 1 {
                out.push(' ');
                format!("-{ref_num}")
            } else {
                String::new()
            };
            out.push_str("<a href=\"#fnref-");
            out.push_str(&escape_href(name));
            out.push_str(&extra);
            out.push_str(
                "\" class=\"footnote-backref\" data-footnote-backref data-footnote-backref-idx=\"",
            );
            out.push_str(&fnix.to_string());
            out.push_str(&extra);
            out.push_str("\" aria-label=\"Back to reference ");
            out.push_str(&fnix.to_string());
            out.push_str(&extra);
            out.push_str("\">↩");
            if ref_num > 1 {
                out.push_str(&format!("<sup class=\"footnote-ref\">{ref_num}</sup>"));
            }
            out.push_str("</a>");
        }
        true
    }
}

fn is_definition(arena: &geist4_core::arena::Arena<'_>, id: NodeId) -> bool {
    matches!(arena.kind(id), Node::Extension(ext) if ext.kind == "footnote_definition")
}

fn is_reference(arena: &geist4_core::arena::Arena<'_>, id: NodeId) -> bool {
    matches!(arena.kind(id), Node::Extension(ext) if ext.kind == "footnote_reference")
}

/// The string value of one of an extension node's attributes.
/// The payload attribute `name` on an extension node, or an empty string.
fn payload_label(arena: &Arena<'_>, id: NodeId) -> String {
    if let Node::Extension(ext) = arena.kind(id) {
        if let Some((_, AttrValue::Str(label))) = ext.attrs.iter().find(|(key, _)| key == "label") {
            return label.clone();
        }
    }
    String::new()
}

/// Sets a payload attribute on an extension node, replacing a same-named one.
fn payload_set(arena: &mut Arena<'_>, id: NodeId, name: &str, value: AttrValue) {
    if let Node::Extension(ext) = arena.kind(id) {
        let kind = ext.kind.clone();
        let mut attrs = ext.attrs.clone();
        if let Some(slot) = attrs.iter_mut().find(|(key, _)| key == name) {
            slot.1 = value;
        } else {
            attrs.push((name.to_owned(), value));
        }
        arena.set_kind(id, Node::Extension(ExtNode { kind, attrs }));
    }
}

fn attr_str<'a>(node: &'a geist4_core::extension::ExtNode, name: &str) -> Option<&'a str> {
    node.attrs
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| match value {
            AttrValue::Str(s) => s.as_str(),
            _ => "",
        })
}

/// Scans a definition marker at the start of `line`, which begins at
/// the line's first non-space byte. Returns the marker's full length
/// — `[^label]:` plus trailing spaces and tabs — and the label's
/// length. The label excludes `]`, space, tab, CR, LF and 0xFF, and
/// must have at least one byte, exactly as the reference scanner's
/// character class does.
fn scan_definition(line: &[u8]) -> Option<(usize, usize)> {
    if line.first() != Some(&b'[') || line.get(1) != Some(&b'^') {
        return None;
    }
    let mut i = 2;
    while let Some(&b) = line.get(i) {
        if matches!(b, b']' | b' ' | b'\t' | b'\r' | b'\n' | 0xFF) {
            break;
        }
        i += 1;
    }
    let label_len = i - 2;
    if label_len == 0 {
        return None;
    }
    if line.get(i) != Some(&b']') || line.get(i + 1) != Some(&b':') {
        return None;
    }
    let mut end = i + 2;
    while matches!(line.get(end), Some(b' ') | Some(b'\t')) {
        end += 1;
    }
    Some((end, label_len))
}

/// Normalizes a label the way the reference does: ASCII whitespace
/// trimmed at both ends, then runs of Unicode whitespace collapsed to
/// one space, then — for the folded form used to match labels — full
/// case folding.
fn normalize_preserve(label: &str) -> String {
    let trimmed =
        label.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r'));
    let mut out = String::with_capacity(trimmed.len());
    let mut last_was_whitespace = false;
    for c in trimmed.chars() {
        if c.is_whitespace() {
            if !last_was_whitespace {
                last_was_whitespace = true;
                out.push(' ');
            }
        } else {
            last_was_whitespace = false;
            out.push(c);
        }
    }
    out
}

fn normalize_fold(label: &str) -> String {
    caseless::default_case_fold_str(&normalize_preserve(label))
}
