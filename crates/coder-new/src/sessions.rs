//! Private local ATIF sessions shared by the terminal and CLI.

use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use serde::Serialize;
use serde_json::Value;

use crate::trajectory;

const MAX_BYTES: u64 = 64 * 1024 * 1024;
pub const INVALID_ID: &str =
    "Session IDs accept letters, numbers, underscores, and hyphens, up to 128 bytes.";

/// A retained conversation in the recent-session picker.
#[derive(Clone, Debug, Serialize)]
pub struct Summary {
    pub id: String,
    pub title: String,
    pub updated_ms: u64,
    pub cwd: Option<PathBuf>,
    pub entries: usize,
    pub path: PathBuf,
}

/// A store rooted at the Coder configuration directory.
#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
}

/// Exclusive access to a mounted conversation, released when dropped.
pub struct Lease {
    store: Store,
    id: String,
    path: PathBuf,
    _lock: File,
}

impl Store {
    #[must_use]
    pub fn under(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn path(&self, id: &str) -> Result<PathBuf, String> {
        validate_id(id)?;
        Ok(self.root.join("sessions").join(format!("{id}.atif.json")))
    }

    /// Acquire the session for one writer or a mounted terminal conversation.
    pub fn lease(&self, id: &str) -> Result<Lease, String> {
        let path = self.path(id)?;
        self.directory(true)?;
        let lock = path.with_extension("lock");
        regular_or_missing(&lock, "The chat session lock is not a regular file.")?;
        let mut options = OpenOptions::new();
        options.create(true).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options
            .open(&lock)
            .map_err(|_| "Cannot lock the chat session.")?;
        file.try_lock()
            .map_err(|_| "Another process is using this chat session.")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|_| "Cannot protect the chat session lock.")?;
        }
        Ok(Lease {
            store: self.clone(),
            id: id.into(),
            path,
            _lock: file,
        })
    }

    pub fn read(&self, id: &str) -> Result<Value, String> {
        let path = self.path(id)?;
        self.directory(false)?;
        let document = read_document(&path)?;
        if document.get("session_id").and_then(Value::as_str) != Some(id) {
            return Err("The chat session ID does not match its file.".into());
        }
        Ok(document)
    }

    pub fn save(&self, id: &str, document: &Value) -> Result<(), String> {
        self.lease(id)?.save(document)
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        self.lease(id)?.delete()
    }

    /// List saved snapshots, including conversations held by an active lease.
    pub fn list(&self) -> Result<Vec<Summary>, String> {
        self.recent(usize::MAX)
    }

    /// Read the newest valid snapshots, stopping when the requested limit is filled.
    pub fn recent(&self, limit: usize) -> Result<Vec<Summary>, String> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        if !self.directory(false)? {
            return Ok(Vec::new());
        }
        let mut candidates = Vec::new();
        for entry in
            fs::read_dir(self.root.join("sessions")).map_err(|_| "Cannot list chat sessions.")?
        {
            let entry = entry.map_err(|_| "Cannot list chat sessions.")?;
            let name = entry.file_name();
            let Some(id) = name
                .to_str()
                .and_then(|name| name.strip_suffix(".atif.json"))
            else {
                continue;
            };
            if validate_id(id).is_err() {
                continue;
            }
            let path = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            if !metadata.is_file() || metadata.len() > MAX_BYTES {
                continue;
            }
            let updated_ms = metadata
                .modified()
                .ok()
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .map(|elapsed| elapsed.as_millis().min(u128::from(u64::MAX)) as u64)
                .unwrap_or_default();
            candidates.push((id.to_owned(), path, updated_ms));
        }
        candidates.sort_by(|left, right| right.2.cmp(&left.2).then_with(|| left.0.cmp(&right.0)));
        let mut summaries = Vec::new();
        for (id, path, updated_ms) in candidates {
            let Ok(document) = read_document(&path) else {
                continue;
            };
            if document.get("session_id").and_then(Value::as_str) != Some(id.as_str()) {
                continue;
            }
            let cwd = document
                .pointer("/extra/repository")
                .or_else(|| document.pointer("/extra/cwd"))
                .and_then(Value::as_str)
                .filter(|cwd| !cwd.is_empty())
                .map(PathBuf::from);
            summaries.push(Summary {
                title: title(&document, &id),
                id,
                updated_ms,
                cwd,
                entries: document
                    .get("steps")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len),
                path,
            });
            if summaries.len() == limit {
                break;
            }
        }
        Ok(summaries)
    }

    fn directory(&self, create: bool) -> Result<bool, String> {
        for path in [&self.root, &self.root.join("sessions")] {
            match fs::symlink_metadata(path) {
                Ok(metadata) if !metadata.is_dir() => {
                    return Err("The chat session directory is not a regular directory.".into());
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    if !create {
                        return Ok(false);
                    }
                    let mut builder = fs::DirBuilder::new();
                    builder.recursive(true);
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::DirBuilderExt;
                        builder.mode(0o700);
                    }
                    builder
                        .create(path)
                        .map_err(|_| "Cannot create the chat session directory.")?;
                    if !fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir()) {
                        return Err("The chat session directory is not a regular directory.".into());
                    }
                }
                Err(_) => return Err("Cannot read the chat session directory.".into()),
            }
            #[cfg(unix)]
            if create {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                    .map_err(|_| "Cannot protect the chat session directory.")?;
            }
        }
        Ok(true)
    }
}

