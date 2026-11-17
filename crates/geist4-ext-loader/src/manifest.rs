//! The manifest parser and validator for format version 1.
//!
//! [`parse`] turns `extension.toml` source into a validated
//! [`Manifest`], enforcing every rule in `docs/extensions/format.md`:
//! the manifest schema (identity, hooks, kinds, options), the one-tier rule,
//! the per-pattern behavior rules, and the cross-rules that tie kinds to
//! behavior entries. Errors are one line each — what failed, which key, and
//! what would satisfy the rule — carrying the byte span of the offending
//! value when it is known.
//!
//! Parsing goes through `toml::Spanned` values so every validation error
//! can point at the bytes it concerns, and behavior entries are sorted
//! back into source order so the `Behavior` list is in declaration order
//! even when patterns are interleaved in the document.

use std::collections::{BTreeMap, HashMap};
use std::ops::Range;

use serde::Deserialize;
use toml::Spanned;

use crate::error::LoadError;
use crate::types::{
    AttributeList, Behavior, BlockContainer, Extension, HtmlFilter, InlineSpan, KindEntry,
    Manifest, OptionDecl, OptionType, OptionValue, Replace, Wasm, WikiLink,
};

/// The manifest source is at most 64 KiB, checked before parsing.
const MAX_MANIFEST_BYTES: usize = 64 * 1024;
/// Templates are at most 1 KiB.
const MAX_TEMPLATE_BYTES: usize = 1024;
/// At most 64 kinds per manifest.
const MAX_KINDS: usize = 64;
/// At most 32 options per manifest.
const MAX_OPTIONS: usize = 32;
/// At most 256 behavior entries per manifest.
const MAX_BEHAVIOR_ENTRIES: usize = 256;
/// At most 512 replacements per manifest.
const MAX_REPLACEMENTS: usize = 512;
/// At most 64 denied tags per `[[html_filter]]` entry.
const MAX_FILTER_TAGS: usize = 64;
/// An `[[inline_span]]` delimiter is 1..=16 bytes.
const MAX_SPAN_DELIMITER: usize = 16;
/// A `[[block_container]]` marker is 3..=16 bytes.
const MIN_MARKER_BYTES: usize = 3;
/// A `[[block_container]]` marker is 3..=16 bytes.
const MAX_MARKER_BYTES: usize = 16;
/// A `[[replace]]` `from` is 1..=32 bytes.
const MAX_REPLACE_FROM: usize = 32;
/// A `[[replace]]` `to` is 1..=128 bytes.
const MAX_REPLACE_TO: usize = 128;
/// A `[[wiki_link]]` prefix/suffix is 1..=16 bytes.
const MAX_WIKI_BRACKET: usize = 16;
/// The extension name is 1..=64 bytes.
const MAX_NAME_BYTES: usize = 64;

/// The extension names reserved for the core built-ins.
const RESERVED_NAMES: [&str; 9] = [
    "tables",
    "strikethrough",
    "autolink",
    "tasklist",
    "tagfilter",
    "footnotes",
    "front_matter",
    "heading_identifiers",
    "description_lists",
];

/// The fixed hook vocabulary.
const HOOKS: [&str; 7] = [
    "front_matter",
    "inline_span",
    "inline_bracket",
    "block_container",
    "post_process",
    "render",
    "html_filter",
];

/// Parse and validate `extension.toml` source into a manifest.
///
/// Enforces every format-v1 rule in `docs/extensions/format.md`. The
/// 64 KiB size limit is checked before any parsing, so an oversized
/// manifest is rejected cheaply. The loader's own format version is not
/// consulted here: `parse` accepts any well-shaped `format` and the
/// loader decides whether it matches its own.
pub fn parse(src: &str) -> Result<Manifest, LoadError> {
    if src.len() > MAX_MANIFEST_BYTES {
        return Err(LoadError::new(format!(
            "manifest: {} bytes exceeds the limit of {MAX_MANIFEST_BYTES} bytes (64 KiB)",
            src.len()
        )));
    }

    let raw = match toml::from_str::<RawManifest>(src) {
        Ok(raw) => raw,
        Err(e) => {
            return Err(LoadError {
                message: e.message().to_owned(),
                span: e.span(),
            });
        }
    };

    let raw_extension = raw
        .extension
        .as_ref()
        .ok_or_else(|| LoadError::new("extension: missing; the [extension] table is required"))?;
    let extension = parse_extension(raw_extension)?;
    let (kinds, kind_spans) = parse_kinds(raw.kinds.as_deref())?;
    let options = parse_options(raw.options.as_ref())?;
    let host = parse_host(raw.host.as_ref())?;

    let entries = collect_entries(&raw);
    check_tier(entries.len(), raw.wasm.is_some(), host)?;
    let wasm = parse_wasm(raw.wasm.as_ref())?;
    let behavior = validate_behavior(entries, &kinds, &kind_spans, wasm.is_some(), host)?;

    Ok(Manifest {
        extension,
        kinds,
        options,
        behavior,
        wasm,
        host,
    })
}

/// The manifest as it comes out of the TOML deserializer: every leaf is
/// a raw spanned value so validation can type-check and locate it.
#[derive(Deserialize)]
struct RawManifest {
    extension: Option<Spanned<RawExtension>>,
    kinds: Option<Vec<Spanned<RawKind>>>,
    options: Option<BTreeMap<String, Spanned<RawOption>>>,
    inline_span: Option<Vec<Spanned<RawInlineSpan>>>,
    block_container: Option<Vec<Spanned<RawBlockContainer>>>,
    replace: Option<Vec<Spanned<RawReplace>>>,
    attribute_list: Option<Vec<Spanned<RawAttributeList>>>,
    wiki_link: Option<Vec<Spanned<RawWikiLink>>>,
    html_filter: Option<Vec<Spanned<RawHtmlFilter>>>,
    wasm: Option<Spanned<RawWasm>>,
    host: Option<Spanned<toml::Value>>,
}

