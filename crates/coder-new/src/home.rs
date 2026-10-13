//! Where Coder keeps its data: `~/.openagents/coder` (#11361). Earlier
//! builds used `~/.openagents/coder-new`; the first start after the change
//! moves what is there into `coder` and leaves `coder-new` as a link to it,
//! so an older build still finds the same files.

use std::fs;
use std::path::{Path, PathBuf};

/// The directory name under `~/.openagents`.
pub const DIR: &str = "coder";
/// The name earlier builds used.
pub const OLD_DIR: &str = "coder-new";

/// `~/.openagents/coder`, after moving an earlier `coder-new` into it.
pub fn data_dir() -> Option<PathBuf> {
    let root = model_access::store::openagents_dir()?;
    let dir = root.join(DIR);
    if !cfg!(test) {
        let _ = migrate(&root);
    }
    Some(dir)
}

/// `~/.openagents/coder/<child>`.
pub fn data_path(child: &str) -> Option<PathBuf> {
    data_dir().map(|dir| dir.join(child))
}

/// For a run with its own state (`--state DIR`): routing records go to
/// `DIR/routing` and System One decision logs to `DIR/decisions`, unless
/// their own variables already say where (#11521). Set in the process
/// environment, before any thread starts, so the hooks Claude Code and
/// Codex run as child processes log there too.
pub fn keep_records_under(state: &Path) {
    for (name, dir) in record_dirs(state, &|name| std::env::var_os(name)) {
        // SAFETY: called at startup, before this process starts threads.
        unsafe { std::env::set_var(name, dir) };
    }
}

/// The variables [`keep_records_under`] sets for `state`: each one the
/// environment (`variable`) leaves unset or empty.
fn record_dirs(
    state: &Path,
    variable: &dyn Fn(&str) -> Option<std::ffi::OsString>,
) -> Vec<(&'static str, PathBuf)> {
    [
        (crate::routing_record::DIR_ENV, "routing"),
        (hook_log::DIR_VAR, "decisions"),
    ]
    .into_iter()
    .filter(|(name, _)| variable(name).is_none_or(|value| value.is_empty()))
    .map(|(name, child)| (name, state.join(child)))
    .collect()
}

/// Moves `ROOT/coder-new` into `ROOT/coder` once. Entries are renamed, so
/// chats, the sign-in, sync state, locks, and the build folder keep their
/// bytes; a directory both sides have is merged, and a file both sides have
/// keeps the one already in `coder`. When `coder-new` is empty afterwards it
/// becomes a symbolic link to `coder`. Returns whether anything moved.
pub fn migrate(root: &Path) -> std::io::Result<bool> {
    let old = root.join(OLD_DIR);
    let new = root.join(DIR);
    let Ok(meta) = fs::symlink_metadata(&old) else {
        return Ok(false);
    };
    if !meta.is_dir() {
        return Ok(false);
    }
    fs::create_dir_all(&new)?;
    let _ = restrict(&new);
    let moved = merge(&old, &new)?;
    if fs::read_dir(&old)?.next().is_none() {
        fs::remove_dir(&old)?;
        #[cfg(unix)]
        let _ = std::os::unix::fs::symlink(DIR, &old);
    }
    Ok(moved)
}

fn merge(from: &Path, into: &Path) -> std::io::Result<bool> {
    let mut moved = false;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let source = entry.path();
        let target = into.join(entry.file_name());
        match fs::symlink_metadata(&target) {
            Err(_) => {
                fs::rename(&source, &target)?;
                moved = true;
            }
            Ok(existing) if existing.is_dir() && entry.file_type()?.is_dir() => {
                moved |= merge(&source, &target)?;
                let _ = fs::remove_dir(&source);
            }
            Ok(_) => {}
        }
    }
    Ok(moved)
}

#[cfg(unix)]
fn restrict(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn restrict(_dir: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_with_its_own_state_keeps_its_records_there() {
        // #11521.
        let state = Path::new("/tmp/audit-state");
        assert_eq!(
            super::record_dirs(state, &|_| None),
            [
                ("CODER_ROUTING_DIR", state.join("routing")),
                ("CODER_DECISIONS_DIR", state.join("decisions")),
            ]
        );
        let routing_set = |name: &str| (name == "CODER_ROUTING_DIR").then(|| "/elsewhere".into());
        assert_eq!(
            super::record_dirs(state, &routing_set),
            [("CODER_DECISIONS_DIR", state.join("decisions"))]
        );
    }

    #[test]
    fn moves_coder_new_into_coder_and_leaves_a_link() {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join(OLD_DIR);
        fs::create_dir_all(old.join("sessions")).unwrap();
        fs::write(old.join("account.json"), "{}").unwrap();
        fs::write(old.join("sessions/a.json"), "a").unwrap();
        fs::create_dir_all(root.path().join(DIR).join("attachments")).unwrap();
        fs::write(root.path().join(DIR).join("attachments/x.png"), "x").unwrap();

        assert!(migrate(root.path()).unwrap());

        let new = root.path().join(DIR);
        assert_eq!(fs::read_to_string(new.join("account.json")).unwrap(), "{}");
        assert_eq!(
            fs::read_to_string(new.join("sessions/a.json")).unwrap(),
            "a"
        );
        assert!(new.join("attachments/x.png").is_file());
        assert!(fs::symlink_metadata(&old).unwrap().is_symlink());
        assert!(old.join("account.json").is_file());
        assert!(!migrate(root.path()).unwrap());
    }

    #[test]
    fn merges_shared_directories_and_keeps_coder_files() {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join(OLD_DIR);
        let new = root.path().join(DIR);
        fs::create_dir_all(old.join("sessions")).unwrap();
        fs::create_dir_all(new.join("sessions")).unwrap();
        fs::write(old.join("sessions/a.json"), "old").unwrap();
        fs::write(new.join("sessions/a.json"), "new").unwrap();
        fs::write(old.join("sessions/b.json"), "b").unwrap();

        migrate(root.path()).unwrap();

        assert_eq!(
            fs::read_to_string(new.join("sessions/a.json")).unwrap(),
            "new"
        );
        assert_eq!(
            fs::read_to_string(new.join("sessions/b.json")).unwrap(),
            "b"
        );
        assert!(old.join("sessions/a.json").is_file());
        assert!(!fs::symlink_metadata(&old).unwrap().is_symlink());
    }

    #[test]
    fn nothing_to_move_is_fine() {
        let root = tempfile::tempdir().unwrap();
        assert!(!migrate(root.path()).unwrap());
        assert!(!root.path().join(DIR).exists());
    }
}
