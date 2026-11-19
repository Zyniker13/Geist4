//! The JSON tree format against its schema.
//!
//! Three layers of evidence: golden serializations pin the exact shape
//! of every node kind, the schema validates them, and every cross-binding
//! case — the full CommonMark spec plus every built-in extension's spec,
//! 1158 documents — parses, serializes and validates. If the schema and
//! the serializer drift apart, these tests fail.

use geist4_core::extension::Options;
use geist4_core::parser::parse_with;
use geist4_ext as ext;
use geist4_json::to_value;
use serde_json::Value;

/// The schema, compiled once per process.
fn schema() -> jsonschema::Validator {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/schema/geist4-tree-v1.json"
    ))
    .expect("the published schema must exist");
    let value = serde_json::from_str(&text).expect("the schema must be valid JSON");
    jsonschema::validator_for(&value).expect("the published schema must compile")
}

/// The extension for one cross-binding option name, or `None` for the
/// plain spec cases.
fn extension_for(name: &str) -> std::sync::Arc<dyn geist4_core::extension::Extension> {
    match name {
        "autolink" => ext::autolink(),
        "description_lists" => ext::deflists(),
        "footnotes" => ext::footnotes(),
        "front_matter" => ext::front_matter(),
        "heading_identifiers" => ext::heading_identifiers(),
        "strikethrough" => ext::strikethrough(),
        "tables" => ext::tables(),
        "tasklist" => ext::tasklist(),
        "tagfilter" => ext::tagfilter(),
        other => panic!("no built-in extension for the option {other}"),
    }
}

/// Parses `source` with the given built-in extensions and serializes it.
fn tree_with(source: &str, names: &[&str]) -> Value {
    let mut options = Options::new();
    for name in names {
        options.extension(extension_for(name));
    }
    let document = parse_with(source, &options);
    to_value(&document, source)
}

/// The tree for `source`, parsed with no extensions.
fn tree(source: &str) -> Value {
    tree_with(source, &[])
}

#[test]
fn every_kind_serializes_and_validates() {
    let validator = schema();
    // One document exercising every core node kind and one extension
    // kind each from the built-in extensions that add nodes. The front
    // matter must come first, and a footnote reference needs its
    // definition, so the document opens with both.
    let source = "\
---
fm: yes
---
# Heading
Setext
=======
Paragraph with *emphasis*, **strong**, [link](/u \"t\"), [ref][r],
![image](/i), an autolink <https://example.com>, `code`,
<span>html</span>, a soft
break, a hard  
break, footnote[^1], ~~strike~~, and #tag.

[^1]: the note

[r]: /r \"reference\"

- loose item
- second

- tight
- list

> quoted
>
> more

    indented code

```rust
fenced code
```

| a | b |
|---|:-:|
| 1 | 2 |

---

<div>html block</div>

1. ordered

term
: definition
";
    let mut options = Options::new();
    options.extension(ext::tables());
    options.extension(ext::footnotes());
    options.extension(ext::front_matter());
    options.extension(ext::heading_identifiers());
    options.extension(ext::autolink());
    options.extension(ext::tasklist());
    options.extension(ext::strikethrough());
    options.extension(ext::deflists());
    let document = parse_with(source, &options);
    let value = to_value(&document, source);
    let errors: Vec<String> = validator
        .iter_errors(&value)
        .map(|error| format!("{error}"))
        .collect();
    assert!(
        errors.is_empty(),
        "the kitchen-sink tree must validate: {errors:?}\n{value}"
    );
    // Spot the kinds that only extensions produce.
    let text = serde_json::to_string(&value).expect("serialization cannot fail");
    for kind in [
        "\"thematic_break\"",
        "\"code_block\"",
        "\"html_block\"",
        "\"code_span\"",
        "\"inline_html\"",
        "\"soft_break\"",
        "\"hard_break\"",
        "\"footnote_definition\"",
        "\"footnote_reference\"",
        "\"front_matter\"",
        "\"table_row\"",
        "\"description_list\"",
    ] {
        assert!(
            text.contains(kind),
            "the kitchen-sink tree must contain {kind}"
        );
    }
}

#[test]
fn golden_shapes_pin_the_fields() {
    // The exact top-level shape.
    let value = tree("Hello\n");
    assert_eq!(
        value,
        serde_json::json!({
            "format": "geist4/tree@1",
            "root": {
                "type": "document",
                "position": {
                    "start": {"line": 1, "column": 1, "offset": 0},
                    "end": {"line": 2, "column": 1, "offset": 6},
                },
                "children": [
                    {
                        "type": "paragraph",
                        "position": {
                            "start": {"line": 1, "column": 1, "offset": 0},
                            "end": {"line": 1, "column": 6, "offset": 5},
                        },
                        "children": [
                            {
                                "type": "text",
                                "value": "Hello",
                                "position": {
                                    "start": {"line": 1, "column": 1, "offset": 0},
                                    "end": {"line": 1, "column": 6, "offset": 5},
                                }
                            }
                        ]
                    }
                ]
            },
            "linkDefs": [],
            "problems": [],
        })
    );

    // A list with its data fields, and a link reference definition.
    let value = tree("2) two\n\n[r]: /r \"title\"\n");
    let text = serde_json::to_string(&value).expect("serialization cannot fail");
    assert!(
        text.contains("\"marker\":\")\"")
            && text.contains("\"ordered\":true")
            && text.contains("\"start\":2"),
        "the list must carry its marker, order and start: {text}"
    );
    assert!(
        text.contains("\"label\":\"r\"")
            && text.contains("\"titled\":true")
            && text.contains("\"title\":\"title\""),
        "the definition must carry its label, title and titled flag: {text}"
    );

    // A shortcut reference on the link and the resolved fields.
    let value = tree("[r]\n\n[r]: /r\n");
    let text = serde_json::to_string(&value).expect("serialization cannot fail");
    assert!(
        text.contains("\"kind\":\"shortcut\"") && text.contains("\"url\":\"/r\""),
        "the link must carry its reference kind and resolved URL: {text}"
    );
}

#[test]
fn utf16_positions_match_the_mdast_convention() {
    // Two UTF-16 code units per surrogate pair, one per BMP character.
    let source = "😀 ok\n";
    let value = tree(source);
    let text = &value["root"]["children"][0]["children"][0];
    assert_eq!(text["type"], "text");
    // The emoji is two UTF-16 units, so "😀 ok" ends at offset 5.
    assert_eq!(text["position"]["end"]["offset"], 5);
    assert_eq!(text["position"]["end"]["column"], 6);
}

/// Every span the parser records must name bytes inside the source it
/// parsed. A span past the end breaks every exporter that maps spans
/// through a line index.
#[test]
fn every_span_stays_in_bounds_across_the_corpus() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/bindings/cases.jsonl"
    );
    let cases = std::fs::read_to_string(path).expect("the cross-binding corpus must exist");
    for (index, line) in cases.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        let case: Value = serde_json::from_str(line)
            .unwrap_or_else(|error| panic!("case {index} must be JSON: {error}"));
        let source = case["source"]
            .as_str()
            .unwrap_or_else(|| panic!("case {index} must carry a source"));
        let names: Vec<&str> = case["options"]
            .as_array()
            .map(|list| list.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let mut options = Options::new();
        for name in names {
            options.extension(extension_for(name));
        }
        let document = parse_with(source, &options);
        let limit = source.len();
        for id in document.arena.iter() {
            let span = document.arena.span(id);
            assert!(
                span.end <= limit,
                "case {} node {} span {:?} exceeds the source length {limit}: {source:?}",
                case["name"].as_str().unwrap_or("?"),
                id.index(),
                span
            );
        }
    }
}