/// The `[extension]` table as written.
#[derive(Deserialize)]
struct RawExtension {
    name: Option<Spanned<toml::Value>>,
    version: Option<Spanned<toml::Value>>,
    format: Option<Spanned<toml::Value>>,
    description: Option<Spanned<toml::Value>>,
    license: Option<Spanned<toml::Value>>,
    homepage: Option<Spanned<toml::Value>>,
    hooks: Option<Vec<Spanned<toml::Value>>>,
    after: Option<Vec<Spanned<toml::Value>>>,
    conflicts: Option<Vec<Spanned<toml::Value>>>,
}

/// One `[[kinds]]` entry as written.
#[derive(Deserialize)]
struct RawKind {
    kind: Option<Spanned<toml::Value>>,
    container: Option<Spanned<toml::Value>>,
    inline_text: Option<Spanned<toml::Value>>,
    attrs: Option<Vec<Spanned<toml::Value>>>,
}

/// One `[options.<name>]` table as written.
#[derive(Deserialize)]
struct RawOption {
    #[serde(rename = "type")]
    ty: Option<Spanned<toml::Value>>,
    default: Option<Spanned<toml::Value>>,
    description: Option<Spanned<toml::Value>>,
}

/// One `[[inline_span]]` entry as written.
#[derive(Deserialize)]
struct RawInlineSpan {
    kind: Option<Spanned<toml::Value>>,
    open: Option<Spanned<toml::Value>>,
    close: Option<Spanned<toml::Value>>,
    html: Option<Spanned<toml::Value>>,
    can_open: Option<Spanned<toml::Value>>,
    can_close: Option<Spanned<toml::Value>>,
    multiline: Option<Spanned<toml::Value>>,
    math: Option<Spanned<toml::Value>>,
}

/// One `[[block_container]]` entry as written.
#[derive(Deserialize)]
struct RawBlockContainer {
    kind: Option<Spanned<toml::Value>>,
    marker: Option<Spanned<toml::Value>>,
    name: Option<Spanned<toml::Value>>,
    info_html: Option<Spanned<toml::Value>>,
    html: Option<Spanned<toml::Value>>,
    unclosed_ok: Option<Spanned<toml::Value>>,
}

/// One `[[replace]]` entry as written.
#[derive(Deserialize)]
struct RawReplace {
    from: Option<Spanned<toml::Value>>,
    to: Option<Spanned<toml::Value>>,
    once: Option<Spanned<toml::Value>>,
}

/// One `[[attribute_list]]` entry as written.
#[derive(Deserialize)]
struct RawAttributeList {
    kind: Option<Spanned<toml::Value>>,
    prefix: Option<Spanned<toml::Value>>,
    suffix: Option<Spanned<toml::Value>>,
}

/// One `[[wiki_link]]` entry as written.
#[derive(Deserialize)]
struct RawWikiLink {
    kind: Option<Spanned<toml::Value>>,
    prefix: Option<Spanned<toml::Value>>,
    suffix: Option<Spanned<toml::Value>>,
    html: Option<Spanned<toml::Value>>,
}

/// One `[[html_filter]]` entry as written.
#[derive(Deserialize)]
struct RawHtmlFilter {
    deny: Option<Vec<Spanned<toml::Value>>>,
}

/// The `[wasm]` section as written.
#[derive(Deserialize)]
struct RawWasm {
    path: Option<Spanned<toml::Value>>,
    fuel: Option<Spanned<toml::Value>>,
    memory_pages: Option<Spanned<toml::Value>>,
    inline_triggers: Option<Vec<Spanned<toml::Value>>>,
}

/// A behavior entry as written, before validation.
///
/// The index is the entry's position within its own pattern array, for
/// the error message's key path.
enum RawEntry<'a> {
    InlineSpan {
        index: usize,
        entry: &'a Spanned<RawInlineSpan>,
    },
    BlockContainer {
        index: usize,
        entry: &'a Spanned<RawBlockContainer>,
    },
    Replace {
        index: usize,
        entry: &'a Spanned<RawReplace>,
    },
    AttributeList {
        index: usize,
        entry: &'a Spanned<RawAttributeList>,
    },
    WikiLink {
        index: usize,
        entry: &'a Spanned<RawWikiLink>,
    },
    HtmlFilter {
        index: usize,
        entry: &'a Spanned<RawHtmlFilter>,
    },
}

impl RawEntry<'_> {
    /// The entry's byte span in the manifest source.
    fn span(&self) -> Range<usize> {
        match self {
            RawEntry::InlineSpan { entry, .. } => entry.span(),
            RawEntry::BlockContainer { entry, .. } => entry.span(),
            RawEntry::Replace { entry, .. } => entry.span(),
            RawEntry::AttributeList { entry, .. } => entry.span(),
            RawEntry::WikiLink { entry, .. } => entry.span(),
            RawEntry::HtmlFilter { entry, .. } => entry.span(),
        }
    }
}

/// An error at a byte span.
fn err(span: Option<Range<usize>>, message: impl std::fmt::Display) -> LoadError {
    LoadError {
        message: message.to_string(),
        span,
    }
}

/// A required leaf value, or a "missing" error naming the key.
fn leaf<'a>(
    value: &'a Option<Spanned<toml::Value>>,
    enclosing: Range<usize>,
    key: &str,
    rule: &str,
) -> Result<&'a Spanned<toml::Value>, LoadError> {
    value
        .as_ref()
        .ok_or_else(|| err(Some(enclosing), format!("{key}: missing; {rule}")))
}

/// The value's string content, or a type error naming the key.
fn spanned_string<'a>(value: &'a Spanned<toml::Value>, key: &str) -> Result<&'a str, LoadError> {
    value
        .get_ref()
        .as_str()
        .ok_or_else(|| err(Some(value.span()), format!("{key}: must be a string")))
}

/// The value's integer content, or a type error naming the key.
fn spanned_int(value: &Spanned<toml::Value>, key: &str) -> Result<i64, LoadError> {
    value
        .get_ref()
        .as_integer()
        .ok_or_else(|| err(Some(value.span()), format!("{key}: must be an integer")))
}

/// The value's boolean content, or a type error naming the key.
fn opt_bool(value: &Option<Spanned<toml::Value>>, key: &str) -> Result<Option<bool>, LoadError> {
    match value {
        None => Ok(None),
        Some(v) => v
            .get_ref()
            .as_bool()
            .map(Some)
            .ok_or_else(|| err(Some(v.span()), format!("{key}: must be a boolean"))),
    }
}

