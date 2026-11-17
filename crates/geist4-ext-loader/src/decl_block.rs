//! The declarative block tier of the extension format (version 1):
//! the `[[block_container]]`, `[[replace]]` and `[[attribute_list]]`
//! entries, compiled onto the core extension API.
//!
//! The tier is two objects. [`BlockTier`] carries the block hooks —
//! one compiled entry per `[[block_container]]`, matched in
//! declaration order — and renders container nodes through the
//! entries' `html` templates. [`PostTier`] carries the post-process
//! pass: `[[replace]]` entries rewrite text nodes in declaration
//! order, and `[[attribute_list]]` entries attach parsed attribute
//! lists to the preceding block or to the preceding inline span.
//! The host composes both, plus the inline tier of `crate::decl_inline`,
//! into one [`geist4_core::extension::Extension`].
//!
//! The semantics follow `docs/extensions/format.md` exactly; the two
//! places where the format leaves the engine's shape open (the
//! attribute list's paragraph-final form, and the info string's
//! rendering) are documented on the implementing functions.

use std::borrow::Cow;

use geist4_core::arena::{Arena, NodeId};
use geist4_core::extension::{
    AttrValue, BlockContinueCtx, BlockStartCtx, CloseCtx, ExtNode, LookCtx, RenderedChild, TreeCtx,
};
use geist4_core::node::Node;
use geist4_core::parse_document;

use crate::types::{AttributeList, Behavior, Manifest, Replace};

/// One compiled `[[block_container]]` entry.
#[derive(Debug, Clone)]
struct ContainerRule {
    /// The kind of the container node.
    kind: String,
    /// The marker run, one character class repeated.
    marker: String,
    /// The word after the marker that opens the container.
    name: String,
    /// The render template, with `{children}` and `{info}`.
    html: String,
    /// The template for the info line's content; empty ignores the info.
    info_html: String,
    /// Whether EOF closes an unclosed container.
    unclosed_ok: bool,
}

/// The state of one open container, kept in its parse frame.
#[derive(Debug)]
struct ContainerState {
    /// The index of the entry this frame belongs to.
    rule: usize,
    /// The info string, trimmed; empty when the fence line had none.
    info: String,
    /// Whether the close hook runs because a close fence matched, as
    /// opposed to the document ending.
    closed_by_fence: bool,
    /// How many same-pattern containers opened inside this one and may
    /// not have closed yet; a close fence passes through to the inner
    /// container while this is above zero.
    inner: usize,
}

/// The declarative block tier: every `[[block_container]]` entry of a
/// manifest, compiled into one set of block hooks.
///
/// Create it with [`BlockTier::from_manifest`]; the host's compiler
/// forwards the hook methods of its [`geist4_core::extension::Extension`]
/// implementation to the tier. The methods carry the same signatures
/// as the trait's hooks.
#[derive(Debug, Clone)]
pub struct BlockTier {
    /// The compiled container entries, in declaration order.
    containers: Vec<ContainerRule>,
}

impl BlockTier {
    /// Compiles the manifest's `[[block_container]]` entries into one
    /// tier.
    ///
    /// A line is offered to the entries in declaration order, and the
    /// first entry whose marker run and name word fit opens the
    /// container. The `kind` each entry names is taken from the
    /// manifest as given; the manifest validator requires it to be a
    /// declared container kind.
    #[must_use]
    pub fn from_manifest(manifest: &Manifest) -> Self {
        let containers: Vec<ContainerRule> = manifest
            .behavior
            .iter()
            .filter_map(|behavior| match behavior {
                Behavior::BlockContainer(entry) => Some(ContainerRule {
                    kind: entry.kind.clone(),
                    marker: entry.marker.clone(),
                    name: entry.name.clone(),
                    html: entry.html.clone(),
                    info_html: entry.info_html.clone(),
                    unclosed_ok: entry.unclosed_ok,
                }),
                _ => None,
            })
            .collect();
        Self { containers }
    }

