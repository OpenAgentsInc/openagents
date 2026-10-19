//! Secrets in private files, for desktops without the Apple Keychain.
//!
//! Linux desktops often run no Secret Service (CoderOS runs none), so the
//! desktop app keeps its sign-in and identity words in files only this user
//! can read: a `0700` directory under the data folder, one `0600` file an
//! item, written in one step. macOS and iOS keep using the Keychain; this
//! store is compiled for them only in tests.
//!
//! An item's file name is a digest of its service and key, so the directory
//! listing names nothing.
use crate::platform::Passwords;
use openagents::StorageError;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The largest item this store reads back.
const MAX_ITEM: u64 = 64 * 1024;

/// Items under `root`, one directory a service.
#[derive(Clone, Debug)]
pub struct FileSecrets {
    root: PathBuf,
}

impl FileSecrets {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn digest(text: &str) -> String {
        Sha256::digest(text.as_bytes())
            .iter()
            .take(16)
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    fn service_dir(&self, service: &str) -> PathBuf {
        self.root.join(Self::digest(service))
    }

    fn item(&self, service: &str, key: &str) -> PathBuf {
        self.service_dir(service).join(Self::digest(key))
    }

    fn read(path: &Path) -> Result<Option<Vec<u8>>, StorageError> {
        match std::fs::metadata(path) {
            Ok(meta) if meta.len() > MAX_ITEM => return Err(StorageError::Unavailable),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(StorageError::Unavailable),
        }
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(StorageError::Unavailable),
        }
    }

    fn private_dir(path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }

    /// Write `bytes` to `path`, readable by this user only. With `exclusive`,
    /// refuse (`Ok(false)`) when the item already exists.
    fn write(&self, path: &Path, bytes: &[u8], exclusive: bool) -> std::io::Result<bool> {
        use std::io::Write as _;
        Self::private_dir(&self.root)?;
        let parent = path.parent().unwrap_or(&self.root);
        Self::private_dir(parent)?;
        let temporary = parent.join(format!(
            ".{}.{}.tmp",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("item"),
            std::process::id()
        ));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        if exclusive {
            // A hard link fails when the name exists, so a racing writer's
            // item is never replaced.
            let linked = std::fs::hard_link(&temporary, path);
            let _ = std::fs::remove_file(&temporary);
            return match linked {
                Ok(()) => Ok(true),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
                Err(e) => Err(e),
            };
        }
        std::fs::rename(&temporary, path).inspect_err(|_| {
            let _ = std::fs::remove_file(&temporary);
        })?;
        Ok(true)
    }

    /// Add the item only when it's absent. `Ok(false)` when one exists.
    pub fn create(&self, service: &str, key: &str, bytes: &[u8]) -> Result<bool, StorageError> {
        self.write(&self.item(service, key), bytes, true)
            .map_err(|_| StorageError::Unavailable)
    }
}

impl Passwords for FileSecrets {
    fn get(&self, service: &str, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        Self::read(&self.item(service, key))
    }
    fn set(&self, service: &str, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        self.write(&self.item(service, key), bytes, false)
            .map(|_| ())
            .map_err(|_| StorageError::Unavailable)
    }
    fn delete(&self, service: &str, key: &str) -> Result<(), StorageError> {
        match std::fs::remove_file(self.item(service, key)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(StorageError::Unavailable),
        }
    }
    fn list(&self, service: &str) -> Result<Vec<Vec<u8>>, StorageError> {
        let entries = match std::fs::read_dir(self.service_dir(service)) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err(StorageError::Unavailable),
        };
        let mut items = Vec::new();
        for entry in entries.flatten().take(256) {
            let name = entry.file_name();
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
            if let Some(bytes) = Self::read(&entry.path())? {
                items.push(bytes);
            }
        }
        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_round_trip_and_stay_private() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileSecrets::new(dir.path().join("secrets"));
        assert_eq!(store.get("svc", "k").unwrap(), None);
        store.set("svc", "k", b"one").unwrap();
        assert_eq!(store.get("svc", "k").unwrap().as_deref(), Some(&b"one"[..]));
        store.set("svc", "k", b"two").unwrap();
        assert_eq!(store.get("svc", "k").unwrap().as_deref(), Some(&b"two"[..]));
        store.set("svc", "other", b"three").unwrap();
        let mut listed = store.list("svc").unwrap();
        listed.sort();
        assert_eq!(listed, vec![b"three".to_vec(), b"two".to_vec()]);
        assert!(store.list("nothing").unwrap().is_empty());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&dir.path().join("secrets")), 0o700);
            assert_eq!(mode(&store.item("svc", "k")), 0o600);
        }
        // File names don't carry the service or key.
        let names: Vec<_> = walk(dir.path());
        assert!(
            names
                .iter()
                .all(|n| !n.contains("svc") && !n.contains("other"))
        );
        store.delete("svc", "k").unwrap();
        store.delete("svc", "k").unwrap();
        assert_eq!(store.get("svc", "k").unwrap(), None);
    }

    #[test]
    fn create_never_replaces_an_item() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileSecrets::new(dir.path());
        assert!(store.create("svc", "k", b"first").unwrap());
        assert!(!store.create("svc", "k", b"second").unwrap());
        assert_eq!(
            store.get("svc", "k").unwrap().as_deref(),
            Some(&b"first"[..])
        );
        assert_eq!(store.list("svc").unwrap().len(), 1);
    }

    fn walk(dir: &Path) -> Vec<String> {
        let mut names = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            names.push(entry.file_name().to_string_lossy().into_owned());
            if entry.path().is_dir() {
                names.extend(walk(&entry.path()));
            }
        }
        names
    }
}
