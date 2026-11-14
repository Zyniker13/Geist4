//! Generates `spec/heading_identifiers.md` from the frozen oracle
//! fixture `tests/fixtures/heading_oracle.json`.
//!
//! Every example's expected output is the reference's HTML from the
//! fixture, byte for byte; the four divergent cases (decision 0021,
//! `docs/decisions/0021-heading-ids-extract-dom-text.md`) carry the
//! engine's HTML instead, each with its reason. The generator fails
//! loudly if any other case differs from the reference, if a recorded
//! divergence stops diverging, or if the fixture holds a case no
//! section lists — regenerating after extending the probe battery
//! cannot silently drop or falsify an example.
//!
//! Run: cargo run -q -p geist4-ext --example gen_heading_spec
use geist4_core::basic_html::render_with;
use geist4_core::extension::Options;
use geist4_core::parser::parse_with;

const FIXTURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/heading_oracle.json"
);
const SPEC_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/spec/heading_identifiers.md");

/// `(case, reason)` — cases where the engine deliberately differs from
/// the reference. Must match `DIVERGENCES` in
/// `tests/heading_identifiers.rs`.
const DIVERGENCES: &[(&str, &str)] = &[
    (
        "entity-text",
        "the reference's unescape deletes the named entity; the engine keeps the decoded text",
    ),
    (
        "html-comment",
        "the reference's lazy tag strip stops at the comment's first `>` and leaks its tail",
    ),
    (
        "html-processing",
        "the reference's tag strip never matches a processing instruction and leaks its inner text",
    ),
    (
        "trailing-space-tag",
        "the reference slugs the stripped text without trimming; the engine trims the extracted text",
    ),
];

/// `(case, reason)` — cases where the engine's rendered HTML differs
/// from the reference for a reason unrelated to heading identifiers;
/// the heading ids themselves must still agree. Must match the id
/// tables in `tests/heading_identifiers.rs` (these cases are not id
/// divergences, so they do not appear there).
const BODY_DIVERGENCES: &[(&str, &str)] = &[
    (
        "image",
        "marked writes void elements HTML5-style; CommonMark writes \
     `<img ... />` and the engine follows CommonMark",
    ),
    (
        "html-spans-newline",
        "marked keeps the paragraph's leading space; CommonMark strips \
     leading and trailing whitespace from paragraph text and the \
     engine follows CommonMark",
    ),
    (
        "in-list",
        "marked tucks the heading directly after `<li>`; CommonMark's \
     list-item example puts block-level content on its own line and \
     the engine follows CommonMark",
    ),
    (
        "in-list-loose",
        "same list-item placement as `in-list`: CommonMark puts \
     block-level content on its own line inside `<li>`",
    ),
];

/// Spec-file section order. Every fixture case must appear in exactly
/// one section; every section case must exist in the fixture.
const SECTIONS: &[(&str, &[&str])] = &[
    (
        "ATX headings",
        &[
            "atx-basic",
            "atx-level2",
            "atx-empty",
            "atx-hashes-only",
            "atx-closing-seq",
        ],
    ),
    (
        "Setext headings",
        &["setext-basic", "setext-underline-h2", "setext-two-lines"],
    ),
    (
        "Repeated headings",
        &["dedup-3", "dedup-case", "dedup-levels"],
    ),
    (
        "Inline content",
        &[
            "code-span",
            "emphasis",
            "strong",
            "link",
            "image",
            "autolink",
            "inner-spaces",
        ],
    ),
    (
        "Raw HTML and entities",
        &[
            "entity-text",
            "entity-tag",
            "html-comment",
            "html-processing",
            "html-open-tag",
            "html-close-tag",
            "html-unmatched",
            "html-spans-newline",
            "trailing-space-tag",
        ],
    ),
    ("Non-ASCII text", &["greek-final-sigma", "cjk", "emoji"]),
    (
        "Headings in containers",
        &["in-blockquote", "in-list", "in-list-loose", "interleaved"],
    ),
    (
        "Document structure",
        &["leading-text", "trailing-paragraph"],
    ),
];

/// Plain-Markdown sources: with the extension enabled, everything
/// that is not a heading renders as the core's CommonMark output.
const PLAIN_CASES: &[&str] = &[
    "#hashtag\n\na # b\n",
    "```\n# included in the code block\n```\n",
    "Intro.\n\n# Heading\n\nOutro.\n",
];

const FENCE: &str = "````````````````````````````````";

fn rendered(markdown: &str) -> String {
    let mut options = Options::new();
    options.extension(geist4_ext::heading_identifiers());
    render_with(&parse_with(markdown, &options), &options)
}

/// Extracts heading ids from rendered HTML, in document order — the
/// same scan `tests/heading_identifiers.rs` performs, kept independent
/// on purpose.
fn heading_ids(html: &str) -> Vec<String> {
    let bytes = html.as_bytes();
    let mut ids = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let is_heading_tag = bytes[i] == b'<'
            && bytes.get(i + 1) == Some(&b'h')
            && bytes.get(i + 2).is_some_and(|b| (b'1'..=b'6').contains(b))
            && bytes
                .get(i + 3)
                .is_some_and(|b| b.is_ascii_whitespace() || *b == b'>' || *b == b'/');
        if !is_heading_tag {
            i += 1;
            continue;
        }
        let Some(tag_end) = bytes[i..].iter().position(|b| *b == b'>') else {
            break;
        };
        let tag = &html[i..i + tag_end];
        if let Some(at) = tag.find(" id=\"") {
            let rest = &tag[at + 5..];
            if let Some(close) = rest.find('"') {
                ids.push(rest[..close].to_owned());
            }
        }
        i += tag_end + 1;
    }
    ids
}