    /// Whether the tier's containers hold blocks: while one is open,
    /// ordinary block parsing continues inside it.
    #[must_use]
    pub fn contains_blocks(&self) -> bool {
        !self.containers.is_empty()
    }

    /// The block-start look: whether a line would open one of the
    /// tier's containers.
    ///
    /// Only an opening fence looks: a close fence must not answer yes,
    /// because a lazy paragraph continuation is joined by exactly the
    /// lines no block start would claim — a close fence under a failed
    /// container prefix joins the paragraph instead of closing, the
    /// way a fenced code fence does inside a block quote.
    #[must_use]
    pub fn block_start_looks(&self, ctx: &LookCtx<'_, '_>) -> bool {
        if ctx.indented {
            return false;
        }
        self.containers
            .iter()
            .any(|rule| scan_open(rule, ctx.line).is_some())
    }

    /// The block-start hook: opens a container on an opening fence
    /// line.
    ///
    /// The fence line is consumed whole; the text after the name word
    /// is the container's info string, kept in the frame's state for
    /// the close hook to render.
    pub fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.indented() {
            return;
        }
        let line = ctx.line();
        for (index, rule) in self.containers.iter().enumerate() {
            if let Some(info_at) = scan_open(rule, line) {
                let info = String::from_utf8_lossy(&line[info_at..])
                    .trim_matches(|c: char| c == ' ' || c == '\t')
                    .to_owned();
                let _ = ctx.open_extension(
                    rule.kind.clone(),
                    Box::new(ContainerState {
                        rule: index,
                        info,
                        closed_by_fence: false,
                        inner: 0,
                    }),
                );
                return;
            }
        }
    }

    /// The block-continuation hook: decides whether the line stays
    /// inside the open container.
    ///
    /// A close fence — the exact marker run and nothing after it but
    /// spaces, indented at most three columns — ends the container: the
    /// line is consumed and the hook returns false. A shorter or longer
    /// run does not close. A line that opens a nested container is
    /// counted so the matching close fence passes through to it. Every
    /// other line, including blank lines, continues the container and
    /// is left for the parser to open blocks with inside it.
    pub fn try_continue(&self, ctx: &mut BlockContinueCtx<'_, '_, '_>) -> bool {
        let index = match ctx.state::<ContainerState>() {
            Some(state) => state.rule,
            None => return false,
        };
        let rule = &self.containers[index];
        // A blank line continues the container; the parser's blank-line
        // handling takes the line from here.
        if ctx.line().is_empty() || ctx.pure_blank() {
            return true;
        }
        // Four or more columns of indent is content, never a fence.
        if ctx.indent() > 3 {
            return true;
        }
        let line = ctx.line();
        if is_close_fence(rule.marker.as_bytes(), line) {
            if let Some(state) = ctx.state::<ContainerState>() {
                if state.inner > 0 {
                    // The fence belongs to a container nested inside
                    // this one: it passes through and this one stays
                    // open.
                    state.inner -= 1;
                    return true;
                }
                state.closed_by_fence = true;
            }
            // Consume the whole line: the close fence is consumed, and
            // the consumed cursor also keeps the line out of the lazy
            // continuation of a paragraph open inside the container.
            ctx.consume_cols(usize::MAX);
            return false;
        }
        // A nested opening fence passes through as an ordinary line
        // here and is counted, so its own close fence closes the inner
        // container, not this one. Indented past three columns the
        // line cannot open a container (try_start declines it), so it
        // is not counted either. Markers whose first byte belongs to a
        // built-in block start (`>` quotes, backtick and tilde fences,
        // `<` HTML, `#` headings) are not counted: the built-in wins
        // the line in the block-start phase, so no container will open
        // to close later.
        if ctx.indent() <= 3
            && scan_open(rule, line).is_some()
            && !matches!(rule.marker.as_bytes()[0], b'>' | b'`' | b'~' | b'<' | b'#')
            && let Some(state) = ctx.state::<ContainerState>()
        {
            state.inner += 1;
        }
        true
    }

    /// The block-close hook: renders the info string and records an
    /// unclosed container the format calls an error.
    ///
    /// The info string is inline-parsed as a one-off document and
    /// rendered through the entry's `info_html` template; the result is
    /// stored as the node's `info` attribute for the render hook. With
    /// `unclosed_ok = false`, a container the document end closed
    /// carries a `parse_problem` attribute; the node's span already
    /// starts at the open fence line, which is the span the format
    /// asks the error to carry.
    pub fn close(&self, state: &mut dyn std::any::Any, ctx: &mut CloseCtx<'_, '_>) {
        let Some(state) = state.downcast_mut::<ContainerState>() else {
            return;
        };
        let rule = &self.containers[state.rule];
        if !state.info.is_empty() && !rule.info_html.is_empty() {
            let rendered = render_info(&state.info, &rule.info_html);
            if !rendered.is_empty() {
                set_ext_attr(ctx.arena, ctx.node, "info", rendered);
            }
        }
        if !state.closed_by_fence && !rule.unclosed_ok {
            set_ext_attr(
                ctx.arena,
                ctx.node,
                "parse_problem",
                "unclosed block container".to_owned(),
            );
        }
    }

    /// Renders one of the tier's container nodes: the entry's `html`
    /// template with `{children}` replaced by the children's block HTML
    /// and `{info}` by the rendered info stored at close — empty when
    /// the entry ignores the info or the container had none.
    pub fn render(&self, node: &ExtNode, children: &[RenderedChild<'_>], out: &mut String) {
        let Some(rule) = self.containers.iter().find(|rule| rule.kind == node.kind) else {
            return;
        };
        let children_html: String = children.iter().map(|child| child.html.as_str()).collect();
        let info = ext_attr_str(node, "info").unwrap_or_default();
        out.push_str(&substitute(&rule.html, &|name| match name {
            "children" => Some(children_html.clone()),
            "info" => Some(info.clone()),
            _ => None,
        }));
    }
}

