//! Tests for the manifest parser: one happy path per shape and one test
//! per rejection rule from `docs/extensions/format.md`.

use std::collections::BTreeMap;

use geist4_ext_loader::error::LoadError;
use geist4_ext_loader::manifest::parse;
use geist4_ext_loader::types::{
    AttributeList, Behavior, BlockContainer, Extension, HtmlFilter, InlineSpan, KindEntry,
    Manifest, OptionDecl, OptionType, OptionValue, Replace, Wasm, WikiLink,
};

/// A `{children}` template for the tests.
const MARK: &str = "<mark>{children}</mark>";

/// The `[extension]` table every test manifest starts from.
fn header() -> String {
    "[extension]\n\
     name = \"sample\"\n\
     version = \"1.0.0\"\n\
     format = \"1.0\"\n\
     description = \"A sample extension\"\n\
     license = \"MIT\"\n\n"
        .to_owned()
}

/// One `[[kinds]]` entry with only a name.
fn kind_decl(name: &str) -> String {
    format!("[[kinds]]\nkind = \"{name}\"\n\n")
}

/// One `[[kinds]]` entry with extra keys after `kind`.
fn kind_decl_extra(name: &str, extra: &str) -> String {
    format!("[[kinds]]\nkind = \"{name}\"\n{extra}\n\n")
}

/// One `[[inline_span]]` entry.
fn inline_span(kind: &str, open: &str, close: &str, html: &str) -> String {
    format!(
        "[[inline_span]]\nkind = \"{kind}\"\nopen = \"{open}\"\nclose = \"{close}\"\nhtml = '{html}'\n\n"
    )
}

/// A base manifest: header, one kind, one inline span.
fn base() -> String {
    header() + &kind_decl("k") + &inline_span("k", "==", "==", MARK)
}

/// `n` valid `[[replace]]` entries.
fn replaces(n: usize) -> String {
    let mut s = String::new();
    for i in 0..n {
        s.push_str(&format!("[[replace]]\nfrom = \"t{i}\"\nto = \"&x;\"\n\n"));
    }
    s
}

/// `n` valid kinds, each used by one valid inline span.
fn many_kinds(n: usize) -> (String, String) {
    let mut kinds = String::new();
    let mut spans = String::new();
    for i in 0..n {
        kinds.push_str(&format!("[[kinds]]\nkind = \"k{i}\"\n\n"));
        spans.push_str(&format!(
            "[[inline_span]]\nkind = \"k{i}\"\nopen = \"==\"\nclose = \"==\"\nhtml = \"<b>{{children}}</b>\"\n\n"
        ));
    }
    (kinds, spans)
}

/// `n` valid options.
fn many_options(n: usize) -> String {
    let mut s = String::new();
    for i in 0..n {
        s.push_str(&format!(
            "[options.o{i}]\ntype = \"string\"\ndefault = \"v{i}\"\n\n"
        ));
    }
    s
}

/// The error `parse` produces for `src`, which must be rejected.
fn err_of(src: &str) -> LoadError {
    parse(src).expect_err("expected the manifest to be rejected")
}

/// Assert `src` is rejected with a message mentioning `needle`.
fn assert_rejected(src: &str, needle: &str) {
    let e = err_of(src);
    assert!(
        e.message.contains(needle),
        "expected the error to mention {needle:?}, got {:?}",
        e.message
    );
}

/// Assert `src` is accepted and return the manifest.
fn ok_of(src: &str) -> Manifest {
    parse(src).expect("expected the manifest to be accepted")
}

/// A full manifest exercising every tier-1 pattern, parsing to the
/// exact data model.
const HAPPY: &str = r#"
[extension]
name = "markstar"
version = "1.2.3"
format = "1.0"
description = "Mark text"
license = "MIT OR Apache-2.0"
homepage = "https://example.com/markstar"
hooks = ["front_matter", "inline_span", "inline_bracket", "block_container", "post_process", "render", "html_filter"]
after = ["strikethrough"]
conflicts = []

[[kinds]]
kind = "highlight"
container = false
inline_text = true
attrs = ["color"]

[[kinds]]
kind = "noteblock"
container = true
inline_text = false

[[kinds]]
kind = "wiki"
attrs = []

[[kinds]]
kind = "attrtarget"

[options.style]
type = "string"
default = "bright"
description = "The highlight style"

[options.enabled]
type = "bool"
default = true

[options.level]
type = "number"
default = 3

[options.loose]
type = "number"
default = 1.5

[[inline_span]]
kind = "highlight"
open = "=="
close = "=="
html = '<mark class="{color}">{children}</mark>'
can_open = true
can_close = true
multiline = true
math = false

[[block_container]]
kind = "noteblock"
marker = ":::"
name = "note"
info_html = '<p class="note-info">{info}</p>'
html = '<div class="note">{children}</div>'
unclosed_ok = true

[[replace]]
from = "<-"
to = "&larr;"
once = true

[[attribute_list]]
kind = "attrtarget"
prefix = "{:"
suffix = "}"

