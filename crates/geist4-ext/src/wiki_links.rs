//! Built-in wiki links: `[[target]]` or `[[target|label]]` becomes a
//! wiki link node, rendered as `<a href="{target}" class="wikilink">`.
//!
//! The semantics are the declarative tier's `[[wiki_link]]` rules,
//! exactly as `conformance/behavior/wiki-links` pins them: the group's
//! own `[` consumes the prefix's first byte, so the content must start
//! with `[`; the group's `]` is the suffix's last byte, so the content
//! must end with `]`. The first `|` splits target from label, and
//! without one the target is also the label. The claim happens before
//! the inline machinery runs, so emphasis inside the brackets stays
//! literal; a real link around the group wins over the wiki claim,
//! because the bracket hook only sees groups that matched no link.
//! Spaces and the reserved `#` and `?` percent-encode into the target
//! but stay literal in the label.

use geist4_core::basic_html::{escape_href, escape_html};
use geist4_core::extension::{
    AttrValue, ExtNode, Extension, InlineBracketCtx, KindDecl, RenderedChild,
};

/// The built-in wiki links extension.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WikiLinks;

impl WikiLinks {
    /// The wiki link kind's name.
    pub const KIND: &'static str = "wiki_link";

    /// Wiki links as a shared extension value.
    #[must_use]
    pub fn extension() -> std::sync::Arc<dyn Extension> {
        std::sync::Arc::new(WikiLinks)
    }
}

impl Extension for WikiLinks {
    fn name(&self) -> &str {
        "wiki_links"
    }

    fn kinds(&self) -> Vec<KindDecl> {
        vec![KindDecl {
            kind: Self::KIND.to_owned(),
            container: false,
            inline_text: true,
            attrs: vec!["target".to_owned(), "children".to_owned()],
        }]
    }

    fn inline_bracket(&self, ctx: &mut InlineBracketCtx<'_, '_, '_, '_>) {
        // The group's own `[` consumed the prefix's first byte, so the
        // content must start with the prefix's remainder.
        if !ctx.content_starts_with(b"[") {
            return;
        }
        let Some(content) = ctx.content() else {
            return;
        };
        // The group's `]` is the suffix's last byte, so the content
        // must end with the suffix's remainder.
        let Some(inner) = content
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        else {
            return;
        };
        // The first `|` splits the target from the rendered children;
        // without one the target is also the label.
        let (target, label) = match inner.split_once('|') {
            Some((target, label)) => (target.to_owned(), label.to_owned()),
            None => (inner.to_owned(), inner.to_owned()),
        };
        // The content borrow ends here; the claim drops every node the
        // group holds, including any delimiters pushed inside it, so
        // emphasis inside the brackets stays literal.
        ctx.claim(
            Self::KIND.to_owned(),
            vec![
                ("target".to_owned(), AttrValue::Str(target)),
                ("children".to_owned(), AttrValue::Str(label)),
            ],
        );
    }

    fn render(&self, node: &ExtNode, children: &[RenderedChild<'_>], out: &mut String) {
        if node.kind != Self::KIND {
            return;
        }
        let target = attr_str(node, "target");
        out.push_str("<a href=\"");
        out.push_str(&encode_target(target));
        out.push_str("\" class=\"wikilink\">");
        // The claimed node has no inline children; the label the group
        // carried rides in the `children` attribute, escaped like
        // inline text.
        out.push_str(&escape_html(attr_str(node, "children")));
        for child in children {
            out.push_str(&child.html);
        }
        out.push_str("</a>");
    }
}

/// The node's string value for `name`, or the empty string when unset.
/// The declared kinds carry strings only, so any other value is
/// unreachable; a non-string renders empty rather than panicking.
fn attr_str<'a>(node: &'a ExtNode, name: &str) -> &'a str {
    node.attrs
        .iter()
        .find(|(attr, _)| attr == name)
        .map_or("", |(_, value)| match value {
            AttrValue::Str(text) => text.as_str(),
            AttrValue::Int(_) | AttrValue::Bool(_) => "",
        })
}

/// The wiki target for an `href`: the core's href escaping, with `#`
/// and `?` encoded past it so a page name stays one page name.
fn encode_target(target: &str) -> String {
    escape_href(target).replace('#', "%23").replace('?', "%3F")
}
