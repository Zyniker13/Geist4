//! The manifest data model for format version 1, as specified in
//! `docs/extensions/format.md`. These types are the loader's contract:
//! `manifest` parses and validates a manifest into them, the declarative
//! compiler consumes them, and the sandbox maps them onto the WIT world.

use std::collections::BTreeMap;

/// A validated extension manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    /// The `[extension]` table.
    pub extension: Extension,
    /// The declared node kinds, in declaration order.
    pub kinds: Vec<KindEntry>,
    /// The declared options, keyed by name.
    pub options: BTreeMap<String, OptionDecl>,
    /// The tier-1 behavior entries, in declaration order.
    pub behavior: Vec<Behavior>,
    /// The tier-2 section, when the behavior is a component.
    pub wasm: Option<Wasm>,
    /// The tier-3 marker; the behavior lives in the host.
    pub host: bool,
}

/// The `[extension]` table.
#[derive(Debug, Clone, PartialEq)]
pub struct Extension {
    /// The unique name: `[a-z][a-z0-9-]*`, 1–64 bytes.
    pub name: String,
    /// The extension's own version, semver `X.Y[.Z]`.
    pub version: String,
    /// The format version, `X.Y`.
    pub format: String,
    /// The one-line description.
    pub description: String,
    /// An SPDX license expression string.
    pub license: String,
    /// An optional homepage URL.
    pub homepage: Option<String>,
    /// The hook vocabulary the behavior uses.
    pub hooks: Vec<String>,
    /// Extensions this one must load after.
    pub after: Vec<String>,
    /// Extensions this one cannot load beside.
    pub conflicts: Vec<String>,
}

/// One declared node kind.
#[derive(Debug, Clone, PartialEq)]
pub struct KindEntry {
    /// The kind name, unique across loaded extensions.
    pub kind: String,
    /// Whether nodes hold children.
    pub container: bool,
    /// Whether children render as inline content.
    pub inline_text: bool,
    /// The accepted attribute names, in order.
    pub attrs: Vec<String>,
}

/// One declared option.
#[derive(Debug, Clone, PartialEq)]
pub struct OptionDecl {
    /// string | bool | number
    pub ty: OptionType,
    /// The default value, of the declared type.
    pub default: OptionValue,
    /// The human description.
    pub description: String,
}

/// The type of an option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionType {
    /// A string value.
    String,
    /// A boolean value.
    Bool,
    /// A number value.
    Number,
}

/// An option or attribute value.
#[derive(Debug, Clone, PartialEq)]
pub enum OptionValue {
    /// A string value.
    String(String),
    /// A boolean value.
    Bool(bool),
    /// A number value.
    Number(f64),
}

/// One tier-1 behavior entry.
#[derive(Debug, Clone, PartialEq)]
pub enum Behavior {
    /// A delimited inline span.
    InlineSpan(InlineSpan),
    /// A fenced block container.
    BlockContainer(BlockContainer),
    /// A literal replacement.
    Replace(Replace),
    /// An attribute list.
    AttributeList(AttributeList),
    /// A wiki link.
    WikiLink(WikiLink),
    /// An output filter.
    HtmlFilter(HtmlFilter),
}

/// A delimited inline span: `==highlight==`, `$x + 1$`.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineSpan {
    /// The kind wrapping the span.
    pub kind: String,
    /// The opening delimiter, 1–16 bytes.
    pub open: String,
    /// The closing delimiter, 1–16 bytes.
    pub close: String,
    /// The render template with `{children}` and `{attr}` substitutions.
    pub html: String,
    /// Whether the opener may open (flanking rules).
    pub can_open: bool,
    /// Whether the closer may close (flanking rules).
    pub can_close: bool,
    /// Whether the span may cross soft breaks.
    pub multiline: bool,
    /// Whether the content follows the inline math guards.
    pub math: bool,
}

/// A fenced block container: `:::note` … `:::`.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockContainer {
    /// The kind of the container node.
    pub kind: String,
    /// The marker run, 3–16 bytes of one repeated character.
    pub marker: String,
    /// The word after the marker that opens the container.
    pub name: String,
    /// The template for the info line; empty ignores the info.
    pub info_html: String,
    /// The render template with `{children}` and `{info}`.
    pub html: String,
    /// Whether EOF closes an unclosed container.
    pub unclosed_ok: bool,
}

/// A literal replacement.
#[derive(Debug, Clone, PartialEq)]
pub struct Replace {
    /// The literal to find, 1–32 bytes, no newline.
    pub from: String,
    /// The literal output, 1–128 bytes.
    pub to: String,
    /// At most once per text node.
    pub once: bool,
}

/// An attribute list: `{:#id .class key=value}`.
#[derive(Debug, Clone, PartialEq)]
pub struct AttributeList {
    /// The kind whose renderer receives the attributes when the target
    /// is an extension node.
    pub kind: String,
    /// The list opener; must start with `{`.
    pub prefix: String,
    /// The list closer.
    pub suffix: String,
}

/// A wiki link: `[[Page]]` or `[[Page|label]]`.
#[derive(Debug, Clone, PartialEq)]
pub struct WikiLink {
    /// The kind of the node.
    pub kind: String,
    /// The bracket prefix, `[[`.
    pub prefix: String,
    /// The bracket suffix, `]]`.
    pub suffix: String,
    /// The render template with `{target}` and `{children}`.
    pub html: String,
}

/// An output filter, the tagfilter's semantics.
#[derive(Debug, Clone, PartialEq)]
pub struct HtmlFilter {
    /// The denied tag names.
    pub deny: Vec<String>,
}

/// The tier-2 section.
#[derive(Debug, Clone, PartialEq)]
pub struct Wasm {
    /// The component file path, relative to the package root.
    pub path: String,
    /// The fuel the extension may burn per call.
    pub fuel: Option<u64>,
    /// The memory the extension may use, in pages.
    pub memory_pages: Option<u32>,
}
