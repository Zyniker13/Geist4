//! Description lists: terms marked with `:` or `~` and their
//! definitions, in the shape comrak 0.51.0 renders.
//!
//! A marker line after a paragraph turns that paragraph into the
//! list's first term; a marker line after a closed paragraph steals
//! it as a new term; a marker line right after a description list
//! adds another definition to its last term. A blank line between a
//! term and its definition, or between the paragraph a marker line
//! steals and the marker, makes that item's definitions loose; every
//! other item stays tight. This is comrak's rule, decided per item at
//! the moment its marker line is parsed.
//!
//! Each item is one open frame. `try_start` claims the marker line —
//! converting nothing, but letting the framework close an open
//! paragraph so the item opens right behind it — `try_continue` holds
//! the frame for blank and indented lines, and `close` arranges the
//! tree: the item gathers the paragraph behind it as its term (its
//! text moves under a `dt`) and its own definition blocks under a
//! `dd`, then joins the `dl` that ends just before it, creating that
//! `dl` when the item starts a list.

use geist4_core::arena::NodeId;
use geist4_core::extension::{
    BlockContinueCtx, BlockStartCtx, CloseCtx, ContinueOutcome, ExtNode, Extension, KindDecl,
    LookCtx, RenderedChild, StartContainer,
};
use geist4_core::node::Node;
use geist4_core::span::Span;
use std::any::Any;

const EXT_NAME: &str = "description_lists";
const KIND_LIST: &str = "description_list";
const KIND_ITEM: &str = "description_item";
const KIND_TERM: &str = "description_term";
const KIND_DETAILS: &str = "description_details";
const KIND_DETAILS_LOOSE: &str = "description_details_loose";

/// Builds the extension for the `Options` registry.
pub fn deflists() -> std::sync::Arc<dyn Extension> {
    std::sync::Arc::new(DescriptionLists)
}

struct DescriptionLists;

/// The open state of one description item.
struct ItemState {
    /// Whether the item's definitions render tight.
    tight: bool,
    /// The marker line's indent past the container prefixes.
    marker_offset: usize,
    /// The marker's width in bytes: the `:` or `~` plus the run of
    /// spaces and tabs after it.
    padding: usize,
}

/// The length of a description marker at the start of `line` — which
/// begins at the line's first non-space byte: one `:` or `~` followed
/// by at least one space or tab, the run counting into the marker the
/// way comrak's scanner counts it.
fn scan_marker(line: &[u8]) -> Option<usize> {
    let (&marker, rest) = line.split_first()?;
    if marker != b':' && marker != b'~' {
        return None;
    }
    let mut end = 1;
    let mut spaced = false;
    for &byte in rest {
        if byte == b' ' || byte == b'\t' {
            end += 1;
            spaced = true;
        } else {
            break;
        }
    }
    if spaced { Some(end) } else { None }
}

/// A tight definition renders its paragraph children without their
/// `<p>` wrapper, exactly as comrak renders tight definitions. The
/// wrapper is there only when the child rendered through the plain
/// paragraph renderer; any other shape passes through untouched.
fn strip_paragraph_wrapper(html: &str) -> Option<&str> {
    html.strip_prefix("<p>")?.strip_suffix("</p>\n")
}

/// Writes the newline a block child needs after `<dd>` when the
/// output does not already end a line: comrak's block renderers open
/// with a conditional newline that a fresh sub-render cannot emit, so
/// the hook supplies it.
fn ensure_newline(out: &mut String) {
    if !out.ends_with('\n') {
        out.push('\n');
    }
}

impl Extension for DescriptionLists {
    fn name(&self) -> &str {
        EXT_NAME
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![
            KindDecl {
                kind: KIND_LIST.to_string(),
                container: true,
                inline_text: false,
                attrs: Vec::new(),
            },
            KindDecl {
                kind: KIND_ITEM.to_string(),
                container: true,
                inline_text: false,
                attrs: Vec::new(),
            },
            // The term holds the paragraph's inline content: its text
            // child parses as inline content and its children render
            // inline, which is what `<dt>` wraps.
            KindDecl {
                kind: KIND_TERM.to_string(),
                container: false,
                inline_text: true,
                attrs: Vec::new(),
            },
            KindDecl {
                kind: KIND_DETAILS.to_string(),
                container: true,
                inline_text: false,
                attrs: Vec::new(),
            },
            KindDecl {
                kind: KIND_DETAILS_LOOSE.to_string(),
                container: true,
                inline_text: false,
                attrs: Vec::new(),
            },
        ]
    }

    fn contains_blocks(&self) -> bool {
        true
    }