[[wiki_link]]
kind = "wiki"
prefix = "[["
suffix = "]]"
html = '<a href="{target}" class="wikilink">{children}</a>'

[[html_filter]]
deny = ["script", "iframe", "style"]
"#;

/// The smallest valid manifest, for default-value checks.
const MINIMAL: &str = r#"
[extension]
name = "sample"
version = "1.0.0"
format = "1.0"
description = "A sample"
license = "MIT"

[[kinds]]
kind = "mark"

[[inline_span]]
kind = "mark"
open = "=="
close = "=="
html = "<mark>{children}</mark>"
"#;

#[test]
fn full_manifest_parses_to_exact_model() {
    let m = ok_of(HAPPY);
    let expected = Manifest {
        extension: Extension {
            name: "markstar".to_owned(),
            version: "1.2.3".to_owned(),
            format: "1.0".to_owned(),
            description: "Mark text".to_owned(),
            license: "MIT OR Apache-2.0".to_owned(),
            homepage: Some("https://example.com/markstar".to_owned()),
            hooks: vec![
                "front_matter".to_owned(),
                "inline_span".to_owned(),
                "inline_bracket".to_owned(),
                "block_container".to_owned(),
                "post_process".to_owned(),
                "render".to_owned(),
                "html_filter".to_owned(),
            ],
            after: vec!["strikethrough".to_owned()],
            conflicts: vec![],
        },
        kinds: vec![
            KindEntry {
                kind: "highlight".to_owned(),
                container: false,
                inline_text: true,
                attrs: vec!["color".to_owned()],
            },
            KindEntry {
                kind: "noteblock".to_owned(),
                container: true,
                inline_text: false,
                attrs: vec![],
            },
            KindEntry {
                kind: "wiki".to_owned(),
                container: false,
                inline_text: true,
                attrs: vec![],
            },
            KindEntry {
                kind: "attrtarget".to_owned(),
                container: false,
                inline_text: true,
                attrs: vec![],
            },
        ],
        options: BTreeMap::from([
            (
                "enabled".to_owned(),
                OptionDecl {
                    ty: OptionType::Bool,
                    default: OptionValue::Bool(true),
                    description: String::new(),
                },
            ),
            (
                "level".to_owned(),
                OptionDecl {
                    ty: OptionType::Number,
                    default: OptionValue::Number(3.0),
                    description: String::new(),
                },
            ),
            (
                "loose".to_owned(),
                OptionDecl {
                    ty: OptionType::Number,
                    default: OptionValue::Number(1.5),
                    description: String::new(),
                },
            ),
            (
                "style".to_owned(),
                OptionDecl {
                    ty: OptionType::String,
                    default: OptionValue::String("bright".to_owned()),
                    description: "The highlight style".to_owned(),
                },
            ),
        ]),
        behavior: vec![
            Behavior::InlineSpan(InlineSpan {
                kind: "highlight".to_owned(),
                open: "==".to_owned(),
                close: "==".to_owned(),
                html: "<mark class=\"{color}\">{children}</mark>".to_owned(),
                can_open: true,
                can_close: true,
                multiline: true,
                math: false,
            }),
            Behavior::BlockContainer(BlockContainer {
                kind: "noteblock".to_owned(),
                marker: ":::".to_owned(),
                name: "note".to_owned(),
                info_html: "<p class=\"note-info\">{info}</p>".to_owned(),
                html: "<div class=\"note\">{children}</div>".to_owned(),
                unclosed_ok: true,
            }),
            Behavior::Replace(Replace {
                from: "<-".to_owned(),
                to: "&larr;".to_owned(),
                once: true,
            }),
            Behavior::AttributeList(AttributeList {
                kind: "attrtarget".to_owned(),
                prefix: "{:".to_owned(),
                suffix: "}".to_owned(),
            }),
            Behavior::WikiLink(WikiLink {
                kind: "wiki".to_owned(),
                prefix: "[[".to_owned(),
                suffix: "]]".to_owned(),
                html: "<a href=\"{target}\" class=\"wikilink\">{children}</a>".to_owned(),
            }),
            Behavior::HtmlFilter(HtmlFilter {
                deny: vec!["script".to_owned(), "iframe".to_owned(), "style".to_owned()],
            }),
        ],
        wasm: None,
        host: false,
    };
    assert_eq!(m, expected);
}

#[test]
fn minimal_manifest_uses_defaults() {
    let m = ok_of(MINIMAL);
    assert_eq!(m.extension.name, "sample");
    assert_eq!(m.extension.homepage, None);
    assert!(m.extension.hooks.is_empty());
    assert!(m.extension.after.is_empty());
    assert!(m.extension.conflicts.is_empty());
    assert_eq!(
        m.kinds,
        vec![KindEntry {
            kind: "mark".to_owned(),
            container: false,
            inline_text: true,
            attrs: vec![],
        }]
    );
    assert!(m.options.is_empty());
    assert_eq!(m.behavior.len(), 1);
    match &m.behavior[0] {
        Behavior::InlineSpan(s) => {
            assert_eq!(s.kind, "mark");
            assert!(s.can_open);
            assert!(s.can_close);
            assert!(s.multiline);
            assert!(!s.math);
        }
        other => panic!("expected an inline span, got {other:?}"),
    }
    assert_eq!(m.wasm, None);
    assert!(!m.host);
}

