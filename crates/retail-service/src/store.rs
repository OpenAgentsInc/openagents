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
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

pub(crate) struct Store {
    pub ledger: Ledger,
    pub journal: Journal,
    pub db: Connection,
    pub custody: Arc<Custody>,
}

/// Reject symlinks in every existing ancestor and require absolute paths.
/// The deployment must keep these directories inaccessible to other users.
pub fn private_dir(path: &Path) -> Result<()> {
    check_path(path)?;
    if !path.exists() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)?;
    }
    let m = fs::symlink_metadata(path)?;
    if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 {
        return Err(Error::Invalid(
            "existing retail directories must be private and owned",
        ));
    }
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
fn private_regular(m: &fs::Metadata) -> Result<()> {
    if !m.is_file()
        || m.uid() != unsafe { libc::geteuid() }
        || m.nlink() != 1
        || m.mode() & 0o077 != 0
    {
        return Err(Error::Invalid(
            "private retail files must be owned, unshared regular files",
        ));
    }
    Ok(())
}
fn private_file(path: &Path) -> Result<File> {
    check_path(path)?;
    let options = || {
        let mut o = OpenOptions::new();
        o.read(true)
            .write(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        o
    };
    let file = match options().create_new(true).open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            private_regular(&fs::symlink_metadata(path)?)?;
            options().open(path)?
        }
        Err(e) => return Err(e.into()),
    };
    private_regular(&file.metadata()?)?;
    Ok(file)
}
/// Original descriptors fence both the exclusive lock and every durable owner.
pub(crate) struct Custody {
    records: Vec<(PathBuf, File, bool)>,
}
impl Custody {
    pub fn check(&self) -> Result<()> {
        for (path, file, directory) in &self.records {
            check_path(path)?;
            let current = fs::symlink_metadata(path)?;
            let original = file.metadata()?;
            if *directory {
                if !current.is_dir()
                    || current.uid() != unsafe { libc::geteuid() }
                    || current.mode() & 0o077 != 0
                {
                    return Err(Error::Conflict("private retail directory custody changed"));
                }
            } else {
                private_regular(&current)?;
                private_regular(&original)?;
                for suffix in ["-journal", "-wal", "-shm"] {
                    let side = PathBuf::from(format!("{}{suffix}", path.display()));
                    match fs::symlink_metadata(&side) {
                        Ok(m) => private_regular(&m)?,
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                        Err(e) => return Err(e.into()),
                    }
                }
            }
            if (current.dev(), current.ino()) != (original.dev(), original.ino()) {
                return Err(Error::Conflict("private retail state custody changed"));
            }
        }
        Ok(())
    }
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
        let ledger_file = private_file(ledger_path)?;
        let metadata = state.join("transport.sqlite");
        let metadata_file = private_file(&metadata)?;
        let lifecycle = state.join("lifecycle.sqlite");
        let lifecycle_file = private_file(&lifecycle)?;
        let custody = Arc::new(Custody {
            records: vec![
                (state.into(), File::open(state)?, true),
                (
                    state.join("credentials"),
                    File::open(state.join("credentials"))?,
                    true,
                ),
                (state.join("service.lock"), lock, false),
                (ledger_path.into(), ledger_file, false),
                (metadata.clone(), metadata_file, false),
                (lifecycle.clone(), lifecycle_file, false),
            ],
        });
        custody.check()?;
        let db = Connection::open(metadata)?;
        db.busy_timeout(Duration::from_secs(5))?;
        db.execute_batch(
            "PRAGMA secure_delete=ON;
CREATE TABLE IF NOT EXISTS offer (
 id TEXT PRIMARY KEY, account TEXT NOT NULL, principal TEXT NOT NULL,
 generation INTEGER NOT NULL, bytes TEXT NOT NULL, confirmation TEXT,
 ending TEXT, done INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS offer_commercial (id TEXT PRIMARY KEY, bytes TEXT NOT NULL);
CREATE TRIGGER IF NOT EXISTS offer_commercial_no_replace BEFORE INSERT ON offer_commercial WHEN EXISTS(SELECT 1 FROM offer_commercial WHERE id=NEW.id) BEGIN SELECT RAISE(ABORT, 'Commercial offer is immutable'); END;
CREATE TRIGGER IF NOT EXISTS offer_commercial_no_update BEFORE UPDATE ON offer_commercial BEGIN SELECT RAISE(ABORT, 'Commercial offer is immutable'); END;
CREATE TRIGGER IF NOT EXISTS offer_commercial_no_delete BEFORE DELETE ON offer_commercial BEGIN SELECT RAISE(ABORT, 'Commercial offer is retained'); END;
CREATE TABLE IF NOT EXISTS funding_commercial (id TEXT PRIMARY KEY, bytes TEXT NOT NULL);
CREATE TRIGGER IF NOT EXISTS funding_commercial_no_replace BEFORE INSERT ON funding_commercial WHEN EXISTS(SELECT 1 FROM funding_commercial WHERE id=NEW.id) BEGIN SELECT RAISE(ABORT, 'Commercial funding is immutable'); END;
CREATE TRIGGER IF NOT EXISTS funding_commercial_no_update BEFORE UPDATE ON funding_commercial BEGIN SELECT RAISE(ABORT, 'Commercial funding is immutable'); END;
CREATE TRIGGER IF NOT EXISTS funding_commercial_no_delete BEFORE DELETE ON funding_commercial BEGIN SELECT RAISE(ABORT, 'Commercial funding is retained'); END;
CREATE TABLE IF NOT EXISTS worker (id INTEGER PRIMARY KEY CHECK(id=1), cursor TEXT NOT NULL);
INSERT OR IGNORE INTO worker(id,cursor) VALUES(1,'');",
        )?;
        let mut journal = Journal::open(lifecycle)?;
        let fence = Arc::clone(&custody);
        journal.install_custody_check(move || {
            fence
                .check()
                .map_err(|_| retail_cloud::Error::Conflict("private retail custody changed"))
        })?;
        Ok(Self {
            ledger: Ledger::open(ledger_path)?,
            journal,
            db,
            custody,
        })
    }
    pub fn check(&self) -> Result<()> {
        self.custody.check()
    }
    pub fn receiver(&self, node: &str) -> Result<String> {
        self.check()?;
        self.db.execute_batch("CREATE TABLE IF NOT EXISTS receiver (id INTEGER PRIMARY KEY CHECK(id=1), node TEXT NOT NULL);")?;
        self.db.execute(
            "INSERT OR IGNORE INTO receiver(id,node) VALUES(1,?)",
            [node],
        )?;
        let pinned: String =
            self.db
                .query_row("SELECT node FROM receiver WHERE id=1", [], |r| r.get(0))?;
        if pinned != node {
            return Err(Error::Conflict("the retail receiver node changed"));
        }
        Ok(pinned)
    }
    pub fn offer(
        &self,
        id: &str,
    ) -> Result<Option<(RetailOffer, String, i64, Option<Confirmation>)>> {
        self.check()?;
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
    pub fn funding_commercial(
        &self,
        id: &str,
        account: &str,
        amount_msat: i64,
    ) -> Result<Option<receipts::purchase::CommercialRef>> {
        self.check()?;
        let bytes: Option<String> = self
            .db
            .query_row(
                "SELECT bytes FROM funding_commercial WHERE id=?",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        bytes
            .map(|bytes| {
                let (original_account, original_amount, reference): (
                    String,
                    i64,
                    receipts::purchase::CommercialRef,
                ) = serde_json::from_str(&bytes)?;
                reference.validate().map_err(|_| Error::Denied)?;
                if original_account != account
                    || original_amount != amount_msat
                    || !reference.matches_native(
                        receipts::purchase::CommercialProduct::Retail,
                        account,
                        None,
                    )
                {
                    return Err(Error::Denied);
                }
                Ok(reference)
            })
            .transpose()
    }
    /// Freeze before requesting an invoice; an interrupted attempt cannot change attribution.
    pub fn freeze_funding_commercial(
        &self,
        id: &str,
        account: &str,
        amount_msat: i64,
        current: Option<&receipts::purchase::CommercialRef>,
    ) -> Result<Option<receipts::purchase::CommercialRef>> {
        if let Some(original) = self.funding_commercial(id, account, amount_msat)? {
            if Some(&original) != current {
                return Err(Error::Conflict("The original funding attribution changed."));
            }
            return Ok(Some(original));
        }
        let Some(reference) = current else {
            return Ok(None);
        };
        reference.validate().map_err(|_| Error::Denied)?;
        if !reference.matches_native(receipts::purchase::CommercialProduct::Retail, account, None) {
            return Err(Error::Denied);
        }
        let count: usize =
            self.db
                .query_row("SELECT COUNT(*) FROM funding_commercial", [], |r| r.get(0))?;
        if count >= RECORD_MAX {
            return Err(Error::Unavailable("The private funding store is full."));
        }
        self.db.execute(
            "INSERT INTO funding_commercial(id,bytes) VALUES(?,?)",
            params![
                id,
                serde_json::to_string(&(account, amount_msat, reference))?
            ],
        )?;
        self.check()?;
        Ok(Some(reference.clone()))
    }
    pub fn insert_offer(
        &self,
        offer: &RetailOffer,
        principal: &str,
        generation: i64,
        now: i64,
        commercial: Option<&receipts::purchase::CommercialRef>,
    ) -> Result<()> {
        self.check()?;
        let count: usize = self
            .db
            .query_row("SELECT COUNT(*) FROM offer", [], |r| r.get(0))?;
        if count >= RECORD_MAX {
            return Err(Error::Unavailable("the private offer store is full"));
        }
        let transaction = self.db.unchecked_transaction()?;
        transaction.execute("INSERT INTO offer(id,account,principal,generation,bytes,created_at) VALUES(?,?,?,?,?,?)",params![offer.offer.id,offer.admission.account,principal,generation,serde_json::to_string(offer)?,now])?;
        if let Some(value) = commercial {
            value.validate().map_err(|_| Error::Denied)?;
            if !value.matches_native(
                receipts::purchase::CommercialProduct::Retail,
                &offer.admission.account,
                None,
            ) {
                return Err(Error::Denied);
            }
            transaction.execute(
                "INSERT INTO offer_commercial(id,bytes) VALUES(?,?)",
                params![offer.offer.id, serde_json::to_string(value)?],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }
    pub fn commercial(&self, id: &str) -> Result<Option<receipts::purchase::CommercialRef>> {
        self.check()?;
        let bytes: Option<String> = self
            .db
            .query_row("SELECT bytes FROM offer_commercial WHERE id=?", [id], |r| {
                r.get(0)
            })
            .optional()?;
        bytes
            .map(|bytes| {
                let value: receipts::purchase::CommercialRef = serde_json::from_str(&bytes)?;
                value.validate().map_err(|_| Error::Denied)?;
                Ok(value)
            })
            .transpose()
    }
    pub fn confirmation(&self, id: &str, value: &Confirmation) -> Result<()> {
        self.check()?;
        self.db.execute(
            "UPDATE offer SET confirmation=? WHERE id=? AND confirmation IS NULL",
            params![serde_json::to_string(value)?, id],
        )?;
        Ok(())
    }
    pub fn ending(&self, id: &str) -> Result<Option<Ending>> {
        self.check()?;
        let value: Option<String> =
            self.db
                .query_row("SELECT ending FROM offer WHERE id=?", [id], |r| r.get(0))?;
        value
            .map(|v| serde_json::from_str(&v).map_err(Error::from))
            .transpose()
    }
    pub fn set_ending(&self, id: &str, ending: Ending) -> Result<()> {
        self.check()?;
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
    private_regular(&meta)?;
    if meta.len() > crate::types::KEY_MAX as u64 || meta.permissions().mode() & 0o077 != 0 {
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

#[cfg(test)]
pub(crate) fn private_file_fixture(path: &Path) {
    private_file(path).unwrap();
}
