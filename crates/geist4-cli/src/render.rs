//! `geist4 render`: a document as HTML, the JSON tree format or mdast,
//! and `geist4 ast`: the tree as pretty JSON.

use geist4_core::extension::Options;
use geist4_core::parser::parse_with;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The formats `render` writes.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub(crate) enum Format {
    /// The HTML the renderer produces, safety policy applied.
    Html,
    /// The JSON tree format, `geist4/tree@1`.
    Json,
    /// The mdast (unist) export.
    Mdast,
}

/// Runs `geist4 render FILE --format html|json|mdast [--highlight]
/// [--pretty]`.
///
/// The file is parsed with every `--ext` extension registered, then
/// serialized in the chosen format to standard output. Loading a
/// package that cannot load exits two; reading the file that cannot be
/// read exits two. A parse never fails. `highlight` enables fenced-code
/// highlighting for HTML output only; the other formats ignore it.
pub(crate) fn run(
    file: &Path,
    format: Format,
    pretty: bool,
    extra: &[PathBuf],
    highlight: bool,
) -> ExitCode {
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
    if highlight {
        options.highlighter(std::sync::Arc::new(geist4_highlight::SyntaxHighlighter));
    }
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("cannot read {}: {error}", file.display());
            return ExitCode::from(2);
        }
    };

    let document = parse_with(&text, &options);
    let output = match format {
        Format::Html => geist4_core::basic_html::render_with(&document, &options),
        Format::Json => geist4_json::to_json(&document, &text, pretty),
        Format::Mdast => {
            let mdast = geist4_mdast::to_mdast(&text, &document);
            geist4_mdast::to_json(&mdast, pretty)
        }
    };
    println!("{output}");
    ExitCode::SUCCESS
}
