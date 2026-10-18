use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::types::{CoreError, CoreResult};

const DRAFT_SCHEMA: &str = "openagents.composer-drafts";
const DRAFT_VERSION: u32 = 1;
const MAX_DRAFT_BYTES: usize = 64 * 1024;
const MAX_DRAFTS: usize = 64;
const MAX_STORE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DraftFile {
    schema: String,
    version: u32,
    account: String,
    sessions: BTreeMap<String, String>,
}

impl DraftFile {
    fn empty(account: &str) -> Self {
        Self {
            schema: DRAFT_SCHEMA.to_owned(),
            version: DRAFT_VERSION,
            account: account.to_owned(),
            sessions: BTreeMap::new(),
        }
    }
}

/// A local, versioned draft book bound to one signed-in account.
/// Session keys come from the Rust session handle, never from a platform call.
pub(crate) struct ComposerDraftStore {
    directory: PathBuf,
    account: String,
    closed: Mutex<bool>,
}

impl ComposerDraftStore {
    pub(crate) fn new(directory: PathBuf, account_id: &str) -> Self {
        let account = hex_digest(account_id.as_bytes());
        Self {
            directory: directory.join("composer-drafts"),
            account,
            closed: Mutex::new(false),
        }
    }

    pub(crate) fn restore(&self, session_id: &str) -> CoreResult<Option<String>> {
        let closed = self.closed.lock().map_err(|_| CoreError::Closed)?;
        if *closed {
            return Err(CoreError::Closed);
        }
        validate_session_id(session_id)?;
        let book = self.read_book()?;
        Ok(book.sessions.get(session_id).cloned())
    }

    pub(crate) fn save(&self, session_id: &str, value: &str) -> CoreResult<()> {
        let closed = self.closed.lock().map_err(|_| CoreError::Closed)?;
        if *closed {
            return Err(CoreError::Closed);
        }
        validate_session_id(session_id)?;
        if value.len() > MAX_DRAFT_BYTES {
            return Err(CoreError::InvalidArgument {
                message: "draft exceeds its byte limit".into(),
            });
        }

        let mut book = self.read_book()?;
        if value.is_empty() {
            book.sessions.remove(session_id);
        } else {
            if !book.sessions.contains_key(session_id) && book.sessions.len() >= MAX_DRAFTS {
                return Err(CoreError::Storage {
                    message: "too many saved composer drafts".into(),
                });
            }
            book.sessions
                .insert(session_id.to_owned(), value.to_owned());
        }
        self.write_book(&book)
    }

    pub(crate) fn close(&self) {
        if let Ok(mut closed) = self.closed.lock() {
            *closed = true;
        }
    }

    fn path(&self) -> PathBuf {
        self.directory.join(format!("{}.json", self.account))
    }

    fn read_book(&self) -> CoreResult<DraftFile> {
        let path = self.path();
        let mut file = match OpenOptions::new().read(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(DraftFile::empty(&self.account));
            }
            Err(error) => return Err(storage_error(error)),
        };
        let size = file.metadata().map_err(storage_error)?.len();
        if size > MAX_STORE_BYTES {
            return Err(CoreError::Storage {
                message: "composer draft store exceeds its byte limit".into(),
            });
        }
        let mut bytes = Vec::with_capacity(size as usize);
        file.read_to_end(&mut bytes).map_err(storage_error)?;
        let book: DraftFile = serde_json::from_slice(&bytes).map_err(storage_error)?;
        if book.schema != DRAFT_SCHEMA || book.version != DRAFT_VERSION {
            return Err(CoreError::Unsupported {
                message: "composer draft store version is not supported".into(),
            });
        }
        if book.account != self.account {
            return Err(CoreError::Storage {
                message: "composer draft store belongs to another account".into(),
            });
        }
        if book.sessions.len() > MAX_DRAFTS
            || book
                .sessions
                .iter()
                .any(|(id, draft)| id.is_empty() || id.len() > 256 || draft.len() > MAX_DRAFT_BYTES)
        {
            return Err(CoreError::Storage {
                message: "composer draft store contains invalid entries".into(),
            });
        }
        Ok(book)
    }

    fn write_book(&self, book: &DraftFile) -> CoreResult<()> {
        fs::create_dir_all(&self.directory).map_err(storage_error)?;
        let bytes = serde_json::to_vec(book).map_err(storage_error)?;
        if bytes.len() as u64 > MAX_STORE_BYTES {
            return Err(CoreError::Storage {
                message: "composer draft store exceeds its byte limit".into(),
            });
        }
        let path = self.path();
        let temp = temp_path(&path);
        let result = write_atomic(&temp, &path, &bytes);
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
}