#[test]
fn format_version_rules() {
    // The loader implements format 1.0: same major, minor not
    // greater, the patch level ignored.
    for good in ["1.0", "1.0.0", "1.0.2"] {
        ok_of(&MINIMAL.replace("format = \"1.0\"", &format!("format = \"{good}\"")));
    }
    for bad in ["0.9", "1.1", "1.1.0", "2.0", "10.0"] {
        assert_rejected(
            &MINIMAL.replace("format = \"1.0\"", &format!("format = \"{bad}\"")),
            "extension.format",
        );
    }
}

/// A manifest whose `[extension]` name is `name`.
fn src_with_name(name: &str) -> String {
    format!(
        "[extension]\nname = \"{name}\"\nversion = \"1.0.0\"\nformat = \"1.0\"\ndescription = \"d\"\nlicense = \"MIT\"\n\n"
    ) + &kind_decl("mark")
        + &inline_span("mark", "==", "==", MARK)
}

/// A manifest whose `[extension]` version is `version`.
fn src_with_version(version: &str) -> String {
    format!(
        "[extension]\nname = \"sample\"\nversion = \"{version}\"\nformat = \"1.0\"\ndescription = \"d\"\nlicense = \"MIT\"\n\n"
    ) + &kind_decl("mark")
        + &inline_span("mark", "==", "==", MARK)
}

/// A manifest whose `[extension]` format is `format`.
fn src_with_format(format: &str) -> String {
    format!(
        "[extension]\nname = \"sample\"\nversion = \"1.0.0\"\nformat = \"{format}\"\ndescription = \"d\"\nlicense = \"MIT\"\n\n"
    ) + &kind_decl("mark")
        + &inline_span("mark", "==", "==", MARK)
}

#[test]
fn rejects_bad_names() {
    for name in ["", "Upper", "1abc", "a_b", "-a", "a b", "café", "a/b"] {
        assert_rejected(&src_with_name(name), "extension.name");
    }
}

#[test]
fn rejects_reserved_names() {
    for name in [
        "tables",
        "strikethrough",
        "autolink",
        "tasklist",
        "tagfilter",
        "footnotes",
        "front_matter",
        "heading_identifiers",
        "description_lists",
    ] {
        assert_rejected(&src_with_name(name), "reserved");
    }
}

#[test]
fn name_length_limits() {
    assert_rejected(&src_with_name(&"a".repeat(65)), "1-64 bytes");
    assert!(parse(&src_with_name(&"a".repeat(64))).is_ok());
}

#[test]
fn rejects_bad_version_shapes() {
    for v in [
        "", "1", "1.", ".1", "1.x", "1.0.0.0", "01.0", "1.0-", "v1.0",
    ] {
        assert_rejected(&src_with_version(v), "extension.version");
    }
}

#[test]
fn accepts_two_and_three_part_versions() {
    for v in ["1.0", "0.1.0", "10.20.30"] {
        assert!(parse(&src_with_version(v)).is_ok());
    }
}

#[test]
fn format_shape_and_major_decoupling() {
    for f in ["", "1", "1.x", "1.0.0.0"] {
        assert_rejected(&src_with_format(f), "extension.format");
    }
    // A well-shaped version with the wrong major or a newer minor is
    // rejected too: the loader implements format 1.0 (see
    // `format_version_rules`).
    for f in ["9.9", "2.0", "1.2"] {
        assert_rejected(&src_with_format(f), "extension.format");
    }
    assert_eq!(ok_of(&src_with_format("1.0.9")).extension.format, "1.0.9");
}

/// A manifest whose `hooks` list is `hooks`.
fn src_with_hooks(hooks: &[&str]) -> String {
    let list: Vec<String> = hooks.iter().map(|h| format!("\"{h}\"")).collect();
    format!(
        "[extension]\nname = \"sample\"\nversion = \"1.0.0\"\nformat = \"1.0\"\ndescription = \"d\"\nlicense = \"MIT\"\nhooks = [{}]\n\n",
        list.join(", ")
    ) + &kind_decl("mark")
        + &inline_span("mark", "==", "==", MARK)
}

#[test]
fn rejects_unknown_hooks() {
    for hook in ["bogus", "inlineSpan", ""] {
        assert_rejected(&src_with_hooks(&[hook]), "unknown hook");
    }
}

#[test]
fn accepts_the_whole_hook_vocabulary() {
    let hooks = [
        "front_matter",
        "inline_span",
        "inline_bracket",
        "block_container",
        "post_process",
        "render",
        "html_filter",
    ];
    let m = ok_of(&src_with_hooks(&hooks));
    assert_eq!(
        m.extension.hooks,
        hooks.iter().map(|h| h.to_owned()).collect::<Vec<_>>()
    );
}

