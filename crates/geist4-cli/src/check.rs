//! `geist4 check`: report the problems a parse records about a
//! document, and any error an extension latched during the parse.

use geist4_core::extension::Options;
use geist4_core::parser::parse_with;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Runs `geist4 check FILE [--ext PATH]...`.
///
/// The file is parsed with every `--ext` extension registered; the
/// parse never fails, so the outcome is what the parse and the
/// extensions recorded: the document's problems, deduplicated by
/// message and position, then each extension's latched error in load
/// order. Nothing found prints `ok` and exits zero; anything found
/// prints to stderr and exits one.
pub(crate) fn run(file: &Path, extra: &[PathBuf]) -> ExitCode {
    let packages = match crate::packages::load(extra) {
        Ok(packages) => packages,
        Err((path, message)) => {
            crate::packages::report_load_failure(&path, &message);
            return ExitCode::from(2);
        }
    };
    let mut options = Options::new();
    for package in &packages {
        options.extension(package.extension().clone());
    }
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("cannot read {}: {error}", file.display());
            return ExitCode::from(2);
        }
    };

    let document = parse_with(&text, &options);

    let mut seen: Vec<(String, usize)> = Vec::new();
    let mut found = false;
    for problem in &document.problems {
        let key = (problem.message.clone(), problem.span.start);
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let (line, column) = line_col(&text, problem.span.start);
        let message = &problem.message;
        eprintln!("{line}:{column}: {message}");
        found = true;
    }

    // Latched errors are read after the parse: a hook that could not
    // continue records one for the host to take now.
    for package in &packages {
        let name = &package.manifest().extension.name;
        if let Some(error) = package.extension().take_error() {
            eprintln!("{name}: {error}");
            found = true;
        }
    }

    if found {
        ExitCode::FAILURE
    } else {
        println!("ok");
        ExitCode::SUCCESS
    }
}

/// The one-based line and character column of a byte offset in `text`.
/// Lines are counted by newline bytes; the column counts characters,
/// not bytes, so multibyte characters count as one.
fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let mut offset = offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let prefix = &text[..offset];
    let line = 1 + prefix.bytes().filter(|byte| *byte == b'\n').count();
    let line_start = prefix
        .bytes()
        .rposition(|byte| byte == b'\n')
        .map_or(0, |position| position + 1);
    let column = text[line_start..offset].chars().count() + 1;
    (line, column)
}
