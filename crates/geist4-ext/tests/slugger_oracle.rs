//! Oracle tests for the github-slugger 2.0.0 port in [`geist4_ext::slugger`].
//!
//! `fixtures/slugger_oracle.json` is machine-generated from the reference
//! implementation by `tools/slugger_probe.mjs` and frozen: it is the
//! source of truth, so do not edit it to make a test pass — fix the port
//! or the generator instead.
//!
//! The fixture records the outputs of the dedupulation scripts but not
//! their inputs; the `DEDUP_SCRIPTS` literal below is transcribed
//! verbatim from `tools/slugger_probe.mjs` (`const dedupScripts`), with
//! every non-ASCII character written as a JSON `\uXXXX` escape so this
//! file stays pure ASCII. The escape sequences, not any re-derivation
//! from the JavaScript, are what these tests execute against.

use std::borrow::Cow;

use geist4_ext::slugger::{self, Slugger};

const ORACLE_JSON: &str = include_str!("fixtures/slugger_oracle.json");

/// The `dedupScripts` literal of `tools/slugger_probe.mjs`: pairs of
/// `[label, args]`, where each arg is an input string or an
/// `[input, maintain_case]` pair.
const DEDUP_SCRIPTS: &str = r#"[
 ["repeatA", ["A","A","A","a","A","a"]],
 ["repeatAa", ["A a","A a","a A","a  a"]],
 ["collide-dash1", ["a","a-1","a","a-2","a","a-1"]],
 ["collide-dash2", ["a","a","a-1","a-2","a","a-1","a-3","a"]],
 ["numSuffix", ["x","x","x","x-1","x","x-1","x-2","x"]],
 ["empty", ["","",""]],
 ["dash", ["-","-","-"]],
 ["space", [" "," "," "]],
 ["case", ["Foo","foo","Foo",["FOO",true],"foo",["Foo",true],["FOO",true]]],
 ["maintain", [["A",true],"a",["A",true],"A",["a",true],["a",false]]],
 ["unicode", ["\u00C9","\u00E9","\u00C9","\u0395","\u03B5","\u0130","i\u0307","\u0130"]],
 ["emoji", ["\uD83D\uDE00","\uD83D\uDE00","\uD83D\uDE00 \uD83C\uDF89","\uD83D\uDE00","\uD83C\uDF89 \uD83D\uDE00","\uD83D\uDE00 \uD83C\uDF89"]],
 ["mixed", ["\u00A71 a","\u00A71 a","\u00A71 a","1. a","1. a"]],
 ["collideNum", ["a-1","a","a","a","a-1","a-1"]]
]"#;

/// Decodes a `slug:` key suffix back to the probed input.
///
/// The probe keys results by the probed input, except that an input
/// probed more than once is keyed for its later occurrences as the
/// `JSON.stringify`d input followed by the pre-increment occurrence
/// count (e.g. `"a­b"0` is the second call with `a­b`). A bare `"` is
/// the raw quote input itself (it has no closing quote, so it cannot be
/// a counter key).
fn decode_key(suffix: &str) -> Option<Cow<'_, str>> {
    if suffix == "\"" {
        return Some(Cow::Borrowed(suffix));
    }
    if suffix.starts_with('"') {
        let close = suffix.rfind('"')?;
        let (literal, digits) = suffix.split_at(close + 1);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        return serde_json::from_str(literal).ok();
    }
    Some(Cow::Borrowed(suffix))
}

