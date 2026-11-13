//! Differential tests against the reference implementation.
//!
//! Inputs are generated deterministically from a Markdown vocabulary and
//! run through both engines; the HTML output must be byte-identical. The
//! test skips itself when the `cmark` binary is not on PATH (CI installs
//! it on Linux and macOS; Windows has no package, so the differential
//! layer runs only where the reference binary exists).

use std::process::Command;

/// The seed for the deterministic generator; fixed so failures reproduce.
const SEED: u64 = 0x4745_4953_5434; // "GEIST4"

/// Deterministic xorshift64* generator.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// The vocabulary: tokens that exercise block structure, emphasis,
/// links, code, HTML and escapes.
const TOKENS: &[&str] = &[
    "# ",
    "## ",
    "### ",
    ">",
    ">>",
    "- ",
    "* ",
    "1. ",
    "2) ",
    "    ",
    "```",
    "```rust",
    "~~~",
    "\n",
    "\n\n",
    " ",
    "\t",
    "a",
    "b",
    "foo",
    "bar",
    "*",
    "**",
    "***",
    "_",
    "__",
    "`",
    "``",
    "[",
    "]",
    "[a]",
    "[a]:",
    "[a]: /u",
    "[a]: /u \"t\"",
    "(",
    ")",
    "(/u)",
    "(\"t\")",
    "!",
    "![a]",
    "<",
    ">",
    "<b>",
    "</b>",
    "<!--",
    "-->",
    "&amp;",
    "&",
    "\\",
    "\\*",
    "\\`",
    "|",
    "---",
    "===",
    "___",
    "___\n",
    "x\n===",
    "a  \n",
    "\r\n",
    "0",
    "1",
    ".",
    ":",
    "\"",
    "'",
    "\u{00E9}",
    "\u{4E2D}",
    "\u{0301}",
    "\u{FFFD}",
    "<http://x.y>",
    "http://x.y",
    "www.x.y",
];

fn generate(rng: &mut Rng, token_count: usize) -> String {
    let mut out = String::new();
    for _ in 0..token_count {
        out.push_str(TOKENS[rng.below(TOKENS.len())]);
    }
    out
}

/// Returns the reference implementation's version, or `None` when the
/// binary is missing. Only 0.31.2 output matches this suite: other
/// versions differ by spec changes, so they must not be compared.
fn cmark_version() -> Option<String> {
    let out = Command::new("cmark").arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let version = text.split_whitespace().nth(2)?;
    Some(version.trim().to_owned())
}

fn reference_html(src: &str) -> String {
    use std::io::Write;
    let mut child = Command::new("cmark")
        .args(["--to", "html", "--width", "0", "--unsafe"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn cmark");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(src.as_bytes())
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait for cmark");
    assert!(out.status.success(), "cmark failed");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn ours_html(src: &str) -> String {
    geist4_core::basic_html::render(&geist4_core::parser::parse_document(src))
}

/// Our output with the final newline the reference CLI emits.
fn ours_ending_in_newline(ours: &str) -> String {
    let mut out = ours.to_owned();
    out.push('\n');
    out
}

#[test]
fn differential_against_reference() {
    match cmark_version() {
        None => {
            eprintln!("skipping: the cmark reference binary is not installed");
            return;
        }
        Some(version) if version != "0.31.2" => {
            eprintln!(
                "skipping: the installed cmark is {version}, but this suite \
                 compares against 0.31.2; install cmark 0.31.2 to run it"
            );
            return;
        }
        Some(_) => {}
    }
    let mut rng = Rng(SEED);
    let mut failures = 0usize;
    let mut first_difference: Option<(String, String, String)> = None;
    for _case in 0..400 {
        let token_count = 8 + rng.below(40);
        let input = generate(&mut rng, token_count);
        let theirs = reference_html(&input);
        let ours = ours_html(&input);
        // The reference CLI ends its output with a newline; our renderer
        // does not (the spec suite's convention). Compare modulo that.
        if ours_ending_in_newline(&ours) != theirs {
            failures += 1;
            if first_difference.is_none() {
                first_difference = Some((input, ours, theirs));
            }
        }
    }
    if let Some((input, ours, theirs)) = first_difference {
        panic!(
            "{failures} of 400 differential cases differ\ninput: {input:?}\nours:   {ours}\ntheirs: {theirs}"
        );
    }
}
