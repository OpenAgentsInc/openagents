//! Reading the immutable host bundles that `scripts/coder-host.py` stages.
//!
//! A bundle lives at `<bundle root>/versions/<sha256>/` and holds the
//! `coder` binary and a `manifest.json` whose `binary_sha256` names the same
//! digest. This module verifies both before the launcher runs a version. It
//! never stages, copies, or selects a bundle: staging stays in the Python
//! installation helper, and the running version belongs to the launcher.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{Error, Result, fsx};

/// The manifest schema the installation helper writes.
pub const BUNDLE_SCHEMA: &str = "openagents.coder.host-bundle.v1";

#[derive(Deserialize)]
struct Manifest {
    schema: String,
    binary_sha256: String,
}

#[derive(Deserialize)]
struct Selection {
    active: Option<String>,
}

/// Verifies the bundle named by `version` and returns its binary's path.
///
/// The directory must be ordinary, the manifest must name this digest, and
/// the retained binary's bytes must still hash to it.
pub fn release(bundle_root: &Path, version: &str) -> Result<PathBuf> {
    if !fsx::is_digest(version) {
        return Err(Error::refused(
            "a host version is a 64-character SHA-256 bundle identity",
        ));
    }
    let directory = bundle_root.join("versions").join(version);
    let meta = std::fs::symlink_metadata(&directory).map_err(|_| {
        Error::refused(format!(
            "bundle {version} is not staged under {}",
            bundle_root.display()
        ))
    })?;
    if !meta.file_type().is_dir() {
        return Err(Error::refused("a bundle directory is not ordinary"));
    }
    let manifest: Manifest = serde_json::from_slice(&fsx::read_bounded(
        &directory.join("manifest.json"),
        fsx::RECORD_MAX,
    )?)?;
    if manifest.schema != BUNDLE_SCHEMA || manifest.binary_sha256 != version {
        return Err(Error::refused("the bundle manifest names another binary"));
    }
    let binary = directory.join("coder");
    if fsx::sha256_file(&binary)? != version {
        return Err(Error::refused(
            "the retained binary no longer matches its digest",
        ));
    }
    Ok(binary)
}

/// The bundle the installation helper selected, if any.
pub fn selected(bundle_root: &Path) -> Result<Option<String>> {
    let Some(bytes) = fsx::read_optional(&bundle_root.join("active.json"))? else {
        return Ok(None);
    };
    let selection: Selection = serde_json::from_slice(&bytes)?;
    Ok(selection.active)
}
