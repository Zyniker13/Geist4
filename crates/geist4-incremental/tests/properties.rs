//! Property tests for the incremental update: for every pair of
//! sources and every editor-like edit chain, `Editor::update` either
//! produces a document value-identical to a full parse or falls back
//! to a full parse (trivially identical); a mismatch is a bug. The
//! oracle is the lockstep tree walk from the differential corpus,
//! shared through the corpus's test binary — this file cross-checks
//! the random cases against it.

use geist4_core::debug::print as dump_tree;
use geist4_core::extension::Options;
use geist4_core::parser::{parse_document, parse_with};
use geist4_incremental::Editor;
use proptest::prelude::*;
use proptest::test_runner::RngSeed;

/// Fixed proptest seed, the pattern of `geist4-core/tests/properties.rs`
/// (decision 0041): the seed only stabilizes the safety-net; the
/// deterministic differential corpus carries the real coverage.
const PROPTEST_SEED: u64 = 2026_1123;

/// The tree-and-collections comparison of `tests/differential.rs`
/// (`assert_same`), duplicated here so this file has no cross-test
/// dependency: kinds, attributes, ownership and child counts in a
/// lockstep walk, plus defs, the definition map and problems. Arena
/// sizes are never compared (full parses legitimately keep unreachable
/// block-phase text nodes).
fn assert_same(
    actual: &geist4_core::parser::Document<'_>,
    expected: &geist4_core::parser::Document<'_>,
) {
    assert_eq!(
        dump_tree(&actual.arena, actual.root),
        dump_tree(&expected.arena, expected.root),
        "incremental tree differs from the full parse's tree"
    );
    let mut stack = vec![(actual.root, expected.root)];
    while let Some((a, e)) = stack.pop() {
        assert_eq!(actual.arena.kind(a), expected.arena.kind(e));
        assert_eq!(actual.arena.attrs(a), expected.arena.attrs(e));
        assert_eq!(actual.arena.extension_of(a), expected.arena.extension_of(e));
        let a_kids = actual.arena.children(a);
        let e_kids = expected.arena.children(e);
        assert_eq!(a_kids.len(), e_kids.len());
        for (a_child, e_child) in a_kids.iter().zip(e_kids.iter()) {
            stack.push((*a_child, *e_child));
        }
    }
    assert_eq!(actual.defs, expected.defs, "definitions differ");
    assert_eq!(
        actual.link_defs, expected.link_defs,
        "definition maps differ"
    );
    assert_eq!(actual.problems, expected.problems, "problems differ");
}

/// Splits a document into paragraph blocks (any nonblank text run
/// separated by blank lines) so edits land at block boundaries or
/// inside blocks, whichever the strategy picks.
fn paragraphs(src: &str) -> Vec<String> {
    src.split("\n\n").map(str::to_string).collect()
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        rng_seed: RngSeed::Fixed(PROPTEST_SEED),
        ..ProptestConfig::default()
    })]

    /// One edit per update: the previous document's source, the new
    /// source, nothing else. The update may fall back; it may never
    /// disagree with a full parse.
    #[test]
    fn one_edit_never_disagrees_with_a_full_parse(
        blocks in proplist("[a-zA-Z0-9 ,.:;*_-]{0,20}", 1..8),
        edit_kind in 0usize..4,
        edit_index in any::<usize>(),
        word in "[a-zA-Z]{0,12}",
    ) {
        let old = blocks.join("\n\n");
        let mut parts = paragraphs(&old);
        let at = if parts.is_empty() { 0 } else { edit_index % parts.len() };
        let new = match edit_kind {
            // Append a line inside the chosen paragraph.
            0 => {
                if parts.is_empty() { old.clone() } else {
                    let joined = format!("{}\n{}", parts[at], word);
                    parts[at] = joined;
                    parts.join("\n\n")
                }
            }
            // Replace the chosen paragraph's first word.
            1 => {
                if parts.is_empty() { old.clone() } else {
                    parts[at] = word.clone() + parts[at].strip_prefix(word.as_str()).unwrap_or(&parts[at]);
                    parts.join("\n\n")
                }
            }
            // Delete the chosen paragraph.
            2 => {
                parts.remove(at);
                parts.join("\n\n")
            }
            // Insert a new paragraph before the chosen one.
            _ => {
                parts.insert(at, word.clone());
                parts.join("\n\n")
            }
        };
        if new == old {
            return Ok(());
        }
        let editor = Editor::new(Options::new());
        let previous = parse_document(&old);
        let (document, _) = editor.update(&previous, &old, &new);
        assert_same(&document, &parse_document(&new));
    }

    /// A chain of edits: every link must be value-identical to the
    /// full parse of its source, which is what an editor session
    /// requires of every intermediate state.
    #[test]
    fn an_edit_chain_matches_at_every_step(
        blocks in proplist("[a-zA-Z0-9 ,.:;]{1,16}", 2..5),
        words in proplist("[a-zA-Z]{0,10}", 1..6),
        edit_index in any::<usize>(),
    ) {
        let mut sources = vec![blocks.join("\n\n")];
        for word in words {
            let mut parts = paragraphs(sources.last().expect("nonempty"));
            if parts.is_empty() {
                parts.push(word);
            } else {
                let at = edit_index % parts.len();
                parts[at] = format!("{} {}", parts[at], word);
            }
            sources.push(parts.join("\n\n"));
        }
        let editor = Editor::new(Options::new());
        let mut previous = parse_document(&sources[0]);
        for pair in sources.windows(2) {
            if pair[1] == pair[0] {
                continue;
            }
            let (document, _) = editor.update(&previous, &pair[0], &pair[1]);
            assert_same(&document, &parse_document(&pair[1]));
            previous = document;
        }
    }
}

/// A strategy producing `Vec<String>`: `size` whitespace-free-ish
/// lines built from the regex, with repetitions allowed. Distinct
/// from `any::<Vec<String>>()` so the sources stay small and the
/// failures shrink to readable paragraph sets.
fn proplist(regex: &str, size: std::ops::Range<usize>) -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec(regex, size)
}

/// A full parse of the new source with the editor's options, for the
/// fallback path's options-must-match guard. Unused today (plain
/// options only), kept for the extension property to come.
#[allow(dead_code)]
fn parse_with_options<'a>(
    source: &'a str,
    options: &'a Options,
) -> geist4_core::parser::Document<'a> {
    parse_with(source, options)
}
