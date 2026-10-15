//! The source pin beside a vendored description, `source.json`: where the
//! files came from at an immutable revision, their license and notices, and
//! every file's SHA-256. [`verify`] refuses a folder whose files differ from
//! the pin, hold an unlisted file, or contain a link.

use std::collections::HashSet;
use std::path::{Component, Path};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::{Error, Geometry, MAX_DESCRIPTION_BYTES, MAX_MESH_BYTES, Robot};

/// The pin's file name inside a vendored folder.
pub const PIN_FILE: &str = "source.json";
/// The most files one pin may list.
pub const MAX_PIN_FILES: usize = 256;
/// The deepest folder nesting a pinned folder may have.
pub const MAX_PIN_DEPTH: usize = 8;

/// Where a vendored description came from.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    /// The upstream repository, an `https://` URL.
    pub repository: String,
    /// The upstream commit: 40 lowercase hex digits, never a branch or tag.
    /// A generated description (CAD Lab's URDF) pins the SHA-256 of the
    /// document it was written from instead: 64 hex digits, with
    /// `repository` naming the program that wrote it.
    pub revision: String,
    /// The folder in the upstream repository the files were copied from.
    pub source_path: String,
    /// The files' license as an SPDX expression.
    pub license: String,
    /// The URDF file, relative to this folder.
    pub description: String,
    /// License and notice files copied with the description, relative to
    /// this folder.
    pub notices: Vec<String>,
    /// Every vendored file but this pin, with its SHA-256.
    pub files: Vec<PinnedFile>,
}

/// One vendored file.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PinnedFile {
    /// Relative to the pinned folder, with `/` separators.
    pub path: String,
    /// Lowercase hex SHA-256 of the file's bytes.
    pub sha256: String,
}

/// Lowercase hex SHA-256 of `bytes`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Reads `folder/source.json` and checks it against the folder's files.
pub fn verify(folder: &Path) -> Result<Pin, Error> {
    let pin_path = folder.join(PIN_FILE);
    let text = read_bounded(&pin_path, MAX_DESCRIPTION_BYTES)?;
    let pin: Pin = serde_json::from_slice(&text)
        .map_err(|e| Error::Pin(format!("{PIN_FILE} is not a pin: {e}")))?;
    if !pin.repository.starts_with("https://") {
        return Err(Error::Pin(format!(
            "repository {:?} is not an https:// URL",
            pin.repository
        )));
    }
    if !hex(&pin.revision, 40) && !hex(&pin.revision, 64) {
        return Err(Error::Pin(format!(
            "revision {:?} is not a 40-digit commit hash or a 64-digit content digest; pin an immutable commit, not a branch or tag",
            pin.revision
        )));
    }
    if pin.license.trim().is_empty() {
        return Err(Error::Pin("names no license".into()));
    }
    if pin.notices.is_empty() {
        return Err(Error::Pin("lists no license or notice files".into()));
    }
    if pin.files.len() > MAX_PIN_FILES {
        return Err(Error::TooLarge {
            what: "the number of pinned files".into(),
            limit: MAX_PIN_FILES as u64,
            actual: pin.files.len() as u64,
        });
    }
    let mut listed = HashSet::new();
    for file in &pin.files {
        relative(&file.path)?;
        if !listed.insert(file.path.as_str()) {
            return Err(Error::Pin(format!("{} is listed twice", file.path)));
        }
        if !hex(&file.sha256, 64) {
            return Err(Error::Pin(format!(
                "{} has {:?}, not a SHA-256 in lowercase hex",
                file.path, file.sha256
            )));
        }
    }
    for needed in std::iter::once(&pin.description).chain(&pin.notices) {
        if !listed.contains(needed.as_str()) {
            return Err(Error::Pin(format!(
                "{needed} is not among the pinned files"
            )));
        }
    }
    for file in &pin.files {
        let bytes = read_bounded(&folder.join(&file.path), MAX_MESH_BYTES)?;
        let actual = sha256_hex(&bytes);
        if actual != file.sha256 {
            return Err(Error::Pin(format!(
                "{} has SHA-256 {actual}, but the pin records {}",
                file.path, file.sha256
            )));
        }
    }
    let mut present = Vec::new();
    walk(folder, "", 0, &mut present)?;
    for path in present {
        if path != PIN_FILE && !listed.contains(path.as_str()) {
            return Err(Error::Pin(format!(
                "{path} is in the folder but not in the pin"
            )));
        }
    }
    Ok(pin)
}

/// Verifies the pin, then loads its description and checks that every mesh
/// the description reads is a pinned file.
pub fn load_pinned(folder: &Path) -> Result<(Pin, Robot), Error> {
    let pin = verify(folder)?;
    let robot = crate::load(&folder.join(&pin.description))?;
    let base = Path::new(&pin.description)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let listed: HashSet<&str> = pin.files.iter().map(|f| f.path.as_str()).collect();
    for link in robot.links() {
        for shape in link.visuals.iter().chain(&link.collisions) {
            if let Geometry::Mesh { file, .. } = &shape.geometry {
                let path = normal(&base.join(file));
                if !listed.contains(path.as_str()) {
                    return Err(Error::Pin(format!(
                        "link {} reads {path}, which is not a pinned file",
                        link.name
                    )));
                }
            }
        }
    }
    Ok((pin, robot))
}

/// `path` with `/` separators and no `.` parts.
fn normal(path: &Path) -> String {
    path.components()
        .filter_map(|part| match part {
            Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn relative(path: &str) -> Result<(), Error> {
    let fine = !path.is_empty()
        && !path.contains('\\')
        && !path.contains("://")
        && Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)));
    if fine {
        Ok(())
    } else {
        Err(Error::Pin(format!(
            "{path:?} is not a plain relative path inside the folder"
        )))
    }
}

fn hex(text: &str, digits: usize) -> bool {
    text.len() == digits && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, Error> {
    let shown = path.display().to_string();
    let metadata = std::fs::symlink_metadata(path).map_err(|e| Error::Read {
        path: shown.clone(),
        reason: e.to_string(),
    })?;
    if !metadata.is_file() {
        return Err(Error::Pin(format!("{shown} is not a plain file")));
    }
    if metadata.len() > limit {
        return Err(Error::TooLarge {
            what: format!("{shown} in bytes"),
            limit,
            actual: metadata.len(),
        });
    }
    std::fs::read(path).map_err(|e| Error::Read {
        path: shown,
        reason: e.to_string(),
    })
}

fn walk(folder: &Path, prefix: &str, depth: usize, out: &mut Vec<String>) -> Result<(), Error> {
    if depth > MAX_PIN_DEPTH {
        return Err(Error::Pin(format!(
            "{prefix} is nested deeper than {MAX_PIN_DEPTH} folders"
        )));
    }
    let entries = std::fs::read_dir(folder).map_err(|e| Error::Read {
        path: folder.display().to_string(),
        reason: e.to_string(),
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| Error::Read {
            path: folder.display().to_string(),
            reason: e.to_string(),
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let kind = entry.file_type().map_err(|e| Error::Read {
            path: path.clone(),
            reason: e.to_string(),
        })?;
        if kind.is_symlink() {
            return Err(Error::Pin(format!("{path} is a link; vendor plain files")));
        }
        if kind.is_dir() {
            walk(&entry.path(), &path, depth + 1, out)?;
        } else {
            out.push(path);
        }
        if out.len() > MAX_PIN_FILES + 1 {
            return Err(Error::TooLarge {
                what: "the number of files in the pinned folder".into(),
                limit: MAX_PIN_FILES as u64 + 1,
                actual: out.len() as u64,
            });
        }
    }
    Ok(())
}
