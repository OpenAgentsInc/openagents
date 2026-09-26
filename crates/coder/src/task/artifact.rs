//! Retain changed candidate bytes separately from the writable workspace.
use super::*;
use coder_boundary::{Change, Snapshot, compare};

const CONTENT_MAX: usize = 8 * 1024 * 1024;
const FILE_MAX: usize = 1024 * 1024;
const ENTRY_MAX: usize = 4096;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub path: PathBuf,
    pub state: String,
    pub digest: Option<String>,
    pub bytes: Option<usize>,
    pub link_target: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub source_snapshot: String,
    pub candidate_snapshot: Option<String>,
    pub complete: bool,
    pub omitted_changes: usize,
    pub changes: Vec<Change>,
    pub entries: Vec<Entry>,
}

fn blob_name(digest: &str) -> Result<String, Error> {
    let hex = digest.strip_prefix("sha256:").ok_or(Error::UnsafePath)?;
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Error::UnsafePath);
    }
    Ok(format!("artifact-{hex}.blob"))
}

fn save(directory: &Path, digest: &str, bytes: &[u8]) -> Result<(), Error> {
    let path = directory.join(blob_name(digest)?);
    if regular_or_absent(&path)? {
        let mut saved = Vec::new();
        let mut file = private_open(&path, false, false)?;
        std::io::Read::by_ref(&mut file)
            .take(FILE_MAX as u64 + 1)
            .read_to_end(&mut saved)?;
        file.sync_all()?;
        if saved != bytes {
            return Err(Error::Corrupt(
                "retained artifact bytes differ from their digest",
            ));
        }
    } else {
        let mut file = private_open(&path, true, true)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    Ok(())
}

pub(super) fn retain(
    directory: &Path,
    before: &Snapshot,
    after: &Snapshot,
) -> Result<(String, String), Error> {
    let verdict = compare(before, after);
    let changes: Vec<_> = verdict.changes().iter().take(ENTRY_MAX).cloned().collect();
    let mut manifest = Manifest {
        schema: "openagents.coder.task-artifacts.v1".into(),
        source_snapshot: before.digest(),
        candidate_snapshot: after.is_complete().then(|| after.digest()),
        complete: !verdict.is_unverifiable(),
        omitted_changes: verdict.changes().len().saturating_sub(ENTRY_MAX),
        changes,
        entries: Vec::new(),
    };
    manifest.complete &= manifest.omitted_changes == 0;
    let mut total = 0;
    for change in &manifest.changes {
        let (path, removed) = match change {
            Change::Created { path } | Change::Modified { path } | Change::Retyped { path } => {
                (path, false)
            }
            Change::Removed { path } => (path, true),
            Change::Renamed { to, .. } => (to, false),
        };
        let mut entry = Entry {
            path: path.clone(),
            state: "unavailable".into(),
            digest: None,
            bytes: None,
            link_target: None,
        };
        if removed {
            entry.state = "removed".into();
        } else {
            let candidate = after.root().join(path);
            match std::fs::symlink_metadata(&candidate) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    match std::fs::read_link(&candidate) {
                        Ok(target) if after.matches_link(path, &target) => {
                            entry.state = "symlink".into();
                            entry.link_target = Some(target);
                        }
                        _ => manifest.complete = false,
                    }
                }
                Ok(metadata) if metadata.is_dir() => entry.state = "directory".into(),
                Ok(metadata)
                    if metadata.is_file()
                        && metadata.len() <= FILE_MAX as u64
                        && total + metadata.len() as usize <= CONTENT_MAX =>
                {
                    let mut bytes = Vec::new();
                    match confined_file(after.root(), path)
                        .and_then(|file| file.take(FILE_MAX as u64 + 1).read_to_end(&mut bytes))
                    {
                        Ok(_)
                            if bytes.len() <= FILE_MAX
                                && total + bytes.len() <= CONTENT_MAX
                                && after.matches_file(path, &bytes) =>
                        {
                            let digest = digest_bytes(&bytes);
                            save(directory, &digest, &bytes)?;
                            total += bytes.len();
                            entry.state = "retained".into();
                            entry.bytes = Some(bytes.len());
                            entry.digest = Some(digest);
                        }
                        _ => manifest.complete = false,
                    }
                }
                _ => {
                    entry.state = "unavailable_or_over_limit".into();
                    manifest.complete = false;
                }
            }
        }
        manifest.entries.push(entry);
    }
    // An uncooperative writer invalidates the observation rather than relabeling
    // later bytes as the completed candidate.
    let final_snapshot = Snapshot::observe(after.root());
    manifest.complete &= final_snapshot.is_complete() && final_snapshot.digest() == after.digest();
    let bytes = serde_json::to_vec(&manifest)
        .map_err(|_| Error::Corrupt("the artifact manifest cannot be encoded"))?;
    let digest = digest_bytes(&bytes);
    let filename = format!("manifest-{}.json", digest.trim_start_matches("sha256:"));
    if bytes.len() > MAX_STORE_BYTES {
        return Err(Error::LimitExceeded);
    }
    let path = directory.join(&filename);
    if regular_or_absent(&path)? {
        let mut file = private_open(&path, false, false)?;
        let mut saved = Vec::new();
        std::io::Read::by_ref(&mut file)
            .take(MAX_STORE_BYTES as u64 + 1)
            .read_to_end(&mut saved)?;
        if saved != bytes {
            return Err(Error::Corrupt("retained manifest differs from its digest"));
        }
        file.sync_all()?;
    } else {
        let mut file = private_open(&path, true, true)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    File::open(directory)?.sync_all()?;
    Ok((filename, digest))
}