#[test]
fn rejects_bad_kind_names() {
    for kind in ["", "Bad", "a_b", "9a", "a b"] {
        assert_rejected(
            &format!(
                "[extension]\nname = \"sample\"\nversion = \"1.0\"\nformat = \"1.0\"\ndescription = \"d\"\nlicense = \"MIT\"\n\n[[kinds]]\nkind = \"{kind}\"\n\n[[inline_span]]\nkind = \"{kind}\"\nopen = \"==\"\nclose = \"==\"\nhtml = \"<b>{{children}}</b>\"\n"
            ),
            "kinds[0].kind",
        );
    }
}

#[test]
fn rejects_kind_without_name() {
    assert_rejected(
        &(header() + "[[kinds]]\ncontainer = true\n\n" + &inline_span("k", "==", "==", MARK)),
        "kinds[0].kind: missing",
    );
}

#[test]
fn rejects_duplicate_kinds() {
    let src = header() + &kind_decl("k") + &kind_decl("k") + &inline_span("k", "==", "==", MARK);
    assert_rejected(&src, "duplicate kind");
}

#[test]
fn rejects_bad_attr_names() {
    for attrs in ["[\"Bad\"]", "[\"_x\"]", "[\"a.b\"]"] {
        let src = header()
            + &kind_decl_extra("k", &format!("attrs = {attrs}"))
            + &inline_span("k", "==", "==", MARK);
        assert_rejected(&src, "attrs[0]");
    }
    // Underscores and dashes are fine in attribute names.
    let src = header()
        + &kind_decl_extra("k", "attrs = [\"data-lang_1\"]")
        + &inline_span("k", "==", "==", MARK);
    assert!(parse(&src).is_ok());
}

#[test]
fn kind_limit_is_enforced() {
    let (kinds, spans) = many_kinds(65);
    assert_rejected(&(header() + &kinds + &spans), "exceeds the limit of 64");
    let (kinds, spans) = many_kinds(64);
    assert!(parse(&(header() + &kinds + &spans)).is_ok());
}

/// An `[options.<name>]` table; `default` is raw TOML value text.
fn option(name: &str, ty: &str, default: &str, extra: &str) -> String {
    format!("[options.\"{name}\"]\ntype = \"{ty}\"\ndefault = {default}\n{extra}\n\n")
}

#[test]
fn rejects_option_type_mismatches() {
    for (ty, default, needle) in [
        ("string", "1", "must be a string"),
        ("bool", "\"yes\"", "must be a boolean"),
        ("number", "\"x\"", "must be a number"),
        ("number", "true", "must be a number"),
    ] {
        let src = header()
            + &kind_decl("k")
            + &inline_span("k", "==", "==", MARK)
            + &option("rows", ty, default, "");
        assert_rejected(&src, needle);
    }
}

#[test]
fn rejects_unknown_option_type() {
    let src = header()
        + &kind_decl("k")
        + &inline_span("k", "==", "==", MARK)
        + &option("rows", "integer", "1", "");
    assert_rejected(&src, "is not a known type");
}

#[test]
fn rejects_bad_option_names() {
    for name in ["Big", "a b", "a.b"] {
        let src = header()
            + &kind_decl("k")
            + &inline_span("k", "==", "==", MARK)
            + &option(name, "string", "\"v\"", "");
        assert_rejected(&src, &format!("options.{name}"));
    }
}

#[test]
fn rejects_options_missing_type_or_default() {
    let src = header()
        + &kind_decl("k")
        + &inline_span("k", "==", "==", MARK)
        + "[options.rows]\ndefault = \"v\"\n\n";
    assert_rejected(&src, "options.rows.type: missing");
    let src = header()
        + &kind_decl("k")
        + &inline_span("k", "==", "==", MARK)
        + "[options.rows]\ntype = \"string\"\n\n";
    assert_rejected(&src, "options.rows.default: missing");
}

#[test]
fn option_limit_is_enforced() {
    assert_rejected(&(header() + &many_options(33)), "exceeds the limit of 32");
    let src = header() + &many_options(32) + &kind_decl("k") + &inline_span("k", "==", "==", MARK);
    assert!(parse(&src).is_ok());
}

#[test]
fn rejects_two_tiers() {
    // Tier-1 entries together with [wasm].
    assert_rejected(
        &(base() + "[wasm]\npath = \"component.wasm\"\n\n"),
        "exactly one behavior tier",
    );
    // Tier-1 entries together with host = true.
    assert_rejected(
        &("host = true\n".to_owned() + &base()),
        "exactly one behavior tier",
    );
    // [wasm] together with [host].
    assert_rejected(
        &(header() + "[wasm]\npath = \"component.wasm\"\n\n[host]\n\n"),
        "exactly one behavior tier",
    );
    // All three at once.
    assert_rejected(
        &("host = true\n".to_owned() + &base() + "[wasm]\npath = \"component.wasm\"\n\n"),
        "exactly one behavior tier",
    );
}

#[test]
fn rejects_no_tier() {
    assert_rejected(&(header() + &kind_decl("k")), "declares no behavior tier");
    assert_rejected(&header(), "declares no behavior tier");
}

