//! The server's sealed store of each account's connections and project
//! sources (#11238).
//!
//! One file per account, named by a digest of the account's chat owner
//! value ([`crate::chat_store::account_owner`]), in a private directory
//! beside the own-Claude custody directory. The whole record is sealed
//! with AES-256-GCM ([`oa_seal`]) under the same kind of keyring, kept
//! outside the directory, with the account bound as associated data, so a
//! file copied to another account's name doesn't open. It holds the Google
//! refresh token: nothing outside this module and the Google calls in
//! [`crate::connections`] reads it, and no page, API answer, tool result,
//! or log carries it.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use oa_connections::core::{Connection, Scope, SecretRef};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::custody::{self, CustodyError};

const SCHEMA: &str = "openagents.web.connections.v1";
/// The largest sealed file read.
const FILE_MAX: u64 = 30 * 1024;
/// The largest record kept, before sealing.
const RECORD_MAX: usize = 18 * 1024;
/// The most sources one project keeps.
pub const MAX_PROJECT_SOURCES: usize = 20;
/// The most sources one account keeps.
pub const MAX_SOURCES: usize = 50;
/// The longest source name kept.
const NAME_MAX: usize = 120;

/// One connection with its secret. The refresh token never leaves the
/// server.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Stored {
    pub integration: String,
    pub name: String,
    #[serde(default)]
    pub identity: Option<String>,
    #[serde(default)]
    pub granted_scopes: Vec<String>,
    pub refresh_token: String,
    pub connected_at: u64,
    /// Google stopped accepting the refresh token: connect again.
    #[serde(default)]
    pub reconnect: bool,
}

impl std::fmt::Debug for Stored {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stored")
            .field("integration", &self.integration)
            .field("name", &self.name)
            .field("identity", &self.identity)
            .field("granted_scopes", &self.granted_scopes)
            .finish_non_exhaustive()
    }
}

impl Stored {
    /// The connection as the core sees it: its secret only a pointer.
    #[must_use]
    pub fn connection(&self, owner: &str) -> Connection {
        Connection {
            integration: self.integration.clone(),
            name: self.name.clone(),
            scope: Scope::account(owner),
            identity: self.identity.clone(),
            granted_scopes: self.granted_scopes.clone(),
            secret: SecretRef::Custody {
                entry: owner.to_owned(),
            },
        }
    }
}

/// A Drive folder or file attached to a project.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Source {
    /// The project's id (`prj_...`).
    pub project: String,
    pub integration: String,
    pub id: String,
    pub name: String,
    /// `folder`, `document`, `spreadsheet`, `pdf`, or `file`.
    pub kind: String,
    pub added_at: u64,
}

impl Source {
    /// Where it opens.
    #[must_use]
    pub fn link(&self) -> String {
        if self.kind == "folder" {
            format!("https://drive.google.com/drive/folders/{}", self.id)
        } else {
            format!("https://drive.google.com/file/d/{}/view", self.id)
        }
    }
}

/// Everything one account keeps here.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Account {
    #[serde(default)]
    pub connections: Vec<Stored>,
    #[serde(default)]
    pub sources: Vec<Source>,
}

impl Account {
    /// The connection to `integration` named `name`.
    #[must_use]
    pub fn connection(&self, integration: &str, name: &str) -> Option<&Stored> {
        self.connections
            .iter()
            .find(|c| c.integration == integration && c.name == name)
    }

    /// The sources attached to `project`, oldest first.
    #[must_use]
    pub fn sources_of(&self, project: &str) -> Vec<&Source> {
        self.sources
            .iter()
            .filter(|s| s.project == project)
            .collect()
    }

    /// Attach `source` (again, it moves nothing). Refuses past the limits.
    ///
    /// # Errors
    ///
    /// A sentence for the person.
    pub fn attach(&mut self, mut source: Source) -> Result<(), &'static str> {
        source.name = source.name.chars().take(NAME_MAX).collect();
        if self
            .sources
            .iter()
            .any(|s| s.project == source.project && s.id == source.id)
        {
            return Ok(());
        }
        if self.sources_of(&source.project).len() >= MAX_PROJECT_SOURCES
            || self.sources.len() >= MAX_SOURCES
        {
            return Err("This project has as many sources as it can keep. Remove one first.");
        }
        self.sources.push(source);
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    schema: String,
    sealed: oa_seal::Sealed,
}

/// The store: a private directory and the keyring that seals it.
pub struct Store {
    root: PathBuf,
    pin: (u64, u64),
    keyring: oa_seal::Keyring,
    /// One change at a time on this server.
    writing: Mutex<()>,
}

impl Store {
    /// Open `root` (created owner-only when missing), sealing under
    /// `keyring`, which must be kept elsewhere.
    ///
    /// # Errors
    ///
    /// A sentence when the directory is shared, a link, or unreadable.
    pub fn open(root: &Path, keyring: oa_seal::Keyring) -> Result<Self, String> {
        if !root.exists() {
            use std::os::unix::fs::DirBuilderExt;
            fs::DirBuilder::new()
                .mode(0o700)
                .create(root)
                .map_err(|_| "The connections directory can't be made.")?;
        }
        custody::checked_root(root).map_err(|_| "The connections directory is unavailable.")?;
        if keyring
            .source()
            .is_some_and(|source| source.starts_with(root))
        {
            return Err("The connections keyring must be kept outside its directory.".into());
        }
        let metadata =
            fs::metadata(root).map_err(|_| "The connections directory is unavailable.")?;
        Ok(Self {
            root: root.into(),
            pin: (metadata.dev(), metadata.ino()),
            keyring,
            writing: Mutex::new(()),
        })
    }

