//! End-to-end tests for the composed tier-1 extension: a manifest
//! carrying several declarative patterns, compiled through `compile`,
//! exercised through the real parser and renderer, and loaded from a
//! package directory through `package::load`.

use geist4_core::basic_html::render_with;
use geist4_core::extension::Options;
use geist4_core::parser::parse_with;
use geist4_ext_loader::types::{self, Behavior, KindEntry, Manifest};
use std::collections::BTreeMap;

const MARK: &str = "<mark>{children}</mark>";

/// A minimal manifest carrying `kinds` and `behavior`.
fn manifest(kinds: Vec<KindEntry>, behavior: Vec<Behavior>) -> Manifest {
    Manifest {
        extension: types::Extension {
            name: "demo".to_owned(),
            version: "1.0.0".to_owned(),
            format: "1.0".to_owned(),
            description: String::new(),
            license: "MIT".to_owned(),
            homepage: None,
            hooks: Vec::new(),
            after: Vec::new(),
            conflicts: Vec::new(),
        },
        kinds,
        options: BTreeMap::new(),
        behavior,
        wasm: None,
        host: false,
    }
}

/// A `[[kinds]]` entry for `name` declaring `attrs`.
fn kind(name: &str, attrs: &[&str]) -> KindEntry {
    KindEntry {
        kind: name.to_owned(),
        container: false,
        inline_text: true,
        attrs: attrs.iter().map(|attr| (*attr).to_owned()).collect(),
    }
}

fn span() -> Behavior {
    Behavior::InlineSpan(types::InlineSpan {
        kind: "hl".to_owned(),
        open: "==".to_owned(),
        close: "==".to_owned(),
        html: MARK.to_owned(),
        can_open: true,
        can_close: true,
        multiline: false,
        math: false,
    })
}

/// Parses and renders `source` with the composed extension for
/// `behavior`.
fn render(source: &str, behavior: Vec<Behavior>) -> String {
    let extension =
        geist4_ext_loader::compile::compile(manifest(vec![kind("hl", &["color"])], behavior));
    let mut options = Options::new();
    options.extension(extension);
    let document = parse_with(source, &options);
    render_with(&document, &options)
}

#[test]
fn inline_span_through_the_composed_extension() {
    assert_eq!(
        render("a ==mark== b\n", vec![span()]),
        "<p>a <mark>mark</mark> b</p>\n"
    );
}

#[test]
fn attributes_flow_through_the_composed_extension() {
    // The attribute-list entry targets the span's kind; a list on the
    // line after the span attaches `color`.
    let behavior = vec![
        Behavior::InlineSpan(types::InlineSpan {
            kind: "hl".to_owned(),
            open: "==".to_owned(),
            close: "==".to_owned(),
            html: "<mark data-color=\"{color}\">{children}</mark>".to_owned(),
            can_open: true,
            can_close: true,
            multiline: false,
            math: false,
        }),
        Behavior::AttributeList(types::AttributeList {
            kind: "hl".to_owned(),
            prefix: "{:".to_owned(),
            suffix: "}".to_owned(),
        }),
    ];
    assert_eq!(
        render("a ==mark=={:color=red} b\n", behavior),
        "<p>a <mark data-color=\"red\">mark</mark> b</p>\n"
    );
}

#[test]
fn block_container_through_the_composed_extension() {
    let kinds = vec![
        kind("hl", &["color"]),
        KindEntry {
            kind: "note".to_owned(),
            container: true,
            inline_text: false,
            attrs: Vec::new(),
        },
    ];
    let behavior = vec![Behavior::BlockContainer(types::BlockContainer {
        kind: "note".to_owned(),
        marker: ":::".to_owned(),
        name: "note".to_owned(),
        info_html: String::new(),
        html: "<div class=\"note\">{children}</div>".to_owned(),
        unclosed_ok: false,
    })];
    let extension = geist4_ext_loader::compile::compile(manifest(kinds, behavior));
    let mut options = Options::new();
    options.extension(extension);
    let document = parse_with(":::note\nhello\n:::\n", &options);
    assert_eq!(
        render_with(&document, &options),
        "<div class=\"note\"><p>hello</p>\n</div>"
    );
}

