//! `geist4 ext manifest`: validate an extension package's manifest and
//! behavior without running its spec. This is the validation half of
//! `ext check`, which the conformance kit's manifest cases use: the
//! outcome is exactly the loader's verdict, nothing more.

use std::path::Path;
use std::process::ExitCode;

/// Runs `geist4 ext manifest PATH`. The package is loaded — manifest
/// parsed, behavior entries validated, and for tier 2 the component
/// loaded — and the loaded set's name check runs. The spec is not
/// read: manifest cases test the format's validation, not behavior.
pub(crate) fn run(path: &Path) -> ExitCode {
    match geist4_ext_loader::package::load(path) {
        Ok(packages) => {
            if let Err(message) = crate::packages::check_names(std::slice::from_ref(&packages)) {
                eprintln!("{message}");
                return ExitCode::from(2);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}: {error}", path.display());
            ExitCode::from(2)
        }
    }
}