impl Lease {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn exists(&self) -> Result<bool, String> {
        self.store.directory(false)?;
        regular_or_missing(&self.path, "The chat session is not a regular file.")?;
        Ok(self.path.exists())
    }

    pub fn read(&self) -> Result<Value, String> {
        self.store.read(&self.id)
    }

    pub fn save(&self, document: &Value) -> Result<(), String> {
        self.store.directory(false)?;
        if document.get("session_id").and_then(Value::as_str) != Some(self.id.as_str()) {
            return Err("The chat session ID does not match its file.".into());
        }
        regular_or_missing(&self.path, "The chat session is not a regular file.")?;
        let temporary = self
            .path
            .with_extension(format!("{}.tmp", atif::log::session_id(atif::now_ms())));
        let result = trajectory::write(&temporary, document).and_then(|()| {
            fs::rename(&temporary, &self.path).map_err(|_| "Cannot save the chat session.".into())
        });
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    pub fn delete(&self) -> Result<(), String> {
        self.store.directory(false)?;
        regular_or_missing(&self.path, "The chat session is not a regular file.")?;
        fs::remove_file(&self.path).map_err(|_| "Cannot remove the chat session.".into())
    }
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
    {
        return Err(INVALID_ID.into());
    }
    Ok(())
}

fn regular_or_missing(path: &Path, error: &str) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(error.into()),
    }
}

/// Read validated ATIF without running its recorded calls.
pub fn read_document(path: &Path) -> Result<Value, String> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "Cannot read the chat session.")?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        return Err("The chat session is not a regular file or exceeds 64 MiB.".into());
    }
    let file = File::open(path).map_err(|_| "Cannot read the chat session.")?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read the chat session.")?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("The chat session exceeds 64 MiB.".into());
    }
    let value = serde_json::from_slice(&bytes)
        .map_err(|_| "This saved chat is damaged and cannot be opened.")?;
    if !atif::validate(&value).is_empty() {
        return Err("The chat session is not valid ATIF.".into());
    }
    trajectory::from_document(&value)?;
    Ok(value)
}

fn title(document: &Value, fallback: &str) -> String {
    let explicit = document
        .pointer("/extra/title")
        .or_else(|| document.get("title"))
        .and_then(Value::as_str)
        .filter(|title| !title.trim().is_empty());
    let first_user = document
        .get("steps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|step| step.get("source").and_then(Value::as_str) == Some("user"))
        .map(|step| message_text(step.get("message").unwrap_or(&Value::Null)));
    let text = explicit
        .or_else(|| first_user.as_deref().filter(|text| !text.trim().is_empty()))
        .or_else(|| {
            document
                .pointer("/extra/directive")
                .and_then(Value::as_str)
                .filter(|text| !text.trim().is_empty())
        })
        .unwrap_or(fallback);
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|character| !character.is_control())
        .take(200)
        .collect()
}