/// Scans `line` — the bytes from the line's first non-space byte — for
/// an opening fence of `rule`: the exact marker run, optional spaces
/// and tabs, the name word, and a word boundary after it.
///
/// Returns the byte offset where the info string starts, just past the
/// name and any following spaces. A longer marker run does not open;
/// a byte other than a space right after the name means the word is a
/// prefix of something else and the line stays ordinary text.
fn scan_open(rule: &ContainerRule, line: &[u8]) -> Option<usize> {
    let marker = rule.marker.as_bytes();
    if !line.starts_with(marker) || line.get(marker.len()) == Some(&marker[0]) {
        return None;
    }
    let mut at = marker.len();
    while matches!(line.get(at), Some(b' ' | b'\t')) {
        at += 1;
    }
    let name = rule.name.as_bytes();
    if !line[at..].starts_with(name) {
        return None;
    }
    at += name.len();
    if let Some(byte) = line.get(at) {
        if !matches!(byte, b' ' | b'\t') {
            return None;
        }
    }
    while matches!(line.get(at), Some(b' ' | b'\t')) {
        at += 1;
    }
    Some(at)
}

/// Whether `line` — from the line's first non-space byte — is a close
/// fence for `marker`: the exact marker run with nothing after it but
/// spaces and tabs. A shorter or longer run does not close.
fn is_close_fence(marker: &[u8], line: &[u8]) -> bool {
    if !line.starts_with(marker) || line.get(marker.len()) == Some(&marker[0]) {
        return false;
    }
    line[marker.len()..]
        .iter()
        .all(|byte| matches!(byte, b' ' | b'\t'))
}

/// Renders the info string through `template`: the info is parsed as a
/// one-off document — one line, so its content is inline — and the
/// rendered paragraph HTML is unwrapped to the inline form the info
/// line carries. An info shaped like a block start keeps its block
/// rendering; the format leaves that one shape open.
fn render_info(info: &str, template: &str) -> String {
    let doc = parse_document(info);
    let rendered = geist4_core::basic_html::render(&doc);
    let trimmed = rendered.trim_end_matches('\n');
    let inline = match trimmed
        .strip_prefix("<p>")
        .and_then(|rest| rest.strip_suffix("</p>"))
    {
        Some(inner) => inner.to_owned(),
        None => trimmed.to_owned(),
    };
    substitute(template, &|name| {
        if name == "children" {
            Some(inline.clone())
        } else {
            None
        }
    })
}

