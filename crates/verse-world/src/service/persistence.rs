//! Exclusively owned atomic host snapshots with explicit durability failures.
use super::{auth::Gateway, save::MAX_BYTES};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

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
        let current = root.join("chamber.json");
        regular_or_absent(&current)?;
        let (revision, last_hash, recovered) = if current.exists() {
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
            let gateway = Gateway::restore(saved.checkpoint.as_bytes(), content, instance)?;
            (
                saved.revision,
                Some(Sha256::digest(saved.checkpoint.as_bytes()).into()),
                Some(gateway),
            )
        } else {
            (0, None, None)
        };
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
        })
    }
    /// Take the validated existing world before committing; never overwrite it with a fresh spawn.
    pub fn recover(&mut self) -> Option<Gateway> {
        self.recovered.take()
    }
    pub fn commit(&mut self, gateway: &Gateway) -> Result<Commit, String> {
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
        let checkpoint = String::from_utf8(gateway.checkpoint()?)
            .map_err(|_| "Cannot encode committed chamber")?;
        let hash = Sha256::digest(checkpoint.as_bytes()).into();
        if self.last_hash == Some(hash) {
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
        let result = (|| {
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
            file.sync_all().map_err(|_| "Cannot sync chamber commit")?;
            std::fs::rename(pending, self.root.join("chamber.json"))
                .map_err(|_| "Cannot replace committed chamber")?;
            File::open(&self.root)
                .and_then(|directory| directory.sync_all())
                .map_err(|_| "Cannot sync chamber commit directory")?;
            Ok::<(), String>(())
        })();
        if let Err(error) = result {
            self.poisoned = true;
            return Err(error);
        }
        self.revision = revision;
        self.last_hash = Some(hash);
        self.owner = Some(gateway.server_identity());
        Ok(Commit {
            revision,
            bytes: bytes.len(),
            written: true,
        })
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
    fn prepared() -> Gateway {
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
        let g = prepared();
        assert_eq!(store.commit(&g).unwrap().revision, 1);
        assert!(store.commit(&prepared()).is_err());
        assert!(!store.commit(&g).unwrap().written);
        drop(store);
        std::fs::write(root.join("next.json"), b"interrupted").unwrap();
        let mut store = Store::open(&root, [8; 32], 120).unwrap();
        assert!(!root.join("next.json").exists());
        assert!(store.commit(&g).is_err());
        let restored = store.recover().unwrap();
        assert!(store.commit(&g).is_err());
        assert_eq!(restored.game().controlled_effects().count(), 2);
        assert_eq!(store.commit(&restored).unwrap().revision, 2);
        assert!(!store.commit(&restored).unwrap().written);
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
        store.commit(&g).unwrap();
        drop(store);
        let mut store = Store::open(&root, [8; 32], 120).unwrap();
        let mut recovered = store.recover().unwrap();
        assert_eq!(recovered.grant_reward(tx.clone()).unwrap(), receipt);
        assert_eq!(recovered.character_rewards(actor).unwrap().experience, 45);
        store.commit(&recovered).unwrap();
        let before = std::fs::read(root.join("chamber.json")).unwrap();
        assert_eq!(recovered.grant_reward(tx.clone()).unwrap(), receipt);
        assert!(!store.commit(&recovered).unwrap().written);
        std::fs::create_dir(root.join("next.json")).unwrap();
        let mut next = tx.clone();
        next.source = [10; 32];
        recovered.grant_reward(next).unwrap();
        assert!(store.commit(&recovered).is_err());
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
        store.commit(&g).unwrap();
        let before = std::fs::read(root.join("chamber.json")).unwrap();
        std::fs::create_dir(root.join("next.json")).unwrap();
        g.tick(1. / 30.).unwrap();
        assert!(store.commit(&g).is_err());
        assert_eq!(std::fs::read(root.join("chamber.json")).unwrap(), before);
        std::fs::remove_dir(root.join("next.json")).unwrap();
        assert!(store.commit(&g).is_err());
        drop(store);
        let mut bad: serde_json::Value = serde_json::from_slice(&before).unwrap();
        bad["revision"] = 99.into();
        std::fs::write(root.join("chamber.json"), serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(Store::open(&root, [8; 32], 120).is_err());
        std::fs::write(root.join("chamber.json"), b"truncated").unwrap();
        assert!(Store::open(&root, [8; 32], 120).is_err());
    }
}
