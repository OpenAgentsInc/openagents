//! Snapshots of the host's state directories, taken before a trial and
//! restored when the trial rolls back.
//!
//! A snapshot copies each configured state directory into
//! `<host root>/snapshots/<id>/<index>/` and records a manifest with each
//! tree's digest. A missing state directory is recorded as absent, and a
//! restore removes whatever the trial created in its place. A snapshot is
//! staged under a hidden name and renamed into place only when complete, so
//! a snapshot that exists is whole.
//!
//! A restore is idempotent. It stages the copy beside the target, moves the
//! target aside, renames the copy into place, removes the old tree, and
//! checks the restored digest. A restore interrupted at any step completes
//! when it runs again.
//!
//! Only directories and ordinary files are copied. A symbolic link, device,
//! FIFO, or socket in a state directory refuses the snapshot rather than
//! following or dropping it.

use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::{Error, Result, fsx};

/// The snapshot manifest schema.
pub const SNAPSHOT_SCHEMA: &str = "openagents.coder.host-snapshot.v1";

/// A test hook called at named points. Production callers pass a hook that
/// always returns `Ok`.
pub type Hook<'a> = &'a mut dyn FnMut(&'static str) -> Result<()>;

/// One state directory in a snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// The absolute state directory.
    pub path: PathBuf,
    /// Whether the directory existed when the snapshot was taken.
    pub present: bool,
    /// The tree digest, when present.
    pub digest: Option<String>,
    /// The number of files copied.
    pub files: u64,
    /// The number of file bytes copied.
    pub bytes: u64,
}

/// What a snapshot holds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Always [`SNAPSHOT_SCHEMA`].
    pub schema: String,
    /// The snapshot identifier.
    pub id: String,
    /// One entry per state directory, in configuration order.
    pub entries: Vec<Entry>,
}

/// Takes a snapshot named `id` of `state_dirs` under `snapshots`, holding at
/// most `max_bytes` of file content, and returns the snapshot directory.
pub fn take(
    snapshots: &Path,
    id: &str,
    state_dirs: &[PathBuf],
    max_bytes: u64,
    hook: Hook<'_>,
) -> Result<PathBuf> {
    fsx::private_dir(snapshots)?;
    let staging = snapshots.join(format!(".staging-{id}"));
    let destination = snapshots.join(id);
    if fs::symlink_metadata(&destination).is_ok() {
        return Err(Error::refused(format!("snapshot {id} already exists")));
    }
    fsx::remove_tree_if_present(&staging)?;
    fs::create_dir(&staging)?;
    fs::set_permissions(&staging, fs::Permissions::from_mode(0o700))?;
    let mut budget = max_bytes;
    let mut entries = Vec::new();
    for (index, state) in state_dirs.iter().enumerate() {
        let copy = staging.join(index.to_string());
        let entry = match fs::symlink_metadata(state) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Entry {
                path: state.clone(),
                present: false,
                digest: None,
                files: 0,
                bytes: 0,
            },
            Err(error) => return Err(error.into()),
            Ok(meta) if !meta.file_type().is_dir() => {
                return Err(Error::refused(format!(
                    "state path {} is not an ordinary directory",
                    state.display()
                )));
            }
            Ok(_) => {
                let (files, bytes) = copy_tree(state, &copy, &mut budget)?;
                Entry {
                    path: state.clone(),
                    present: true,
                    digest: Some(tree_digest(state)?),
                    files,
                    bytes,
                }
            }
        };
        entries.push(entry);
    }
    let manifest = Manifest {
        schema: SNAPSHOT_SCHEMA.into(),
        id: id.into(),
        entries,
    };
    fsx::atomic_write(
        &staging.join("manifest.json"),
        &serde_json::to_vec_pretty(&manifest)?,
        0o600,
    )?;
    fsx::sync_dir(&staging)?;
    hook("snapshot-staged")?;
    fs::rename(&staging, &destination)?;
    fsx::sync_dir(snapshots)?;
    Ok(destination)
}

/// Reads a snapshot's manifest.
pub fn manifest(snapshot: &Path) -> Result<Manifest> {
    let manifest: Manifest = serde_json::from_slice(&fsx::read_bounded(
        &snapshot.join("manifest.json"),
        fsx::RECORD_MAX,
    )?)?;
    if manifest.schema != SNAPSHOT_SCHEMA {
        return Err(Error::refused("unsupported snapshot schema"));
    }
    Ok(manifest)
}