fn message_text(value: &Value) -> String {
    value.as_str().map(str::to_owned).unwrap_or_else(|| {
        value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::{Chat, Entry};
    use serde_json::json;

    fn document(id: &str, prompt: &str, cwd: &Path) -> Value {
        let mut chat = Chat::default();
        chat.entries.push(Entry::User(prompt.into()));
        trajectory::document(&chat, id, "test/local", cwd)
    }

    #[test]
    fn snapshots_are_private_atomic_and_exclusive() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::under(temp.path().join("state"));
        assert!(store.list().unwrap().is_empty());
        assert!(store.read("missing").is_err());
        assert!(!store.root.exists());
        let lease = store.lease("one").unwrap();
        assert_eq!(lease.id(), "one");
        assert!(!lease.exists().unwrap());
        assert!(store.lease("one").is_err());
        let first = document("one", "First prompt", temp.path());
        lease.save(&first).unwrap();
        assert!(lease.exists().unwrap());
        assert_eq!(lease.read().unwrap(), first);
        assert_eq!(store.read("one").unwrap(), first);
        assert!(store.read("missing").is_err());
        assert!(
            !store
                .path("missing")
                .unwrap()
                .with_extension("lock")
                .exists()
        );
        assert_eq!(store.list().unwrap()[0].title, "First prompt");
        let saved = fs::read(lease.path()).unwrap();
        assert!(lease.save(&json!({"session_id":"one"})).is_err());
        assert_eq!(fs::read(lease.path()).unwrap(), saved);
        assert!(
            lease
                .save(&document("other", "Wrong ID", temp.path()))
                .is_err()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(lease.path()).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(store.root.join("sessions"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        drop(lease);
        store
            .save("one", &document("one", "Updated prompt", temp.path()))
            .unwrap();
        assert_eq!(store.list().unwrap()[0].title, "Updated prompt");
        store.delete("one").unwrap();
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn recent_metadata_uses_titles_messages_timestamps_and_working_directories() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::under(temp.path().join("state"));
        let mut first = document("one", "First\n  user prompt", temp.path());
        first["extra"]["title"] = json!("Named conversation");
        store.save("one", &first).unwrap();
        OpenOptions::new()
            .write(true)
            .open(store.path("one").unwrap())
            .unwrap()
            .set_modified(UNIX_EPOCH + std::time::Duration::from_secs(1))
            .unwrap();
        store
            .save(
                "two",
                &document("two", "Second\n  user prompt", temp.path()),
            )
            .unwrap();
        let rows = store.list().unwrap();
        assert_eq!(
            rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
            ["two", "one"]
        );
        assert_eq!(rows[0].title, "Second user prompt");
        assert_eq!(rows[1].title, "Named conversation");
        assert_eq!(rows[1].updated_ms, 1000);
        assert_eq!(rows[0].cwd.as_deref(), Some(temp.path()));
        assert_eq!(rows[0].entries, 1);
    }

    #[test]
    fn recent_limits_valid_sessions_in_newest_first_order_and_skips_invalid_files() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::under(temp.path().join("state"));
        for (id, seconds) in [("oldest", 1), ("middle", 2), ("newest", 3)] {
            store.save(id, &document(id, id, temp.path())).unwrap();
            OpenOptions::new()
                .write(true)
                .open(store.path(id).unwrap())
                .unwrap()
                .set_modified(UNIX_EPOCH + std::time::Duration::from_secs(seconds))
                .unwrap();
        }
        let damaged = store.path("damaged").unwrap();
        fs::write(&damaged, "invalid JSON").unwrap();
        OpenOptions::new()
            .write(true)
            .open(damaged)
            .unwrap()
            .set_modified(UNIX_EPOCH + std::time::Duration::from_secs(4))
            .unwrap();
        let rows = store.recent(2).unwrap();
        assert_eq!(
            rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
            ["newest", "middle"]
        );
        assert!(store.recent(0).unwrap().is_empty());
        assert_eq!(store.list().unwrap().len(), 3);
    }

    #[test]
    fn invalid_ids_and_damaged_files_do_not_appear_as_resumable_sessions() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::under(temp.path().join("state"));
        for id in ["", "../escape", "with space", "with.dot"] {
            assert!(store.path(id).is_err());
            assert!(store.lease(id).is_err());
        }
        let lease = store.lease("damaged").unwrap();
        fs::write(lease.path(), "invalid JSON").unwrap();
        assert!(lease.read().is_err());
        assert!(store.list().unwrap().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_rejected_without_writing_their_targets() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let root_link = temp.path().join("linked-root");
        symlink(&outside, &root_link).unwrap();
        assert!(Store::under(root_link).lease("one").is_err());
        assert!(!outside.join("sessions").exists());
        let store = Store::under(temp.path().join("state"));
        fs::create_dir(&store.root).unwrap();
        symlink(&outside, store.root.join("sessions")).unwrap();
        assert!(store.lease("one").is_err());
        fs::remove_file(store.root.join("sessions")).unwrap();
        let lease = store.lease("one").unwrap();
        let target = outside.join("target");
        fs::write(&target, "kept").unwrap();
        symlink(&target, lease.path()).unwrap();
        assert!(lease.exists().is_err());
        assert!(lease.read().is_err());
        assert!(lease.save(&document("one", "Prompt", temp.path())).is_err());
        assert!(store.list().unwrap().is_empty());
        assert_eq!(fs::read_to_string(&target).unwrap(), "kept");
        drop(lease);
        let lock = store.path("locked").unwrap().with_extension("lock");
        symlink(&target, lock).unwrap();
        assert!(store.lease("locked").is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "kept");
    }
}
