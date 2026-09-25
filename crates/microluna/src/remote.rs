//! A separate place where a session's tools run, such as a fresh
//! container, instead of the host.
//!
//! A [`crate::Workspace`] given a [`Remote`] runs every command there and
//! reads, writes, and patches files there too, so the session sees one
//! consistent directory that isn't on the host. Nothing the session makes
//! comes back to the host unless the caller copies it out. The remote's
//! own confinement is its business: [`Remote::offline`] says whether it
//! has a network.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

/// A command's result, boxed so a [`Remote`] can be held as a trait object.
pub type Running<'a> = Pin<Box<dyn Future<Output = supervise::Ended> + Send + 'a>>;

/// A place outside the host where a session's tools run.
pub trait Remote: Send + Sync + std::fmt::Debug {
    /// The word a command's record carries as its boundary, such as
    /// `writer-container`.
    fn word(&self) -> &'static str;

    /// The session's working directory there, as an absolute path.
    fn root(&self) -> &str;

    /// Whether the place has no network.
    fn offline(&self) -> bool;

    /// Runs `command` with `sh -c` in the root, bounded by `wall`.
    fn run(&self, command: &str, wall: Duration) -> Running<'_>;

    /// Reads at most `max` bytes of the file at `path`, relative to the
    /// root.
    ///
    /// # Errors
    ///
    /// A sentence for the model when the file can't be read.
    fn read(&self, path: &str, max: usize) -> Result<Vec<u8>, String>;

    /// Writes the file at `path`, relative to the root, making its parent
    /// directories, or removes it when `contents` is `None`.
    ///
    /// # Errors
    ///
    /// A sentence for the model when the file can't be written.
    fn write(&self, path: &str, contents: Option<&[u8]>) -> Result<(), String>;
}

/// The most a remote file read returns, so a patch sees whole files.
pub const READ_MAX: usize = 8 * 1024 * 1024;

/// Resolves a path the model named to one relative to `root`, by its words
/// alone: an absolute path must be under `root`, and `..` can't climb above
/// it. Links are the remote's own business: they lead only to its files.
///
/// # Errors
///
/// A sentence for the model when the path leaves the root.
pub fn resolve(root: &str, path: &str) -> Result<String, String> {
    let named = path.trim();
    let root = root.trim_end_matches('/');
    let relative = if let Some(rest) = named.strip_prefix('/') {
        let rest = format!("/{rest}");
        match rest.strip_prefix(root) {
            Some(inner) if inner.is_empty() || inner.starts_with('/') => {
                inner.trim_start_matches('/').to_string()
            }
            _ => return Err(format!("{path} is outside the workspace")),
        }
    } else {
        named.to_string()
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in relative.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(format!("{path} climbs out of the workspace"));
                }
            }
            part => parts.push(part),
        }
    }
    if parts.is_empty() {
        return Err(format!("{path} is the workspace itself, not a file"));
    }
    Ok(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::resolve;

    #[test]
    fn a_remote_path_stays_under_its_root() {
        assert_eq!(resolve("/w", "a/b.py").unwrap(), "a/b.py");
        assert_eq!(resolve("/w", "/w/a/./b.py").unwrap(), "a/b.py");
        assert_eq!(resolve("/w/", "a/../b.py").unwrap(), "b.py");
        assert!(resolve("/w", "/app/out.txt").is_err());
        assert!(resolve("/w", "/wx/out.txt").is_err());
        assert!(resolve("/w", "../out.txt").is_err());
        assert!(resolve("/w", "a/../../out.txt").is_err());
        assert!(resolve("/w", "/w").is_err());
    }
}
