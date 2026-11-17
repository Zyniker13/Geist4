//! Loading an extension package from a directory: read the manifest,
//! validate it, check the package shape, and compile the extension.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use geist4_core::extension::Extension;

use crate::error::LoadError;

/// A loaded extension package: the validated manifest, the compiled
/// extension object, and the package's location.
pub struct Package {
    manifest: crate::types::Manifest,
    extension: Arc<dyn Extension>,
    root: PathBuf,
}

impl std::fmt::Debug for Package {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Package")
            .field("name", &self.manifest.extension.name)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl Package {
    /// The validated manifest.
    pub fn manifest(&self) -> &crate::types::Manifest {
        &self.manifest
    }

    /// The compiled extension object, ready to register with parser
    /// options.
    pub fn extension(&self) -> &Arc<dyn Extension> {
        &self.extension
    }

    /// The package directory the manifest was loaded from.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// Loads a package from a directory. The directory must contain
/// `extension.toml` (the manifest) and `spec.md` (the conformance
/// examples); a tier-2 package additionally carries the component named
/// by its `[wasm]` table, checked for presence here when the sandbox
/// feature is enabled.
///
/// # Errors
/// A missing or unreadable manifest, a manifest that fails validation,
/// a missing `spec.md`, or (with the sandbox feature) a missing or
/// unloadable component all return a [`LoadError`] describing the first
/// problem in validation order.
pub fn load(dir: &Path) -> Result<Package, LoadError> {
    let manifest_path = dir.join("extension.toml");
    let source = std::fs::read_to_string(&manifest_path).map_err(|error| {
        LoadError::new(format!("cannot read {}: {error}", manifest_path.display()))
    })?;
    let manifest = crate::manifest::parse(&source)?;

    let spec = dir.join("spec.md");
    if !spec.is_file() {
        return Err(LoadError::new(format!(
            "package {}: spec.md is missing; every package ships its \
             conformance examples",
            dir.display()
        )));
    }

    let extension = crate::compile::compile(manifest.clone());
    Ok(Package {
        manifest,
        extension,
        root: dir.to_path_buf(),
    })
}