/// The declarative post-process tier: the `[[replace]]` and
/// `[[attribute_list]]` entries of a manifest, compiled into one
/// post-process pass.
///
/// Create it with [`PostTier::from_manifest`]; the host's compiler
/// forwards [`geist4_core::extension::Extension::post_process`] to
/// [`PostTier::post_process`].
#[derive(Debug, Clone)]
pub struct PostTier {
    /// The compiled replacement entries, in declaration order.
    replaces: Vec<Replace>,
    /// The compiled attribute-list entries, in declaration order.
    attr_lists: Vec<AttributeList>,
    /// The inline span kinds an inline attribute list may attach to,
    /// from the manifest's `[[inline_span]]` entries.
    span_kinds: Vec<String>,
}

impl PostTier {
    /// Compiles the manifest's `[[replace]]` and `[[attribute_list]]`
    /// entries into one post-process tier.
    ///
    /// Replacement entries keep their declaration order; attribute-list
    /// entries are tried in declaration order, the first whose prefix
    /// and suffix fit the text claiming the list. The inline span kinds
    /// come from the manifest's `[[inline_span]]` entries, whose spans
    /// an inline list attaches to.
    #[must_use]
    pub fn from_manifest(manifest: &Manifest) -> Self {
        let mut replaces = Vec::new();
        let mut attr_lists = Vec::new();
        let mut span_kinds = Vec::new();
        for behavior in &manifest.behavior {
            match behavior {
                Behavior::Replace(entry) => replaces.push(entry.clone()),
                Behavior::AttributeList(entry) => attr_lists.push(entry.clone()),
                Behavior::InlineSpan(entry) => span_kinds.push(entry.kind.clone()),
                _ => {}
            }
        }
        Self {
            replaces,
            attr_lists,
            span_kinds,
        }
    }

    /// The post-process pass.
    ///
    /// The passes run in the format's section order: replacements
    /// first, then attribute lists. Replacement never reaches code or
    /// raw HTML because those live in node kinds of their own — the
    /// pass touches `Node::Text` nodes only, and no such node exists
    /// inside a code span, a code block or raw HTML. Attribute lists
    /// attach block by block and then span by span.
    pub fn post_process(&self, ctx: &mut TreeCtx<'_, '_, '_, '_>) {
        if !self.replaces.is_empty() {
            apply_replaces(ctx.arena, ctx.root, &self.replaces);
        }
        if !self.attr_lists.is_empty() {
            for paragraph in collect_paragraphs(ctx.arena, ctx.root) {
                attach_block_attr_list(ctx.arena, paragraph, &self.attr_lists);
            }
            let mut spans = Vec::new();
            for id in walk_all(ctx.arena, ctx.root) {
                if let Node::Extension(ext) = ctx.arena.kind(id)
                    && self.span_kinds.contains(&ext.kind)
                {
                    spans.push(id);
                }
            }
            for span in spans {
                attach_inline_attr_list(ctx.arena, span, &self.attr_lists);
            }
        }
    }
}

/// One parsed item of an attribute list.
#[derive(Debug, Clone, PartialEq)]
enum AttrItem {
    /// `#name` sets the `id`.
    Id(String),
    /// `.name` appends to the `class`.
    Class(String),
    /// `key=value` sets `key` to the value, entity-decoded.
    Pair(String, String),
    /// A bare `key` sets `key` to the empty string.
    Flag(String),
}

/// Applies the replacement entries to every text node under `root`,
/// in order, each on the result of the previous.
fn apply_replaces(arena: &mut Arena<'_>, root: NodeId, rules: &[Replace]) {
    for id in walk_all(arena, root) {
        let Node::Text(cow) = arena.kind(id) else {
            continue;
        };
        let mut text = cow.to_string();
        let original = text.clone();
        for rule in rules {
            if rule.from.is_empty() {
                continue;
            }
            if rule.once {
                if let Some(at) = text.find(&rule.from) {
                    text.replace_range(at..at + rule.from.len(), &rule.to);
                }
            } else {
                text = text.replace(&rule.from, &rule.to);
            }
        }
        if text != original {
            arena.set_kind(id, Node::Text(Cow::Owned(text)));
        }
    }
}