fn example(markdown: &str, expected: &str) -> String {
    for text in [markdown, expected] {
        assert!(
            !text.contains(FENCE),
            "example text contains a {FENCE} fence run: {text:?}"
        );
    }
    format!("{FENCE} example\n{markdown}.\n{expected}{FENCE}\n")
}

fn main() {
    let fixture: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(FIXTURE_PATH).expect("read fixture"))
            .expect("valid fixture");

    let mut listed: Vec<&str> = Vec::new();
    let mut body = String::new();
    for (title, cases) in SECTIONS {
        body.push_str(&format!("## {title}\n\n"));
        for case in *cases {
            let case_data = fixture
                .get(*case)
                .unwrap_or_else(|| panic!("section case {case:?} is not in the fixture"));
            listed.push(case);
            let markdown = case_data["markdown"].as_str().expect("markdown");
            let reference = case_data["html"].as_str().expect("html");
            let engine = rendered(markdown);
            if let Some((_, reason)) = DIVERGENCES
                .iter()
                .find(|(divergent, _)| *divergent == *case)
            {
                assert_ne!(
                    engine, reference,
                    "{case}: recorded as a divergence but the engine now matches the reference; \
                     remove it from the divergence tables"
                );
                body.push_str(&example(markdown, &engine));
                body.push_str(&format!(
                    "Divergence from the reference (decision 0021): {reason}.\n\n"
                ));
            } else if let Some((_, reason)) = BODY_DIVERGENCES
                .iter()
                .find(|(body_divergent, _)| *body_divergent == *case)
            {
                assert_ne!(
                    engine, reference,
                    "{case}: recorded as a body divergence but the engine now matches the \
                     reference; remove it from the body divergence tables"
                );
                assert_eq!(
                    heading_ids(&engine),
                    heading_ids(reference),
                    "{case}: the body divergence must not change the heading ids"
                );
                body.push_str(&example(markdown, &engine));
                body.push_str(&format!(
                    "Expected output differs from the reference outside the identifiers: \
                     {reason}; the heading identifiers agree.\n\n"
                ));
            } else {
                assert_eq!(
                    engine, reference,
                    "{case}: the engine's HTML differs from the reference without a documented \
                     divergence:\nengine:   {engine:?}\nreference: {reference:?}"
                );
                body.push_str(&example(markdown, reference));
                body.push('\n');
            }
        }
    }

    for (case, _) in DIVERGENCES {
        assert!(
            listed.contains(case),
            "recorded divergence {case:?} is not in any section"
        );
    }
    for (case, _) in BODY_DIVERGENCES {
        assert!(
            listed.contains(case),
            "recorded body divergence {case:?} is not in any section"
        );
    }
    let unlisted: Vec<&String> = fixture
        .keys()
        .filter(|key| !listed.contains(&key.as_str()))
        .collect();
    assert!(
        unlisted.is_empty(),
        "fixture cases missing from every section: {unlisted:?} — add them to SECTIONS"
    );

    let mut plain = String::new();
    plain.push_str("## Ordinary Markdown is unchanged\n\n");
    for source in PLAIN_CASES {
        plain.push_str(&example(source, &rendered(source)));
        plain.push('\n');
    }

    let spec = format!(
        "# Heading identifiers\n\
         \n\
         \n\
         These examples run with the `heading_identifiers` extension \
         enabled. Every expected output agrees with the reference \
         implementation and with the engine, byte for byte, except the \
         divergences marked below: four identifier divergences \
         (decision 0021) and four differences in the surrounding \
         markup where the reference itself does not follow CommonMark; \
         in the latter the heading identifiers still agree.\n\
         \n\
         The reference is marked with marked-gfm-heading-id, the only \
         real-world heading-identifier pipeline drivable end to end; \
         its output over this battery is frozen in \
         `tests/fixtures/heading_oracle.json`, generated by \
         `tools/heading_oracle_probe.mjs`. The engine derives a \
         heading's identifier from its decoded text content — GitHub's \
         DOM text semantics — while the reference derives it from the \
         heading's raw source text passed through an entity-eating \
         unescape and a lazy tag-strip regex. The four divergences \
         each trace to a non-conformance of the reference, not of the \
         engine; see decision 0021 \
         (`docs/decisions/0021-heading-ids-extract-dom-text.md`).\n\
         \n\
         The `Ordinary Markdown is unchanged` examples show the \
         engine's core CommonMark output (the 652-example core suite \
         covers it); the extension only adds identifiers to headings.\n\
         \n\
         Regenerate this file with \
         `cargo run -q -p geist4-ext --example gen_heading_spec`.\n\
         \n\
         {body}\
         {plain}"
    );
    std::fs::write(SPEC_PATH, spec).expect("write spec");
    println!(
        "wrote {SPEC_PATH}: {} fixture cases + {} plain cases",
        listed.len(),
        PLAIN_CASES.len()
    );
}
