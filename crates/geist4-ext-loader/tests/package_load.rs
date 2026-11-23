//! The `package::load` and `io_message` error paths no other suite
//! drives: a missing manifest, an unreadable one, a missing `spec.md`,
//! the success shape, and the cross-platform I/O message helper.

use std::path::Path;

use geist4_ext_loader::error::LoadError;
use geist4_ext_loader::package;

/// A unique temp directory for one test.
fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("geist4-pkg-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp package dir");
    dir
}

fn remove(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

/// A minimal valid tier-1 manifest.
const MANIFEST: &str = "[extension]\n\
     name = \"sample\"\n\
     version = \"1.0.0\"\n\
     format = \"1.0\"\n\
     description = \"d\"\n\
     license = \"MIT\"\n\
     \n\
     [[html_filter]]\n\
     deny = [\"script\"]\n\
     \n";

/// A missing directory declines with the fixed wording for NotFound.
#[test]
fn missing_dir_reports_not_found() {
    let dir = std::env::temp_dir().join(format!("geist4-pkg-absent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let error = package::load(&dir).expect_err("an absent directory must fail");
    assert!(
        error.message.contains("cannot read")
            && error.message.contains("no such file or directory"),
        "the NotFound wording is the cross-platform contract: {}",
        error.message
    );
    assert_eq!(error.span, None, "an I/O failure carries no span");
}

/// A directory whose manifest is a directory (unreadable as a file)
/// declines through the non-NotFound branch.
#[test]
fn unreadable_manifest_reports_plain_os_text() {
    let dir = temp_dir("unreadable");
    std::fs::create_dir_all(dir.join("extension.toml")).expect("make the manifest a directory");
    let error = package::load(&dir).expect_err("a directory-as-manifest must fail");
    remove(&dir);
    assert!(
        error.message.contains("cannot read"),
        "the message names the file: {}",
        error.message
    );
    // A directory read yields `Is a directory` on Unix — not one of the
    // two fixed kinds — so the OS text passes through.
    assert!(
        !error.message.contains("no such file or directory")
            && !error.message.contains("permission denied"),
        "a rare kind keeps the OS text: {}",
        error.message
    );
}

/// A directory without `spec.md` declines after a valid manifest.
#[test]
fn missing_spec_md_declines() {
    let dir = temp_dir("nospec");
    std::fs::write(dir.join("extension.toml"), MANIFEST).expect("write manifest");
    let error = package::load(&dir).expect_err("a package without spec.md must fail");
    remove(&dir);
    assert!(
        error.message.contains("spec.md is missing"),
        "the wording is the contract: {}",
        error.message
    );
}

/// A complete tier-1 package loads: the manifest, the extension object
/// and the root path all round-trip, and the debug output names them.
#[test]
fn complete_package_loads() {
    let dir = temp_dir("happy");
    std::fs::write(dir.join("extension.toml"), MANIFEST).expect("write manifest");
    std::fs::write(dir.join("spec.md"), "# sample\n\nplain\n").expect("write spec");
    let package = package::load(&dir).expect("a complete tier-1 package loads");
    remove(&dir);
    assert_eq!(package.manifest().extension.name, "sample");
    assert_eq!(package.root(), Path::new(&dir));
    let debug = format!("{package:?}");
    assert!(
        debug.contains("sample") && debug.contains("Package"),
        "the debug output names the package: {debug}"
    );
}

/// A malformed manifest declines through the manifest parser, and the
/// TOML syntax error carries the span it points at.
#[test]
fn malformed_manifest_carries_span() {
    let dir = temp_dir("badtoml");
    std::fs::write(dir.join("extension.toml"), "[extension\n").expect("write manifest");
    let error = package::load(&dir).expect_err("a broken TOML manifest must fail");
    remove(&dir);
    assert!(
        error.span.is_some(),
        "a TOML syntax error carries its span: {:?}",
        error
    );
}

/// `LoadError`'s constructors, Display and Error impl.
#[test]
fn load_error_shape() {
    let plain = LoadError::new("boom");
    assert_eq!(plain.message, "boom");
    assert_eq!(plain.span, None);
    assert_eq!(plain.to_string(), "boom");

    let spanned = LoadError::with_span("boom", 3..7);
    assert_eq!(spanned.message, "boom");
    assert_eq!(spanned.span, Some(3..7));
    assert_eq!(spanned.to_string(), "boom");

    // The type is a std error and round-trips through a Box.
    let boxed: Box<dyn std::error::Error> = Box::new(plain);
    assert_eq!(boxed.to_string(), "boom");
}
