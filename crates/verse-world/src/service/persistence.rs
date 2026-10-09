//! Exclusively owned journal and snapshots with explicit durability failures.
use super::{auth::Gateway, save::MAX_BYTES};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub mod backup;
#[cfg(test)]
mod checks;
mod journal;
pub mod migration;
pub(super) mod writer;

const FILE_BYTES: usize = MAX_BYTES * 2 + 4096;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Committed {
    version: u32,
    revision: u64,
    digest: [u8; 32],
    checkpoint: String,
}
fn digest(revision: u64, checkpoint: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"verse.chamber.commit.v1\0");
    h.update(revision.to_be_bytes());
    h.update(checkpoint.as_bytes());
    h.finalize().into()
}
/// The slowest reward-history publications in this process, for soak
/// diagnosis (#10559).
pub fn slow_history_syncs() -> serde_json::Value {
    serde_json::to_value(super::rewards::history::slow_syncs()).unwrap_or_default()
}
#[derive(Clone, Copy, Default)]
pub(in crate::service) struct CommitTimings {
    pub preparation: Option<f64>,
    pub history_sync: Option<f64>,
    pub journal_encoding: Option<f64>,
    pub journal_write: Option<f64>,
    pub journal_sync: Option<f64>,
    pub snapshot_compaction: Option<f64>,
}
fn observed<T>(
    slot: &mut Option<f64>,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let start = std::time::Instant::now();
    let result = operation();
    *slot = Some(start.elapsed().as_secs_f64());
    result
}
pub struct Commit {
    pub revision: u64,
    pub bytes: usize,
    pub written: bool,
}
/// Retain this value for the host lifetime; dropping it releases the writer lock.
pub struct Store {
    _lock: File,
    root: PathBuf,
    content: [u8; 32],
    instance: u64,
    revision: u64,
    last_hash: Option<[u8; 32]>,
    recovered: Option<Gateway>,
    owner: Option<[u8; 32]>,
    poisoned: bool,
    history: super::rewards::history::History,
    journal: File,
    state: Option<serde_json::Value>,
    records: usize,
    #[cfg(test)]
    hook: Option<std::sync::Arc<dyn Fn(&str) + Send + Sync>>,
}
impl Store {
    pub fn open(root: &Path, content: [u8; 32], instance: u64) -> Result<Self, String> {
        if instance == 0 || root.as_os_str().is_empty() {
            return Err("Invalid chamber storage context".into());
        }
        let root = if root.is_absolute() {
            root.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|_| "Cannot resolve chamber storage directory")?
                .join(root)
        };
        let root = root.as_path();
        if !root.exists() {
            let mut created: Vec<_> = root
                .ancestors()
                .take_while(|p| !p.exists())
                .map(Path::to_path_buf)
                .collect();
            std::fs::create_dir_all(root).map_err(|_| "Cannot create chamber storage directory")?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))
                    .map_err(|_| "Cannot secure chamber storage directory")?;
            }
            if let Some(parent) = created
                .last()
                .and_then(|p| p.parent())
                .map(Path::to_path_buf)
            {
                created.push(parent);
            }
            for directory in &created {
                File::open(directory)
                    .and_then(|file| file.sync_all())
                    .map_err(|_| "Cannot sync chamber storage directory creation")?;
            }
        }
        let metadata = std::fs::symlink_metadata(root)
            .map_err(|_| "Cannot inspect chamber storage directory")?;
        if !metadata.is_dir() {
            return Err("Chamber storage must be a directory".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err("Chamber storage directory must have owner-only permissions".into());
            }
        }
        regular_or_absent(&root.join("writer.lock"))?;
        let mut options = OpenOptions::new();
        options.create(true).truncate(false).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options
            .open(root.join("writer.lock"))
            .map_err(|_| "Cannot open chamber writer lock")?;
        lock.try_lock()
            .map_err(|_| "Chamber storage already has a writer or cannot lock")?;
        for marker in ["restore.pending", "backup.pending", "backup.json"] {
            if std::fs::symlink_metadata(root.join(marker)).is_ok() {
                return Err("Chamber storage is an incomplete restore or a backup archive".into());
            }
        }
        migration::recover_pending(root)?;
        let history = super::rewards::history::History::open(&root.join("rewards"))?;
        history.defer_writes();
        let log = root.join("journal.jsonl");
        regular_or_absent(&log)?;
        let mut options = OpenOptions::new();
        options.read(true).append(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut journal = options
            .open(log)
            .map_err(|_| "Cannot open chamber journal")?;
        journal
            .sync_all()
            .map_err(|_| "Cannot sync chamber journal")?;
        File::open(root)
            .and_then(|f| f.sync_all())
            .map_err(|_| "Cannot sync chamber journal directory")?;
        let current = root.join("chamber.json");
        regular_or_absent(&current)?;
        let (revision, last_hash, recovered, state, records) = if current.exists() {
            let file = File::open(&current).map_err(|_| "Cannot open committed chamber")?;
            let mut bytes = vec![];
            file.take(FILE_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "Cannot read committed chamber")?;
            if bytes.len() > FILE_BYTES {
                return Err("Committed chamber byte budget exceeded".into());
            }
            let saved: Committed =
                serde_json::from_slice(&bytes).map_err(|_| "Invalid committed chamber")?;
            if saved.version != 1
                || saved.revision == 0
                || saved.checkpoint.len() > MAX_BYTES
                || saved.digest != digest(saved.revision, &saved.checkpoint)
            {
                return Err("Committed chamber checksum or version is invalid".into());
            }
            let mut state = journal::expand(saved.checkpoint.as_bytes())?;
            let records = journal::replay(&mut journal, saved.revision, &mut state)?;
            let revision = saved
                .revision
                .checked_add(records as u64)
                .ok_or("Chamber commit revisions exhausted")?;
            let gateway = super::save::decode_with_history(
                &journal::contract(&state)?,
                content,
                instance,
                Some(history.clone()),
            )?;
            (
                revision,
                Some(journal::hash(&state)?),
                Some(gateway),
                Some(state),
                records,
            )
        } else {
            if journal
                .metadata()
                .map_err(|_| "Cannot inspect chamber journal")?
                .len()
                != 0
            {
                return Err("Chamber journal has no committed base snapshot".into());
            }
            (0, None, None, None, 0)
        };
        if records == 0 {
            journal
                .set_len(0)
                .and_then(|_| journal.sync_all())
                .map_err(|_| "Cannot discard compacted chamber journal")?;
        }
        let pending = root.join("next.json");
        regular_or_absent(&pending)?;
        if pending.exists() {
            std::fs::remove_file(pending)
                .map_err(|_| "Cannot discard interrupted chamber write")?;
        }
        let owner = recovered.as_ref().map(Gateway::server_identity);
        Ok(Self {
            _lock: lock,
            root: root.into(),
            content,
            instance,
            revision,
            last_hash,
            recovered,
            owner,
            poisoned: false,
            history,
            journal,
            state,
            records,
            #[cfg(test)]
            hook: None,
        })
    }
    /// Take the validated existing world before committing; never overwrite it with a fresh spawn.
    pub fn recover(&mut self) -> Option<Gateway> {
        self.recovered.take()
    }
    pub(super) fn prepare(
        &mut self,
        gateway: &mut Gateway,
    ) -> Result<super::save::Prepared, String> {
        if self.poisoned || self.recovered.is_some() {
            return Err("Chamber storage is unavailable or recovery is still pending".into());
        }
        if gateway.content() != Some(self.content)
            || gateway.game().player_life().instance != self.instance
        {
            return Err("Chamber storage context is incompatible".into());
        }
        if self
            .owner
            .is_some_and(|owner| owner != gateway.server_identity())
        {
            return Err("Chamber storage belongs to another host authority".into());
        }
        if let Err(error) = gateway.chamber.rewards.attach(self.history.clone()) {
            self.poisoned = true;
            return Err(error);
        }
        super::save::Prepared::capture(gateway)
    }
    #[cfg(test)]
    pub(in crate::service) fn boundary(&self, stage: &str) {
        if let Some(hook) = &self.hook {
            hook(stage);
        }
    }
    #[cfg(test)]
    pub(in crate::service) fn inject(&mut self, hook: std::sync::Arc<dyn Fn(&str) + Send + Sync>) {
        self.hook = Some(hook);
    }
    pub fn commit(&mut self, gateway: &mut Gateway) -> Result<Commit, String> {
        let prepared = self.prepare(gateway)?;
        self.commit_prepared(prepared)
    }
    pub(super) fn commit_prepared(
        &mut self,
        prepared: super::save::Prepared,
    ) -> Result<Commit, String> {
        self.commit_measured(prepared).0
    }
    pub(super) fn commit_measured(
        &mut self,
        prepared: super::save::Prepared,
    ) -> (Result<Commit, String>, CommitTimings) {
        let mut timings = CommitTimings::default();
        let result = self.commit_inner(prepared, &mut timings);
        (result, timings)
    }
    fn commit_inner(
        &mut self,
        prepared: super::save::Prepared,
        timings: &mut CommitTimings,
    ) -> Result<Commit, String> {
        if self.poisoned || self.recovered.is_some() {
            return Err("Chamber storage is unavailable or recovery is still pending".into());
        }
        if prepared.content != self.content
            || prepared.instance() != self.instance
            || self.owner.is_some_and(|owner| owner != prepared.owner)
        {
            return Err("Chamber persistence copy has a foreign authority or context".into());
        }
        #[cfg(test)]
        self.boundary("before_encode");
        let (state, hash) = observed(&mut timings.preparation, || {
            let state = prepared.expanded()?;
            let hash = journal::hash(&state)?;
            Ok((state, hash))
        })?;
        if self.last_hash == Some(hash) {
            if let Err(error) = observed(&mut timings.history_sync, || self.history.synchronize()) {
                self.poisoned = true;
                return Err(error);
            }
            return Ok(Commit {
                revision: self.revision,
                bytes: 0,
                written: false,
            });
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or("Chamber commit revisions exhausted")?;
        let result = (|| {
            regular_or_absent(&self.root.join("next.json"))?;
            observed(&mut timings.history_sync, || self.history.synchronize())?;
            #[cfg(test)]
            self.boundary("after_history");
            let mut bytes = 0;
            if let Some(old) = &self.state {
                if self.records < journal::INTERVAL {
                    // The retained parent and newly computed state digests bind these exact copies.
                    bytes += journal::append(
                        &mut self.journal,
                        revision,
                        old,
                        &state,
                        self.last_hash.ok_or("Committed chamber digest is absent")?,
                        hash,
                        timings,
                    )?;
                    self.records += 1;
                    #[cfg(test)]
                    self.boundary("after_journal");
                }
            }
            if self.state.is_none()
                || self.records >= journal::INTERVAL
                || self
                    .journal
                    .metadata()
                    .map_err(|_| "Cannot inspect chamber journal")?
                    .len()
                    >= journal::LOG_BYTES
            {
                bytes += observed(&mut timings.snapshot_compaction, || {
                    let checkpoint = String::from_utf8(journal::contract(&state)?)
                        .map_err(|_| "Cannot encode committed chamber")?;
                    let bytes = self.snapshot(revision, checkpoint)?;
                    #[cfg(test)]
                    self.boundary("before_journal_clear");
                    self.journal
                        .set_len(0)
                        .and_then(|_| self.journal.sync_all())
                        .map_err(|_| "Cannot compact chamber journal")?;
                    self.records = 0;
                    #[cfg(test)]
                    self.boundary("after_journal_clear");
                    Ok(bytes)
                })?;
            }
            Ok(bytes)
        })();
        let bytes = match result {
            Ok(bytes) => bytes,
            Err(error) => {
                self.poisoned = true;
                return Err(error);
            }
        };
        self.revision = revision;
        self.last_hash = Some(hash);
        self.state = Some(state);
        self.owner = Some(prepared.owner);
        Ok(Commit {
            revision,
            bytes,
            written: true,
        })
    }
    fn snapshot(&self, revision: u64, checkpoint: String) -> Result<usize, String> {
        let saved = Committed {
            version: 1,
            revision,
            digest: digest(revision, &checkpoint),
            checkpoint,
        };
        let bytes = serde_json::to_vec(&saved).map_err(|_| "Cannot encode committed chamber")?;
        if bytes.len() > FILE_BYTES {
            return Err("Committed chamber byte budget exceeded".into());
        }
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let pending = self.root.join("next.json");
        let mut file = options
            .open(&pending)
            .map_err(|_| "Cannot stage chamber commit")?;
        file.write_all(&bytes)
            .map_err(|_| "Cannot write chamber commit")?;
        #[cfg(test)]
        self.boundary("before_snapshot_sync");
        file.sync_all().map_err(|_| "Cannot sync chamber commit")?;
        #[cfg(test)]
        self.boundary("after_snapshot_sync");
        std::fs::rename(pending, self.root.join("chamber.json"))
            .map_err(|_| "Cannot replace committed chamber")?;
        #[cfg(test)]
        self.boundary("after_snapshot_rename");
        File::open(&self.root)
            .and_then(|f| f.sync_all())
            .map_err(|_| "Cannot sync chamber commit directory")?;
        #[cfg(test)]
        self.boundary("after_snapshot_directory");
        Ok(bytes.len())
    }
}
fn regular_or_absent(path: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err("Chamber storage entry must be a regular file".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Cannot inspect chamber storage entry".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::net::tests::{gateway, key};
    pub(super) fn prepared() -> Gateway {
        gateway(&[key(101), key(102), key(103)])
            .with_content([8; 32])
            .unwrap()
    }
    #[test]
    fn atomic_store_recovers_validated_world_and_excludes_other_writers() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("state");
        let mut store = Store::open(&root, [8; 32], 120).unwrap();
        assert!(store.recover().is_none());
        assert!(Store::open(&root, [8; 32], 120).is_err());
        let mut g = prepared();
        assert_eq!(store.commit(&mut g).unwrap().revision, 1);
        assert!(store.commit(&mut prepared()).is_err());
        assert!(!store.commit(&mut g).unwrap().written);
        drop(store);
        std::fs::write(root.join("next.json"), b"interrupted").unwrap();
        let mut store = Store::open(&root, [8; 32], 120).unwrap();
        assert!(!root.join("next.json").exists());
        assert!(store.commit(&mut g).is_err());
        let mut restored = store.recover().unwrap();
        assert!(store.commit(&mut g).is_err());
        assert_eq!(restored.game().controlled_effects().count(), 2);
        assert_eq!(store.commit(&mut restored).unwrap().revision, 2);
        assert!(!store.commit(&mut restored).unwrap().written);
        drop(store);
        assert!(Store::open(&root, [9; 32], 120).is_err());
        assert!(Store::open(&root, [8; 32], 121).is_err());
    }
    #[test]
    fn committed_rewards_recover_once_and_failed_grants_do_not_reach_disk() {
        use crate::service::rewards::{Entry, Transaction};
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("state");
        let mut store = Store::open(&root, [8; 32], 120).unwrap();
        let mut g = prepared();
        let actor = g.game().player_life().actor;
        let tx = Transaction {
            acceptance: None,
            outfit: None,
            equipment: None,
            spent: vec![],
            instance: 120,
            actor,
            source: [9; 32],
            experience: 45,
            items: vec![Entry { id: 1, count: 2 }],
            quests: vec![Entry { id: 3, count: 1 }],
        };
        let receipt = g.grant_reward(tx.clone()).unwrap();
        store.commit(&mut g).unwrap();
        drop(store);
        let mut store = Store::open(&root, [8; 32], 120).unwrap();
        let mut recovered = store.recover().unwrap();
        assert_eq!(recovered.grant_reward(tx.clone()).unwrap(), receipt);
        assert_eq!(recovered.character_rewards(actor).unwrap().experience, 45);
        store.commit(&mut recovered).unwrap();
        let before = std::fs::read(root.join("chamber.json")).unwrap();
        assert_eq!(recovered.grant_reward(tx.clone()).unwrap(), receipt);
        assert!(!store.commit(&mut recovered).unwrap().written);
        std::fs::create_dir(root.join("next.json")).unwrap();
        let mut next = tx.clone();
        next.source = [10; 32];
        recovered.grant_reward(next).unwrap();
        assert!(store.commit(&mut recovered).is_err());
        assert_eq!(std::fs::read(root.join("chamber.json")).unwrap(), before);
        drop(store);
        std::fs::remove_dir(root.join("next.json")).unwrap();
        let mut store = Store::open(&root, [8; 32], 120).unwrap();
        let mut recovered = store.recover().unwrap();
        assert_eq!(recovered.character_rewards(actor).unwrap().experience, 45);
        assert_eq!(recovered.grant_reward(tx).unwrap(), receipt);
    }
    #[test]
    fn corrupt_state_and_failed_replacement_never_become_acknowledged_commits() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("state");
        let mut store = Store::open(&root, [8; 32], 120).unwrap();
        let mut g = prepared();
        store.commit(&mut g).unwrap();
        let before = std::fs::read(root.join("chamber.json")).unwrap();
        std::fs::create_dir(root.join("next.json")).unwrap();
        g.tick(1. / 30.).unwrap();
        assert!(store.commit(&mut g).is_err());
        assert_eq!(std::fs::read(root.join("chamber.json")).unwrap(), before);
        std::fs::remove_dir(root.join("next.json")).unwrap();
        assert!(store.commit(&mut g).is_err());
        drop(store);
        let mut bad: serde_json::Value = serde_json::from_slice(&before).unwrap();
        bad["revision"] = 99.into();
        std::fs::write(root.join("chamber.json"), serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(Store::open(&root, [8; 32], 120).is_err());
        std::fs::write(root.join("chamber.json"), b"truncated").unwrap();
        assert!(Store::open(&root, [8; 32], 120).is_err());
    }
    #[test]
    fn archived_rewards_recover_after_the_old_lifetime_limit_and_ignore_uncommitted_writes() {
        use crate::service::rewards::Transaction;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("state");
        let mut store = Store::open(&root, [8; 32], 120).unwrap();
        let mut g = prepared();
        store.commit(&mut g).unwrap();
        let actor = g.game().player_life().actor;
        let tx = |n: u64| {
            let mut source = [3; 32];
            source[..8].copy_from_slice(&n.to_be_bytes());
            Transaction {
                acceptance: None,
                outfit: None,
                equipment: None,
                spent: vec![],
                instance: 120,
                actor,
                source,
                experience: 1,
                items: vec![],
                quests: vec![],
            }
        };
        let original = g.grant_reward(tx(1)).unwrap();
        for n in 2..=4200 {
            g.grant_reward(tx(n)).unwrap();
        }
        store.commit(&mut g).unwrap();
        let committed = std::fs::read(root.join("chamber.json")).unwrap();
        assert!(committed.len() < 128 * 1024);
        for n in 4201..=4400 {
            g.grant_reward(tx(n)).unwrap();
        }
        drop(g);
        drop(store);
        let mut store = Store::open(&root, [8; 32], 120).unwrap();
        let mut recovered = store.recover().unwrap();
        assert_eq!(recovered.character_rewards(actor).unwrap().experience, 4200);
        assert_eq!(recovered.grant_reward(tx(1)).unwrap(), original);
        assert_eq!(recovered.grant_reward(tx(4201)).unwrap().revision, 4201);
        let mut conflict = tx(1);
        conflict.experience = 2;
        assert!(recovered.grant_reward(conflict).is_err());
        store.commit(&mut recovered).unwrap();
        drop(recovered);
        drop(store);
        let mut store = Store::open(&root, [8; 32], 120).unwrap();
        let recovered = store.recover().unwrap();
        assert_eq!(recovered.character_rewards(actor).unwrap().experience, 4201);
        let saved: serde_json::Value =
            serde_json::from_slice(&recovered.checkpoint().unwrap()).unwrap();
        drop(recovered);
        drop(store);
        let digest: String = saved["ledger"]["root"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| format!("{:02x}", b.as_u64().unwrap()))
            .collect();
        std::fs::remove_file(root.join("rewards").join(digest)).unwrap();
        assert!(Store::open(&root, [8; 32], 120).is_err());
    }
}