#[test]
fn wasm_section_and_limits() {
    let m = ok_of(
        &(header() + "[wasm]\npath = \"component.wasm\"\nfuel = 1000000\nmemory_pages = 16\n\n"),
    );
    assert_eq!(
        m.wasm,
        Some(Wasm {
            path: "component.wasm".to_owned(),
            fuel: Some(1_000_000),
            memory_pages: Some(16),
            inline_triggers: vec![],
        })
    );
    // fuel and memory_pages are optional.
    let m = ok_of(&(header() + "[wasm]\npath = \"c.wasm\"\n\n"));
    assert_eq!(
        m.wasm,
        Some(Wasm {
            path: "c.wasm".to_owned(),
            fuel: None,
            memory_pages: None,
            inline_triggers: vec![],
        })
    );
    assert_rejected(&(header() + "[wasm]\npath = \"\"\n\n"), "non-empty");
    assert_rejected(&(header() + "[wasm]\nfuel = 5\n\n"), "wasm.path: missing");
    assert_rejected(
        &(header() + "[wasm]\npath = \"c.wasm\"\nfuel = 0\n\n"),
        "wasm.fuel",
    );
    assert_rejected(
        &(header() + "[wasm]\npath = \"c.wasm\"\nfuel = -3\n\n"),
        "wasm.fuel",
    );
    assert_rejected(
        &(header() + "[wasm]\npath = \"c.wasm\"\nmemory_pages = 0\n\n"),
        "wasm.memory_pages",
    );
    assert_rejected(
        &(header() + "[wasm]\npath = \"c.wasm\"\nmemory_pages = 5000000000\n\n"),
        "wasm.memory_pages",
    );
}

#[test]
fn wasm_inline_triggers() {
    // A declared list reaches the manifest, in bytes.
    let m = ok_of(&(header() + "[wasm]\npath = \"c.wasm\"\ninline_triggers = [\"%\", \"^\"]\n\n"));
    assert_eq!(m.wasm.expect("wasm").inline_triggers, vec![b'%', b'^']);
    // A core trigger byte is rejected: the core never hands it over.
    assert_rejected(
        &(header() + "[wasm]\npath = \"c.wasm\"\ninline_triggers = [\"*\"]\n\n"),
        "wasm.inline_triggers",
    );
    // More than one byte is rejected.
    assert_rejected(
        &(header() + "[wasm]\npath = \"c.wasm\"\ninline_triggers = [\"ab\"]\n\n"),
        "wasm.inline_triggers",
    );
    // A duplicate byte is rejected.
    assert_rejected(
        &(header() + "[wasm]\npath = \"c.wasm\"\ninline_triggers = [\"%\", \"%\"]\n\n"),
        "wasm.inline_triggers",
    );
}

#[test]
fn host_manifests() {
    // The [host] table form.
    let m = ok_of(&(header() + "[host]\n\n"));
    assert!(m.host);
    assert!(m.behavior.is_empty());
    assert_eq!(m.wasm, None);
    // The host = true form.
    let m = ok_of(&("host = true\n".to_owned() + &header()));
    assert!(m.host);
    // host = false is no tier at all.
    assert_rejected(
        &("host = false\n".to_owned() + &header()),
        "no behavior tier",
    );
    // Anything else is a type error.
    assert_rejected(&("host = \"yes\"\n".to_owned() + &header()), "host:");
}

#[test]
fn rejects_span_delimiters_out_of_range() {
    assert_rejected(
        &(header() + &kind_decl("k") + &inline_span("k", "", "==", MARK)),
        "inline_span[0].open: must be 1-16 bytes",
    );
    assert_rejected(
        &(header() + &kind_decl("k") + &inline_span("k", &"=".repeat(17), "==", MARK)),
        "inline_span[0].open: must be 1-16 bytes",
    );
    assert_rejected(
        &(header() + &kind_decl("k") + &inline_span("k", "==", "", MARK)),
        "inline_span[0].close: must be 1-16 bytes",
    );
    // At the limits is fine: one byte and sixteen bytes.
    assert!(parse(&(header() + &kind_decl("k") + &inline_span("k", "*", "==", MARK))).is_ok());
    assert!(
        parse(&(header() + &kind_decl("k") + &inline_span("k", &"=".repeat(16), "==", MARK)))
            .is_ok()
    );
}

#[test]
fn rejects_unknown_template_substitutions() {
    assert_rejected(
        &(header() + &kind_decl("k") + &inline_span("k", "==", "==", "<b>{bogus}</b>")),
        "unknown substitution",
    );
    // An attribute the kind declares is allowed.
    let src = header()
        + &kind_decl_extra("k", "attrs = [\"color\"]")
        + &inline_span("k", "==", "==", "<m class=\"{color}\">{children}</m>");
    assert!(parse(&src).is_ok());
    // An empty substitution is unknown, and an unterminated brace is
    // literal text.
    assert_rejected(
        &(header() + &kind_decl("k") + &inline_span("k", "==", "==", "<b>{}</b>")),
        "unknown substitution",
    );
    // An unterminated brace is literal text (on a non-inline kind, so
    // the missing {children} does not matter).
    assert!(
        parse(
            &(header()
                + &kind_decl_extra("k", "inline_text = false")
                + &inline_span("k", "==", "==", "<b{unclosed"))
        )
        .is_ok()
    );
}

