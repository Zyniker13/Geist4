//! Loading extension packages for the CLI's commands, and the checks
//! the loaded set must pass before any of it runs.

use geist4_ext_loader::package::Package;
use std::path::{Path, PathBuf};

/// Loads each path as an extension package, in order, stopping at the
/// first failure. The error carries the path as given and the loader's
/// one-line message.
///
/// # Errors
///
/// Returns the failing path and the [`LoadError`](geist4_ext_loader::error::LoadError)
/// message when a package cannot be loaded.
pub(crate) fn load(paths: &[PathBuf]) -> Result<Vec<Package>, (PathBuf, String)> {
    let mut loaded = Vec::with_capacity(paths.len());
    for path in paths {
        match geist4_ext_loader::package::load(path) {
            Ok(package) => loaded.push(package),
            Err(error) => return Err((path.clone(), error.to_string())),
        }
    }
    Ok(loaded)
}

/// Prints a load failure the way both commands report it: the path as
/// given, then the loader's message.
pub(crate) fn report_load_failure(path: &Path, message: &str) {
    eprintln!("{}: {message}", path.display());
}

/// Checks the loaded set for duplicate names and declared conflicts,
/// walking it in load order. The `after` field is ordering information
/// only and is not validated here.
///
/// # Errors
///
/// Returns a one-line message when two packages share a name or one
/// package's `conflicts` names another loaded extension.
pub(crate) fn check_names(packages: &[Package]) -> Result<(), String> {
    for (index, package) in packages.iter().enumerate() {
        let name = &package.manifest().extension.name;
        let duplicate = packages[..index]
            .iter()
            .any(|other| other.manifest().extension.name == *name);
        if duplicate {
            return Err(format!("duplicate extension name '{name}'"));
        }
    }
    for package in packages {
        let own = &package.manifest().extension.name;
        for conflict in &package.manifest().extension.conflicts {
            if conflict == own {
                continue;
            }
            let loaded = packages
                .iter()
                .any(|other| &other.manifest().extension.name == conflict);
            if loaded {
                return Err(format!("extension '{own}' conflicts with '{conflict}'"));
            }
        }
    }
    Ok(())
}