/// An array of strings, or a type error naming each offending element.
fn string_list(list: Option<&[Spanned<toml::Value>]>, key: &str) -> Result<Vec<String>, LoadError> {
    let Some(list) = list else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(list.len());
    for (i, v) in list.iter().enumerate() {
        out.push(spanned_string(v, &format!("{key}[{i}]"))?.to_owned());
    }
    Ok(out)
}

/// `[a-z][a-z0-9-]*`
fn is_lower_dash_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    match bytes.first() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    bytes[1..]
        .iter()
        .all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

/// `[a-z][a-z0-9_-]*`
fn is_lower_word_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    match bytes.first() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    bytes[1..]
        .iter()
        .all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'_')
}

/// `[a-zA-Z][a-zA-Z0-9-]*`
fn is_tag_name(tag: &str) -> bool {
    let bytes = tag.as_bytes();
    match bytes.first() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    bytes[1..]
        .iter()
        .all(|&c| c.is_ascii_alphanumeric() || c == b'-')
}

/// Semver `X.Y[.Z]`: two or three dot-separated numeric components
/// without leading zeros.
fn is_semver_shape(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    (parts.len() == 2 || parts.len() == 3) && parts.iter().all(|p| is_semver_number(p))
}

/// One numeric semver component.
fn is_semver_number(part: &str) -> bool {
    !part.is_empty()
        && part.bytes().all(|b| b.is_ascii_digit())
        && (part == "0" || !part.starts_with('0'))
}

/// A well-formed SPDX expression: license identifiers joined by `AND`
/// and `OR`, with `WITH` exceptions and parentheses for grouping.
///
/// Which licenses the identifiers name is not checked.
fn is_spdx_expression(expr: &str) -> bool {
    let Ok(tokens) = tokenize_spdx(expr) else {
        return false;
    };
    let mut pos = 0;
    spdx_expr(&tokens, &mut pos) && pos == tokens.len()
}

/// One SPDX expression token.
#[derive(Debug, PartialEq)]
enum SPDXToken {
    /// A license or exception identifier.
    Ident(String),
    /// The `AND` operator.
    And,
    /// The `OR` operator.
    Or,
    /// The `WITH` exception keyword.
    With,
    /// An opening parenthesis.
    Open,
    /// A closing parenthesis.
    Close,
}

/// Split an SPDX expression into tokens; `Err` on a bad character.
fn tokenize_spdx(expr: &str) -> Result<Vec<SPDXToken>, ()> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = expr.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '(' {
            tokens.push(SPDXToken::Open);
            i += 1;
        } else if c == ')' {
            tokens.push(SPDXToken::Close);
            i += 1;
        } else if c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '+' {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric()
                    || chars[i] == '-'
                    || chars[i] == '.'
                    || chars[i] == '+')
            {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            tokens.push(match word.as_str() {
                "AND" => SPDXToken::And,
                "OR" => SPDXToken::Or,
                "WITH" => SPDXToken::With,
                _ => SPDXToken::Ident(word),
            });
        } else {
            return Err(());
        }
    }
    Ok(tokens)
}

/// SPDX grammar: `term ((AND | OR) term)*`.
fn spdx_expr(tokens: &[SPDXToken], pos: &mut usize) -> bool {
    if !spdx_term(tokens, pos) {
        return false;
    }
    while *pos < tokens.len() {
        match tokens[*pos] {
            SPDXToken::And | SPDXToken::Or => {
                *pos += 1;
                if !spdx_term(tokens, pos) {
                    return false;
                }
            }
            SPDXToken::Close => break,
            _ => return false,
        }
    }
    true
}

/// SPDX grammar: `ident [WITH ident] | ( expr )`.
fn spdx_term(tokens: &[SPDXToken], pos: &mut usize) -> bool {
    match tokens.get(*pos) {
        Some(SPDXToken::Open) => {
            *pos += 1;
            if !spdx_expr(tokens, pos) {
                return false;
            }
            match tokens.get(*pos) {
                Some(SPDXToken::Close) => {
                    *pos += 1;
                    true
                }
                _ => false,
            }
        }
        Some(SPDXToken::Ident(_)) => {
            *pos += 1;
            if matches!(tokens.get(*pos), Some(SPDXToken::With)) {
                *pos += 1;
                if !matches!(tokens.get(*pos), Some(SPDXToken::Ident(_))) {
                    return false;
                }
                *pos += 1;
            }
            true
        }
        _ => false,
    }
}

/// Check a render template: at most 1 KiB, and every `{..}` substitution
/// a name `is_allowed` accepts.
fn check_template(
    template: &str,
    span: Range<usize>,
    key: &str,
    is_allowed: &dyn Fn(&str) -> bool,
    allowed_desc: &str,
) -> Result<(), LoadError> {
    if template.len() > MAX_TEMPLATE_BYTES {
        return Err(err(
            Some(span),
            format!(
                "{key}: template is {} bytes; the limit is {MAX_TEMPLATE_BYTES} bytes (1 KiB)",
                template.len()
            ),
        ));
    }
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            let Some(rel) = template[i + 1..].find('}') else {
                // No closing brace: the rest is literal text.
                break;
            };
            let name = &template[i + 1..i + 1 + rel];
            if !is_allowed(name) {
                return Err(err(
                    Some(span),
                    format!("{key}: unknown substitution \"{{{name}}}\"; allowed: {allowed_desc}"),
                ));
            }
            i += rel + 2;
        } else {
            i += 1;
        }
    }
    Ok(())
}