#[test]
fn html_filter_through_the_composed_extension() {
    let behavior = vec![Behavior::HtmlFilter(types::HtmlFilter {
        deny: vec!["script".to_owned()],
    })];
    assert_eq!(
        render("a <script>x</script> b\n", behavior.clone()),
        "<p>a &lt;script>x&lt;/script> b</p>\n"
    );
    // A name that merely starts like the denied tag is not denied.
    let behavior = behavior.clone();
    assert_eq!(
        render("a <scripty>x</scripty> b\n", behavior),
        "<p>a <scripty>x</scripty> b</p>\n"
    );
}

#[test]
fn replace_through_the_composed_extension() {
    let behavior = vec![Behavior::Replace(types::Replace {
        from: "(c)".to_owned(),
        to: "©".to_owned(),
        once: false,
    })];
    assert_eq!(render("(c) Geist4\n", behavior), "<p>© Geist4</p>\n");
}

#[test]
fn composed_extension_reports_its_name_and_kinds() {
    let extension =
        geist4_ext_loader::compile::compile(manifest(vec![kind("hl", &["color"])], vec![span()]));
    assert_eq!(extension.name(), "demo");
    assert_eq!(extension.kinds().len(), 1);
    assert_eq!(extension.kinds()[0].kind, "hl");
}

#[test]
fn package_load_rejects_a_missing_manifest() {
    let dir = std::env::temp_dir().join("geist4-package-missing");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let error = geist4_ext_loader::package::load(&dir).unwrap_err();
    assert!(error.message.contains("cannot read"), "{}", error.message);
}

#[test]
fn package_load_rejects_a_missing_spec() {
    let dir = std::env::temp_dir().join("geist4-package-nospec");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("extension.toml"),
        "[extension]\nname = \"demo\"\nversion = \"1.0.0\"\nformat = \"1.0\"\ndescription = \"d\"\nlicense = \"MIT\"\n\
         [[kinds]]\nkind = \"hl\"\ninline_text = true\n\
         [[inline_span]]\nkind = \"hl\"\nopen = \"==\"\nclose = \"==\"\nhtml = \"<mark>{children}</mark>\"\n",
    )
    .unwrap();
    let error = geist4_ext_loader::package::load(&dir).unwrap_err();
    assert!(error.message.contains("spec.md"), "{}", error.message);
}

#[test]
fn package_load_rejects_an_invalid_manifest() {
    let dir = std::env::temp_dir().join("geist4-package-invalid");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("extension.toml"), "not [ valid").unwrap();
    let error = geist4_ext_loader::package::load(&dir).unwrap_err();
    assert!(
        error.message.contains("key with no value"),
        "{}",
        error.message
    );
}

#[test]
fn package_load_accepts_a_valid_package() {
    let dir = std::env::temp_dir().join("geist4-package-ok");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("extension.toml"),
        "[extension]\nname = \"demo\"\nversion = \"1.0.0\"\nformat = \"1.0\"\ndescription = \"d\"\nlicense = \"MIT\"\n\
         [[kinds]]\nkind = \"hl\"\ninline_text = true\nattrs = [\"color\"]\n\
         [[inline_span]]\nkind = \"hl\"\nopen = \"==\"\nclose = \"==\"\nhtml = \"<mark>{children}</mark>\"\n",
    )
    .unwrap();
    std::fs::write(dir.join("spec.md"), "# spec\n").unwrap();
    let package = geist4_ext_loader::package::load(&dir).unwrap();
    assert_eq!(package.extension().name(), "demo");
    assert_eq!(package.manifest().kinds.len(), 1);
}