    fn check(&self) -> Result<(), CustodyError> {
        custody::checked_root(&self.root)?;
        let metadata = fs::metadata(&self.root).map_err(|_| CustodyError::Unavailable)?;
        if (metadata.dev(), metadata.ino()) != self.pin {
            return Err(CustodyError::Unavailable);
        }
        Ok(())
    }

    fn name(owner: &str) -> String {
        let mut hash = Sha256::new();
        hash.update(SCHEMA.as_bytes());
        hash.update([0]);
        hash.update(owner.as_bytes());
        format!("{:x}.json", hash.finalize())
    }

    fn bound(owner: &str) -> Vec<u8> {
        let mut out = SCHEMA.as_bytes().to_vec();
        out.push(0);
        out.extend_from_slice(owner.as_bytes());
        out
    }

    /// The account's record (empty when it has none).
    ///
    /// # Errors
    ///
    /// [`CustodyError::Unavailable`] when the file doesn't open.
    pub fn load(&self, owner: &str) -> Result<Account, CustodyError> {
        self.check()?;
        let Some(mut bytes) = custody::read_private(&self.root.join(Self::name(owner)), FILE_MAX)?
        else {
            return Ok(Account::default());
        };
        let file = serde_json::from_slice::<File>(&bytes);
        bytes.fill(0);
        let file = file.map_err(|_| CustodyError::Unavailable)?;
        if file.schema != SCHEMA {
            return Err(CustodyError::Unavailable);
        }
        let plain = self
            .keyring
            .open(&Self::bound(owner), &file.sealed)
            .map_err(|_| CustodyError::Unavailable)?;
        serde_json::from_slice(&plain).map_err(|_| CustodyError::Unavailable)
    }

    fn save(&self, owner: &str, account: &Account) -> Result<(), CustodyError> {
        let path = self.root.join(Self::name(owner));
        if account.connections.is_empty() && account.sources.is_empty() {
            if fs::symlink_metadata(&path).is_ok() {
                custody::erase(&path)?;
            }
            return Ok(());
        }
        let mut plain = serde_json::to_vec(account).map_err(|_| CustodyError::Unavailable)?;
        if plain.len() > RECORD_MAX {
            plain.fill(0);
            return Err(CustodyError::Invalid);
        }
        let sealed = self.keyring.seal(&Self::bound(owner), &plain);
        plain.fill(0);
        let sealed = sealed.map_err(|_| CustodyError::Unavailable)?;
        let bytes = serde_json::to_vec(&File {
            schema: SCHEMA.into(),
            sealed,
        })
        .map_err(|_| CustodyError::Unavailable)?;
        self.check()?;
        custody::write_private(&self.root, &Self::name(owner), &bytes)
    }

    /// Change the account's record with `change` and keep it; returns what
    /// `change` returned.
    ///
    /// # Errors
    ///
    /// [`CustodyError`] when it can't be read or kept
    /// ([`CustodyError::Invalid`]: too large).
    pub fn update<T>(
        &self,
        owner: &str,
        change: impl FnOnce(&mut Account) -> T,
    ) -> Result<T, CustodyError> {
        let _writing = self.writing.lock().map_err(|_| CustodyError::Unavailable)?;
        let mut account = self.load(owner)?;
        let out = change(&mut account);
        self.save(owner, &account)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap().join("connections");
        let store = Store::open(&root, oa_seal::Keyring::scratch("test").unwrap().0).unwrap();
        (dir, store)
    }

    fn stored() -> Stored {
        Stored {
            integration: "google".into(),
            name: "default".into(),
            identity: Some("a@b.c".into()),
            granted_scopes: vec![oa_connections::google::DRIVE_READONLY.into()],
            refresh_token: "1//secret-refresh".into(),
            connected_at: 1,
            reconnect: false,
        }
    }

    #[test]
    fn the_record_is_sealed_per_account_and_removing_everything_erases_it() {
        let (_dir, store) = store();
        let mode = fs::metadata(&store.root).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        store
            .update("acct-a", |account| account.connections.push(stored()))
            .unwrap();
        let file = store.root.join(Store::name("acct-a"));
        let on_disk = fs::read_to_string(&file).unwrap();
        assert!(!on_disk.contains("secret-refresh"));
        assert!(!on_disk.contains("a@b.c"));
        assert_eq!(
            store
                .load("acct-a")
                .unwrap()
                .connection("google", "default"),
            Some(&stored())
        );
        assert!(!format!("{:?}", stored()).contains("secret-refresh"));
        // Another account's name finds nothing, and a copied file doesn't open.
        assert_eq!(store.load("acct-b").unwrap(), Account::default());
        fs::copy(&file, store.root.join(Store::name("acct-b"))).unwrap();
        assert_eq!(store.load("acct-b"), Err(CustodyError::Unavailable));
        store
            .update("acct-a", |account| account.connections.clear())
            .unwrap();
        assert!(!file.exists());
    }

    #[test]
    fn sources_are_kept_once_and_capped_per_project() {
        let mut account = Account::default();
        let source = |id: usize| Source {
            project: "prj_1".into(),
            integration: "google".into(),
            id: format!("folder-{id:012}"),
            name: "x".repeat(500),
            kind: "folder".into(),
            added_at: 1,
        };
        account.attach(source(0)).unwrap();
        account.attach(source(0)).unwrap();
        assert_eq!(account.sources.len(), 1);
        assert_eq!(account.sources[0].name.chars().count(), NAME_MAX);
        for id in 1..MAX_PROJECT_SOURCES {
            account.attach(source(id)).unwrap();
        }
        assert!(account.attach(source(99)).is_err());
        let (_dir, store) = store();
        store.update("acct", |a| *a = account.clone()).unwrap();
        assert_eq!(
            store.load("acct").unwrap().sources_of("prj_1").len(),
            MAX_PROJECT_SOURCES
        );
    }
}