/// The ids of every node under `root`, in pre-order.
fn walk_all(arena: &Arena<'_>, root: NodeId) -> Vec<NodeId> {
    let mut order = Vec::new();
    let mut work = vec![root];
    while let Some(id) = work.pop() {
        order.push(id);
        let children: Vec<NodeId> = arena.children(id).to_vec();
        work.extend(children.into_iter().rev());
    }
    order
}

/// Collects every paragraph node under `root`.
fn collect_paragraphs(arena: &Arena<'_>, root: NodeId) -> Vec<NodeId> {
    walk_all(arena, root)
        .into_iter()
        .filter(|id| matches!(arena.kind(*id), Node::Paragraph))
        .collect()
}

/// Attaches an attribute list to `paragraph` when its final line is
/// exactly one list, following the format's paragraph-final rule.
///
/// The line's content is read off the paragraph's trailing text nodes
/// — the inline parser splits a line's text into several nodes (at an
/// entity's `&`, for one), so one node is not one line — walking back
/// until the last soft break. A paragraph whose trailing inline
/// content is not plain text cannot carry a list this way. A list that
/// parses is consumed and merged: when the paragraph holds nothing but
/// the list line, the attributes go to the paragraph's preceding
/// sibling block and the spent paragraph is removed; when the
/// paragraph has content before the list line, the attributes go to
/// the paragraph itself and the list line is dropped — the list
/// terminates the block it sits inside, which is the paragraph here.
/// A list with no preceding sibling to attach to stays literal text.
fn attach_block_attr_list(arena: &mut Arena<'_>, paragraph: NodeId, rules: &[AttributeList]) {
    let Some(parent) = arena.parent(paragraph) else {
        return;
    };
    let Some(index) = arena
        .children(parent)
        .iter()
        .position(|child| *child == paragraph)
    else {
        return;
    };
    // The paragraph's final line: the trailing text nodes, back to the
    // last soft break.
    let children = arena.children(paragraph).to_vec();
    let mut parts: Vec<String> = Vec::new();
    let mut trailing = 0usize;
    let mut softbreak = false;
    for child in children.iter().rev() {
        match arena.kind(*child) {
            Node::Text(cow) => {
                parts.push(cow.to_string());
                trailing += 1;
            }
            Node::SoftBreak => {
                softbreak = true;
                break;
            }
            _ => return,
        }
    }
    let line = parts.into_iter().rev().collect::<String>();
    let Some(items) = parse_whole_list(rules, &line) else {
        return;
    };
    if !softbreak {
        // The paragraph is exactly the list line: the list attaches to
        // the preceding sibling block and the spent paragraph goes.
        if index == 0 {
            return;
        }
        let target = arena.children(parent)[index - 1];
        merge_attributes(arena, target, &items);
        arena.remove_child(parent, index);
        return;
    }
    // The list ends a paragraph with content: it attaches to the
    // paragraph itself and the spent line goes.
    merge_attributes(arena, paragraph, &items);
    for _ in 0..trailing {
        let tail = arena.children(paragraph).len() - 1;
        arena.remove_child(paragraph, tail);
    }
    let tail = arena.children(paragraph).len() - 1;
    arena.remove_child(paragraph, tail);
}

