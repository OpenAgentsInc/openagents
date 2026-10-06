//! The replay store: one consumption key `network:payment_hash` inserted
//! exactly once, retained until at least `invoice_end + skew + 3600`.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    #[error("consumption key {0} is already used")]
    Duplicate(String),
    #[error("replay store: {0}")]
    Io(#[from] std::io::Error),
    #[error("replay store: {0}")]
    Encode(#[from] serde_json::Error),
}

/// What one consumed key records. No preimage or invoice is retained here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayEntry {
    pub key: String,
    pub network: String,
    pub payment_hash: String,
    pub amount_msat: u64,
    pub consumed_at: u64,
    pub retain_until: u64,
    /// The server's own reference to what was bought, for reconciliation.
    pub purchase: String,
}

pub trait ReplayStore {
    /// Insert `entry` under its key. Fails with `Duplicate` if the key exists,
    /// whichever process got there first.
    fn insert(&self, entry: &ReplayEntry) -> Result<(), ReplayError>;
    fn get(&self, key: &str) -> Result<Option<ReplayEntry>, ReplayError>;
    /// Give a consumed key back, so the same proof can be settled again.
    /// Only for a settlement whose purchase never ran: the multi-route front
    /// calls it when its settlement hook refused before execution.
    fn release(&self, key: &str) -> Result<(), ReplayError>;
    /// Remove entries whose `retain_until` has passed. Returns how many.
    fn sweep(&self, now: u64) -> Result<usize, ReplayError>;
}

/// A directory with one JSON file per key. `File::create_new` is `O_EXCL`,
/// so two processes racing one key get exactly one success without a lock.
pub struct FileReplayStore {
    dir: PathBuf,
}

impl FileReplayStore {
    pub fn open(dir: &Path) -> Result<Self, ReplayError> {
        fs::create_dir_all(dir)?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, key: &str) -> PathBuf {
        // Keys are `lnbtc:<32 hex>:<64 hex>`; a colon is a valid file name
        // byte on Unix but not everywhere, so store them with a dash.
        self.dir.join(format!("{}.json", key.replace(':', "-")))
    }
}

impl ReplayStore for FileReplayStore {
    fn insert(&self, entry: &ReplayEntry) -> Result<(), ReplayError> {
        let path = self.path(&entry.key);
        let mut file = match fs::File::create_new(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                return Err(ReplayError::Duplicate(entry.key.clone()));
            }
            Err(error) => return Err(error.into()),
        };
        let bytes = serde_json::to_vec(entry)?;
        if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
            // Our own half-written file must not block the legitimate retry.
            let _ = fs::remove_file(&path);
            return Err(error.into());
        }
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<ReplayEntry>, ReplayError> {
        match fs::read(self.path(key)) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn release(&self, key: &str) -> Result<(), ReplayError> {
        match fs::remove_file(self.path(key)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn sweep(&self, now: u64) -> Result<usize, ReplayError> {
        let mut removed = 0;
        for item in fs::read_dir(&self.dir)? {
            let path = item?.path();
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            let Ok(bytes) = fs::read(&path) else { continue };
            let Ok(entry) = serde_json::from_slice::<ReplayEntry>(&bytes) else {
                continue;
            };
            if entry.retain_until < now {
                fs::remove_file(&path)?;
                removed += 1;
            }
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, retain_until: u64) -> ReplayEntry {
        ReplayEntry {
            key: key.into(),
            network: "lnbtc:000000000019d6689c085ae165831e93".into(),
            payment_hash: "ab".repeat(32),
            amount_msat: 1000,
            consumed_at: 1,
            retain_until,
            purchase: "p1".into(),
        }
    }

    fn temp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "x402-replay-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn inserts_once_and_reads_back() {
        let dir = temp();
        let store = FileReplayStore::open(&dir).unwrap();
        let first = entry("lnbtc:aa:bb", 10);
        store.insert(&first).unwrap();
        assert!(matches!(
            store.insert(&first),
            Err(ReplayError::Duplicate(key)) if key == "lnbtc:aa:bb"
        ));
        assert_eq!(store.get("lnbtc:aa:bb").unwrap(), Some(first.clone()));
        assert_eq!(store.get("lnbtc:aa:cc").unwrap(), None);
        store.release("lnbtc:aa:bb").unwrap();
        assert_eq!(store.get("lnbtc:aa:bb").unwrap(), None);
        store.insert(&first).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sweep_respects_retain_until() {
        let dir = temp();
        let store = FileReplayStore::open(&dir).unwrap();
        store.insert(&entry("k:1", 100)).unwrap();
        store.insert(&entry("k:2", 200)).unwrap();
        assert_eq!(store.sweep(100).unwrap(), 0, "equality is still retained");
        assert_eq!(store.sweep(150).unwrap(), 1);
        assert!(store.get("k:1").unwrap().is_none());
        assert!(store.get("k:2").unwrap().is_some());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn concurrent_inserts_admit_exactly_one() {
        let dir = temp();
        let store = std::sync::Arc::new(FileReplayStore::open(&dir).unwrap());
        let handles: Vec<_> = (0..16)
            .map(|_| {
                let store = store.clone();
                std::thread::spawn(move || store.insert(&entry("race:1", 10)).is_ok())
            })
            .collect();
        let wins = handles
            .into_iter()
            .filter_map(|handle| handle.join().unwrap().then_some(()))
            .count();
        assert_eq!(wins, 1);
        fs::remove_dir_all(dir).unwrap();
    }
}

impl<S: ReplayStore + ?Sized> ReplayStore for std::sync::Arc<S> {
    fn insert(&self, entry: &ReplayEntry) -> Result<(), ReplayError> {
        (**self).insert(entry)
    }
    fn get(&self, key: &str) -> Result<Option<ReplayEntry>, ReplayError> {
        (**self).get(key)
    }
    fn release(&self, key: &str) -> Result<(), ReplayError> {
        (**self).release(key)
    }
    fn sweep(&self, now: u64) -> Result<usize, ReplayError> {
        (**self).sweep(now)
    }
}
