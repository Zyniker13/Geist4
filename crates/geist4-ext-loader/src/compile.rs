//! The top-level compiler: one extension manifest becomes one
//! [`Extension`] object. Tier-1 behavior entries delegate to the
//! declarative tiers (inline, block, post-processing), the HTML filter
//! entries are implemented here, and tier 2/3 wiring joins later
//! milestones.

use std::sync::Arc;

use geist4_core::extension::{
    BlockContinueCtx, BlockStartCtx, CloseCtx, ExtNode, Extension, InlineBracketCtx,
    InlineTriggerCtx, KindDecl, LookCtx, RenderedChild, TreeCtx,
};

use crate::types::{Behavior, Manifest};

/// One loaded extension manifest compiled onto the extension API.
pub struct Compiled {
    manifest: Manifest,
    inline: crate::decl_inline::InlineTier,
    block: crate::decl_block::BlockTier,
    post: crate::decl_block::PostTier,
    /// Kinds rendered by the block tier (the container kinds).
    block_kinds: Vec<String>,
    /// Kinds rendered by the inline tier (the span and wiki kinds).
    inline_kinds: Vec<String>,
    /// Every denied raw-HTML tag start, from the filter entries.
    denied_tags: Vec<String>,
}

/// Compiles a validated manifest into an extension object. The manifest
/// must have come through [`crate::manifest::parse`]; compiling an
/// unvalidated manifest skips the checks the loader guarantees.
pub fn compile(manifest: Manifest) -> Arc<dyn Extension> {
    let inline_kinds = manifest
        .behavior
        .iter()
        .filter(|entry| matches!(entry, Behavior::InlineSpan(_) | Behavior::WikiLink(_)))
        .map(|entry| match entry {
            Behavior::InlineSpan(entry) => entry.kind.clone(),
            Behavior::WikiLink(entry) => entry.kind.clone(),
            _ => unreachable!("filtered above"),
        })
        .collect();
    let block_kinds = manifest
        .behavior
        .iter()
        .filter_map(|entry| match entry {
            Behavior::BlockContainer(entry) => Some(entry.kind.clone()),
            _ => None,
        })
        .collect();
    let denied_tags = manifest
        .behavior
        .iter()
        .filter_map(|entry| match entry {
            Behavior::HtmlFilter(entry) => Some(entry.deny.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    Arc::new(Compiled {
        inline: crate::decl_inline::InlineTier::from_manifest(&manifest),
        block: crate::decl_block::BlockTier::from_manifest(&manifest),
        post: crate::decl_block::PostTier::from_manifest(&manifest),
        block_kinds,
        inline_kinds,
        denied_tags,
        manifest,
    })
}

impl Compiled {
    /// The manifest this extension was compiled from.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
}

impl Extension for Compiled {
    fn name(&self) -> &str {
        &self.manifest.extension.name
    }

    fn kinds(&self) -> Vec<KindDecl> {
        self.manifest
            .kinds
            .iter()
            .map(|entry| KindDecl {
                kind: entry.kind.clone(),
                container: entry.container,
                inline_text: entry.inline_text,
                attrs: entry.attrs.clone(),
            })
            .collect()
    }

    fn inline_triggers(&self) -> &[u8] {
        self.inline.inline_triggers()
    }

    fn inline_trigger(&self, ctx: &mut InlineTriggerCtx<'_, '_, '_, '_>) {
        self.inline.inline_trigger(ctx);
    }

    fn inline_bracket(&self, ctx: &mut InlineBracketCtx<'_, '_, '_, '_>) {
        self.inline.inline_bracket(ctx);
    }

    fn contains_blocks(&self) -> bool {
        self.block.contains_blocks()
    }

    fn block_start_looks(&self, ctx: &LookCtx<'_, '_>) -> bool {
        self.block.block_start_looks(ctx)
    }

    fn try_start(&self, ctx: &mut BlockStartCtx<'_, '_, '_, '_>) {
        self.block.try_start(ctx);
    }

    fn try_continue(&self, ctx: &mut BlockContinueCtx<'_, '_, '_>) -> bool {
        self.block.try_continue(ctx)
    }

    fn close(&self, state: &mut dyn std::any::Any, ctx: &mut CloseCtx<'_, '_>) {
        self.block.close(state, ctx);
    }

    fn post_process(&self, ctx: &mut TreeCtx<'_, '_, '_, '_>) {
        self.inline.post_process(ctx);
        self.post.post_process(ctx);
    }

    fn filters_html(&self) -> bool {
        !self.denied_tags.is_empty()
    }

    fn html_filter(&self, literal: &[u8]) -> bool {
        // `literal` starts at the `<`. The denied tag is the name right
        // after it (or after `</`), compared case-insensitively; a name
        // that merely starts like a denied tag is not denied.
        let body = &literal[1..];
        let body = match body.first() {
            Some(&b'/') => &body[1..],
            _ => body,
        };
        let name = tag_name(body);
        !self
            .denied_tags
            .iter()
            .any(|deny| name.eq_ignore_ascii_case(deny))
    }

    fn render(&self, node: &ExtNode, children: &[RenderedChild<'_>], out: &mut String) {
        if self.block_kinds.contains(&node.kind) {
            self.block.render(node, children, out);
        } else if self.inline_kinds.contains(&node.kind) {
            self.inline.render(node, children, out);
        }
    }
}

/// Reads a tag name from the bytes after `<`: an optional `/` then the
/// alphanumeric run, lowercased. A name that merely starts like a denied
/// tag scans as a longer name and does not match.
fn tag_name(body: &[u8]) -> String {
    let start = usize::from(body.first() == Some(&b'/'));
    let mut end = start;
    while end < body.len()
        && (body[end].is_ascii_alphanumeric()
            || (end > start && (body[end] == b'-' || body[end] == b'_')))
    {
        end += 1;
    }
    String::from_utf8_lossy(&body[start..end]).to_ascii_lowercase()
}
