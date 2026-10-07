//! Private transport records beside the existing lifecycle and money ledgers.

use crate::types::{Confirmation, RECORD_MAX};
use crate::{Error, Result};
use fs2::FileExt;
use pay_ledger::Ledger;
use retail_cloud::{journal::Journal, offer::RetailOffer};
use route_contract::price_book::Ending;
use rusqlite::{Connection, OptionalExtension, params};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path};
use std::time::Duration;

pub(crate) struct Store {
    pub ledger: Ledger,
    pub journal: Journal,
    pub db: Connection,
    pub _lock: File,
}

/// Reject symlinks in every existing ancestor and require absolute paths.
/// The deployment must keep these directories inaccessible to other users.
pub fn private_dir(path: &Path) -> Result<()> {
    check_path(path)?;
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}
pub(crate) fn check_path(path: &Path) -> Result<()> {
    if !path.is_absolute() || path.components().any(|p| matches!(p, Component::ParentDir)) {
        return Err(Error::Invalid(
            "an absolute path without parent traversal is required",
        ));
    }
    let mut prefix = std::path::PathBuf::new();
    for part in path.components() {
        prefix.push(part);
        match fs::symlink_metadata(&prefix) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(Error::Invalid(
                    "symlinks are not admitted for private state",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
fn private_file(path: &Path) -> Result<File> {
    check_path(path)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(Error::Invalid("private state must be a regular file"));
    }
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    Ok(file)
}
impl Store {
    pub fn open(state: &Path, ledger_path: &Path) -> Result<Self> {
        private_dir(state)?;
        private_dir(&state.join("credentials"))?;
        let lock = private_file(&state.join("service.lock"))?;
        lock.try_lock_exclusive()
            .map_err(|_| Error::Conflict("the retail worker already owns this state"))?;
        // Do not create an account or a credential while opening the service.
        // Existing compute principals remain the authentication authority.
        private_file(ledger_path)?;
        let metadata = state.join("transport.sqlite");
        private_file(&metadata)?;
        let db = Connection::open(metadata)?;
        db.busy_timeout(Duration::from_secs(5))?;
        db.execute_batch(
            "PRAGMA secure_delete=ON;
CREATE TABLE IF NOT EXISTS offer (
 id TEXT PRIMARY KEY, account TEXT NOT NULL, principal TEXT NOT NULL,
 generation INTEGER NOT NULL, bytes TEXT NOT NULL, confirmation TEXT,
 ending TEXT, done INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS worker (id INTEGER PRIMARY KEY CHECK(id=1), cursor TEXT NOT NULL);
INSERT OR IGNORE INTO worker(id,cursor) VALUES(1,'');",
        )?;
        Ok(Self {
            ledger: Ledger::open(ledger_path)?,
            journal: Journal::open(state.join("lifecycle.sqlite"))?,
            db,
            _lock: lock,
        })
    }
    pub fn offer(
        &self,
        id: &str,
    ) -> Result<Option<(RetailOffer, String, i64, Option<Confirmation>)>> {
        let row = self
            .db
            .query_row(
                "SELECT bytes,principal,generation,confirmation FROM offer WHERE id=?",
                [id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(bytes, principal, generation, confirmation)| {
            Ok((
                serde_json::from_str(&bytes)?,
                principal,
                generation,
                confirmation.map(|c| serde_json::from_str(&c)).transpose()?,
            ))
        })
        .transpose()
    }
    pub fn insert_offer(
        &self,
        offer: &RetailOffer,
        principal: &str,
        generation: i64,
        now: i64,
    ) -> Result<()> {
        let count: usize = self
            .db
            .query_row("SELECT COUNT(*) FROM offer", [], |r| r.get(0))?;
        if count >= RECORD_MAX {
            return Err(Error::Unavailable("the private offer store is full"));
        }
        self.db.execute("INSERT INTO offer(id,account,principal,generation,bytes,created_at) VALUES(?,?,?,?,?,?)",params![offer.offer.id,offer.admission.account,principal,generation,serde_json::to_string(offer)?,now])?;
        Ok(())
    }
    pub fn confirmation(&self, id: &str, value: &Confirmation) -> Result<()> {
        self.db.execute(
            "UPDATE offer SET confirmation=? WHERE id=? AND confirmation IS NULL",
            params![serde_json::to_string(value)?, id],
        )?;
        Ok(())
    }
    pub fn ending(&self, id: &str) -> Result<Option<Ending>> {
        let value: Option<String> =
            self.db
                .query_row("SELECT ending FROM offer WHERE id=?", [id], |r| r.get(0))?;
        value
            .map(|v| serde_json::from_str(&v).map_err(Error::from))
            .transpose()
    }
    pub fn set_ending(&self, id: &str, ending: Ending) -> Result<()> {
        self.db.execute(
            "UPDATE offer SET ending=? WHERE id=? AND ending IS NULL",
            params![serde_json::to_string(&ending)?, id],
        )?;
        Ok(())
    }
}

/// Create and sync the private vault file before persisting confirmation.
/// Retrying may only supply the identical credential, never rotate it.
pub(crate) fn vault_write(state: &Path, id: &str, key: &str) -> Result<()> {
    let path = state.join("credentials").join(id);
    check_path(&path)?;
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)
    {
        Ok(mut file) => {
            file.write_all(key.as_bytes())?;
            file.sync_all()?;
            File::open(path.parent().ok_or(Error::Invalid("credential parent"))?)?.sync_all()?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let old =
                vault_read(state, id)?.ok_or(Error::Conflict("the credential file disappeared"))?;
            if old.as_str() != key {
                return Err(Error::Conflict("the confirmed credential cannot change"));
            }
        }
        Err(e) => return Err(e.into()),
    }
    Ok(())
}
/// Transient vault bytes are overwritten when their owner drops.
pub(crate) struct Secret(String);
impl Secret {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn take(&mut self) -> String {
        std::mem::take(&mut self.0)
    }
}
impl Drop for Secret {
    fn drop(&mut self) {
        let mut bytes = std::mem::take(&mut self.0).into_bytes();
        bytes.fill(0);
    }
}
pub(crate) fn vault_read(state: &Path, id: &str) -> Result<Option<Secret>> {
    let path = state.join("credentials").join(id);
    check_path(&path)?;
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let meta = file.metadata()?;
    if !meta.is_file()
        || meta.len() > crate::types::KEY_MAX as u64
        || meta.permissions().mode() & 0o077 != 0
    {
        return Err(Error::Invalid(
            "the credential file is not private or bounded",
        ));
    }
    let mut key = String::new();
    file.read_to_string(&mut key)?;
    Ok(Some(Secret(key)))
}
pub(crate) fn vault_remove(state: &Path, id: &str) -> Result<()> {
    let path = state.join("credentials").join(id);
    check_path(&path)?;
    match fs::remove_file(&path) {
        Ok(()) => {
            File::open(path.parent().ok_or(Error::Invalid("credential parent"))?)?.sync_all()?;
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
