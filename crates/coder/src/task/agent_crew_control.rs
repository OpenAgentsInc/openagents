//! Durable cohort admission and pending-subject revocation over native agents.
//! Individual records and controllers remain the lifecycle authority. This
//! barrier grants no sending, disclosure, payment, or approval capability.

use super::agent::{Record, State, Store};
use coder_host::access::crew::{Control, ControlAction, Selection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const SCHEMA: &str = "openagents.crew-control.v1";
const MAX_BYTES: usize = 192 * 1024;
const MAX_HISTORY: usize = 128;

/// Native adapter for the external host outbox. Revoke local pending subjects
/// synchronously and retry safely by epoch; never deliver or contact a recipient
/// here. Already admitted delivery keeps its original uncertain or known result.
pub trait Revoker: Send + Sync {
    /// An enabled adapter syncs its own epoch-bound revocation before returning.
    fn revoke(&self, member: &str, epoch: u64) -> Result<Revocation, String>;
}
/// Bounded cleanup metadata without recipient, message, credential, or lead data.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revocation {
    pub enabled: bool,
    pub pending_revoked: u32,
    pub admitted: u32,
    pub unknown: u32,
    pub receipt_sha256: Option<String>,
}
impl Revocation {
    pub fn validate(&self) -> Result<(), String> {
        if !self.enabled
            && (self.pending_revoked != 0
                || self.admitted != 0
                || self.unknown != 0
                || self.receipt_sha256.is_some())
            || self.enabled
                && self.receipt_sha256.as_ref().is_none_or(|digest| {
                    digest.len() != 64
                        || !digest
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
        {
            return Err("The outbox returned invalid bounded revocation metadata.".into());
        }
        Ok(())
    }
}
#[derive(Default)]
pub struct NoOutbox;
impl Revoker for NoOutbox {
    fn revoke(&self, _: &str, _: u64) -> Result<Revocation, String> {
        Ok(Revocation {
            enabled: false,
            pending_revoked: 0,
            admitted: 0,
            unknown: 0,
            receipt_sha256: None,
        })
    }
}
pub type DispatchRevoker = Arc<dyn Revoker>;

/// A revocation reference, not approval or a send permission. The outbox must
/// separately bind and validate the owner's exact subject before checkpointing.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stamp {
    pub member: String,
    pub pubkey: String,
    pub epoch: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cohort {
    pub selection: Selection,
    pub action: ControlAction,
    pub epoch: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub request: String,
    pub fingerprint: String,
    pub owner: String,
    pub control: Control,
    pub epoch: u64,
    pub at: u64,
    pub state: String,
    pub selected: Vec<String>,
    pub members: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    pub schema: String,
    pub epoch: u64,
    pub cohorts: BTreeMap<String, Cohort>,
    pub members: BTreeMap<String, u64>,
    pub all_sales_epoch: u64,
    pub history: BTreeMap<String, Receipt>,
    pub digest: String,
}
impl Default for Book {
    fn default() -> Self {
        let mut book = Self {
            schema: SCHEMA.into(),
            epoch: 0,
            cohorts: BTreeMap::new(),
            members: BTreeMap::new(),
            all_sales_epoch: 0,
            history: BTreeMap::new(),
            digest: String::new(),
        };
        book.seal().expect("an initial cohort book serializes");
        book
    }
}
impl Book {
    fn computed(&self) -> Result<String, String> {
        let mut value = serde_json::to_value(self).map_err(|e| e.to_string())?;
        value.as_object_mut().unwrap().remove("digest");
        nostr::contracts::digest_value(&value).map_err(|e| e.to_string())
    }
    fn seal(&mut self) -> Result<(), String> {
        self.digest = self.computed()?;
        Ok(())
    }
    fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA
            || self.digest != self.computed()?
            || self.cohorts.len() > 64
            || self.members.len() > 1024
            || self.history.len() > MAX_HISTORY
            || self.all_sales_epoch > self.epoch
        {
            return Err("Crew control state is invalid or exceeds its bound.".into());
        }
        for (name, cohort) in &self.cohorts {
            coder_host::access::agent::name(name).map_err(|e| e.to_string())?;
            Control {
                cohort: name.clone(),
                selection: cohort.selection.clone(),
                action: ControlAction::Stop,
                expected: None,
                reason: "Validate the retained selection.".into(),
            }
            .validate()
            .map_err(|e| e.to_string())?;
            if cohort.epoch == 0 || cohort.epoch > self.epoch {
                return Err("Invalid cohort revision.".into());
            }
        }
        for (name, epoch) in &self.members {
            coder_host::access::agent::name(name).map_err(|e| e.to_string())?;
            if *epoch == 0 || *epoch > self.epoch {
                return Err("Invalid member revocation epoch.".into());
            }
        }
        for (key, receipt) in &self.history {
            receipt.control.validate().map_err(|e| e.to_string())?;
            if *key != nostr::contracts::digest_bytes(receipt.request.as_bytes())
                || receipt.request.len() > 128
                || receipt.owner.is_empty()
                || receipt.owner.len() > 128
                || receipt.epoch == 0
                || receipt.epoch > self.epoch
                || !matches!(receipt.state.as_str(), "applying" | "complete" | "partial")
                || receipt.members.len() > 32
                || receipt.selected.is_empty()
                || receipt.selected.len() > 32
                || receipt
                    .selected
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != receipt.selected.len()
                || receipt
                    .selected
                    .iter()
                    .any(|name| coder_host::access::agent::name(name).is_err())
                || receipt
                    .members
                    .keys()
                    .any(|name| !receipt.selected.contains(name))
                || receipt.fingerprint
                    != nostr::contracts::digest_value(
                        &serde_json::to_value(&receipt.control).map_err(|e| e.to_string())?,
                    )
                    .map_err(|e| e.to_string())?
            {
                return Err("Invalid retained crew control receipt.".into());
            }
        }
        Ok(())
    }
    fn matches(selection: &Selection, record: &Record) -> bool {
        record.job_role.is_some()
            && match selection {
                Selection::AllSales => true,
                Selection::Members(names) => names.contains(&record.name),
            }
    }
    pub fn blocked(&self, record: &Record) -> bool {
        self.cohorts.values().any(|cohort| {
            cohort.action != ControlAction::Resume && Self::matches(&cohort.selection, record)
        }) || self.history.values().any(|receipt| {
            receipt.state == "applying" && Self::matches(&receipt.control.selection, record)
        })
    }
    pub fn stamp(&self, record: &Record) -> Result<Option<Stamp>, String> {
        if record.job_role.is_none() {
            return Ok(None);
        }
        if record.state != State::Active || self.blocked(record) {
            return Err(
                "The owner's crew control blocks new work and pending dispatch subjects.".into(),
            );
        }
        Ok(Some(Stamp {
            member: record.name.clone(),
            pubkey: record
                .pubkey
                .clone()
                .ok_or("The sales member has no native key.")?,
            epoch: self
                .members
                .get(&record.name)
                .copied()
                .unwrap_or(0)
                .max(self.all_sales_epoch),
        }))
    }
    pub fn check_stamp(&self, record: &Record, stamp: &Option<Stamp>) -> Result<(), String> {
        if self.stamp(record)? != *stamp {
            return Err(
                "Crew admission or recipient identity changed; an old subject cannot resume."
                    .into(),
            );
        }
        Ok(())
    }
    pub(crate) fn revoke_member(&mut self, member: &str) -> Result<u64, String> {
        coder_host::access::agent::name(member).map_err(|e| e.to_string())?;
        self.epoch = self
            .epoch
            .checked_add(1)
            .ok_or("Crew control epoch overflow.")?;
        self.members.insert(member.into(), self.epoch);
        Ok(self.epoch)
    }

    pub fn begin(
        &mut self,
        request: &str,
        owner: &str,
        control: Control,
        names: &[String],
        now: u64,
    ) -> Result<Receipt, String> {
        control.validate().map_err(|e| e.to_string())?;
        if request.is_empty()
            || request.len() > 128
            || owner.is_empty()
            || owner.len() > 128
            || names.is_empty()
            || names.len() > 32
            || names
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != names.len()
            || names
                .iter()
                .any(|name| coder_host::access::agent::name(name).is_err())
        {
            return Err("Select 1 to 32 current native sales members.".into());
        }
        let key = nostr::contracts::digest_bytes(request.as_bytes());
        let fingerprint = nostr::contracts::digest_value(
            &serde_json::to_value(&control).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if let Some(old) = self.history.get(&key) {
            if old.fingerprint != fingerprint || old.owner != owner {
                return Err("Crew control retry differs from its original owner request.".into());
            }
            return Ok(old.clone());
        }
        if self.history.values().any(|r| r.state == "applying") {
            return Err("A crew control has unresolved interrupted cleanup; retry that exact owner request.".into());
        }
        if control
            .expected
            .as_ref()
            .is_some_and(|digest| digest != &self.digest)
        {
            return Err("Crew control changed; inspect the current digest before resuming.".into());
        }
        if let Some(old) = self.cohorts.get(&control.cohort) {
            if old.selection != control.selection {
                return Err(
                    "A named cohort's member selection is immutable; choose a new cohort.".into(),
                );
            }
        }
        if control.action == ControlAction::Resume && !self.cohorts.contains_key(&control.cohort) {
            return Err("Unknown crew cohort; resume cannot create one.".into());
        }
        if self.history.len() >= MAX_HISTORY {
            return Err("Crew control history is full; no old revocation is discarded.".into());
        }
        self.epoch = self
            .epoch
            .checked_add(1)
            .ok_or("Crew control epoch overflow.")?;
        for name in names {
            self.members.insert(name.clone(), self.epoch);
        }
        if control.selection == Selection::AllSales {
            self.all_sales_epoch = self.epoch;
        }
        self.cohorts.insert(
            control.cohort.clone(),
            Cohort {
                selection: control.selection.clone(),
                action: control.action,
                epoch: self.epoch,
            },
        );
        let receipt = Receipt {
            request: request.into(),
            fingerprint,
            owner: owner.into(),
            control,
            epoch: self.epoch,
            at: now,
            state: "applying".into(),
            selected: names.to_vec(),
            members: BTreeMap::new(),
        };
        self.history.insert(key, receipt.clone());
        Ok(receipt)
    }
    pub fn finish(
        &mut self,
        request: &str,
        members: BTreeMap<String, Value>,
    ) -> Result<Receipt, String> {
        let key = nostr::contracts::digest_bytes(request.as_bytes());
        let receipt = self
            .history
            .get_mut(&key)
            .ok_or("Unknown crew control request.")?;
        receipt.state = if members
            .values()
            .any(|m| m["state"] == "partial" || m["state"] == "unknown")
        {
            "partial"
        } else {
            "complete"
        }
        .into();
        receipt.members = members;
        Ok(receipt.clone())
    }
    /// Only a resume transaction may inspect its prospective admission. It
    /// remains applying for every concurrent reader until completion is sealed.
    pub(crate) fn remains_blocked_after(&self, record: &Record, request: &str) -> bool {
        self.cohorts
            .values()
            .any(|c| c.action != ControlAction::Resume && Self::matches(&c.selection, record))
            || self.history.values().any(|r| {
                r.request != request
                    && r.state == "applying"
                    && Self::matches(&r.control.selection, record)
            })
    }
}

/// Exclusive descriptor-bound custody while reading and changing native gates.
/// The stable OS lock is released by a crash; its path is never unlinked.
pub struct Guard {
    dir: PathBuf,
    root: PathBuf,
    root_directory: File,
    directory: File,
    lock: File,
    state: Option<File>,
    original: String,
    pub book: Book,
}
impl Guard {
    /// Capture a pending subject's generation from the current native member.
    /// This grants no approval or delivery authority.
    pub fn pending_stamp(&self, member: &str) -> Result<Stamp, String> {
        self.check()?;
        let store = Store::new(&self.root, member)?;
        let record = store.load()?.ok_or("The native sales member is absent.")?;
        store.custody(&record)?;
        self.book
            .stamp(&record)?
            .ok_or_else(|| "Select a native sales member.".into())
    }
    pub fn open(root: &Path) -> Result<Self, String> {
        let dir = root.join("crew-control");
        create_private_directory(&dir)?;
        let root_directory = open_directory(root)?;
        let directory = open_directory(&dir)?;
        let lock_path = dir.join("writer.lock");
        let lock = private_open(&lock_path, true)?;
        let mut acquired = false;
        for _ in 0..200 {
            if lock.try_lock().is_ok() {
                acquired = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if !acquired {
            return Err("Crew admission is busy with another native control.".into());
        }
        let (book, state) = read_book(&dir)?;
        let guard = Self {
            dir,
            root: root.to_owned(),
            root_directory,
            directory,
            lock,
            state,
            original: book.digest.clone(),
            book,
        };
        guard.check()?;
        Ok(guard)
    }
    pub fn check(&self) -> Result<(), String> {
        same_inode(&self.root_directory, &self.root)?;
        same_inode(&self.directory, &self.dir)?;
        same_inode(&self.lock, &self.dir.join("writer.lock"))?;
        let (book, state) = read_book(&self.dir)?;
        if book.digest != self.original {
            return Err("Crew control changed outside its native writer.".into());
        }
        match (&self.state, state) {
            (Some(held), Some(_)) => same_inode(held, &self.dir.join("state.json"))?,
            (None, None) => {}
            _ => return Err("Crew control state custody changed.".into()),
        }
        Ok(())
    }
    pub fn save(&mut self) -> Result<(), String> {
        self.check()?;
        self.book.seal()?;
        self.book.validate()?;
        let bytes = serde_json::to_vec_pretty(&self.book).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES {
            return Err("Crew control checkpoint exceeds 192 KiB.".into());
        }
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let temp = self
            .dir
            .join(format!(".state-{}-{suffix}", std::process::id()));
        let mut file = private_new(&temp)?;
        let result = (|| {
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(|e| e.to_string())?;
            self.check()?;
            same_inode(&file, &temp)?;
            std::fs::rename(&temp, self.dir.join("state.json")).map_err(|e| e.to_string())?;
            self.directory.sync_all().map_err(|e| e.to_string())?;
            let (written, state) = read_book(&self.dir)?;
            if written.digest != self.book.digest {
                return Err("Written crew checkpoint changed.".into());
            }
            self.original = written.digest;
            self.state = state;
            self.check()
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(temp);
        }
        result
    }
    /// Run only the external outbox's durable admission checkpoint under the
    /// epoch fence. Network delivery happens after this returns. This adds no
    /// approval: the outbox must verify its own exact owner-approved subject.
    pub fn checkpoint_pending<T>(
        &self,
        stamp: &Stamp,
        checkpoint: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        self.check()?;
        let store = Store::new(&self.root, &stamp.member)?;
        let record = store.load()?.ok_or("The native sales member is absent.")?;
        store.custody(&record)?;
        self.book.check_stamp(&record, &Some(stamp.clone()))?;
        let value = checkpoint()?;
        self.check()?;
        let current = store
            .load()?
            .ok_or("The native sales member disappeared.")?;
        store.custody(&current)?;
        self.book.check_stamp(&current, &Some(stamp.clone()))?;
        Ok(value)
    }
}
fn read_book(dir: &Path) -> Result<(Book, Option<File>), String> {
    let path = dir.join("state.json");
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((Book::default(), None));
        }
        Err(error) => return Err(error.to_string()),
        Ok(_) => {}
    }
    let mut file = private_open(&path, false)?;
    if file.metadata().map_err(|e| e.to_string())?.len() > MAX_BYTES as u64 {
        return Err("Crew state exceeds 192 KiB.".into());
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("Crew state exceeds 192 KiB.".into());
    }
    let book: Book = nostr::contracts::parse_strict(&bytes)
        .map_err(|e| e.to_string())
        .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))?;
    book.validate()?;
    Ok((book, Some(file)))
}
#[cfg(unix)]
fn create_private_directory(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    if !path.try_exists().map_err(|e| e.to_string())? {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)
            .map_err(|e| e.to_string())?;
    }
    let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    // SAFETY: geteuid has no pointer arguments.
    if !meta.is_dir()
        || meta.file_type().is_symlink()
        || meta.mode() & 0o077 != 0
        || meta.uid() != unsafe { libc::geteuid() }
    {
        return Err("Crew control needs a private user-owned directory without a symlink.".into());
    }
    Ok(())
}
#[cfg(unix)]
fn open_directory(path: &Path) -> Result<File, String> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| e.to_string())
}
#[cfg(unix)]
fn private_open(path: &Path, create: bool) -> Result<File, String> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let file = OpenOptions::new()
        .read(true)
        .write(create)
        .create(create)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| e.to_string())?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    // SAFETY: geteuid has no pointer arguments.
    if !meta.is_file()
        || meta.nlink() != 1
        || meta.mode() & 0o077 != 0
        || meta.uid() != unsafe { libc::geteuid() }
    {
        return Err("Crew state and lock files must be private, user-owned regular files without hard links.".into());
    }
    Ok(file)
}
#[cfg(unix)]
fn private_new(path: &Path) -> Result<File, String> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| e.to_string())
}
#[cfg(unix)]
fn same_inode(file: &File, path: &Path) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    let held = file.metadata().map_err(|e| e.to_string())?;
    let current = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if current.file_type().is_symlink()
        || held.dev() != current.dev()
        || held.ino() != current.ino()
        || held.mode() != current.mode()
        || held.uid() != current.uid()
        || current.mode() & 0o077 != 0
        || current.uid() != unsafe { libc::geteuid() }
        || held.is_file() && (held.nlink() != 1 || current.nlink() != 1)
    {
        return Err("Crew controller custody changed; the old authority refuses effects.".into());
    }
    Ok(())
}
#[cfg(not(unix))]
fn create_private_directory(_: &Path) -> Result<(), String> {
    Err("Native crew controls require the selected Unix host custody adapter.".into())
}
#[cfg(not(unix))]
fn open_directory(_: &Path) -> Result<File, String> {
    Err("Native crew controls require the selected Unix host custody adapter.".into())
}
#[cfg(not(unix))]
fn private_open(_: &Path, _: bool) -> Result<File, String> {
    Err("Native crew controls require the selected Unix host custody adapter.".into())
}
#[cfg(not(unix))]
fn private_new(_: &Path) -> Result<File, String> {
    Err("Native crew controls require the selected Unix host custody adapter.".into())
}
#[cfg(not(unix))]
fn same_inode(_: &File, _: &Path) -> Result<(), String> {
    Err("Native crew controls require the selected Unix host custody adapter.".into())
}

#[cfg(all(test, unix))]
#[path = "agent_crew_control_tests.rs"]
mod tests;
