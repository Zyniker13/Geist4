//! The regression corpus: one Markdown file per bug the suite has
//! caught, each paired with the exact expected HTML. The runner walks
//! `tests/regressions/*.md`, finds the matching `*.html`, and requires
//! a byte-identical render.
//!
//! Leading lines in a `.md` file that start with `////` carry the bug
//! description; the runner strips them before parsing, so the body is
//! exactly the reproducing input. Expectations are written in the
//! spec-suite HTML convention (no document-final newline) and are also
//! verified against the reference implementation when one is available
//! (see the differential test).

use std::path::PathBuf;

/// Renders a corpus file to the spec-suite HTML convention (no final
/// newline), as the `.html` expectations are written that way.
fn html(src: &str) -> String {
    geist4_core::basic_html::render(&geist4_core::parser::parse_document(src))
}

/// Line endings are transport, not content: a checkout may carry CRLF
/// (Windows), while the expectations were generated with LF. Normalize
/// both sides before comparing; the parser treats the three line-ending
/// forms identically.
fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Drops the leading `////` comment lines, keeping the body bytes.
fn strip_header(text: &str) -> &str {
    let mut rest = text;
    for line in text.split_inclusive('\n') {
        if line.starts_with("////") {
            rest = &rest[line.len()..];
        } else {
            break;
        }
    }
    rest
}

#[test]
fn regression_corpus() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/regressions");
    let mut cases = 0usize;
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("regressions directory exists")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    entries.sort();
    assert!(!entries.is_empty(), "the corpus is not empty");
    for markdown_path in entries {
        let markdown = std::fs::read_to_string(&markdown_path).expect("read .md");
        let expected_path = markdown_path.with_extension("html");
        let expected = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|_| panic!("{} has no paired .html", markdown_path.display()));
        cases += 1;
        assert_eq!(
            html(&normalize(strip_header(&markdown))),
            normalize(&expected),
            "{} failed",
            markdown_path.display()
        );
    }
    assert!(cases >= 1);
}
