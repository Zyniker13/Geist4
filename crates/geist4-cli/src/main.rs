//! The Geist4 command-line tool: `render`, `check`, `ast`, `format`,
//! `ext check`.
//!
//! This milestone adds `check` and `ext check`. Loading files is the
//! CLI's job: the core itself takes a `&str` and touches nothing
//! outside its caller.

mod check;
mod ext_check;
mod manifest_check;
mod packages;
mod render;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

/// The Geist4 command-line tool.
#[derive(Debug, Parser)]
#[command(name = "geist4", version, about = "The Geist4 Markdown tool")]
struct Cli {
    /// The subcommand to run.
    #[command(subcommand)]
    command: Command,
}

/// The subcommands the tool knows.
#[derive(Debug, Subcommand)]
enum Command {
    /// Work with extension packages.
    Ext {
        /// The extension subcommand to run.
        #[command(subcommand)]
        command: ExtCommand,
    },
    /// Render a document as HTML, the JSON tree or mdast.
    Render {
        /// The Markdown file to render.
        file: PathBuf,
        /// The output format.
        #[arg(long = "format", value_enum, default_value_t = render::Format::Html)]
        format: render::Format,
        /// Extension packages to load and register for the parse.
        #[arg(long = "ext")]
        ext: Vec<PathBuf>,
        /// Highlight fenced code blocks in HTML output (off by
        /// default, so plain output stays byte-identical).
        #[arg(long)]
        highlight: bool,
        /// Pretty-print JSON output.
        #[arg(long)]
        pretty: bool,
    },
    /// Print the document's tree as pretty JSON, the JSON tree format.
    Ast {
        /// The Markdown file to print.
        file: PathBuf,
        /// Extension packages to load and register for the parse.
        #[arg(long = "ext")]
        ext: Vec<PathBuf>,
    },
    /// Check a Markdown document for problems.
    Check {
        /// The Markdown file to check.
        file: PathBuf,
        /// Extension packages to load and register for the parse.
        #[arg(long = "ext")]
        ext: Vec<PathBuf>,
    },
}

/// The extension package subcommands.
#[derive(Debug, Subcommand)]
enum ExtCommand {
    /// Run a package's conformance examples against the engine.
    Check {
        /// The extension package directory to check.
        path: PathBuf,
        /// Additional extension packages to load alongside it.
        #[arg(long = "ext")]
        ext: Vec<PathBuf>,
    },
    /// Validate a package's manifest and behavior without running
    /// its spec.
    Manifest {
        /// The extension package directory to validate.
        path: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Ext {
            command: ExtCommand::Check { path, ext },
        } => ext_check::run(&path, &ext),
        Command::Ext {
            command: ExtCommand::Manifest { path },
        } => manifest_check::run(&path),
        Command::Render {
            file,
            format,
            ext,
            highlight,
            pretty,
        } => render::run(&file, format, pretty, &ext, highlight),
        Command::Ast { file, ext } => render::run(&file, render::Format::Json, true, &ext, false),
        Command::Check { file, ext } => check::run(&file, &ext),
    }
}