/// Attaches an attribute list to the inline span `span` when text
/// follows it with the list at its start: `span{.class}` with the list
/// immediately after the span's closing delimiter.
///
/// The following siblings are joined for the parse — the inline pass
/// may split one text run into several nodes (`{color=red}` splits at
/// `=`) — and the consumed prefix is written back over the nodes it
/// covered. A space or the line's end must follow the list, so
/// `{.class}` only attaches where the list ends the token. The
/// attributes merge into the span node's own attribute list, the
/// channel the declared kind names for extension targets.
fn attach_inline_attr_list(arena: &mut Arena<'_>, span: NodeId, rules: &[AttributeList]) {
    let Some(parent) = arena.parent(span) else {
        return;
    };
    let Some(index) = arena
        .children(parent)
        .iter()
        .position(|child| *child == span)
    else {
        return;
    };
    // The run of text nodes after the span; the parse runs over their
    // concatenation.
    let run: Vec<NodeId> = arena
        .children(parent)
        .iter()
        .skip(index + 1)
        .copied()
        .take_while(|child| matches!(arena.kind(*child), Node::Text(_)))
        .collect();
    if run.is_empty() {
        return;
    }
    let joined: String = run
        .iter()
        .map(|child| match arena.kind(*child) {
            Node::Text(cow) => cow.to_string(),
            _ => String::new(),
        })
        .collect();
    let Some((items, rest)) = parse_prefixed_list(rules, &joined) else {
        return;
    };
    if !rest.is_empty() && !rest.starts_with([' ', '\t']) {
        return;
    }
    merge_attributes(arena, span, &items);
    let consumed = joined.len() - rest.len();
    // Rewrite the run: drop the nodes fully under the consumed prefix,
    // trim the one holding the boundary, keep the tail beyond it.
    let mut taken = 0;
    let mut rewritten: Vec<(NodeId, Option<String>)> = Vec::new();
    for child in &run {
        let Node::Text(cow) = arena.kind(*child) else {
            unreachable!("selected as text above");
        };
        let text = cow.to_string();
        if taken + text.len() <= consumed {
            taken += text.len();
            rewritten.push((*child, None));
            continue;
        }
        let keep = &text[consumed - taken..];
        if keep.is_empty() {
            rewritten.push((*child, None));
        } else {
            rewritten.push((*child, Some(keep.to_owned())));
        }
        break;
    }
    for (child, replacement) in rewritten {
        // Positions are taken fresh from the current child list, which
        // already reflects earlier removals.
        let at = arena
            .children(parent)
            .iter()
            .position(|candidate| *candidate == child)
            .unwrap_or(0);
        match replacement {
            Some(text) => arena.set_kind(child, Node::Text(Cow::Owned(text))),
            None => {
                arena.remove_child(parent, at);
            }
        }
    }
}

/// Parses `text` as one whole attribute list under the first fitting
/// rule: the list must cover the whole text, as a standalone line's
/// content does.
fn parse_whole_list(rules: &[AttributeList], text: &str) -> Option<Vec<AttrItem>> {
    for rule in rules {
        if let Some((items, rest)) = parse_one_list(rule, text)
            && rest.is_empty()
        {
            return Some(items);
        }
    }
    None
}

/// Parses `text` as a leading attribute list under the first fitting
/// rule, returning the items and the text after the list's suffix.
fn parse_prefixed_list<'t>(
    rules: &[AttributeList],
    text: &'t str,
) -> Option<(Vec<AttrItem>, &'t str)> {
    for rule in rules {
        if let Some((items, rest)) = parse_one_list(rule, text) {
            return Some((items, rest));
        }
    }
    None
}

/// Parses `text` as one list under `rule`, returning the items and the
/// text after the list's suffix.
fn parse_one_list<'t>(rule: &AttributeList, text: &'t str) -> Option<(Vec<AttrItem>, &'t str)> {
    let mut rest = text.strip_prefix(&rule.prefix)?;
    let mut items = Vec::new();
    loop {
        rest = rest.trim_start_matches([' ', '\t']);
        if let Some(after) = rest.strip_prefix(&rule.suffix) {
            return Some((items, after));
        }
        if items.len() >= 32 {
            // Over the entry cap the list is not a list.
            return None;
        }
        let (item, after) = parse_one_item(rest, &rule.suffix)?;
        items.push(item);
        rest = after;
    }
}