/// The `[extension]` table: identity, hooks and ordering metadata.
fn parse_extension(ext: &Spanned<RawExtension>) -> Result<Extension, LoadError> {
    let raw = ext.get_ref();
    let table_span = ext.span();

    let name_v = leaf(
        &raw.name,
        table_span.clone(),
        "extension.name",
        "a name matching [a-z][a-z0-9-]* of at most 64 bytes is required",
    )?;
    let name = spanned_string(name_v, "extension.name")?;
    if RESERVED_NAMES.contains(&name) {
        return Err(err(
            Some(name_v.span()),
            format!("extension.name: \"{name}\" is reserved for a built-in extension"),
        ));
    }
    if !is_lower_dash_name(name) || name.len() > MAX_NAME_BYTES {
        return Err(err(
            Some(name_v.span()),
            "extension.name: must match [a-z][a-z0-9-]* and be 1-64 bytes",
        ));
    }

    let version_v = leaf(
        &raw.version,
        table_span.clone(),
        "extension.version",
        "a semver version like 1.0 or 1.2.3 is required",
    )?;
    let version = spanned_string(version_v, "extension.version")?;
    if !is_semver_shape(version) {
        return Err(err(
            Some(version_v.span()),
            "extension.version: must be a semver version like 1.0 or 1.2.3",
        ));
    }

    let format_v = leaf(
        &raw.format,
        table_span.clone(),
        "extension.format",
        "a format version like 1.0 is required",
    )?;
    let format = spanned_string(format_v, "extension.format")?;
    if !is_semver_shape(format) {
        return Err(err(
            Some(format_v.span()),
            "extension.format: must be a semver version like 1.0",
        ));
    }

    let description_v = leaf(
        &raw.description,
        table_span.clone(),
        "extension.description",
        "a one-line description is required",
    )?;
    let description = spanned_string(description_v, "extension.description")?;

    let license_v = leaf(
        &raw.license,
        table_span.clone(),
        "extension.license",
        "an SPDX expression like MIT OR Apache-2.0 is required",
    )?;
    let license = spanned_string(license_v, "extension.license")?;
    if !is_spdx_expression(license) {
        return Err(err(
            Some(license_v.span()),
            "extension.license: must be a well-formed SPDX expression like MIT OR Apache-2.0",
        ));
    }

    let homepage = match &raw.homepage {
        Some(v) => Some(spanned_string(v, "extension.homepage")?.to_owned()),
        None => None,
    };

    let hooks = match &raw.hooks {
        Some(list) => {
            let mut hooks = Vec::with_capacity(list.len());
            for (i, hv) in list.iter().enumerate() {
                let hook = spanned_string(hv, &format!("extension.hooks[{i}]"))?;
                if !HOOKS.contains(&hook) {
                    return Err(err(
                        Some(hv.span()),
                        format!(
                            "extension.hooks[{i}]: unknown hook \"{hook}\"; one of {}",
                            HOOKS.join(", ")
                        ),
                    ));
                }
                hooks.push(hook.to_owned());
            }
            hooks
        }
        None => Vec::new(),
    };

    let after = string_list(raw.after.as_deref(), "extension.after")?;
    let conflicts = string_list(raw.conflicts.as_deref(), "extension.conflicts")?;

    Ok(Extension {
        name: name.to_owned(),
        version: version.to_owned(),
        format: format.to_owned(),
        description: description.to_owned(),
        license: license.to_owned(),
        homepage,
        hooks,
        after,
        conflicts,
    })
}

/// The `[[kinds]]` entries, in declaration order, with each entry's
/// source span for the cross-rule errors later.
fn parse_kinds(
    kinds: Option<&[Spanned<RawKind>]>,
) -> Result<(Vec<KindEntry>, Vec<Range<usize>>), LoadError> {
    let Some(list) = kinds else {
        return Ok((Vec::new(), Vec::new()));
    };
    if list.len() > MAX_KINDS {
        return Err(err(
            None,
            format!(
                "kinds: {} kinds exceeds the limit of {MAX_KINDS}",
                list.len()
            ),
        ));
    }
    let mut seen: HashMap<&str, usize> = HashMap::with_capacity(list.len());
    let mut spans = Vec::with_capacity(list.len());
    let mut out = Vec::with_capacity(list.len());
    for (i, entry) in list.iter().enumerate() {
        let entry_span = entry.span();
        let raw = entry.get_ref();
        let kind_v = leaf(
            &raw.kind,
            entry_span.clone(),
            &format!("kinds[{i}].kind"),
            "a kind name matching [a-z][a-z0-9-]* is required",
        )?;
        let kind = spanned_string(kind_v, &format!("kinds[{i}].kind"))?;
        if !is_lower_dash_name(kind) {
            return Err(err(
                Some(kind_v.span()),
                format!("kinds[{i}].kind: must match [a-z][a-z0-9-]*"),
            ));
        }
        if seen.insert(kind, i).is_some() {
            return Err(err(
                Some(kind_v.span()),
                format!(
                    "kinds[{i}].kind: duplicate kind \"{kind}\"; kinds must be unique within the manifest"
                ),
            ));
        }
        let container =
            opt_bool(&raw.container, &format!("kinds[{i}].container"))?.unwrap_or(false);
        let inline_text =
            opt_bool(&raw.inline_text, &format!("kinds[{i}].inline_text"))?.unwrap_or(true);
        let mut attrs = Vec::new();
        if let Some(list) = &raw.attrs {
            for (j, av) in list.iter().enumerate() {
                let attr = spanned_string(av, &format!("kinds[{i}].attrs[{j}]"))?;
                if !is_lower_word_name(attr) {
                    return Err(err(
                        Some(av.span()),
                        format!("kinds[{i}].attrs[{j}]: must match [a-z][a-z0-9_-]*"),
                    ));
                }
                attrs.push(attr.to_owned());
            }
        }
        spans.push(entry_span);
        out.push(KindEntry {
            kind: kind.to_owned(),
            container,
            inline_text,
            attrs,
        });
    }
    Ok((out, spans))
}

/// The `[options.*]` tables, keyed by option name.
fn parse_options(
    options: Option<&BTreeMap<String, Spanned<RawOption>>>,
) -> Result<BTreeMap<String, OptionDecl>, LoadError> {
    let mut out = BTreeMap::new();
    let Some(map) = options else {
        return Ok(out);
    };
    if map.len() > MAX_OPTIONS {
        return Err(err(
            None,
            format!(
                "options: {} options exceeds the limit of {MAX_OPTIONS}",
                map.len()
            ),
        ));
    }
    for (name, entry) in map {
        if !is_lower_word_name(name) {
            return Err(err(
                Some(entry.span()),
                format!("options.{name}: must match [a-z][a-z0-9_-]*"),
            ));
        }
        let raw = entry.get_ref();
        let ty_v = leaf(
            &raw.ty,
            entry.span(),
            &format!("options.{name}.type"),
            "one of string, bool, number is required",
        )?;
        let ty_str = spanned_string(ty_v, &format!("options.{name}.type"))?;
        let ty = match ty_str {
            "string" => OptionType::String,
            "bool" => OptionType::Bool,
            "number" => OptionType::Number,
            other => {
                return Err(err(
                    Some(ty_v.span()),
                    format!(
                        "options.{name}.type: \"{other}\" is not a known type; one of string, bool, number"
                    ),
                ));
            }
        };
        let default_v = leaf(
            &raw.default,
            entry.span(),
            &format!("options.{name}.default"),
            &format!("a default of type {ty_str} is required"),
        )?;
        let default = option_value(default_v, ty, &format!("options.{name}.default"))?;
        let description = match &raw.description {
            Some(dv) => spanned_string(dv, &format!("options.{name}.description"))?.to_owned(),
            None => String::new(),
        };
        out.insert(
            name.clone(),
            OptionDecl {
                ty,
                default,
                description,
            },
        );
    }
    Ok(out)
}