/// Read only a manifest whose exact identity the completed owner result names.
pub fn manifest(directory: &Path, task: &Task) -> Result<Option<Manifest>, Error> {
    let Some(result) = task.run.as_ref().and_then(|run| run.result.as_ref()) else {
        return Ok(None);
    };
    let (Some(filename), Some(digest)) = (&result.artifact_file, &result.artifact_digest) else {
        return Ok(None);
    };
    if filename
        != &format!(
            "manifest-{}.json",
            digest.strip_prefix("sha256:").ok_or(Error::UnsafePath)?
        )
    {
        return Err(Error::UnsafePath);
    }
    blob_name(digest)?;
    let mut bytes = Vec::new();
    private_open(&directory.join(filename), false, false)?
        .take(MAX_STORE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_STORE_BYTES || digest_bytes(&bytes) != *digest {
        return Err(Error::Corrupt("artifact manifest digest mismatch"));
    }
    let value = parse_strict_bounded(&bytes, MAX_STORE_BYTES)
        .map_err(|_| Error::Corrupt("invalid artifact manifest"))?;
    let manifest: Manifest = serde_json::from_value(value)
        .map_err(|_| Error::Corrupt("invalid artifact manifest schema"))?;
    if manifest.schema != "openagents.coder.task-artifacts.v1" {
        return Err(Error::UnsupportedSchema);
    }
    Ok(Some(manifest))
}

/// Retrieve exact retained bytes by their manifest path, never by an arbitrary
/// client-supplied filesystem path. Missing or altered bytes refuse the read.
pub fn read(directory: &Path, task_id: &str, path: &Path) -> Result<Vec<u8>, Error> {
    let store = Store::open(directory)?;
    let task = store.show(task_id)?;
    let manifest = manifest(&store.dir, &task)?.ok_or(Error::NotFound)?;
    let entry = manifest
        .entries
        .iter()
        .find(|entry| entry.path == path)
        .ok_or(Error::NotFound)?;
    read_entry(&store.dir, entry)
}

fn read_entry(directory: &Path, entry: &Entry) -> Result<Vec<u8>, Error> {
    let digest = entry.digest.as_deref().ok_or(Error::NotFound)?;
    let mut bytes = Vec::new();
    private_open(&directory.join(blob_name(digest)?), false, false)?
        .take(FILE_MAX as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > FILE_MAX || Some(bytes.len()) != entry.bytes || digest_bytes(&bytes) != digest
    {
        return Err(Error::Corrupt("retained artifact digest mismatch"));
    }
    Ok(bytes)
}

/// Check current availability without dropping the original manifest entries.
pub fn faults(directory: &Path, manifest: &Manifest) -> Vec<String> {
    manifest
        .entries
        .iter()
        .filter(|entry| entry.state == "retained")
        .filter_map(|entry| {
            read_entry(directory, entry)
                .err()
                .map(|error| format!("{}: {error}", entry.path.display()))
        })
        .collect()
}

/// Resolve each component against an already opened directory, refusing links
/// at every level. A renamed parent cannot redirect a subsequent child open.
pub(super) fn confined_file(root: &Path, relative: &Path) -> std::io::Result<File> {
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let mut directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(root)?;
    let components: Vec<_> = relative.components().collect();
    if components.is_empty() {
        return Err(std::io::Error::other("empty artifact path"));
    }
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(name) = component else {
            return Err(std::io::Error::other("artifact path is not relative"));
        };
        let name = std::ffi::CString::new(name.as_bytes()).map_err(std::io::Error::other)?;
        let leaf = index + 1 == components.len();
        let flags = libc::O_RDONLY
            | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | libc::O_NONBLOCK
            | if leaf { 0 } else { libc::O_DIRECTORY };
        // SAFETY: the directory FD is live and name is a NUL-terminated single
        // component. The returned descriptor is owned exactly once by File.
        let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if fd == -1 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: openat returned a new descriptor that no other owner holds.
        let file = unsafe { File::from_raw_fd(fd) };
        if leaf {
            let metadata = file.metadata()?;
            if !metadata.is_file() || metadata.nlink() != 1 {
                return Err(std::io::Error::other(
                    "artifact is not a singly linked regular file",
                ));
            }
            return Ok(file);
        }
        directory = file;
    }
    Err(std::io::Error::other("artifact has no file component"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parent_links_leaf_links_hardlinks_and_path_escape_refuse() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("private"), "outside").unwrap();
        symlink(outside.path(), root.path().join("parent")).unwrap();
        symlink(outside.path().join("private"), root.path().join("leaf")).unwrap();
        std::fs::hard_link(outside.path().join("private"), root.path().join("hard")).unwrap();
        for path in ["parent/private", "leaf", "hard", "../outside", "/outside"] {
            assert!(
                confined_file(root.path(), Path::new(path)).is_err(),
                "{path}"
            );
        }
        std::fs::create_dir(root.path().join("safe")).unwrap();
        std::fs::write(root.path().join("safe/file"), "retained").unwrap();
        let mut contents = String::new();
        confined_file(root.path(), Path::new("safe/file"))
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        assert_eq!(contents, "retained");
    }
}