/// Parses one attribute-list item at the start of `rest`, returning it
/// and the text after it.
fn parse_one_item<'t>(rest: &'t str, suffix: &str) -> Option<(AttrItem, &'t str)> {
    let bytes = rest.as_bytes();
    match bytes.first()? {
        b'#' | b'.' => {
            let name_len = ident_len(&bytes[1..]);
            if name_len == 0 {
                return None;
            }
            let name = rest[1..1 + name_len].to_owned();
            let item = if bytes[0] == b'#' {
                AttrItem::Id(name)
            } else {
                AttrItem::Class(name)
            };
            Some((item, &rest[1 + name_len..]))
        }
        _ => {
            let key_len = ident_len(bytes);
            if key_len == 0 {
                return None;
            }
            let key = rest[..key_len].to_owned();
            let rest = &rest[key_len..];
            match rest.strip_prefix('=') {
                Some(after) => {
                    let (value, rest) = parse_value(after, suffix)?;
                    Some((AttrItem::Pair(key, decode_entities(&value)), rest))
                }
                None => Some((AttrItem::Flag(key), rest)),
            }
        }
    }
}

/// Parses a `key=`'s value: double-quoted with no escapes, or unquoted
/// up to the next space, tab or the list's suffix.
fn parse_value<'t>(rest: &'t str, suffix: &str) -> Option<(String, &'t str)> {
    if let Some(quoted) = rest.strip_prefix('"') {
        let end = quoted.find('"')?;
        return Some((quoted[..end].to_owned(), &quoted[end + 1..]));
    }
    let mut end = rest.len();
    for (at, character) in rest.char_indices() {
        if matches!(character, ' ' | '\t') || rest[at..].starts_with(suffix) {
            end = at;
            break;
        }
    }
    Some((rest[..end].to_owned(), &rest[end..]))
}

/// The length of the identifier or key at the start of `bytes`:
/// `[A-Za-z:._-]+` as the format's grammar spells it.
fn ident_len(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .take_while(|byte| {
            matches!(
                **byte,
                b'A'..=b'Z' | b'a'..=b'z' | b':' | b'.' | b'_' | b'-'
            )
        })
        .count()
}

