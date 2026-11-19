//! Fuzz: deterministic pseudo-random token sequences from a tricky pool
//! must round-trip through the writer. The seed is fixed so failures are
//! reproducible; raise the iteration count locally when touching the
//! escaper or the emphasis writer.

use geist4_core::basic_html::render;
use geist4_core::parser::parse_document;
use geist4_writer::to_common_mark;

const TOKENS: &[&str] = &[
    "a", "*", "_", "**", "***", "`", "``", "[", "]", "(", ")", "!", "<", ">", "&", "\\", "\"", "#",
    ">", "-", "+", "1.", "=", "~", "|", "'", ":", "/", "%", " ", "  ", "\t", "\n", "\n\n", "&amp;",
    "&#10;", "&#32;", "<b>", "<div>", "é", "—", "ü", "x=y", "a;b", "(x)", "*a*", "_a_", "[a]",
    "[a]: u", "~~", "```", "1)", ".", "!", "&#", "a&b", "\\*", "\\_", "\\[",
];

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[test]
fn fuzz_round_trip() {
    let mut rng = Rng(0x5eed_1234_abcd_9876);
    let mut failures = 0usize;
    for _ in 0..4000 {
        let count = (rng.next() % 14) as usize + 1;
        let mut source = String::new();
        for _ in 0..count {
            source.push_str(TOKENS[(rng.next() as usize) % TOKENS.len()]);
        }
        let doc1 = parse_document(&source);
        let once = to_common_mark(&doc1);
        let html1 = render(&doc1);
        let doc2 = parse_document(&once);
        let html2 = render(&doc2);
        if html1 != html2 {
            failures += 1;
            if failures <= 8 {
                eprintln!(
                    "HTML FAIL src={source:?}\n  written={once:?}\n  h1={html1:?}\n  h2={html2:?}"
                );
            }
            continue;
        }
        let twice = to_common_mark(&doc2);
        if twice != once {
            failures += 1;
            if failures <= 8 {
                eprintln!("IDEM FAIL src={source:?}\n  w1={once:?}\n  w2={twice:?}");
            }
        }
    }
    assert_eq!(failures, 0, "{failures} fuzz failures");
}