/// The option default, type-checked against the declared type.
fn option_value(
    value: &Spanned<toml::Value>,
    ty: OptionType,
    key: &str,
) -> Result<OptionValue, LoadError> {
    let v = value.get_ref();
    let bad = |wanted: &str| err(Some(value.span()), format!("{key}: must be {wanted}"));
    match ty {
        OptionType::String => v
            .as_str()
            .map(|s| OptionValue::String(s.to_owned()))
            .ok_or_else(|| bad("a string")),
        OptionType::Bool => v
            .as_bool()
            .map(OptionValue::Bool)
            .ok_or_else(|| bad("a boolean")),
        OptionType::Number => match v {
            toml::Value::Integer(i) => Ok(OptionValue::Number(*i as f64)),
            toml::Value::Float(f) => Ok(OptionValue::Number(*f)),
            _ => Err(bad("a number")),
        },
    }
}

/// The tier-3 marker: `host = true` or a `[host]` table.
fn parse_host(host: Option<&Spanned<toml::Value>>) -> Result<bool, LoadError> {
    match host {
        None => Ok(false),
        Some(v) => match v.get_ref() {
            toml::Value::Boolean(b) => Ok(*b),
            toml::Value::Table(_) => Ok(true),
            _ => Err(err(Some(v.span()), "host: must be true or a [host] table")),
        },
    }
}

/// Exactly one behavior tier: tier-1 entries, `[wasm]`, or `[host]`.
fn check_tier(entries: usize, wasm: bool, host: bool) -> Result<(), LoadError> {
    let mut declared: Vec<&str> = Vec::new();
    if entries > 0 {
        declared.push("tier-1 behavior entries");
    }
    if wasm {
        declared.push("[wasm]");
    }
    if host {
        declared.push("[host]");
    }
    match declared.len() {
        0 => Err(err(
            None,
            "behavior: the manifest declares no behavior tier; add tier-1 behavior entries, a [wasm] section, or [host]",
        )),
        1 => Ok(()),
        _ => Err(err(
            None,
            format!(
                "behavior: a manifest declares exactly one behavior tier; found {}",
                declared.join(", ")
            ),
        )),
    }
}

/// The `[wasm]` section: a non-empty path and positive, in-range limits.
fn parse_wasm(wasm: Option<&Spanned<RawWasm>>) -> Result<Option<Wasm>, LoadError> {
    let Some(w) = wasm else {
        return Ok(None);
    };
    let raw = w.get_ref();
    let path_v = leaf(
        &raw.path,
        w.span(),
        "wasm.path",
        "the component file path is required",
    )?;
    let path = spanned_string(path_v, "wasm.path")?;
    if path.is_empty() {
        return Err(err(
            Some(path_v.span()),
            "wasm.path: must be a non-empty path to the component file",
        ));
    }
    let fuel = match &raw.fuel {
        None => None,
        Some(fv) => {
            let units = spanned_int(fv, "wasm.fuel")?;
            if units <= 0 {
                return Err(err(
                    Some(fv.span()),
                    "wasm.fuel: must be a positive number of fuel units",
                ));
            }
            Some(units as u64)
        }
    };
    let memory_pages = match &raw.memory_pages {
        None => None,
        Some(mv) => {
            let pages = spanned_int(mv, "wasm.memory_pages")?;
            if pages <= 0 {
                return Err(err(
                    Some(mv.span()),
                    "wasm.memory_pages: must be a positive number of memory pages",
                ));
            }
            if pages > u32::MAX as i64 {
                return Err(err(
                    Some(mv.span()),
                    "wasm.memory_pages: must be at most 4294967295 pages",
                ));
            }
            Some(pages as u32)
        }
    };
    let inline_triggers = parse_inline_triggers(raw.inline_triggers.as_deref())?;
    Ok(Some(Wasm {
        path: path.to_owned(),
        fuel,
        memory_pages,
        inline_triggers,
    }))
}

/// The `[wasm] inline_triggers` list: the bytes the engine registers
/// for the component's `inline-trigger` hook.
fn parse_inline_triggers(list: Option<&[Spanned<toml::Value>]>) -> Result<Vec<u8>, LoadError> {
    let Some(list) = list else {
        return Ok(Vec::new());
    };
    if list.len() > 32 {
        return Err(err(
            Some(list[32].span()),
            "wasm.inline_triggers: at most 32 trigger bytes",
        ));
    }
    let mut bytes = Vec::with_capacity(list.len());
    for item in list {
        let value = spanned_string(item, "wasm.inline_triggers")?;
        let raw = value.as_bytes();
        if raw.len() != 1 || !raw[0].is_ascii_graphic() {
            return Err(err(
                Some(item.span()),
                "wasm.inline_triggers: each trigger must be one printable ASCII byte",
            ));
        }
        if is_core_trigger_byte(raw[0]) {
            return Err(err(
                Some(item.span()),
                "wasm.inline_triggers: the core's own trigger bytes never reach the hook",
            ));
        }
        if bytes.contains(&raw[0]) {
            return Err(err(
                Some(item.span()),
                "wasm.inline_triggers: each trigger byte is declared once",
            ));
        }
        bytes.push(raw[0]);
    }
    Ok(bytes)
}

