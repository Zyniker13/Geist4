//! GFM task list items: a list item whose content starts with `[ ]`,
//! `[x]` or `[X]` followed by whitespace carries a checkbox, rendered
//! as `<input type="checkbox" ... />` between the `<li>` and the
//! item's content.
//!
//! The semantics follow the reference implementation exactly, quirks
//! included. The scanner runs on the whole line from its very first
//! byte, so a marker the line still carries is part of the match: the
//! box is recognized only on a line that starts a list item, which is
//! why a task list inside a block quote (`> - [x] done`) is not a task
//! list at all. The checked state comes from a plain substring search
//! over the same whole line, so `- [ ] [x] done` renders checked with
//! `[x] done` as text. Consuming the box stops the block-start walk,
//! so `- [ ] # done` keeps the `#` as text; and because the scanner
//! demands at least one whitespace character after the box, `- [x]done`
//! stays literal text.
//!
//! Whitespace classes: the box's required space follows the reference
//! scanner's class (space, tab, vertical tab, form feed), but the
//! engine's content-whitespace trims follow CommonMark 0.31.2, where a
//! vertical tab or form feed is whitespace and is trimmed at the ends
//! of the item's content. The cmark-gfm fork's older byte table keeps
//! those bytes, so its output differs from this engine on an item
//! whose content ends in one; see decision 0016.

use geist4_core::extension::{
    AttrValue, BlockStartCtx, CoreNode, Extension, OverrideChildren, StartContainer,
};
use geist4_core::node::Node;

/// The GFM task list item extension.
///
/// The extension claims the list items it recognizes during parsing:
/// the block-start hook scans the line, marks the item with the
/// checked state as an attribute, and consumes the three box bytes so
/// the rest of the line parses as the item's content. The render
/// override turns a claimed item into `<li>` plus the checkbox.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Tasklist;

impl Tasklist {
    /// The checked state attribute the extension sets on claimed items.
    pub const CHECKED: &'static str = "checked";
}

impl Extension for Tasklist {
    fn name(&self) -> &str {
        "tasklist"
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        // The reference hook only ever acts when the walk sits in a
        // list item; every other container declines.
        if !matches!(ctx.container(), StartContainer::Container) {
            return;
        }
        let Some(item) = ctx.container_node() else {
            return;
        };
        let is_item = matches!(ctx.arena().kind(item), Node::Item(_));
        if !is_item {
            return;
        }
        // The scanner sees the line from its first byte, list marker
        // and container prefixes included, like the reference's
        // line-starting scan.
        let line = ctx.raw_line();
        if !scan_tasklist(line) {
            return;
        }
        // The checked state is a substring search over the whole line:
        // any `[x]` or `[X]` anywhere on the line checks the box, even
        // when the item's own box is empty.
        let checked = contains(line, b"[x]") || contains(line, b"[X]");
        let index = ctx.extension();
        let arena = ctx.arena();
        arena.set_extension(item, index);
        arena.set_attr(item, Self::CHECKED, AttrValue::Bool(checked));
        // The box itself leaves the content; the walk stops, so what
        // follows it cannot start a new block.
        ctx.consume(3);
    }

    fn render_override(
        &self,
        node: &CoreNode<'_>,
        children: &mut OverrideChildren<'_, '_>,
        out: &mut String,
    ) {
        if node.kind != "item" {
            return;
        }
        let checked = matches!(
            node.attrs.iter().find(|(name, _)| name == Self::CHECKED),
            Some((_, AttrValue::Bool(true)))
        );
        out.push_str("<li><input type=\"checkbox\"");
        if checked {
            out.push_str(" checked=\"\"");
        }
        out.push_str(" disabled=\"\" /> ");
        while children.render_next(out) {}
        out.push_str("</li>\n");
    }
}

/// The task list scanner, byte for byte the reference's `scan_tasklist`
/// regular expression: optional space, then one of the list markers —
/// a dash, a plus, an asterisk, or a digit run followed by any single
/// character — then at least one space, then the box, then at least one
/// space. The box's letter is matched the way the reference's generated
/// scanner does: lower and upper case `x` both count, nothing else.
#[must_use]
pub fn scan_tasklist(line: &[u8]) -> bool {
    let mut i = 0;
    while i < line.len() && is_space(line[i]) {
        i += 1;
    }
    match line.get(i) {
        Some(b'-') | Some(b'+') | Some(b'*') => scan_box_after(line, i + 1),
        Some(c) if c.is_ascii_digit() => {
            // The digit run's splits into digits plus one any-character
            // are all candidates, longest first as the generated
            // scanner matches; any split that reaches the box matches.
            let mut end = i;
            while end < line.len() && line[end].is_ascii_digit() {
                end += 1;
            }
            let digits = end - i;
            for k in (1..=digits).rev() {
                let marker_end = i + k + 1;
                if marker_end > line.len() || line[marker_end - 1] == b'\n' {
                    continue;
                }
                if scan_box_after(line, marker_end) {
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

/// Whether at least one space, then the box, then at least one space
/// sit right at `at`.
fn scan_box_after(line: &[u8], at: usize) -> bool {
    let mut i = at;
    let start = i;
    while i < line.len() && is_space(line[i]) {
        i += 1;
    }
    if i == start {
        return false;
    }
    // The box: a space, `x` or `X` between brackets.
    if line.get(i) != Some(&b'[') {
        return false;
    }
    match line.get(i + 1) {
        Some(b' ') | Some(b'x') | Some(b'X') => {}
        _ => return false,
    }
    if line.get(i + 2) != Some(&b']') {
        return false;
    }
    let after = i + 3;
    let mut j = after;
    while j < line.len() && is_space(line[j]) {
        j += 1;
    }
    j > after
}

/// Whether `haystack` contains `needle` as a byte substring.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// The reference's space characters: space, tab, vertical tab and form
/// feed. Carriage returns never appear in a line's bytes here.
fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | 0x0B | 0x0C)
}

/// The task list extension as a shared extension value.
#[must_use]
pub fn tasklist() -> std::sync::Arc<dyn Extension> {
    std::sync::Arc::new(Tasklist)
}
