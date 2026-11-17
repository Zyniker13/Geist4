//! `geist4 ext check`: run an extension package's conformance examples
//! against the engine.

use geist4_core::basic_html::render_with;
use geist4_core::extension::Options;
use geist4_core::parser::parse_with;
use geist4_spec_harness::{Example, Failure};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// How many failing examples are printed in full before the rest are
/// only counted.
const MAX_PRINTED_FAILURES: usize = 10;

/// Runs `geist4 ext check PATH [--ext PATH]...`.
///
/// The package at `path` is the one under test; every `--ext` package
/// is loaded alongside it. The whole set is checked for duplicate
/// names and declared conflicts, then the package's `spec.md` is run:
/// each example is parsed and rendered with all loaded extensions
/// registered, in the order that honors a package's `after` — the
/// other extensions first, the one under test last.
pub(crate) fn run(path: &Path, extra: &[PathBuf]) -> ExitCode {
    // The loaded set: the package under test first, then the extras in
    // flag order.
    let mut set = vec![path.to_path_buf()];
    set.extend(extra.iter().cloned());
    let packages = match crate::packages::load(&set) {
        Ok(packages) => packages,
        Err((path, message)) => {
            crate::packages::report_load_failure(&path, &message);
            return ExitCode::from(2);
        }
    };
    if let Err(message) = crate::packages::check_names(&packages) {
        eprintln!("{message}");
        return ExitCode::from(2);
    }

    let tested = &packages[0];
    let spec_path = tested.root().join("spec.md");
    let spec = match std::fs::read_to_string(&spec_path) {
        Ok(spec) => spec,
        Err(error) => {
            eprintln!("cannot read {}: {error}", spec_path.display());
            return ExitCode::from(2);
        }
    };
    let examples = match geist4_spec_harness::parse(&spec) {
        Ok(examples) => examples,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    if examples.is_empty() {
        eprintln!("spec.md contains no examples");
        return ExitCode::from(2);
    }

    // Registration order: the other extensions first, then the one
    // under test, which is what honoring `after` asks for.
    let mut options = Options::new();
    for package in packages.iter().skip(1) {
        options.extension(package.extension().clone());
    }
    options.extension(tested.extension().clone());

    let mut render = |example: &Example| {
        let document = parse_with(&example.markdown, &options);
        render_with(&document, &options)
    };
    let report = geist4_spec_harness::run(&examples, &mut render);
    if report.failed() == 0 {
        return ExitCode::SUCCESS;
    }
    eprintln!("{report}");
    for Failure {
        number,
        section,
        markdown,
        expected,
        actual,
    } in report.failures().iter().take(MAX_PRINTED_FAILURES)
    {
        eprintln!("example {number} ({section})");
        eprintln!("markdown:");
        eprintln!("{markdown}");
        eprintln!("expected:");
        eprintln!("{expected}");
        eprintln!("actual:");
        eprintln!("{actual}");
    }
    let hidden = report.failures().len().saturating_sub(MAX_PRINTED_FAILURES);
    if hidden > 0 {
        eprintln!("... and {hidden} more");
    }
    ExitCode::FAILURE
}