/// Decodes the character references an attribute value may carry: the
/// four named forms the renderer escapes and the numeric forms.
/// Anything else stays as written.
fn decode_entities(value: &str) -> String {
    if !value.contains('&') {
        return value.to_owned();
    }
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let decoded = decode_one_entity(rest);
        match decoded {
            Decoded::Named(text, len) => {
                out.push_str(text);
                rest = &rest[len..];
            }
            Decoded::Character(character, len) => {
                out.push(character);
                rest = &rest[len..];
            }
            Decoded::None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// One entity-decoding outcome at the start of `rest`.
enum Decoded {
    /// A named form's replacement text and its source length.
    Named(&'static str, usize),
    /// A numeric form's character and its source length.
    Character(char, usize),
    /// Not an entity; the `&` stays literal.
    None,
}

/// Decodes one entity at the start of `rest`.
fn decode_one_entity(rest: &str) -> Decoded {
    for (text, literal) in [
        ("&", "&amp;"),
        ("<", "&lt;"),
        (">", "&gt;"),
        ("\"", "&quot;"),
    ] {
        if rest.starts_with(literal) {
            return Decoded::Named(text, literal.len());
        }
    }
    let bytes = rest.as_bytes();
    if !rest.starts_with("&#") {
        return Decoded::None;
    }
    let (radix, digits_at) = if bytes
        .get(2)
        .is_some_and(|byte| *byte == b'x' || *byte == b'X')
    {
        (16, 3)
    } else {
        (10, 2)
    };
    let mut end = digits_at;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end == digits_at || bytes.get(end) != Some(&b';') {
        return Decoded::None;
    }
    match u32::from_str_radix(&rest[digits_at..end], radix)
        .ok()
        .and_then(char::from_u32)
    {
        Some(character) => Decoded::Character(character, end + 1),
        None => Decoded::None,
    }
}

/// Merges parsed attribute items into `target`'s attributes.
///
/// `#id` sets, `.class` appends space-separated, `key=value` sets and
/// a bare `key` sets the empty string. An extension target carries its
/// attributes in the node — the channel the `[[attribute_list]]`
/// entry's declared kind names — and a core target takes them through
/// the arena's attribute table, which the core renderer reads for the
/// kinds that render attributes.
fn merge_attributes(arena: &mut Arena<'_>, target: NodeId, items: &[AttrItem]) {
    for item in items {
        match item {
            AttrItem::Id(value) => set_attribute(arena, target, "id", value.clone()),
            AttrItem::Class(value) => append_class(arena, target, value.clone()),
            AttrItem::Pair(key, value) => set_attribute(arena, target, key, value.clone()),
            AttrItem::Flag(key) => set_attribute(arena, target, key, String::new()),
        }
    }
}

/// Sets one attribute on `target`, on the extension node or in the
/// arena's table as the node's kind calls for.
fn set_attribute(arena: &mut Arena<'_>, target: NodeId, name: &str, value: String) {
    if matches!(arena.kind(target), Node::Extension(_)) {
        let (kind, mut attrs) = match arena.kind(target) {
            Node::Extension(ext) => (ext.kind.clone(), ext.attrs.clone()),
            _ => return,
        };
        match attrs.iter_mut().find(|(attr, _)| attr == name) {
            Some(slot) => slot.1 = AttrValue::Str(value),
            None => attrs.push((name.to_owned(), AttrValue::Str(value))),
        }
        arena.set_kind(target, Node::Extension(ExtNode { kind, attrs }));
    } else {
        arena.set_attr(target, name, AttrValue::Str(value));
    }
}

/// Appends one class to `target`'s `class`, space-separated.
fn append_class(arena: &mut Arena<'_>, target: NodeId, value: String) {
    let existing = match arena.kind(target) {
        Node::Extension(ext) => ext_attr_str(ext, "class"),
        _ => match arena.attr(target, "class") {
            Some(AttrValue::Str(text)) => Some(text.clone()),
            _ => None,
        },
    };
    let combined = match existing {
        Some(previous) => format!("{previous} {value}"),
        None => value,
    };
    set_attribute(arena, target, "class", combined);
}

/// The string value of `name` on an extension node's attributes.
fn ext_attr_str(node: &ExtNode, name: &str) -> Option<String> {
    node.attrs
        .iter()
        .find(|(attr, _)| attr == name)
        .and_then(|(_, value)| match value {
            AttrValue::Str(text) => Some(text.clone()),
            _ => None,
        })
}

/// Sets one attribute on an extension node, replacing an existing
/// value for the name.
fn set_ext_attr(arena: &mut Arena<'_>, target: NodeId, name: &str, value: String) {
    let (kind, mut attrs) = match arena.kind(target) {
        Node::Extension(ext) => (ext.kind.clone(), ext.attrs.clone()),
        _ => return,
    };
    match attrs.iter_mut().find(|(attr, _)| attr == name) {
        Some(slot) => slot.1 = AttrValue::Str(value),
        None => attrs.push((name.to_owned(), AttrValue::Str(value))),
    }
    arena.set_kind(target, Node::Extension(ExtNode { kind, attrs }));
}

/// Writes `template` out, replacing `{name}` spans the resolver knows
/// with their values and emitting every other `{...}` span literally,
/// as the format requires: the template is emitted literally except
/// for the substitutions, with no escaping provided.
fn substitute(template: &str, resolve: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(template.len());
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
    out
}

#[cfg(test)]
mod private_tests {
    use super::*;

    #[test]
    fn list_parses_across_the_inline_pass_splits() {
        // The inline pass splits `a ==x=={color=red} b` into text nodes
        // at the `=`; the joined run must parse as one list.
        let rules = [AttributeList {
            kind: "hl".to_owned(),
            prefix: "{:".to_owned(),
            suffix: "}".to_owned(),
        }];
        let (items, rest) = parse_prefixed_list(&rules, "{:color=red} b").unwrap();
        assert_eq!(
            items,
            vec![AttrItem::Pair("color".to_owned(), "red".to_owned())]
        );
        assert_eq!(rest, " b");
    }
}
