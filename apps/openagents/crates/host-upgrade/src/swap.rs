//! Swapping bundles: two renames on one disk, with the replaced bundle kept
//! for a roll back. A running app keeps its open binary through a rename.

use std::path::Path;

/// Put `staged` at `bundle`, moving the bundle there now to `previous`.
/// On failure the bundle is back where it was.
///
/// # Errors
/// Either rename fails.
pub fn swap_in(staged: &Path, bundle: &Path, previous: &Path) -> Result<(), String> {
    if !staged.is_dir() {
        return Err(format!("the staged app {} is missing", staged.display()));
    }
    if let Some(parent) = previous.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("couldn't create {}: {e}", parent.display()))?;
    }
    if previous.exists() {
        std::fs::remove_dir_all(previous)
            .map_err(|e| format!("couldn't clear {}: {e}", previous.display()))?;
    }
    let had_bundle = bundle.exists();
    if had_bundle {
        std::fs::rename(bundle, previous)
            .map_err(|e| format!("couldn't move the current app aside: {e}"))?;
    }
    if let Err(e) = std::fs::rename(staged, bundle) {
        if had_bundle {
            let _ = std::fs::rename(previous, bundle);
        }
        return Err(format!("couldn't move the new app into place: {e}"));
    }
    Ok(())
}

/// Put `previous` back at `bundle`, keeping the bundle that failed beside it
/// as `<previous>.failed` for a look.
///
/// # Errors
/// There's no previous bundle, or a rename fails.
pub fn roll_back(bundle: &Path, previous: &Path) -> Result<(), String> {
    if !previous.is_dir() {
        return Err(format!("there's no previous app at {}", previous.display()));
    }
    let failed = previous.with_extension("failed");
    let _ = std::fs::remove_dir_all(&failed);
    if bundle.exists() {
        std::fs::rename(bundle, &failed)
            .map_err(|e| format!("couldn't move the new app aside: {e}"))?;
    }
    std::fs::rename(previous, bundle).map_err(|e| format!("couldn't put the old app back: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle(path: &Path, version: &str) {
        std::fs::create_dir_all(path.join("Contents")).unwrap();
        std::fs::write(path.join("Contents").join("version"), version).unwrap();
    }

    fn version(path: &Path) -> String {
        std::fs::read_to_string(path.join("Contents").join("version")).unwrap()
    }

    #[test]
    fn swaps_in_and_rolls_back() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("App.app");
        let staged = dir.path().join("stage").join("App.app");
        let previous = dir.path().join("previous").join("App.app");
        bundle(&app, "old");
        bundle(&staged, "new");
        swap_in(&staged, &app, &previous).unwrap();
        assert_eq!(version(&app), "new");
        assert_eq!(version(&previous), "old");
        assert!(!staged.exists());

        roll_back(&app, &previous).unwrap();
        assert_eq!(version(&app), "old");
        assert_eq!(version(&previous.with_extension("failed")), "new");

        // A missing stage changes nothing.
        let err = swap_in(&staged, &app, &previous).unwrap_err();
        assert!(err.contains("missing"), "{err}");
        assert_eq!(version(&app), "old");
    }
}