/// Restores every state directory to the snapshot's contents.
pub fn restore(snapshot: &Path, hook: Hook<'_>) -> Result<()> {
    let manifest = manifest(snapshot)?;
    for (index, entry) in manifest.entries.iter().enumerate() {
        let target = &entry.path;
        let parent = target
            .parent()
            .ok_or_else(|| Error::refused("a state directory has no parent"))?;
        let name = target
            .file_name()
            .ok_or_else(|| Error::refused("a state directory has no name"))?
            .to_string_lossy()
            .into_owned();
        let staging = parent.join(format!(".{name}.restore"));
        let trash = parent.join(format!(".{name}.rollback-trash"));
        fsx::remove_tree_if_present(&staging)?;
        if entry.present {
            let mut unbounded = u64::MAX;
            copy_tree(&snapshot.join(index.to_string()), &staging, &mut unbounded)?;
        }
        hook("restore-staged")?;
        if fs::symlink_metadata(target).is_ok() {
            fsx::remove_tree_if_present(&trash)?;
            fs::rename(target, &trash)?;
            fsx::sync_dir(parent)?;
        }
        hook("restore-moved")?;
        if entry.present {
            fs::rename(&staging, target)?;
        }
        fsx::sync_dir(parent)?;
        fsx::remove_tree_if_present(&trash)?;
        if entry.present && Some(tree_digest(target)?) != entry.digest {
            return Err(Error::refused(format!(
                "restored state {} does not match its snapshot",
                target.display()
            )));
        }
    }
    Ok(())
}

