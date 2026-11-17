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
        Command::Check { file, ext } => check::run(&file, &ext),
    }
}
