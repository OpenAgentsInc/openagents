//! Files a delegate copies or compares: a workspace copied whole, and a
//! tree's files by path and digest.

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};

/// Directories a lane's diff ignores: caches a test run rewrites, and the
/// Git database, which a merge of files doesn't carry.
pub const UNMERGED: [&str; 5] = [
    ".git",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
];

/// Copies `from` into `to`, `.git` included, replacing what `to` held.
///
/// # Errors
///
/// Returns a message when a file can't be copied.
pub fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    if to.exists() {
        std::fs::remove_dir_all(to).map_err(|error| format!("{}: {error}", to.display()))?;
    }
    std::fs::create_dir_all(to).map_err(|error| format!("{}: {error}", to.display()))?;
    for entry in std::fs::read_dir(from)
        .map_err(|error| format!("{}: {error}", from.display()))?
        .flatten()
    {
        let path = entry.path();
        let target = to.join(entry.file_name());
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        if kind.is_dir() {
            copy_tree(&path, &target)?;
        } else if kind.is_symlink() {
            let link = std::fs::read_link(&path).map_err(|error| error.to_string())?;
            std::os::unix::fs::symlink(link, &target).map_err(|error| error.to_string())?;
        } else {
            std::fs::copy(&path, &target)
                .map_err(|error| format!("{}: {error}", path.display()))?;
        }
    }
    Ok(())
}

/// Every file under `dir` a merge compares, by path relative to `dir`,
/// with its SHA-256 (a symbolic link by its target), skipping
/// [`UNMERGED`] directories and compiled Python files.
#[must_use]
pub fn tree(dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let Ok(relative) = path.strip_prefix(dir) else {
                continue;
            };
            let relative = relative.to_string_lossy().into_owned();
            if kind.is_dir() {
                if !UNMERGED.contains(&name.as_str()) {
                    stack.push(path);
                }
            } else if kind.is_symlink() {
                let target = std::fs::read_link(&path).unwrap_or_default();
                out.insert(
                    relative,
                    sha256(format!("link:{}", target.display()).as_bytes()),
                );
            } else if !name.ends_with(".pyc")
                && let Ok(bytes) = std::fs::read(&path)
            {
                out.insert(relative, sha256(&bytes));
            }
        }
    }
    out
}

/// The most workspace files listed, and read for names.
pub const MAX_FILES: usize = 4_000;

/// Directories the file list for evidence skips, besides [`UNMERGED`].
const UNLISTED: [&str; 6] = ["node_modules", "target", ".venv", "venv", "dist", "build"];

/// The workspace's files, relative to `dir`, sorted, without caches,
/// dependency trees, and build output, at most [`MAX_FILES`].
#[must_use]
pub fn workspace_files(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if !UNMERGED.contains(&name.as_str()) && !UNLISTED.contains(&name.as_str()) {
                    stack.push(path);
                }
            } else if let Ok(relative) = path.strip_prefix(dir) {
                out.push(relative.to_string_lossy().into_owned());
                if out.len() >= MAX_FILES {
                    out.sort();
                    return out;
                }
            }
        }
    }
    out.sort();
    out
}

/// Names that are a project's configuration or documentation, never its
/// input.
pub const NOT_INPUT: &[&str] = &[
    "package.json",
    "package-lock.json",
    "composer.json",
    "tsconfig.json",
    "jsconfig.json",
    "requirements.txt",
    "constraints.txt",
    "cmakelists.txt",
    "robots.txt",
];

/// Extensions of files that are code or documentation, never data.
pub const CODE: &[&str] = &[
    "py", "pyc", "sh", "bash", "js", "mjs", "ts", "rs", "go", "c", "h", "cc", "cpp", "hpp", "java",
    "rb", "pl", "pm", "v", "md", "rst", "toml", "cfg", "ini", "lock", "bas", "frm", "cls",
];

/// Whether a file name can be a task input: not hidden, not code or
/// documentation, and not a project's configuration or lockfile.
#[must_use]
pub fn is_input_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    !name.starts_with('.')
        && !NOT_INPUT.contains(&lower.as_str())
        && !lower.starts_with("requirements")
        && !lower.starts_with("readme")
        && !lower.starts_with("license")
        && !lower.starts_with("changelog")
        && !lower.ends_with(".lock")
        && !lower.starts_with("tsconfig")
        && lower != "makefile"
        && !extension(name).is_some_and(|e| CODE.contains(&e.as_str()))
}

/// The lowercase extension of `path`'s file name, if it has one.
#[must_use]
pub fn extension(path: &str) -> Option<String> {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.')
        .filter(|(stem, _)| !stem.is_empty())
        .map(|(_, e)| e.to_lowercase())
}

/// The SHA-256 of `bytes`, in lowercase hex.
#[must_use]
pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