#[test]
fn every_cross_binding_case_validates() {
    let validator = schema();
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/bindings/cases.jsonl"
    );
    let cases = std::fs::read_to_string(path).expect("the cross-binding corpus must exist");
    let count = cases.lines().filter(|line| !line.trim().is_empty()).count();
    assert!(count >= 1000, "the corpus must be intact, found {count}");
    let mut validated = 0;
    for (index, line) in cases.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        let case: Value = serde_json::from_str(line)
            .unwrap_or_else(|error| panic!("case {index} must be JSON: {error}"));
        let source = case["source"]
            .as_str()
            .unwrap_or_else(|| panic!("case {index} must carry a source"));
        let names: Vec<&str> = case["options"]
            .as_array()
            .map(|list| list.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let value = tree_with(source, &names);
        let errors: Vec<String> = validator
            .iter_errors(&value)
            .map(|error| format!("{error}"))
            .collect();
        assert!(
            errors.is_empty(),
            "case {} ({}) must validate: {errors:?}",
            case["name"].as_str().unwrap_or("?"),
            index
        );
        validated += 1;
    }
    assert_eq!(validated, count, "every case must be validated");
}

/// The bug that motivated the bounds test: a definition term is built
/// from the paragraph before it, and its moved text must keep the
/// source offsets the paragraph's inline phase had, not the term's own
/// empty span.
#[test]
fn deflists_moved_text_keeps_its_source_spans() {
    let value = tree_with("Term **here**\n: Def\n", &["description_lists"]);
    let term = &value["root"]["children"][0]["children"][0]["children"][0];
    assert_eq!(term["type"], "description_term");
    let strong = &term["children"][1];
    assert_eq!(strong["type"], "strong");
    assert_eq!(strong["position"]["start"]["offset"], 5);
    assert_eq!(strong["position"]["end"]["offset"], 13);
}

#[test]
fn problems_and_defs_carry_positions() {
    let value = tree("[broken]\n\n[dup]: /a\n[dup]: /b\n");
    assert_eq!(value["problems"].as_array().map(Vec::len), Some(1));
    let problem = &value["problems"][0];
    assert!(problem["message"].as_str().is_some());
    assert!(problem["position"]["start"]["line"].as_u64().is_some());
    assert_eq!(value["linkDefs"].as_array().map(Vec::len), Some(2));
    for def in value["linkDefs"].as_array().expect("defs").iter().take(1) {
        assert!(def["titled"].is_boolean());
        assert!(def["position"]["end"]["offset"].as_u64().is_some());
    }
}
