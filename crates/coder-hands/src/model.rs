//! The pinned landmark model.
//!
//! `model.toml` beside this crate names the URL a tracker fetches at first
//! use, the file it lands as under `~/.openagents/quest/models/`, and the
//! SHA-256 the file has to carry before the tracker loads it. The manifest
//! is compiled in, so a binary carries its own pin. [`model_path`] says
//! where the file lands and [`ensure_model`] puts it there for the camera
//! daemon. The directory keeps the name it had when a second tracker
//! shared the cache, so a host that already holds the file keeps it.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The variable that names a model file of your own, which skips the pin.
pub const MODEL_OVERRIDE: &str = "CODER_QUEST_HAND_MODEL";

/// The manifest text, as the tree carries it.
pub const MANIFEST: &str = include_str!("../model.toml");

/// One pinned model: where it comes from, what it is called, and what it
/// hashes to.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    /// The URL the tracker fetches, which answers without a token.
    pub url: String,
    /// The SHA-256 of the file, as 64 lowercase hex digits.
    pub sha256: String,
    /// The file name under the models directory.
    pub file: String,
    /// The file's size in bytes, for the reader of the manifest.
    pub size: u64,
}

impl Manifest {
    /// Reads a manifest from its TOML text.
    pub fn parse(text: &str) -> Result<Manifest, String> {
        let manifest: Manifest =
            toml::from_str(text).map_err(|err| format!("model manifest: {err}"))?;
        if manifest.sha256.len() != 64
            || !manifest.sha256.chars().all(|c| c.is_ascii_hexdigit())
            || manifest.sha256.chars().any(|c| c.is_ascii_uppercase())
        {
            return Err(format!(
                "model manifest: sha256 is not 64 lowercase hex digits: {}",
                manifest.sha256
            ));
        }
        if !manifest.url.starts_with("https://") {
            return Err(format!(
                "model manifest: url is not https: {}",
                manifest.url
            ));
        }
        if manifest.file.is_empty() || manifest.file.contains('/') {
            return Err(format!(
                "model manifest: file is not a bare name: {}",
                manifest.file
            ));
        }
        Ok(manifest)
    }

    /// The manifest the tree pins.
    pub fn pinned() -> Result<Manifest, String> {
        Manifest::parse(MANIFEST)
    }

    /// Checks that the file at `path` hashes to the pinned digest. The
    /// error names the path, the digest the file has, the digest the pin
    /// expects, and the URL the pin fetches.
    pub fn verify(&self, path: &Path) -> Result<(), String> {
        let bytes = std::fs::read(path).map_err(|err| format!("{}: {err}", path.display()))?;
        let got = sha256_hex(&bytes);
        if got == self.sha256 {
            return Ok(());
        }
        Err(format!(
            "{} has SHA-256 {got}; the pinned model from {} has {}",
            path.display(),
            self.url,
            self.sha256
        ))
    }
}

/// Where the model lives: [`MODEL_OVERRIDE`] names a file of your own,
/// which skips the pin, and otherwise the pinned file under
/// `~/.openagents/quest/models/`.
pub fn model_path(manifest: &Manifest) -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var(MODEL_OVERRIDE) {
        return Ok(PathBuf::from(p));
    }
    let home = std::env::var("HOME").map_err(|_| "HOME is unset".to_string())?;
    Ok(PathBuf::from(home)
        .join(".openagents/quest/models")
        .join(&manifest.file))
}

/// Puts the pinned model at `path` when it is not there. A file already
/// there passes the digest check or is fetched again; a fetched file that
/// fails it is removed, and the error names the URL and the digest. A path
/// named in [`MODEL_OVERRIDE`] is taken as it is.
pub fn ensure_model(manifest: &Manifest, path: &Path) -> Result<(), String> {
    if std::env::var_os(MODEL_OVERRIDE).is_some() {
        return Ok(());
    }
    if path.exists() && manifest.verify(path).is_ok() {
        return Ok(());
    }
    let parent = path
        .parent()
        .ok_or_else(|| "model path has no parent".to_string())?;
    std::fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    let tmp = parent.join(format!("{}.part", manifest.file));
    let status = Command::new("curl")
        .args(["-fsSL", "--retry", "3", "-o"])
        .arg(&tmp)
        .arg(&manifest.url)
        .status()
        .map_err(|err| format!("curl: {err}"))?;
    if !status.success() {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!(
            "could not fetch the hand model from {} (curl exit {status}); set {MODEL_OVERRIDE} to a file of your own",
            manifest.url
        ));
    }
    if let Err(err) = manifest.verify(&tmp) {
        let _ = std::fs::remove_file(&tmp);
        return Err(err);
    }
    std::fs::rename(&tmp, path).map_err(|err| format!("install model: {err}"))?;
    Ok(())
}