    /// Deliberately mirrors `try_start`'s guard: the lazy continuation
    /// fires only when the block-start phase would claim nothing, and
    /// this hook is the parser's way of asking. A marker line against a
    /// container whose last child is this extension's own item is a
    /// coming sibling definition — the item's close forms the list the
    /// line then joins — so it must not lazily continue the definition
    /// paragraph inside that item. Against any other container (a
    /// block quote, say) the line stays lazy paragraph text, exactly
    /// as the reference implementation renders `> Term` with `: def`
    /// after it.
    fn block_start_looks(&self, ctx: &LookCtx<'_, '_>) -> bool {
        if ctx.indented {
            return false;
        }
        if scan_marker(ctx.line).is_none() {
            return false;
        }
        let Some(last) = ctx.arena.last_child(ctx.container) else {
            return false;
        };
        match ctx.arena.kind(last) {
            // This extension's own item: its close builds the list this
            // line's definition joins.
            Node::Extension(ext) if ext.kind == KIND_ITEM => true,
            // This extension's list: the line would continue its last
            // term, when that item ended in a definition.
            Node::Extension(ext) if ext.kind == KIND_LIST => {
                let Some(item) = ctx.arena.last_child(last) else {
                    return false;
                };
                let Some(details) = ctx.arena.last_child(item) else {
                    return false;
                };
                matches!(
                    ctx.arena.kind(details),
                    Node::Extension(ext)
                        if ext.kind == KIND_DETAILS || ext.kind == KIND_DETAILS_LOOSE
                )
            }
            _ => false,
        }
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        if ctx.indented() {
            return;
        }
        let Some(matched) = scan_marker(ctx.line()) else {
            return;
        };
        let marker_offset = ctx.indent();
        match ctx.container() {
            // The open paragraph above the marker line becomes the
            // term. Opening the item closes that paragraph (its text
            // is already in its node) and the item lands right behind
            // it, where `close` finds it; the rest of the marker line
            // starts this item's first definition. An open paragraph
            // means the definition is tight, as comrak decides.
            StartContainer::Paragraph => {
                let state = ItemState {
                    tight: true,
                    marker_offset,
                    padding: matched,
                };
                ctx.open_extension_keep_rest(KIND_ITEM.to_owned(), Box::new(state));
                ctx.seek_content(matched);
            }
            // A marker line after closed blocks: behind a closed
            // paragraph it steals that paragraph as the term, loose as
            // comrak's closed-paragraph path decides; behind a
            // description list it becomes that list's last term's next
            // definition, tight or loose as that definition is.
            start => {
                let holder = match start {
                    StartContainer::Container => ctx.container_node(),
                    StartContainer::Extension(index) if index == ctx.extension() => ctx.open_node(),
                    _ => None,
                };
                let Some(holder) = holder else {
                    return;
                };
                let arena = ctx.arena();
                let Some(last) = arena.last_child(holder) else {
                    return;
                };
                let tight = match arena.kind(last) {
                    Node::Paragraph => false,
                    Node::Extension(ext) if ext.kind == KIND_LIST => {
                        let Some(item) = arena.last_child(last) else {
                            return;
                        };
                        let Some(details) = arena.last_child(item) else {
                            return;
                        };
                        match arena.kind(details) {
                            Node::Extension(ext) if ext.kind == KIND_DETAILS => true,
                            Node::Extension(ext) if ext.kind == KIND_DETAILS_LOOSE => false,
                            _ => return,
                        }
                    }
                    _ => return,
                };
                let state = ItemState {
                    tight,
                    marker_offset,
                    padding: matched,
                };
                ctx.open_extension_keep_rest(KIND_ITEM.to_owned(), Box::new(state));
                ctx.seek_content(matched);
            }
        }
    }

    fn try_continue(&self, ctx: &mut BlockContinueCtx<'_, '_, '_>) -> ContinueOutcome {
        let Some(state) = ctx.state::<ItemState>() else {
            return ContinueOutcome::Close;
        };
        let marker_offset = state.marker_offset;
        let padding = state.padding;
        // A blank line keeps the item open; its definition blocks end.
        if ctx.pure_blank() {
            return ContinueOutcome::Continue;
        }
        // A line indented as far as the marker continues the item, the
        // marker's padding consumed; anything else closes it and frees
        // the line for other blocks.
        if ctx.indent() >= marker_offset + padding {
            ctx.consume_cols(padding);
            return ContinueOutcome::Continue;
        }
        ContinueOutcome::Close
    }

