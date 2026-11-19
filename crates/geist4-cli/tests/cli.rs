//! Integration tests for the `geist4` binary's `check` and `ext check`
//! subcommands. Each test builds its fixture packages in a unique
//! temporary directory and runs the compiled binary against them, the
//! way a user would.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// The next unique suffix for the temporary fixture directories.
static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

/// A unique temporary directory, removed when the test ends.
struct TempDir(PathBuf);

impl TempDir {
    /// Creates the directory.
    fn new(tag: &str) -> Self {
        let unique = NEXT_DIRECTORY.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("geist4-cli-{tag}-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).expect("the temporary directory is created");
        Self(path)
    }

    /// The directory's path.
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs the `geist4` binary with `args` and returns its output.
fn run_cli<I, S>(args: I) -> std::process::Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    std::process::Command::new(env!("CARGO_BIN_EXE_geist4"))
        .args(args)
        .output()
        .expect("the geist4 binary runs")
}

/// The stderr as text.
fn stderr_of(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The stdout as text.
fn stdout_of(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The fence line the spec example format opens and closes examples
/// with: exactly thirty-two backticks.
fn fence() -> String {
    "`".repeat(32)
}

/// One spec example in the CommonMark example format.
fn example(markdown: &str, expected: &str) -> String {
    format!(
        "{} example\n{markdown}\n.\n{expected}\n{}\n",
        fence(),
        fence()
    )
}

/// A spec file holding `examples` under one section heading.
fn spec(examples: &str) -> String {
    format!("# Examples\n\n{examples}")
}

/// The manifest text for a package whose behavior is one inline span,
/// with `extra` added to the `[extension]` table.
fn manifest(name: &str, kind: &str, open: &str, close: &str, html: &str, extra: &str) -> String {
    format!(
        "[extension]\nname = \"{name}\"\nversion = \"1.0.0\"\nformat = \"1.0\"\n\
         description = \"A test extension\"\nlicense = \"MIT\"\n{extra}\
         [[kinds]]\nkind = \"{kind}\"\ninline_text = true\nattrs = [\"color\"]\n\
         [[inline_span]]\nkind = \"{kind}\"\nopen = \"{open}\"\nclose = \"{close}\"\n\
         html = \"{html}\"\n"
    )
}

/// The manifest for the `==highlight==` package named `name`.
fn highlight_manifest(name: &str, extra: &str) -> String {
    manifest(name, "hl", "==", "==", "<mark>{children}</mark>", extra)
}

/// Writes one fixture package into `root` and returns its directory.
fn package(root: &Path, name: &str, manifest_text: &str, spec_text: &str) -> PathBuf {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).expect("the package directory is created");
    std::fs::write(dir.join("extension.toml"), manifest_text).expect("the manifest is written");
    std::fs::write(dir.join("spec.md"), spec_text).expect("the spec is written");
    dir
}

#[test]
fn ext_check_passes_a_correct_package() {
    let temp = TempDir::new("pass");
    let alpha = package(
        temp.path(),
        "alpha",
        &highlight_manifest("alpha", ""),
        &spec(&example("a ==mark== b", "<p>a <mark>mark</mark> b</p>")),
    );
    let output = run_cli(["ext", "check", alpha.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr_of(&output));
    assert_eq!(stdout_of(&output), "", "success stays quiet");
}

#[test]
fn ext_check_runs_with_a_registered_dependency() {
    let temp = TempDir::new("dependency");
    let main = package(
        temp.path(),
        "main",
        &highlight_manifest("main", ""),
        &spec(
            &(example("a ==mark== b", "<p>a <mark>mark</mark> b</p>")
                + &example("a ++u++ b", "<p>a <u>u</u> b</p>")),
        ),
    );
    let other = package(
        temp.path(),
        "other",
        &manifest("other", "ul", "++", "++", "<u>{children}</u>", ""),
        &spec(&example("a ++u++ b", "<p>a <u>u</u> b</p>")),
    );
    let output = run_cli([
        "ext",
        "check",
        main.to_str().unwrap(),
        "--ext",
        other.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr_of(&output));
}

#[test]
fn ext_check_reports_failing_examples() {
    let temp = TempDir::new("failing");
    let alpha = package(
        temp.path(),
        "alpha",
        &highlight_manifest("alpha", ""),
        &spec(&example("a ==mark== b", "<p>wrong</p>")),
    );
    let output = run_cli(["ext", "check", alpha.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr_of(&output);
    assert!(stderr.contains("passed 0 of 1"), "{stderr}");
    assert!(stderr.contains("example 1"), "{stderr}");
    assert!(stderr.contains("expected:"), "{stderr}");
    assert!(stderr.contains("<p>wrong</p>"), "{stderr}");
    assert!(stderr.contains("actual:"), "{stderr}");
    assert!(stderr.contains("<p>a <mark>mark</mark> b</p>"), "{stderr}");
}

#[test]
fn ext_check_reports_a_broken_manifest() {
    let temp = TempDir::new("manifest");
    let broken = temp.path().join("broken");
    std::fs::create_dir_all(&broken).expect("the package directory is created");
    std::fs::write(broken.join("extension.toml"), "not [ valid").expect("the manifest is written");
    std::fs::write(broken.join("spec.md"), spec(&example("x", "<p>x</p>")))
        .expect("the spec is written");
    let output = run_cli(["ext", "check", broken.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr_of(&output);
    assert!(stderr.contains("broken"), "{stderr}");
}

#[test]
fn ext_manifest_validates_without_running_the_spec() {
    let temp = TempDir::new("manifestcheck");
    // A valid package passes even when its spec examples are wrong:
    // this command is the validation half of `ext check`.
    let alpha = package(
        temp.path(),
        "alpha",
        &highlight_manifest("alpha", ""),
        &spec(&example("a ==mark== b", "<p>wrong</p>")),
    );
    let output = run_cli(["ext", "manifest", alpha.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr_of(&output));
    assert_eq!(stdout_of(&output), "", "success stays quiet");

    // An unsupported format version is rejected with the loader's
    // one-line message.
    let old_manifest =
        highlight_manifest("old", "").replace("format = \"1.0\"", "format = \"2.0\"");
    let old = package(
        temp.path(),
        "old",
        &old_manifest,
        &spec(&example("x", "<p>x</p>")),
    );
    let output = run_cli(["ext", "manifest", old.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr_of(&output);
    assert!(stderr.contains("extension.format"), "{stderr}");
    assert!(stderr.contains("not supported"), "{stderr}");
}

#[test]
fn ext_check_rejects_conflicting_packages() {
    let temp = TempDir::new("conflict");
    let alpha = package(
        temp.path(),
        "alpha",
        &highlight_manifest("alpha", "conflicts = [\"beta\"]\n"),
        &spec(&example("a ==mark== b", "<p>a <mark>mark</mark> b</p>")),
    );
    let beta = package(
        temp.path(),
        "beta",
        &manifest("beta", "ul", "++", "++", "<u>{children}</u>", ""),
        &spec(&example("a ++u++ b", "<p>a <u>u</u> b</p>")),
    );
    let output = run_cli([
        "ext",
        "check",
        alpha.to_str().unwrap(),
        "--ext",
        beta.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr_of(&output);
    assert!(stderr.contains("conflicts"), "{stderr}");
    assert!(stderr.contains("alpha"), "{stderr}");
    assert!(stderr.contains("beta"), "{stderr}");
}

#[test]
fn ext_check_rejects_duplicate_names() {
    let temp = TempDir::new("duplicate");
    let first = package(
        temp.path(),
        "first",
        &highlight_manifest("dup", ""),
        &spec(&example("a ==mark== b", "<p>a <mark>mark</mark> b</p>")),
    );
    let second = package(
        temp.path(),
        "second",
        &highlight_manifest("dup", ""),
        &spec(&example("a ==mark== b", "<p>a <mark>mark</mark> b</p>")),
    );
    let output = run_cli([
        "ext",
        "check",
        first.to_str().unwrap(),
        "--ext",
        second.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr_of(&output);
    assert!(
        stderr.contains("duplicate extension name 'dup'"),
        "{stderr}"
    );
}

#[test]
fn ext_check_reports_a_spec_without_examples() {
    let temp = TempDir::new("empty-spec");
    let alpha = package(
        temp.path(),
        "alpha",
        &highlight_manifest("alpha", ""),
        "# A section with no examples\n",
    );
    let output = run_cli(["ext", "check", alpha.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr_of(&output);
    assert!(stderr.contains("spec.md contains no examples"), "{stderr}");
}

#[test]
fn ext_check_prints_at_most_ten_failing_examples_in_full() {
    let temp = TempDir::new("cap");
    let mut examples = String::new();
    for number in 0..12 {
        let _ = std::write!(
            &mut examples,
            "{}",
            example(&format!("word {number}"), "<p>incorrect</p>")
        );
    }
    let alpha = package(
        temp.path(),
        "alpha",
        &highlight_manifest("alpha", ""),
        &spec(&examples),
    );
    let output = run_cli(["ext", "check", alpha.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr_of(&output);
    assert!(stderr.contains("passed 0 of 12"), "{stderr}");
    assert!(stderr.contains("... and 2 more"), "{stderr}");
    assert!(!stderr.contains("example 11"), "{stderr}");
    assert!(!stderr.contains("example 12"), "{stderr}");
}

#[test]
fn check_accepts_a_clean_document() {
    let temp = TempDir::new("clean");
    let file = temp.path().join("doc.md");
    std::fs::write(&file, "hello world\n").expect("the document is written");
    let output = run_cli(["check", file.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr_of(&output));
    assert_eq!(stdout_of(&output), "ok\n");
    assert_eq!(stderr_of(&output), "");
}

#[test]
fn check_reports_a_broken_reference_with_its_position() {
    let temp = TempDir::new("reference");
    let file = temp.path().join("doc.md");
    std::fs::write(&file, "[text][missing]\n").expect("the document is written");
    let output = run_cli(["check", file.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    // The parser records the failed group twice with the same message
    // and span; the report deduplicates the pair to one line.
    assert_eq!(
        stderr_of(&output),
        "1:1: no link reference definition for 'missing'\n"
    );
}

#[test]
fn check_reports_each_failing_line() {
    let temp = TempDir::new("lines");
    let file = temp.path().join("doc.md");
    std::fs::write(&file, "first\n\nsee [word] text\n").expect("the document is written");
    let output = run_cli(["check", file.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr_of(&output);
    assert!(
        stderr.contains("3:1: no link reference definition for 'word'"),
        "{stderr}"
    );
}

#[test]
fn check_reports_a_missing_file() {
    let temp = TempDir::new("missing-file");
    let file = temp.path().join("absent.md");
    let output = run_cli(["check", file.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr_of(&output);
    assert!(stderr.contains("cannot read"), "{stderr}");
}

#[test]
fn check_reports_an_extension_load_failure() {
    let temp = TempDir::new("load-failure");
    let broken = temp.path().join("broken");
    std::fs::create_dir_all(&broken).expect("the package directory is created");
    std::fs::write(broken.join("extension.toml"), "not [ valid").expect("the manifest is written");
    let file = temp.path().join("doc.md");
    std::fs::write(&file, "hello world\n").expect("the document is written");
    let output = run_cli([
        "check",
        file.to_str().unwrap(),
        "--ext",
        broken.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr_of(&output);
    assert!(stderr.contains("broken"), "{stderr}");
}

#[test]
fn render_html_matches_the_renderer() {
    let temp = TempDir::new("render-html");
    let file = temp.path().join("doc.md");
    std::fs::write(&file, "# Hi\n\nSome *text*.\n").expect("the document is written");
    let output = run_cli(["render", file.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout_of(&output),
        "<h1>Hi</h1>\n<p>Some <em>text</em>.</p>\n\n"
    );
}

#[test]
fn render_json_carries_the_format_tag() {
    let temp = TempDir::new("render-json");
    let file = temp.path().join("doc.md");
    std::fs::write(&file, "*a* [b](/u)\n\n[r]: /r\n").expect("the document is written");
    let output = run_cli(["render", file.to_str().unwrap(), "--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = stdout_of(&output);
    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("the tree output is JSON");
    assert_eq!(value["format"], "geist4/tree@1");
    assert_eq!(value["root"]["children"][0]["type"], "paragraph");
    assert!(value["linkDefs"].as_array().expect("defs").len() == 1);
    assert!(value["problems"].as_array().expect("problems").is_empty());
}

#[test]
fn ast_prints_the_same_tree_pretty() {
    let temp = TempDir::new("ast");
    let file = temp.path().join("doc.md");
    std::fs::write(&file, "# One\n\n*two*.\n").expect("the document is written");
    let ast = run_cli(["ast", file.to_str().unwrap()]);
    assert_eq!(ast.status.code(), Some(0));
    let json = run_cli([
        "render",
        file.to_str().unwrap(),
        "--format",
        "json",
        "--pretty",
    ]);
    assert_eq!(json.status.code(), Some(0));
    assert_eq!(stdout_of(&ast), stdout_of(&json));
    let value: serde_json::Value =
        serde_json::from_str(&stdout_of(&ast)).expect("the ast output is JSON");
    assert_eq!(value["format"], "geist4/tree@1");
}

#[test]
fn render_mdast_exports_the_unist_shape() {
    let temp = TempDir::new("render-mdast");
    let file = temp.path().join("doc.md");
    std::fs::write(&file, "# One\n").expect("the document is written");
    let output = run_cli(["render", file.to_str().unwrap(), "--format", "mdast"]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = stdout_of(&output);
    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("the mdast output is JSON");
    assert_eq!(value["type"], "root");
    assert_eq!(value["children"][0]["type"], "heading");
    assert_eq!(value["children"][0]["depth"], 1);
}

#[test]
fn render_reports_a_missing_file() {
    let temp = TempDir::new("render-missing");
    let file = temp.path().join("absent.md");
    let output = run_cli(["render", file.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr_of(&output).contains("cannot read"));
}