/// A digest of a directory tree: every relative path, its kind, its
/// permission bits, and each file's content digest, in sorted order.
pub fn tree_digest(root: &Path) -> Result<String> {
    let mut lines = Vec::new();
    walk(root, Path::new(""), &mut lines)?;
    lines.sort();
    let mut hasher = Sha256::new();
    for line in lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn walk(root: &Path, relative: &Path, lines: &mut Vec<String>) -> Result<()> {
    let here = root.join(relative);
    for item in fs::read_dir(&here)? {
        let item = item?;
        let path = relative.join(item.file_name());
        let meta = fs::symlink_metadata(root.join(&path))?;
        let mode = meta.mode() & 0o7777;
        let shown = path.to_string_lossy();
        if meta.file_type().is_dir() {
            lines.push(format!("d {mode:o} {shown}"));
            walk(root, &path, lines)?;
        } else if meta.file_type().is_file() {
            let content = fsx::sha256_file(&root.join(&path))?;
            lines.push(format!("f {mode:o} {shown} {content}"));
        } else {
            return Err(Error::refused(format!(
                "state entry {shown} is not a directory or an ordinary file"
            )));
        }
    }
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path, budget: &mut u64) -> Result<(u64, u64)> {
    let meta = fs::symlink_metadata(source)?;
    fs::create_dir(destination)?;
    fs::set_permissions(
        destination,
        fs::Permissions::from_mode(meta.mode() & 0o7777),
    )?;
    let (mut files, mut bytes) = (0u64, 0u64);
    let mut items: Vec<_> = fs::read_dir(source)?.collect::<std::io::Result<_>>()?;
    items.sort_by_key(fs::DirEntry::file_name);
    for item in items {
        let from = item.path();
        let to = destination.join(item.file_name());
        let meta = fs::symlink_metadata(&from)?;
        if meta.file_type().is_dir() {
            let (f, b) = copy_tree(&from, &to, budget)?;
            files += f;
            bytes += b;
        } else if meta.file_type().is_file() {
            if meta.len() > *budget {
                return Err(Error::refused(
                    "host state exceeds the snapshot byte limit; raise snapshot_max_bytes or free space",
                ));
            }
            *budget -= meta.len();
            fs::copy(&from, &to)?;
            fs::set_permissions(&to, fs::Permissions::from_mode(meta.mode() & 0o7777))?;
            fs::File::open(&to)?.sync_all()?;
            files += 1;
            bytes += meta.len();
        } else {
            return Err(Error::refused(format!(
                "state entry {} is not a directory or an ordinary file",
                from.display()
            )));
        }
    }
    fsx::sync_dir(destination)?;
    Ok((files, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(_: &'static str) -> Result<()> {
        Ok(())
    }

    #[test]
    fn restore_returns_changed_added_and_removed_state() {
        let temp = tempfile::tempdir().unwrap();
        let state = temp.path().join("tasks");
        let absent = temp.path().join("absent");
        fs::create_dir_all(state.join("nested")).unwrap();
        fs::write(state.join("tasks.json"), "before").unwrap();
        fs::write(state.join("nested/keep"), "keep").unwrap();
        fs::set_permissions(state.join("nested/keep"), fs::Permissions::from_mode(0o600)).unwrap();
        let before = tree_digest(&state).unwrap();
        let snapshots = temp.path().join("snapshots");
        let taken = take(
            &snapshots,
            "s1",
            &[state.clone(), absent.clone()],
            1 << 20,
            &mut ok,
        )
        .unwrap();

        fs::write(state.join("tasks.json"), "after").unwrap();
        fs::remove_file(state.join("nested/keep")).unwrap();
        fs::write(state.join("new"), "new").unwrap();
        fs::create_dir(&absent).unwrap();
        fs::write(absent.join("created"), "x").unwrap();

        restore(&taken, &mut ok).unwrap();
        assert_eq!(tree_digest(&state).unwrap(), before);
        assert_eq!(
            fs::read_to_string(state.join("tasks.json")).unwrap(),
            "before"
        );
        assert!(!absent.exists());
        let mode = fs::metadata(state.join("nested/keep")).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn an_interrupted_restore_completes_when_it_runs_again() {
        for point in ["restore-staged", "restore-moved"] {
            let temp = tempfile::tempdir().unwrap();
            let state = temp.path().join("tasks");
            fs::create_dir(&state).unwrap();
            fs::write(state.join("tasks.json"), "before").unwrap();
            let taken = take(
                &temp.path().join("snapshots"),
                "s1",
                std::slice::from_ref(&state),
                1 << 20,
                &mut ok,
            )
            .unwrap();
            fs::write(state.join("tasks.json"), "after").unwrap();
            let mut crash = |at: &'static str| {
                if at == point {
                    Err(Error::Crash(at))
                } else {
                    Ok(())
                }
            };
            assert!(matches!(restore(&taken, &mut crash), Err(Error::Crash(_))));
            restore(&taken, &mut ok).unwrap();
            assert_eq!(
                fs::read_to_string(state.join("tasks.json")).unwrap(),
                "before",
                "{point}"
            );
            assert!(!temp.path().join(".tasks.rollback-trash").exists());
            assert!(!temp.path().join(".tasks.restore").exists());
        }
    }

    #[test]
    fn an_interrupted_snapshot_leaves_only_staging() {
        let temp = tempfile::tempdir().unwrap();
        let state = temp.path().join("tasks");
        fs::create_dir(&state).unwrap();
        let snapshots = temp.path().join("snapshots");
        let mut crash = |at: &'static str| Err(Error::Crash(at));
        assert!(
            take(
                &snapshots,
                "s1",
                std::slice::from_ref(&state),
                1 << 20,
                &mut crash
            )
            .is_err()
        );
        assert!(!snapshots.join("s1").exists());
        assert!(snapshots.join(".staging-s1").exists());
        take(&snapshots, "s1", &[state], 1 << 20, &mut ok).unwrap();
        assert!(!snapshots.join(".staging-s1").exists());
    }

    #[test]
    fn links_and_oversized_state_refuse() {
        let temp = tempfile::tempdir().unwrap();
        let state = temp.path().join("tasks");
        fs::create_dir(&state).unwrap();
        fs::write(state.join("big"), vec![0u8; 100]).unwrap();
        let snapshots = temp.path().join("snapshots");
        assert!(
            take(
                &snapshots,
                "small",
                std::slice::from_ref(&state),
                10,
                &mut ok
            )
            .is_err()
        );
        std::os::unix::fs::symlink("/etc/hosts", state.join("link")).unwrap();
        assert!(take(&snapshots, "link", &[state], 1 << 20, &mut ok).is_err());
    }
}