/// The core's built-in inline trigger bytes, which an extension can
/// never claim.
fn is_core_trigger_byte(byte: u8) -> bool {
    matches!(
        byte,
        b'\\' | b'`' | b'&' | b'<' | b'*' | b'_' | b'[' | b'!' | b']'
    )
}

/// Collect every behavior entry, across patterns, as written.
fn collect_entries(raw: &RawManifest) -> Vec<RawEntry<'_>> {
    let mut entries = Vec::new();
    if let Some(list) = &raw.inline_span {
        for (index, entry) in list.iter().enumerate() {
            entries.push(RawEntry::InlineSpan { index, entry });
        }
    }
    if let Some(list) = &raw.block_container {
        for (index, entry) in list.iter().enumerate() {
            entries.push(RawEntry::BlockContainer { index, entry });
        }
    }
    if let Some(list) = &raw.replace {
        for (index, entry) in list.iter().enumerate() {
            entries.push(RawEntry::Replace { index, entry });
        }
    }
    if let Some(list) = &raw.attribute_list {
        for (index, entry) in list.iter().enumerate() {
            entries.push(RawEntry::AttributeList { index, entry });
        }
    }
    if let Some(list) = &raw.wiki_link {
        for (index, entry) in list.iter().enumerate() {
            entries.push(RawEntry::WikiLink { index, entry });
        }
    }
    if let Some(list) = &raw.html_filter {
        for (index, entry) in list.iter().enumerate() {
            entries.push(RawEntry::HtmlFilter { index, entry });
        }
    }
    entries
}

/// Validate the behavior entries against the kinds and each pattern's
/// rules, returning them in declaration (source) order. A tier-2 or
/// tier-3 manifest declares no entries; its kinds are used by the
/// component or the host instead, so the every-kind-used check does
/// not apply there.
fn validate_behavior(
    mut entries: Vec<RawEntry<'_>>,
    kinds: &[KindEntry],
    kind_spans: &[Range<usize>],
    wasm: bool,
    host: bool,
) -> Result<Vec<Behavior>, LoadError> {
    let replaces = entries
        .iter()
        .filter(|e| matches!(e, RawEntry::Replace { .. }))
        .count();
    if replaces > MAX_REPLACEMENTS {
        return Err(err(
            None,
            format!("replace: {replaces} entries exceeds the limit of {MAX_REPLACEMENTS}"),
        ));
    }
    if entries.len() > MAX_BEHAVIOR_ENTRIES {
        return Err(err(
            None,
            format!(
                "behavior: {} entries exceeds the limit of {MAX_BEHAVIOR_ENTRIES}",
                entries.len()
            ),
        ));
    }

    // The deserialized struct groups entries by pattern; sort them back
    // into source order so the behavior list is in declaration order.
    entries.sort_by_key(|e| e.span().start);

    let kind_index: HashMap<&str, usize> = kinds
        .iter()
        .enumerate()
        .map(|(i, k)| (k.kind.as_str(), i))
        .collect();
    let mut used = vec![false; kinds.len()];

    let mut built = Vec::with_capacity(entries.len());
    for entry in &entries {
        let behavior = match entry {
            RawEntry::InlineSpan { index, entry } => {
                validate_inline_span(entry, *index, kinds, &kind_index, &mut used)?
            }
            RawEntry::BlockContainer { index, entry } => {
                validate_block_container(entry, *index, kinds, &kind_index, &mut used)?
            }
            RawEntry::Replace { index, entry } => validate_replace(entry, *index)?,
            RawEntry::AttributeList { index, entry } => {
                validate_attribute_list(entry, *index, &kind_index, &mut used)?
            }
            RawEntry::WikiLink { index, entry } => {
                validate_wiki_link(entry, *index, &kind_index, &mut used)?
            }
            RawEntry::HtmlFilter { index, entry } => validate_html_filter(entry, *index)?,
        };
        built.push(behavior);
    }

    if !wasm && !host {
        for (i, kind) in kinds.iter().enumerate() {
            if !used[i] {
                return Err(err(
                    Some(kind_spans[i].clone()),
                    format!(
                        "kinds[{i}].kind: kind \"{}\" is declared but no behavior entry uses it; every kind is used by exactly one behavior entry",
                        kind.kind
                    ),
                ));
            }
        }
    }
    Ok(built)
}

/// Claim a kind for one behavior entry: it must exist and not already be
/// used by another entry. Returns the kind's index.
fn claim_kind(
    kind: &str,
    kind_span: Range<usize>,
    key: &str,
    kind_index: &HashMap<&str, usize>,
    used: &mut [bool],
) -> Result<usize, LoadError> {
    let Some(&i) = kind_index.get(kind) else {
        return Err(err(
            Some(kind_span),
            format!(
                "{key}: kind \"{kind}\" is not declared in [[kinds]]; every behavior entry references a declared kind"
            ),
        ));
    };
    if used[i] {
        return Err(err(
            Some(kind_span),
            format!(
                "{key}: kind \"{kind}\" is already used by another behavior entry; every kind is used by exactly one behavior entry"
            ),
        ));
    }
    used[i] = true;
    Ok(i)
}