/// The SHA-256 of `bytes` as lowercase hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// A file of `bytes` in the temporary directory, named for this test
    /// run so parallel tests do not share it.
    fn scratch(bytes: &[u8]) -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("coder-hands-model-{}-{n}.onnx", std::process::id()));
        std::fs::write(&path, bytes).expect("write the scratch file");
        path
    }

    #[test]
    fn the_pinned_manifest_parses() {
        let manifest = Manifest::pinned().expect("the tree's manifest parses");
        assert!(manifest.url.starts_with("https://"), "{}", manifest.url);
        assert_eq!(manifest.sha256.len(), 64);
        assert!(manifest.file.ends_with(".onnx"), "{}", manifest.file);
        assert!(manifest.size > 0);
    }

    #[test]
    fn a_manifest_with_a_short_digest_is_refused() {
        let err = Manifest::parse(
            "url = \"https://example.invalid/m.onnx\"\nsha256 = \"abc\"\nfile = \"m.onnx\"\nsize = 1\n",
        )
        .unwrap_err();
        assert!(err.contains("sha256"), "{err}");
    }

    #[test]
    fn a_manifest_with_a_plain_http_url_is_refused() {
        let text = format!(
            "url = \"http://example.invalid/m.onnx\"\nsha256 = \"{}\"\nfile = \"m.onnx\"\nsize = 1\n",
            sha256_hex(b"x")
        );
        let err = Manifest::parse(&text).unwrap_err();
        assert!(err.contains("https"), "{err}");
    }

    #[test]
    fn a_matching_file_passes_the_digest_check() {
        let bytes = b"a small model";
        let path = scratch(bytes);
        let manifest = Manifest {
            url: "https://example.invalid/m.onnx".into(),
            sha256: sha256_hex(bytes),
            file: "m.onnx".into(),
            size: bytes.len() as u64,
        };
        let checked = manifest.verify(&path);
        let _ = std::fs::remove_file(&path);
        checked.expect("the digest matches");
    }

    #[test]
    fn a_wrong_file_is_refused_with_the_url_and_both_digests() {
        let path = scratch(b"not the model");
        let manifest = Manifest {
            url: "https://example.invalid/m.onnx".into(),
            sha256: sha256_hex(b"the model"),
            file: "m.onnx".into(),
            size: 9,
        };
        let err = manifest.verify(&path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(err.contains("https://example.invalid/m.onnx"), "{err}");
        assert!(err.contains(&sha256_hex(b"the model")), "{err}");
        assert!(err.contains(&sha256_hex(b"not the model")), "{err}");
    }

    #[test]
    fn a_missing_file_is_named() {
        let manifest = Manifest::pinned().expect("manifest");
        let err = manifest
            .verify(Path::new("/nonexistent/coder-hands/model.onnx"))
            .unwrap_err();
        assert!(err.contains("/nonexistent/coder-hands/model.onnx"), "{err}");
    }

    #[test]
    fn the_model_lands_under_the_openagents_directory() {
        let manifest = Manifest::pinned().expect("manifest");
        if std::env::var_os(MODEL_OVERRIDE).is_some() {
            return;
        }
        let path = model_path(&manifest).expect("a home");
        assert!(
            path.ends_with(format!(".openagents/quest/models/{}", manifest.file)),
            "{}",
            path.display()
        );
    }

    #[test]
    fn a_cached_file_that_passes_the_check_is_kept() {
        if std::env::var_os(MODEL_OVERRIDE).is_some() {
            return;
        }
        let bytes = b"the cached model";
        let path = scratch(bytes);
        let manifest = Manifest {
            url: "https://example.invalid/m.onnx".into(),
            sha256: sha256_hex(bytes),
            file: "m.onnx".into(),
            size: bytes.len() as u64,
        };
        let kept = ensure_model(&manifest, &path);
        let after = std::fs::read(&path).expect("still there");
        let _ = std::fs::remove_file(&path);
        kept.expect("a good cache needs no fetch");
        assert_eq!(after, bytes);
    }

    #[test]
    fn sha256_hex_is_the_known_digest_of_the_empty_input() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
