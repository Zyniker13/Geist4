//! The kinds of nodes a parsed document can contain.

use std::borrow::Cow;

/// A link reference definition collected during block parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkDef<'a> {
    /// The raw label as written, between the brackets.
    pub label: Cow<'a, str>,
    /// The destination URL as written.
    pub destination: Cow<'a, str>,
    /// The optional title as written.
    pub title: Option<Cow<'a, str>>,
}

/// What kind of node this is.
///
/// Block nodes form the tree's structure; leaf blocks hold inline nodes as
/// children. Inline nodes carry the content inside paragraphs, headings and
/// list items.
#[derive(Debug, Clone, PartialEq)]
pub enum Node<'a> {
    /// The document root.
    Document,
    /// A `>` block quote.
    BlockQuote,
    /// A list; its children are [`Node::Item`] nodes.
    List(ListData),
    /// One item of a list.
    Item(ItemData),
    /// An ATX or setext heading; children are its inline content.
    Heading(HeadingData),
    /// A `---`, `***` or `___` rule.
    ThematicBreak,
    /// A paragraph; children are its inline content.
    Paragraph,
    /// An indented or fenced code block.
    CodeBlock(CodeBlockData<'a>),
    /// A raw HTML block (one of the seven kinds).
    HtmlBlock(HtmlBlockData<'a>),
    /// Inline text, the default inline content.
    Text(Cow<'a, str>),
    /// An inline code span.
    CodeSpan(Cow<'a, str>),
    /// A raw HTML tag or other inline HTML construct, verbatim.
    InlineHtml(Cow<'a, str>),
    /// A line break within a paragraph, rendered as a newline.
    SoftBreak,
    /// A hard line break (two trailing spaces or a backslash).
    HardBreak,
    /// Emphasis (`*foo*`).
    Emphasis,
    /// Strong emphasis (`**foo**`).
    Strong,
    /// A link or image; children are the link text.
    Link(LinkData<'a>),
    /// An extension node: a kind an extension declared, with its
    /// attributes and children. Unrendered kinds produce no output and
    /// are reported by `check`.
    Extension(crate::extension::ExtNode),
}

/// The data of a [`Node::List`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListData {
    /// Whether the list is ordered (`1.`) rather than bullet (`-`).
    pub ordered: bool,
    /// The marker character: `-`, `+`, `*`, or the ordered delimiter `.`/`)`.
    pub marker: u8,
    /// The number the first item claims to be, for ordered lists.
    pub start: u64,
    /// Whether the list is tight (no blank lines inside).
    pub tight: bool,
}

/// The data of a [`Node::Item`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemData {
    /// How many columns the item's marker plus padding occupy.
    pub padding: u32,
}

/// The data of a [`Node::Heading`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingData {
    /// The heading level, 1 to 6.
    pub level: u8,
    /// Whether the heading came from a setext underline rather than `#`s.
    pub setext: bool,
}

/// The data of a [`Node::CodeBlock`].
#[derive(Debug, Clone, PartialEq)]
pub struct CodeBlockData<'a> {
    /// Whether the block came from a ``` or ~~~ fence.
    pub fenced: bool,
    /// The fence character (backtick or tilde) when fenced.
    pub fence_char: u8,
    /// The info string after the opening fence, when present.
    pub info: Option<Cow<'a, str>>,
    /// The literal code text, lines separated by `\n`, ending with `\n`.
    pub literal: Cow<'a, str>,
}

/// The data of a [`Node::HtmlBlock`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlBlockData<'a> {
    /// Which of the seven HTML block kinds this is (1 to 7).
    pub kind: u8,
    /// The raw HTML text, lines separated by `\n`.
    pub literal: Cow<'a, str>,
}

/// The data of a [`Node::Link`].
#[derive(Debug, Clone, PartialEq)]
pub struct LinkData<'a> {
    /// The destination URL, cleaned per the specification.
    pub url: Cow<'a, str>,
    /// The optional title, cleaned per the specification.
    pub title: Option<Cow<'a, str>>,
    /// Whether this is an image (`![text](url)`) rather than a link.
    pub image: bool,
}