/// Every `slug:` entry of the oracle must equal the ported free `slug`
/// (maintain-case off), and every non-`slug:` entry — the probe's
/// occurrence counter for a raw input — must agree with the counter-key
/// encoding that recorded the input's later occurrences.
#[test]
fn pure_slug_matches_oracle() {
    let oracle: serde_json::Value =
        serde_json::from_str(ORACLE_JSON).expect("oracle fixture parses");
    let results = oracle["results"].as_object().expect("results is an object");
    let mut checked = 0;
    for (key, value) in results {
        let Some(suffix) = key.strip_prefix("slug:") else {
            // Occurrence-counter record for a raw input; the probe writes
            // 0 on first sight and pre-increments on each later one, so
            // the value is the number of extra occurrences.
            let extra = value.as_u64().unwrap_or_else(|| {
                panic!("occurrence counter for {key:?} is a non-negative integer")
            });
            let expected = results
                .get(&format!("slug:{key}"))
                .unwrap_or_else(|| panic!("counter entry {key:?} has no slug: record"))
                .as_str()
                .unwrap_or_else(|| panic!("slug record for {key:?} is a string"));
            assert_eq!(
                slugger::slug(key, false),
                expected,
                "counter entry {key:?} ({extra} extra occurrences)"
            );
            continue;
        };
        let input =
            decode_key(suffix).unwrap_or_else(|| panic!("cannot decode fixture key {key:?}"));
        let expected = value
            .as_str()
            .unwrap_or_else(|| panic!("result for {key:?} must be a string"));
        assert_eq!(slugger::slug(&input, false), expected, "input {input:?}");
        checked += 1;
    }
    assert!(
        checked >= 300,
        "expected the full oracle slug table, got only {checked} slug entries"
    );
}

/// The dedup scripts replay github-slugger's occurrence counting: each
/// script starts from a fresh [`Slugger`] and its oracle outputs must
/// match call by call, including the `-1`, `-2` suffixes and the
/// while-loop recheck against colliding `original-<n>` slugs.
#[test]
fn dedup_scripts_match_oracle() {
    let oracle: serde_json::Value =
        serde_json::from_str(ORACLE_JSON).expect("oracle fixture parses");
    let dedup = oracle["dedup"].as_object().expect("dedup is an object");
    let scripts: serde_json::Value =
        serde_json::from_str(DEDUP_SCRIPTS).expect("embedded dedup scripts parse");
    let scripts = scripts.as_array().expect("dedup scripts are an array");
    assert_eq!(
        scripts.len(),
        dedup.len(),
        "script labels cover the fixture"
    );
    for entry in scripts {
        let pair = entry
            .as_array()
            .expect("script entry is a [label, args] pair");
        let label = pair[0].as_str().expect("script label is a string");
        let args = pair[1].as_array().expect("script args are an array");
        let expected = dedup
            .get(label)
            .unwrap_or_else(|| panic!("fixture has no dedup script {label:?}"))
            .as_array()
            .unwrap_or_else(|| panic!("dedup {label:?} outputs are an array"));
        assert_eq!(expected.len(), args.len(), "dedup {label:?} output count");
        let mut slugger = Slugger::new();
        for (i, arg) in args.iter().enumerate() {
            let (input, maintain_case) = match arg {
                serde_json::Value::String(input) => (input.as_str(), false),
                serde_json::Value::Array(pair) => (
                    pair[0].as_str().expect("maintain-case input is a string"),
                    pair[1].as_bool().expect("maintain-case flag is a bool"),
                ),
                other => panic!("unexpected dedup arg in {label:?}: {other}"),
            };
            let expected_out = expected[i]
                .as_str()
                .unwrap_or_else(|| panic!("dedup {label:?} output {i} is a string"));
            assert_eq!(
                slugger.slug(input, maintain_case),
                expected_out,
                "dedup {label:?} step {i} ({input:?})"
            );
        }
    }
}

/// The probe's maintain-case one-offs each use a fresh slugger with
/// `maintainCase = true`; keys are the raw inputs.
#[test]
fn maintain_case_oneoffs_match_oracle() {
    let oracle: serde_json::Value =
        serde_json::from_str(ORACLE_JSON).expect("oracle fixture parses");
    let maintain = oracle["maintain"]
        .as_object()
        .expect("maintain is an object");
    assert!(!maintain.is_empty(), "maintain oneoffs are present");
    for (input, expected) in maintain {
        let expected = expected
            .as_str()
            .unwrap_or_else(|| panic!("maintain output for {input:?} is a string"));
        let mut slugger = Slugger::new();
        assert_eq!(
            slugger.slug(input, true),
            expected,
            "maintain-case {input:?}"
        );
    }
}
