//! Refusals for worktree retirement outside Unix hosts.

use std::path::Path;

pub use crate::records::Undo;

const UNAVAILABLE: &str = "Automatic worktree cleanup is not available on Windows";

/// Check a worktree for retirement.
///
/// # Errors
/// Worktree retirement is unavailable on this platform.
pub fn removable(_path: &Path) -> Result<Undo, String> {
    Err(UNAVAILABLE.into())
}

/// Check a worktree against its published commit.
///
/// # Errors
/// Worktree retirement is unavailable on this platform.
pub fn removable_with(_path: &Path, _published: Option<&str>) -> Result<Undo, String> {
    Err(UNAVAILABLE.into())
}

/// Remove a checked worktree.
///
/// # Errors
/// Worktree retirement is unavailable on this platform.
pub fn remove(_undo: &Undo) -> Result<(), String> {
    Err(UNAVAILABLE.into())
}

/// Remove a worktree whose content is published.
///
/// # Errors
/// Worktree retirement is unavailable on this platform.
pub fn remove_published(_undo: &Undo) -> Result<(), String> {
    Err(UNAVAILABLE.into())
}

/// Restore an archived worktree.
///
/// # Errors
/// Worktree retirement is unavailable on this platform.
pub fn restore(_undo: &Undo) -> Result<(), String> {
    Err(UNAVAILABLE.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retirement_refuses_without_changing_files() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("keep.txt");
        std::fs::write(&file, "keep").unwrap();
        let undo = Undo {
            repo: root.path().into(),
            path: root.path().join("absent-worktree"),
            branch: None,
            commit: "a".repeat(40),
        };
        assert!(removable(root.path()).is_err());
        assert!(removable_with(root.path(), Some(&undo.commit)).is_err());
        assert!(remove(&undo).is_err());
        assert!(remove_published(&undo).is_err());
        assert!(restore(&undo).is_err());
        assert_eq!(std::fs::read_to_string(file).unwrap(), "keep");
        assert!(!undo.path.exists());
    }
}