fn validate_session_id(session_id: &str) -> CoreResult<()> {
    if session_id.is_empty() || session_id.len() > 256 {
        Err(CoreError::InvalidArgument {
            message: "invalid session identifier".into(),
        })
    } else {
        Ok(())
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn temp_path(path: &Path) -> PathBuf {
    static NEXT_TEMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let serial = NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    path.with_extension(format!("{}.{}.tmp", std::process::id(), serial))
}

fn write_atomic(temp: &Path, path: &Path, bytes: &[u8]) -> CoreResult<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp)
        .map_err(storage_error)?;
    file.write_all(bytes).map_err(storage_error)?;
    file.sync_all().map_err(storage_error)?;
    fs::rename(temp, path).map_err(storage_error)
}

fn storage_error(error: impl std::fmt::Display) -> CoreError {
    CoreError::Storage {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drafts_restore_only_for_the_same_account_and_session() {
        let directory = tempfile::tempdir().unwrap();
        let account_a = ComposerDraftStore::new(directory.path().into(), "account-a");
        account_a.save("chat-a", "saved text").unwrap();

        let account_b = ComposerDraftStore::new(directory.path().into(), "account-b");
        assert_eq!(account_b.restore("chat-a").unwrap(), None);
        assert_eq!(
            account_a.restore("chat-a").unwrap().as_deref(),
            Some("saved text")
        );
        assert_eq!(account_a.restore("chat-b").unwrap(), None);

        let reopened = ComposerDraftStore::new(directory.path().into(), "account-a");
        assert_eq!(
            reopened.restore("chat-a").unwrap().as_deref(),
            Some("saved text")
        );
    }

    #[test]
    fn empty_draft_cancels_saved_text() {
        let directory = tempfile::tempdir().unwrap();
        let store = ComposerDraftStore::new(directory.path().into(), "account-a");
        store.save("chat-a", "discard me").unwrap();
        store.save("chat-a", "").unwrap();
        assert_eq!(store.restore("chat-a").unwrap(), None);
    }

    #[test]
    fn unknown_store_versions_are_rejected_without_replacing_the_file() {
        let directory = tempfile::tempdir().unwrap();
        let store = ComposerDraftStore::new(directory.path().into(), "account-a");
        fs::create_dir_all(&store.directory).unwrap();
        let path = store.path();
        let future = DraftFile {
            schema: DRAFT_SCHEMA.into(),
            version: DRAFT_VERSION + 1,
            account: store.account.clone(),
            sessions: BTreeMap::new(),
        };
        fs::write(&path, serde_json::to_vec(&future).unwrap()).unwrap();

        assert!(matches!(
            store.restore("chat-a"),
            Err(CoreError::Unsupported { .. })
        ));
        assert!(matches!(
            store.save("chat-a", "new"),
            Err(CoreError::Unsupported { .. })
        ));
        let persisted: DraftFile = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(persisted.version, DRAFT_VERSION + 1);
    }

    #[test]
    fn closing_the_account_fences_outstanding_session_handles() {
        let directory = tempfile::tempdir().unwrap();
        let store = ComposerDraftStore::new(directory.path().into(), "account-a");
        store.save("chat-a", "private text").unwrap();
        store.close();
        assert!(matches!(store.restore("chat-a"), Err(CoreError::Closed)));
        assert!(matches!(
            store.save("chat-a", "changed"),
            Err(CoreError::Closed)
        ));
    }
}