/// Validate one `[[inline_span]]` entry.
fn validate_inline_span(
    entry: &Spanned<RawInlineSpan>,
    index: usize,
    kinds: &[KindEntry],
    kind_index: &HashMap<&str, usize>,
    used: &mut [bool],
) -> Result<Behavior, LoadError> {
    let raw = entry.get_ref();
    let key = format!("inline_span[{index}]");

    let kind_v = leaf(
        &raw.kind,
        entry.span(),
        &format!("{key}.kind"),
        "the kind this span wraps is required",
    )?;
    let kind = spanned_string(kind_v, &format!("{key}.kind"))?;
    let kind_i = claim_kind(
        kind,
        kind_v.span(),
        &format!("{key}.kind"),
        kind_index,
        used,
    )?;
    let attrs = &kinds[kind_i].attrs;

    let open_v = leaf(
        &raw.open,
        entry.span(),
        &format!("{key}.open"),
        &format!("an opening delimiter of 1-{MAX_SPAN_DELIMITER} bytes is required"),
    )?;
    let open = spanned_string(open_v, &format!("{key}.open"))?;
    if open.is_empty() || open.len() > MAX_SPAN_DELIMITER {
        return Err(err(
            Some(open_v.span()),
            format!("{key}.open: must be 1-{MAX_SPAN_DELIMITER} bytes"),
        ));
    }
    let close_v = leaf(
        &raw.close,
        entry.span(),
        &format!("{key}.close"),
        &format!("a closing delimiter of 1-{MAX_SPAN_DELIMITER} bytes is required"),
    )?;
    let close = spanned_string(close_v, &format!("{key}.close"))?;
    if close.is_empty() || close.len() > MAX_SPAN_DELIMITER {
        return Err(err(
            Some(close_v.span()),
            format!("{key}.close: must be 1-{MAX_SPAN_DELIMITER} bytes"),
        ));
    }

    let html_v = leaf(
        &raw.html,
        entry.span(),
        &format!("{key}.html"),
        "a render template is required",
    )?;
    let html = spanned_string(html_v, &format!("{key}.html"))?;
    let allowed = |name: &str| name == "children" || attrs.iter().any(|a| a == name);
    let allowed_desc = if attrs.is_empty() {
        "{children}".to_owned()
    } else {
        format!(
            "{{children}} or one of the kind's attributes ({})",
            attrs.join(", ")
        )
    };
    check_template(
        html,
        html_v.span(),
        &format!("{key}.html"),
        &allowed,
        &allowed_desc,
    )?;
    if kinds[kind_i].inline_text && !html.contains("{children}") {
        return Err(err(
            Some(html_v.span()),
            format!(
                "{key}.html: the template has no {{children}} substitution; an inline_text kind would drop its children"
            ),
        ));
    }

    Ok(Behavior::InlineSpan(InlineSpan {
        kind: kind.to_owned(),
        open: open.to_owned(),
        close: close.to_owned(),
        html: html.to_owned(),
        can_open: opt_bool(&raw.can_open, &format!("{key}.can_open"))?.unwrap_or(true),
        can_close: opt_bool(&raw.can_close, &format!("{key}.can_close"))?.unwrap_or(true),
        multiline: opt_bool(&raw.multiline, &format!("{key}.multiline"))?.unwrap_or(true),
        math: opt_bool(&raw.math, &format!("{key}.math"))?.unwrap_or(false),
    }))
}

/// Validate one `[[block_container]]` entry.
fn validate_block_container(
    entry: &Spanned<RawBlockContainer>,
    index: usize,
    kinds: &[KindEntry],
    kind_index: &HashMap<&str, usize>,
    used: &mut [bool],
) -> Result<Behavior, LoadError> {
    let raw = entry.get_ref();
    let key = format!("block_container[{index}]");

    let kind_v = leaf(
        &raw.kind,
        entry.span(),
        &format!("{key}.kind"),
        "the kind of the container node is required",
    )?;
    let kind = spanned_string(kind_v, &format!("{key}.kind"))?;
    let kind_i = claim_kind(
        kind,
        kind_v.span(),
        &format!("{key}.kind"),
        kind_index,
        used,
    )?;
    if !kinds[kind_i].container {
        return Err(err(
            Some(kind_v.span()),
            format!("{key}.kind: kind \"{kind}\" must be a container kind (container = true)"),
        ));
    }

    let marker_v = leaf(
        &raw.marker,
        entry.span(),
        &format!("{key}.marker"),
        &format!(
            "a marker of {MIN_MARKER_BYTES}-{MAX_MARKER_BYTES} bytes of one repeated character is required"
        ),
    )?;
    let marker = spanned_string(marker_v, &format!("{key}.marker"))?;
    let mut chars = marker.chars();
    if !(MIN_MARKER_BYTES..=MAX_MARKER_BYTES).contains(&marker.len())
        || !chars.next().is_some_and(|first| chars.all(|c| c == first))
    {
        return Err(err(
            Some(marker_v.span()),
            format!(
                "{key}.marker: must be {MIN_MARKER_BYTES}-{MAX_MARKER_BYTES} bytes of one repeated character"
            ),
        ));
    }

    let name_v = leaf(
        &raw.name,
        entry.span(),
        &format!("{key}.name"),
        "the opening word after the marker is required",
    )?;
    let name = spanned_string(name_v, &format!("{key}.name"))?;
    if !is_lower_dash_name(name) {
        return Err(err(
            Some(name_v.span()),
            format!("{key}.name: must match [a-z][a-z0-9-]*"),
        ));
    }

    let info_html = match &raw.info_html {
        Some(v) => {
            let info_html = spanned_string(v, &format!("{key}.info_html"))?;
            check_template(
                info_html,
                v.span(),
                &format!("{key}.info_html"),
                &|name: &str| name == "info",
                "{info}",
            )?;
            info_html.to_owned()
        }
        None => String::new(),
    };

    let html_v = leaf(
        &raw.html,
        entry.span(),
        &format!("{key}.html"),
        "a render template is required",
    )?;
    let html = spanned_string(html_v, &format!("{key}.html"))?;
    check_template(
        html,
        html_v.span(),
        &format!("{key}.html"),
        &|name: &str| name == "children" || name == "info",
        "{{children}} and {{info}}",
    )?;

    Ok(Behavior::BlockContainer(BlockContainer {
        kind: kind.to_owned(),
        marker: marker.to_owned(),
        name: name.to_owned(),
        info_html,
        html: html.to_owned(),
        unclosed_ok: opt_bool(&raw.unclosed_ok, &format!("{key}.unclosed_ok"))?.unwrap_or(true),
    }))
}

/// Validate one `[[replace]]` entry.
fn validate_replace(entry: &Spanned<RawReplace>, index: usize) -> Result<Behavior, LoadError> {
    let raw = entry.get_ref();
    let key = format!("replace[{index}]");

    let from_v = leaf(
        &raw.from,
        entry.span(),
        &format!("{key}.from"),
        &format!("the literal to find, 1-{MAX_REPLACE_FROM} bytes with no newline, is required"),
    )?;
    let from = spanned_string(from_v, &format!("{key}.from"))?;
    if from.is_empty() || from.len() > MAX_REPLACE_FROM {
        return Err(err(
            Some(from_v.span()),
            format!("{key}.from: must be 1-{MAX_REPLACE_FROM} bytes"),
        ));
    }
    if from.contains('\n') {
        return Err(err(
            Some(from_v.span()),
            format!("{key}.from: must not contain a newline"),
        ));
    }

    let to_v = leaf(
        &raw.to,
        entry.span(),
        &format!("{key}.to"),
        &format!("the replacement output, 1-{MAX_REPLACE_TO} bytes, is required"),
    )?;
    let to = spanned_string(to_v, &format!("{key}.to"))?;
    if to.is_empty() || to.len() > MAX_REPLACE_TO {
        return Err(err(
            Some(to_v.span()),
            format!("{key}.to: must be 1-{MAX_REPLACE_TO} bytes"),
        ));
    }

    Ok(Behavior::Replace(Replace {
        from: from.to_owned(),
        to: to.to_owned(),
        once: opt_bool(&raw.once, &format!("{key}.once"))?.unwrap_or(true),
    }))
}