#[test]
fn rejects_templates_over_1kib() {
    assert_rejected(
        &(header()
            + &kind_decl("k")
            + &inline_span("k", "==", "==", &format!("<b>{}</b>", "x".repeat(1018)))),
        "1 KiB",
    );
    // Exactly 1024 bytes is fine. The templates carry {children} so the
    // size rule, not the children rule, is what fires.
    let src = header()
        + &kind_decl("k")
        + &inline_span(
            "k",
            "==",
            "==",
            &format!("<b>{}{{children}}</b>", "x".repeat(1007)),
        );
    assert!(parse(&src).is_ok());
}

#[test]
fn inline_text_template_needs_children() {
    assert_rejected(
        &(header() + &kind_decl("k") + &inline_span("k", "==", "==", "<b></b>")),
        "{children}",
    );
    // A non-inline kind may omit {children}.
    let src = header()
        + &kind_decl_extra("k", "inline_text = false")
        + &inline_span("k", "==", "==", "<b></b>");
    assert!(parse(&src).is_ok());
}

#[test]
fn rejects_bad_markers() {
    for marker in ["::--", "::", ":", "==x=="] {
        let src = header()
            + &kind_decl_extra("box", "container = true")
            + &format!(
                "[[block_container]]\nkind = \"box\"\nmarker = \"{marker}\"\nname = \"note\"\nhtml = \"<div>{{children}}</div>\"\n\n"
            );
        assert_rejected(&src, "one repeated character");
    }
    // A uniform 17-byte run is too long.
    let src = header()
        + &kind_decl_extra("box", "container = true")
        + &format!(
            "[[block_container]]\nkind = \"box\"\nmarker = \"{}\"\nname = \"note\"\nhtml = \"<div>{{children}}</div>\"\n\n",
            ":".repeat(17)
        );
    assert_rejected(&src, "one repeated character");
    // Uniform markers at both length ends are fine, including a
    // repeated multi-byte character.
    for marker in [":::", &":".repeat(16), &"\u{b7}".repeat(4)] {
        let src = header()
            + &kind_decl_extra("box", "container = true")
            + &format!(
                "[[block_container]]\nkind = \"box\"\nmarker = \"{marker}\"\nname = \"note\"\nhtml = \"<div>{{children}}</div>\"\n\n"
            );
        assert!(parse(&src).is_ok());
    }
}

#[test]
fn block_container_rules() {
    // The kind must be declared and be a container kind.
    assert_rejected(
        &(header()
            + &kind_decl("plain")
            + "[[block_container]]\nkind = \"plain\"\nmarker = \":::\"\nname = \"note\"\nhtml = \"<div>{children}</div>\"\n\n"),
        "must be a container kind",
    );
    let ok = header()
        + &kind_decl_extra("box", "container = true")
        + "[[block_container]]\nkind = \"box\"\nmarker = \":::\"\nname = \"note\"\nhtml = \"<div>{children}</div>\"\n\n";
    let m = ok_of(&ok);
    match &m.behavior[0] {
        Behavior::BlockContainer(b) => {
            assert_eq!(b.info_html, "");
            assert!(b.unclosed_ok);
        }
        other => panic!("expected a block container, got {other:?}"),
    }
    // The opening word has the name charset.
    assert_rejected(
        &(header()
            + &kind_decl_extra("box", "container = true")
            + "[[block_container]]\nkind = \"box\"\nmarker = \":::\"\nname = \"Note\"\nhtml = \"<div>{children}</div>\"\n\n"),
        "block_container[0].name",
    );
    // An unknown substitution is rejected here too.
    assert_rejected(
        &(header()
            + &kind_decl_extra("box", "container = true")
            + "[[block_container]]\nkind = \"box\"\nmarker = \":::\"\nname = \"note\"\nhtml = \"<div>{bogus}</div>\"\n\n"),
        "unknown substitution",
    );
    // {info} is allowed in the container template.
    let src = header()
        + &kind_decl_extra("box", "container = true")
        + "[[block_container]]\nkind = \"box\"\nmarker = \":::\"\nname = \"note\"\nhtml = \"<div>{children}{info}</div>\"\n\n";
    assert!(parse(&src).is_ok());
}

