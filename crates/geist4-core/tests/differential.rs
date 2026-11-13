//! Differential tests against the reference implementation.
//!
//! Inputs are generated deterministically from a Markdown vocabulary and
//! run through both engines; the HTML output must be byte-identical. The
//! test skips itself when the `cmark` binary is not on PATH (CI installs
//! it on Linux and macOS; Windows has no package, so the differential
//! layer runs only where the reference binary exists).

use std::path::PathBuf;
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
    // The first line looks like `cmark 0.31.2 - CommonMark converter`;
    // pick the token that is a dotted triple of numbers rather than
    // counting positions, which vary between builds.
    text.split_whitespace()
        .find(|token| is_version_token(token))
        .map(|version| version.trim().to_owned())
}

fn is_version_token(token: &str) -> bool {
    let all_digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    let mut parts = token.split('.');
    all_digits(parts.next().unwrap_or(""))
        && all_digits(parts.next().unwrap_or(""))
        && all_digits(parts.next().unwrap_or(""))
        && parts.next().is_none()
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

/// The reference CLI's output, with its one trailing newline removed,
/// compared against our own output which never ends in a newline. For an
/// empty render the reference emits nothing at all, so appending to ours
/// would invent a newline that has no counterpart.
fn theirs_without_newline(theirs: &str) -> &str {
    theirs.strip_suffix('\n').unwrap_or(theirs)
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
        if ours != theirs_without_newline(&theirs) {
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

/// The fuzzer's minimized corpus, double-checked against the reference
/// implementation: every input the fuzzer found interesting must render
/// identically, not merely without crashing. Runs only where the
/// reference binary exists at the pinned version; skips when the corpus
/// has not been checked out.
#[test]
fn differential_over_fuzz_corpus() {
    match cmark_version() {
        None => {
            eprintln!("skipping: the cmark reference binary is not installed");
            return;
        }
        Some(version) if version != "0.31.2" => {
            eprintln!("skipping: the installed cmark is {version}, not 0.31.2");
            return;
        }
        Some(_) => {}
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/seeds");
    let Ok(corpus_dirs) = std::fs::read_dir(&dir) else {
        eprintln!("skipping: no fuzz seed corpus directory");
        return;
    };
    let mut files: Vec<PathBuf> = corpus_dirs
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.is_dir())
        .flat_map(|sub| std::fs::read_dir(sub).expect("read corpus subdirectory"))
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.is_file())
        .collect();
    files.sort();
    if files.is_empty() {
        eprintln!("skipping: the fuzz corpus is empty");
        return;
    }
    let mut failures = 0usize;
    let mut checked = 0usize;
    let mut first_difference: Option<(PathBuf, String, String, String)> = None;
    for path in files {
        let bytes = std::fs::read(&path).expect("read corpus input");
        if bytes.len() > 256 * 1024 {
            continue;
        }
        let input = String::from_utf8_lossy(&bytes).into_owned();
        let theirs = reference_html(&input);
        let ours = ours_html(&input);
        checked += 1;
        if ours != theirs_without_newline(&theirs) {
            failures += 1;
            if std::env::var_os("GEIST4_DIFF_DUMP").is_some() {
                let name = format!("{:04}", failures);
                let dir = std::path::PathBuf::from(
                    std::env::var_os("GEIST4_DIFF_DUMP").expect("dump dir"),
                );
                std::fs::write(dir.join(format!("{name}.md")), &input).ok();
                std::fs::write(dir.join(format!("{name}.ours")), &ours).ok();
                std::fs::write(dir.join(format!("{name}.theirs")), &theirs).ok();
            }
            if first_difference.is_none() {
                first_difference = Some((path, input, ours, theirs));
            }
        }
    }
    assert!(checked > 0, "no corpus inputs were checked");
    if let Some((path, input, ours, theirs)) = first_difference {
        panic!(
            "{failures} of {checked} fuzz-corpus inputs differ\nfile: {}\ninput: {input:?}\nours:   {ours}\ntheirs: {theirs}",
            path.display()
        );
    }
}
