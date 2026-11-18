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

/// Builds the package's extension object: a tier-2 manifest loads its
/// component into the sandbox (with the `sandbox` feature) or refuses
/// clearly (without it); a tier-1 or tier-3 manifest compiles as
/// before. The tier-2 path also runs the component's load-time check,
/// so a package that cannot initialize fails to load, not at first
/// parse.
fn build_extension(
    manifest: &crate::types::Manifest,
    dir: &Path,
) -> Result<Arc<dyn Extension>, LoadError> {
    match &manifest.wasm {
        Some(wasm) => {
            #[cfg(feature = "sandbox")]
            {
                load_wasm_extension(manifest, wasm, dir)
            }
            #[cfg(not(feature = "sandbox"))]
            {
                let _ = (wasm, dir);
                Err(LoadError::new(format!(
                    "wasm.path: the package declares a tier-2 WebAssembly component, but \
                     this build was compiled without the `sandbox` feature; rebuild the \
                     loader with --features sandbox to run tier-2 packages"
                )))
            }
        }
        None => Ok(crate::compile::compile(manifest.clone())),
    }
}

/// Loads a tier-2 package's component from its file, checks it, and
/// wraps it in the adapter. Only compiled with the `sandbox` feature.
#[cfg(feature = "sandbox")]
fn load_wasm_extension(
    manifest: &crate::types::Manifest,
    wasm: &crate::types::Wasm,
    dir: &Path,
) -> Result<Arc<dyn Extension>, LoadError> {
    let component_path = dir.join(&wasm.path);
    let extension =
        crate::sandbox::SandboxExtension::load(&component_path, wasm.fuel, wasm.memory_pages)
            .map_err(|error| LoadError::new(error.to_string()))?;
    extension
        .check()
        .map_err(|error| LoadError::new(format!("component fails its load-time check: {error}")))?;
    Ok(Arc::new(crate::adapter::WasmExtension::new(
        manifest.clone(),
        extension,
    )))
}

/// Loads a package from a directory. The directory must contain
/// `extension.toml` (the manifest) and `spec.md` (the conformance
/// examples); a tier-2 package additionally carries the component named
/// by its `[wasm]` table, checked for presence here when the sandbox
/// feature is enabled.
///
/// # Errors
/// A missing or unreadable manifest, a manifest that fails validation,
/// a missing `spec.md`, or (with the sandbox feature) a missing,
/// unloadable or failed-check component all return a [`LoadError`]
/// describing the first problem in validation order.
pub fn load(dir: &Path) -> Result<Package, LoadError> {
    let manifest_path = dir.join("extension.toml");
    let source = std::fs::read_to_string(&manifest_path).map_err(|error| {
        LoadError::new(format!(
            "cannot read {}: {}",
            manifest_path.display(),
            crate::error::io_message(&error)
        ))
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

    let extension = build_extension(&manifest, dir)?;
    Ok(Package {
        manifest,
        extension,
        root: dir.to_path_buf(),
    })
}