#[test]
fn replace_rules() {
    // A newline (via a TOML escape) in `from` is rejected.
    assert_rejected(
        &(header() + "[[replace]]\nfrom = \"a\\nb\"\nto = \"x\"\n\n"),
        "must not contain a newline",
    );
    assert_rejected(
        &(header() + "[[replace]]\nfrom = \"\"\nto = \"x\"\n\n"),
        "from: must be 1-32 bytes",
    );
    assert_rejected(
        &(header() + &format!("[[replace]]\nfrom = \"{}\"\nto = \"x\"\n\n", "a".repeat(33))),
        "from: must be 1-32 bytes",
    );
    assert_rejected(
        &(header() + "[[replace]]\nfrom = \"a\"\nto = \"\"\n\n"),
        "to: must be 1-128 bytes",
    );
    assert_rejected(
        &(header()
            + &format!(
                "[[replace]]\nfrom = \"a\"\nto = \"{}\"\n\n",
                "b".repeat(129)
            )),
        "to: must be 1-128 bytes",
    );
    // At the limits is fine, and `once` defaults to true.
    let src = header()
        + &format!(
            "[[replace]]\nfrom = \"{}\"\nto = \"{}\"\n\n",
            "a".repeat(32),
            "b".repeat(128)
        );
    let m = ok_of(&src);
    match &m.behavior[0] {
        Behavior::Replace(r) => assert!(r.once),
        other => panic!("expected a replace, got {other:?}"),
    }
}

#[test]
fn replace_and_entry_limits() {
    assert_rejected(&(header() + &replaces(513)), "exceeds the limit of 512");
    assert_rejected(&(header() + &replaces(300)), "exceeds the limit of 256");
    assert!(parse(&(header() + &replaces(256))).is_ok());
}

#[test]
fn attribute_list_rules() {
    assert_rejected(
        &(header()
            + &kind_decl("target")
            + "[[attribute_list]]\nkind = \"target\"\nprefix = \"[:\"\nsuffix = \"}\"\n\n"),
        "must start with",
    );
    assert_rejected(
        &(header()
            + &kind_decl("target")
            + "[[attribute_list]]\nkind = \"target\"\nprefix = \"{\"\nsuffix = \"\"\n\n"),
        "must not be empty",
    );
    assert_rejected(
        &(header()
            + &kind_decl("target")
            + "[[attribute_list]]\nkind = \"nope\"\nprefix = \"{\"\nsuffix = \"}\"\n\n"),
        "not declared",
    );
    let m = ok_of(
        &(header()
            + &kind_decl("target")
            + "[[attribute_list]]\nkind = \"target\"\nprefix = \"{\"\nsuffix = \"}\"\n\n"),
    );
    match &m.behavior[0] {
        Behavior::AttributeList(a) => {
            assert_eq!(a.prefix, "{");
            assert_eq!(a.suffix, "}");
        }
        other => panic!("expected an attribute list, got {other:?}"),
    }
}

#[test]
fn wiki_link_rules() {
    let entry = |prefix: &str, suffix: &str, html: &str| {
        header()
            + &kind_decl("w")
            + &format!(
                "[[wiki_link]]\nkind = \"w\"\nprefix = \"{prefix}\"\nsuffix = \"{suffix}\"\nhtml = '{html}'\n\n"
            )
    };
    assert_rejected(
        &entry("", "]]", MARK),
        "wiki_link[0].prefix: must be 1-16 bytes",
    );
    assert_rejected(
        &entry(&"[".repeat(17), "]]", MARK),
        "wiki_link[0].prefix: must be 1-16 bytes",
    );
    assert_rejected(
        &entry("[[", "", MARK),
        "wiki_link[0].suffix: must be 1-16 bytes",
    );
    assert_rejected(&entry("[[", "]]", "<a>{bogus}</a>"), "unknown substitution");
    let m = ok_of(&entry("[[", "]]", "<a href=\"{target}\">{children}</a>"));
    match &m.behavior[0] {
        Behavior::WikiLink(w) => {
            assert_eq!(w.prefix, "[[");
            assert_eq!(w.suffix, "]]");
        }
        other => panic!("expected a wiki link, got {other:?}"),
    }
}

#[test]
fn html_filter_rules() {
    assert_rejected(
        &(header() + "[[html_filter]]\ndeny = [\"3bad\", \"ok-tag\", \"-x\"]\n\n"),
        "deny[0]",
    );
    assert_rejected(&(header() + "[[html_filter]]\n\n"), "deny: missing");
    let tags: Vec<String> = (0..65).map(|i| format!("\"t{i}\"")).collect();
    assert_rejected(
        &(header() + &format!("[[html_filter]]\ndeny = [{}]\n\n", tags.join(", "))),
        "exceeds the limit of 64",
    );
    let tags: Vec<String> = (0..64).map(|i| format!("\"t{i}\"")).collect();
    assert!(
        parse(&(header() + &format!("[[html_filter]]\ndeny = [{}]\n\n", tags.join(", ")))).is_ok()
    );
}

#[test]
fn kind_cross_rules() {
    // A behavior entry references an undeclared kind.
    assert_rejected(
        &(header() + &kind_decl("k") + &inline_span("other", "==", "==", MARK)),
        "not declared",
    );
    // A declared kind no behavior entry uses.
    assert_rejected(
        &(header() + &kind_decl("k") + &kind_decl("unused") + &inline_span("k", "==", "==", MARK)),
        "no behavior entry uses it",
    );
    // A kind used by two entries.
    assert_rejected(
        &(header()
            + &kind_decl("k")
            + &inline_span("k", "==", "==", MARK)
            + &inline_span("k", "$$", "$$", MARK)),
        "already used",
    );
}