/// Validate one `[[attribute_list]]` entry.
fn validate_attribute_list(
    entry: &Spanned<RawAttributeList>,
    index: usize,
    kind_index: &HashMap<&str, usize>,
    used: &mut [bool],
) -> Result<Behavior, LoadError> {
    let raw = entry.get_ref();
    let key = format!("attribute_list[{index}]");

    let kind_v = leaf(
        &raw.kind,
        entry.span(),
        &format!("{key}.kind"),
        "the kind whose renderer receives the attributes is required",
    )?;
    let kind = spanned_string(kind_v, &format!("{key}.kind"))?;
    claim_kind(
        kind,
        kind_v.span(),
        &format!("{key}.kind"),
        kind_index,
        used,
    )?;

    let prefix_v = leaf(
        &raw.prefix,
        entry.span(),
        &format!("{key}.prefix"),
        "a list opener starting with { is required",
    )?;
    let prefix = spanned_string(prefix_v, &format!("{key}.prefix"))?;
    if !prefix.starts_with('{') {
        return Err(err(
            Some(prefix_v.span()),
            format!("{key}.prefix: must start with \"{{\""),
        ));
    }

    let suffix_v = leaf(
        &raw.suffix,
        entry.span(),
        &format!("{key}.suffix"),
        "a list closer is required",
    )?;
    let suffix = spanned_string(suffix_v, &format!("{key}.suffix"))?;
    if suffix.is_empty() {
        return Err(err(
            Some(suffix_v.span()),
            format!("{key}.suffix: must not be empty"),
        ));
    }

    Ok(Behavior::AttributeList(AttributeList {
        kind: kind.to_owned(),
        prefix: prefix.to_owned(),
        suffix: suffix.to_owned(),
    }))
}

/// Validate one `[[wiki_link]]` entry.
fn validate_wiki_link(
    entry: &Spanned<RawWikiLink>,
    index: usize,
    kind_index: &HashMap<&str, usize>,
    used: &mut [bool],
) -> Result<Behavior, LoadError> {
    let raw = entry.get_ref();
    let key = format!("wiki_link[{index}]");

    let kind_v = leaf(
        &raw.kind,
        entry.span(),
        &format!("{key}.kind"),
        "the kind of the node is required",
    )?;
    let kind = spanned_string(kind_v, &format!("{key}.kind"))?;
    claim_kind(
        kind,
        kind_v.span(),
        &format!("{key}.kind"),
        kind_index,
        used,
    )?;

    let prefix_v = leaf(
        &raw.prefix,
        entry.span(),
        &format!("{key}.prefix"),
        &format!("a bracket prefix of 1-{MAX_WIKI_BRACKET} bytes is required"),
    )?;
    let prefix = spanned_string(prefix_v, &format!("{key}.prefix"))?;
    if prefix.is_empty() || prefix.len() > MAX_WIKI_BRACKET {
        return Err(err(
            Some(prefix_v.span()),
            format!("{key}.prefix: must be 1-{MAX_WIKI_BRACKET} bytes"),
        ));
    }

    let suffix_v = leaf(
        &raw.suffix,
        entry.span(),
        &format!("{key}.suffix"),
        &format!("a bracket suffix of 1-{MAX_WIKI_BRACKET} bytes is required"),
    )?;
    let suffix = spanned_string(suffix_v, &format!("{key}.suffix"))?;
    if suffix.is_empty() || suffix.len() > MAX_WIKI_BRACKET {
        return Err(err(
            Some(suffix_v.span()),
            format!("{key}.suffix: must be 1-{MAX_WIKI_BRACKET} bytes"),
        ));
    }

    let html_v = leaf(
        &raw.html,
        entry.span(),
        &format!("{key}.html"),
        "a render template is required",
    )?;
    let html = spanned_string(html_v, &format!("{key}.html"))?;
    check_template(
        html,
        html_v.span(),
        &format!("{key}.html"),
        &|name: &str| name == "target" || name == "children",
        "{{target}} and {{children}}",
    )?;

    Ok(Behavior::WikiLink(WikiLink {
        kind: kind.to_owned(),
        prefix: prefix.to_owned(),
        suffix: suffix.to_owned(),
        html: html.to_owned(),
    }))
}

/// Validate one `[[html_filter]]` entry.
fn validate_html_filter(
    entry: &Spanned<RawHtmlFilter>,
    index: usize,
) -> Result<Behavior, LoadError> {
    let raw = entry.get_ref();
    let key = format!("html_filter[{index}]");

    let Some(list) = &raw.deny else {
        return Err(err(
            Some(entry.span()),
            format!("{key}.deny: missing; a list of tag names to deny is required"),
        ));
    };
    if list.len() > MAX_FILTER_TAGS {
        return Err(err(
            Some(entry.span()),
            format!(
                "{key}.deny: {} entries exceeds the limit of {MAX_FILTER_TAGS}",
                list.len()
            ),
        ));
    }
    let mut deny = Vec::with_capacity(list.len());
    for (j, tv) in list.iter().enumerate() {
        let tag = spanned_string(tv, &format!("{key}.deny[{j}]"))?;
        if !is_tag_name(tag) {
            return Err(err(
                Some(tv.span()),
                format!("{key}.deny[{j}]: \"{tag}\" must match [a-zA-Z][a-zA-Z0-9-]*"),
            ));
        }
        deny.push(tag.to_owned());
    }

    Ok(Behavior::HtmlFilter(HtmlFilter { deny }))
}
