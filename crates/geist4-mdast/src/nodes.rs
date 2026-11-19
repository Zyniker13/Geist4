//! The mdast node types, in the exact field order the reference
//! implementation serializes them, so the exported trees match its JSON
//! byte for byte.

use serde::Serialize;

use crate::Position;

macro_rules! mdast_struct {
    (
        $(#[$doc:meta])*
        $name:ident {
            $($(#[$field_doc:meta])* [$serde:literal] $field:ident : $ty:ty),* $(,)?
        }
    ) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct $name {
            #[serde(rename = "type")]
            pub type_: &'static str,
            $(#[serde(rename = $serde)] pub $field: $ty,)*
        }
    };
}

mdast_struct!(
    /// The document root.
    Root { ["children"] children: Vec<Mdast>, ["position"] position: Position }
);
mdast_struct!(
    /// A `>` block quote.
    BlockQuote { ["children"] children: Vec<Mdast>, ["position"] position: Position }
);
mdast_struct!(
    /// A list; `children` are list items.
    List {
        ["ordered"] ordered: bool,
        ["start"] start: Option<u64>,
        ["spread"] spread: bool,
        ["children"] children: Vec<Mdast>,
        ["position"] position: Position
    }
);
mdast_struct!(
    /// One list item.
    ListItem {
        ["spread"] spread: bool,
        ["checked"] checked: Option<bool>,
        ["children"] children: Vec<Mdast>,
        ["position"] position: Position
    }
);
mdast_struct!(
    /// An ATX or setext heading.
    Heading {
        ["depth"] depth: u8,
        ["children"] children: Vec<Mdast>,
        ["position"] position: Position
    }
);
mdast_struct!(
    /// A `---`, `***` or `___` rule.
    ThematicBreak { ["position"] position: Position }
);
mdast_struct!(
    /// A paragraph.
    Paragraph { ["children"] children: Vec<Mdast>, ["position"] position: Position }
);
mdast_struct!(
    /// A fenced or indented code block.
    Code {
        ["lang"] lang: Option<String>,
        ["meta"] meta: Option<String>,
        ["value"] value: String,
        ["position"] position: Position
    }
);
mdast_struct!(
    /// A raw HTML block or inline HTML run.
    Html { ["value"] value: String, ["position"] position: Position }
);
mdast_struct!(
    /// Literal text.
    Text { ["value"] value: String, ["position"] position: Position }
);
mdast_struct!(
    /// A `` `code` `` span.
    InlineCode { ["value"] value: String, ["position"] position: Position }
);
mdast_struct!(
    /// `*emphasis*`.
    Emphasis { ["children"] children: Vec<Mdast>, ["position"] position: Position }
);
mdast_struct!(
    /// `**strong**`.
    Strong { ["children"] children: Vec<Mdast>, ["position"] position: Position }
);
mdast_struct!(
    /// An inline link.
    Link {
        ["title"] title: Option<String>,
        ["url"] url: String,
        ["children"] children: Vec<Mdast>,
        ["position"] position: Position
    }
);
mdast_struct!(
    /// An image.
    Image {
        ["title"] title: Option<String>,
        ["url"] url: String,
        ["alt"] alt: String,
        ["position"] position: Position
    }
);
mdast_struct!(
    /// A hard line break.
    Break { ["position"] position: Position }
);
mdast_struct!(
    /// A reference link, resolved or not.
    LinkReference {
        ["children"] children: Vec<Mdast>,
        ["position"] position: Position,
        ["label"] label: String,
        ["identifier"] identifier: String,
        ["referenceType"] reference_type: String
    }
);
mdast_struct!(
    /// A reference image.
    ImageReference {
        ["alt"] alt: String,
        ["position"] position: Position,
        ["label"] label: String,
        ["identifier"] identifier: String,
        ["referenceType"] reference_type: String
    }
);
mdast_struct!(
    /// A link reference definition.
    Definition {
        ["identifier"] identifier: String,
        ["label"] label: String,
        ["title"] title: Option<String>,
        ["url"] url: String,
        ["position"] position: Position
    }
);

/// One mdast node: any of the types the export produces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Mdast {
    /// The document root.
    Root(Root),
    /// A `>` block quote.
    BlockQuote(BlockQuote),
    /// A list.
    List(List),
    /// One list item.
    ListItem(ListItem),
    /// An ATX or setext heading.
    Heading(Heading),
    /// A `---`, `***` or `___` rule.
    ThematicBreak(ThematicBreak),
    /// A paragraph.
    Paragraph(Paragraph),
    /// A fenced or indented code block.
    Code(Code),
    /// A raw HTML block or inline HTML run.
    Html(Html),
    /// Literal text.
    Text(Text),
    /// A `` `code` `` span.
    InlineCode(InlineCode),
    /// `*emphasis*`.
    Emphasis(Emphasis),
    /// `**strong**`.
    Strong(Strong),
    /// An inline link.
    Link(Link),
    /// An image.
    Image(Image),
    /// A hard line break.
    Break(Break),
    /// A reference link.
    LinkReference(LinkReference),
    /// A reference image.
    ImageReference(ImageReference),
    /// A link reference definition.
    Definition(Definition),
}

/// The `referenceType` value for a full reference: `[text][label]`.
pub(crate) const FULL: &str = "full";
/// The `referenceType` value for a collapsed reference: `[text][]`.
pub(crate) const COLLAPSED: &str = "collapsed";
/// The `referenceType` value for a shortcut reference: `[text]`.
pub(crate) const SHORTCUT: &str = "shortcut";