#[test]
fn rejects_overlarge_manifest() {
    // 64 KiB exactly is accepted; one more byte is rejected before any
    // TOML parsing happens.
    let minimal = header() + &kind_decl("k") + &inline_span("k", "==", "==", MARK);
    let pad = 64 * 1024 - minimal.len();
    assert!(pad >= 2);
    let padded = format!("#{}\n{}", "z".repeat(pad - 2), minimal);
    assert_eq!(padded.len(), 64 * 1024);
    assert!(parse(&padded).is_ok());
    assert_rejected(&format!("#{}\n{}", "z".repeat(pad - 1), minimal), "64 KiB");
    // A 10 MB manifest is rejected the same cheap way.
    assert_rejected(&"x".repeat(10 * 1024 * 1024), "64 KiB");
}

#[test]
fn toml_syntax_errors_carry_spans() {
    for bad in [
        "[extension\nname = \"x\"\n",
        "a = \n",
        "[extension]\nname = \n",
        "extension]\n",
    ] {
        let e = err_of(bad);
        assert!(e.span.is_some(), "no span for {bad:?}");
    }
    // Duplicate keys are a parse error too.
    let e = err_of("[extension]\nname = \"a\"\nname = \"b\"\n");
    assert!(e.span.is_some());
}

#[test]
fn validation_errors_carry_spans() {
    let src = src_with_name("tables");
    let e = err_of(&src);
    let span = e.span.expect("expected a span on a validation error");
    assert!(span.start < span.end);
    assert!(span.end <= src.len());
    // Messages are one line.
    assert!(!e.message.contains('\n'));
}

#[test]
fn license_expressions() {
    for license in [
        "MIT",
        "MIT OR Apache-2.0",
        "(MIT OR Apache-2.0) AND LGPL-2.1+",
        "LicenseRef-Proprietary",
    ] {
        assert!(
            parse(&src_with_license(license)).is_ok(),
            "{license:?} should be accepted"
        );
    }
    for license in [
        "",
        "MIT AND",
        "OR MIT",
        "MIT OR",
        "(MIT",
        "MIT)",
        "MIT XOR Apache",
        "MIT AND (Apache-2.0",
    ] {
        assert_rejected(&src_with_license(license), "SPDX");
    }
}

/// A manifest whose `[extension]` license is `license`.
fn src_with_license(license: &str) -> String {
    format!(
        "[extension]\nname = \"sample\"\nversion = \"1.0.0\"\nformat = \"1.0\"\ndescription = \"d\"\nlicense = \"{license}\"\n\n"
    ) + &kind_decl("mark")
        + &inline_span("mark", "==", "==", MARK)
}

#[test]
fn missing_extension_fields() {
    let variants: [(&str, &str); 5] = [
        (
            "name",
            "[extension]\nversion = \"1.0\"\nformat = \"1.0\"\ndescription = \"d\"\nlicense = \"MIT\"\n",
        ),
        (
            "version",
            "[extension]\nname = \"sample\"\nformat = \"1.0\"\ndescription = \"d\"\nlicense = \"MIT\"\n",
        ),
        (
            "format",
            "[extension]\nname = \"sample\"\nversion = \"1.0\"\ndescription = \"d\"\nlicense = \"MIT\"\n",
        ),
        (
            "description",
            "[extension]\nname = \"sample\"\nversion = \"1.0\"\nformat = \"1.0\"\nlicense = \"MIT\"\n",
        ),
        (
            "license",
            "[extension]\nname = \"sample\"\nversion = \"1.0\"\nformat = \"1.0\"\ndescription = \"d\"\n",
        ),
    ];
    for (field, ext) in variants {
        assert_rejected(
            &(ext.to_owned() + "\n"),
            &format!("extension.{field}: missing"),
        );
    }
}

#[test]
fn declaration_order_is_preserved() {
    let src = header()
        + &kind_decl("k")
        + "[[replace]]\nfrom = \"a\"\nto = \"1\"\n\n"
        + &inline_span("k", "==", "==", MARK)
        + "[[replace]]\nfrom = \"b\"\nto = \"2\"\n\n";
    let m = ok_of(&src);
    assert_eq!(m.behavior.len(), 3);
    assert!(matches!(&m.behavior[0], Behavior::Replace(r) if r.from == "a"));
    assert!(matches!(&m.behavior[1], Behavior::InlineSpan(_)));
    assert!(matches!(&m.behavior[2], Behavior::Replace(r) if r.from == "b"));
}

#[test]
fn shape_mismatches_are_rejected() {
    // A root-level key of the wrong shape is rejected by serde with
    // its own message and span.
    assert!(parse(&("kinds = 5\n".to_owned() + &header())).is_err());
    assert!(parse(&("options = 5\n".to_owned() + &header())).is_err());
    // A wrong-typed scalar is caught by the validator.
    assert!(parse(&("[wasm]\npath = 5\n\n".to_owned() + &header())).is_err());
}