    fn close(&self, state: &mut dyn Any, ctx: &mut CloseCtx) {
        let Some(state) = state.downcast_mut::<ItemState>() else {
            return;
        };
        let tight = state.tight;
        let arena = &mut ctx.arena;
        let item = ctx.node;
        let start = ctx.start_offset;

        // The term: the paragraph right behind the item, when there is
        // one — the paragraph an open marker line converted, or the
        // closed paragraph the marker line follows. Its children move
        // under a `dt` node and the paragraph leaves the tree.
        let prev = arena.parent(item).and_then(|parent| {
            let index = arena.children(parent).iter().position(|&c| c == item)?;
            if index == 0 {
                None
            } else {
                Some(arena.children(parent)[index - 1])
            }
        });
        let mut term_source: Option<(NodeId, NodeId)> = None;
        let term = match prev {
            Some(prev) => match arena.kind(prev) {
                Node::Paragraph => {
                    let term = arena.alloc(
                        Node::Extension(ExtNode {
                            kind: KIND_TERM.to_owned(),
                            attrs: Vec::new(),
                        }),
                        Span::empty(start),
                    );
                    let mut moved = Vec::new();
                    while let Some(child) = arena.pop_child(prev) {
                        moved.push(child);
                    }
                    moved.reverse();
                    for child in moved {
                        arena.push_child(term, child);
                    }
                    if let Some(parent) = arena.parent(prev) {
                        if let Some(index) = arena.children(parent).iter().position(|&c| c == prev)
                        {
                            arena.remove_child(parent, index);
                        }
                    }
                    // The moved text carries the paragraph's inline
                    // source map: the term is the leaf the inline phase
                    // parses now, and without the map it would place
                    // every inline at the term's own empty span.
                    term_source = Some((prev, term));
                    Some(term)
                }
                _ => None,
            },
            None => None,
        };

        // The item's definition blocks gather under one definition
        // node, tight or loose by how the item began.
        let details = arena.alloc(
            Node::Extension(ExtNode {
                kind: if tight {
                    KIND_DETAILS
                } else {
                    KIND_DETAILS_LOOSE
                }
                .to_owned(),
                attrs: Vec::new(),
            }),
            Span::empty(start),
        );
        let mut moved = Vec::new();
        while let Some(child) = arena.pop_child(item) {
            moved.push(child);
        }
        moved.reverse();
        for child in moved {
            arena.push_child(details, child);
        }
        arena.push_child(item, details);
        if let Some(term) = term {
            arena.insert_child(item, 0, term);
        }

        // The item joins the description list that ends just before
        // it, or starts one where it stands.
        let Some(parent) = arena.parent(item) else {
            return;
        };
        let Some(index) = arena.children(parent).iter().position(|&c| c == item) else {
            return;
        };
        let prev_is_list = match index.checked_sub(1).map(|i| arena.children(parent)[i]) {
            Some(prev) => {
                matches!(arena.kind(prev), Node::Extension(ext) if ext.kind == KIND_LIST)
            }
            None => false,
        };
        if prev_is_list {
            let list = arena.children(parent)[index - 1];
            arena.remove_child(parent, index);
            arena.push_child(list, item);
        } else {
            let list = arena.alloc(
                Node::Extension(ExtNode {
                    kind: KIND_LIST.to_owned(),
                    attrs: Vec::new(),
                }),
                Span::new(start, ctx.end_offset),
            );
            arena.insert_child(parent, index, list);
            arena.remove_child(parent, index + 1);
            arena.push_child(list, item);
        }
        if let Some((source, term)) = term_source {
            if let Some(map) = ctx.take_text_map(source) {
                ctx.set_text_map(term, map);
            }
        }
    }

    fn render(&self, node: &ExtNode, children: &[RenderedChild<'_>], out: &mut String) {
        match node.kind.as_str() {
            KIND_LIST => {
                out.push_str("<dl>\n");
                for child in children {
                    out.push_str(&child.html);
                }
                out.push_str("</dl>\n");
            }
            // The term child carries its own `<dt>` and the definition
            // children their own `<dd>`, so the item only orders them.
            KIND_ITEM => {
                for child in children {
                    out.push_str(&child.html);
                }
            }
            KIND_TERM => {
                out.push_str("<dt>");
                for child in children {
                    out.push_str(&child.html);
                }
                out.push_str("</dt>\n");
            }
            KIND_DETAILS | KIND_DETAILS_LOOSE => {
                let tight = node.kind == KIND_DETAILS;
                out.push_str("<dd>");
                for child in children {
                    if tight && child.core_kind == Some("paragraph") {
                        if let Some(body) = strip_paragraph_wrapper(&child.html) {
                            out.push_str(body);
                            continue;
                        }
                    }
                    ensure_newline(out);
                    out.push_str(&child.html);
                }
                out.push_str("</dd>\n");
            }
            _ => {}
        }
    }
}
