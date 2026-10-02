//! Backup and restore of the wallet home: the seed, `config.json`, and the
//! node's SQLite store. The seed alone recovers on-chain funds; channel
//! funds need the store, because the counterparty holds the only other
//! copy of each channel's state.
//!
//! A backup is a directory the caller names. The store is copied with
//! SQLite's `VACUUM INTO`, which reads a consistent snapshot even while the
//! node is writing, so a backup may run beside a resident node. Every file
//! is listed with its SHA-256 in `backup.json`, and the wallet home records
//! the last backup in `last-backup.json` so `wallet info` can report it.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::{CONFIG_FILE, SEED_FILE, STORE_DIR};
use crate::model::WalletError;

pub const MANIFEST_FILE: &str = "backup.json";
pub const LAST_BACKUP_FILE: &str = "last-backup.json";
const STORE_FILE: &str = "ldk_node_data.sqlite";
const VERSION: u32 = 1;

/// One backed-up file: its path under the backup directory, size, and digest.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

/// `backup.json`: what a backup directory holds.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub version: u32,
    pub created_at: u64,
    pub home: String,
    /// True when the node's store was copied; false for a seed-only backup
    /// of a wallet that has never opened its node.
    pub store: bool,
    pub files: Vec<Entry>,
}

/// `last-backup.json`: the wallet home's memory of its newest backup.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LastBackup {
    pub at: u64,
    pub path: String,
    pub files: usize,
    pub bytes: u64,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn setup(path: &Path, error: impl std::fmt::Display) -> WalletError {
    WalletError::Setup(format!("{}: {error}", path.display()))
}

fn digest(path: &Path) -> Result<Entry, WalletError> {
    let bytes = std::fs::read(path).map_err(|error| setup(path, error))?;
    Ok(Entry {
        path: String::new(),
        bytes: bytes.len() as u64,
        sha256: hex::encode(Sha256::digest(&bytes)),
    })
}

fn create_private_dir(path: &Path) -> Result<(), WalletError> {
    std::fs::create_dir_all(path).map_err(|error| setup(path, error))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .map_err(|error| setup(path, error))?;
    }
    Ok(())
}

fn copy_private(from: &Path, to: &Path) -> Result<(), WalletError> {
    std::fs::copy(from, to).map_err(|error| setup(from, error))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(to, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| setup(to, error))?;
    }
    Ok(())
}

/// Snapshot the SQLite store at `from` into `to` through a fresh
/// connection. Works beside a running node; the copy is one transaction's
/// view of the store.
fn snapshot_store(from: &Path, to: &Path) -> Result<(), WalletError> {
    let connection = rusqlite::Connection::open_with_flags(
        from,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| setup(from, error))?;
    connection
        .busy_timeout(std::time::Duration::from_secs(10))
        .map_err(|error| setup(from, error))?;
    let target = to.display().to_string();
    connection
        .execute("VACUUM INTO ?1", [target])
        .map(|_| ())
        .map_err(|error| setup(to, error))
}

/// Write a backup of the wallet at `home` into `dest`, which must not
/// already hold one. Returns the manifest.
pub fn write(home: &Path, dest: &Path) -> Result<Manifest, WalletError> {
    let seed = home.join(SEED_FILE);
    if !seed.exists() {
        return Err(WalletError::Setup(format!(
            "no seed at {}; run `openagents x402 node init`",
            seed.display()
        )));
    }
    if dest.join(MANIFEST_FILE).exists() {
        return Err(WalletError::Invalid(format!(
            "{} already holds a backup; name a new directory",
            dest.display()
        )));
    }
    create_private_dir(dest)?;
    let mut files = Vec::new();
    let mut plan: Vec<(PathBuf, PathBuf)> = vec![(seed, dest.join(SEED_FILE))];
    let config = home.join(CONFIG_FILE);
    if config.exists() {
        plan.push((config, dest.join(CONFIG_FILE)));
    }
    for (from, to) in plan {
        copy_private(&from, &to)?;
        let mut entry = digest(&to)?;
        entry.path = relative(dest, &to);
        files.push(entry);
    }
    let store = home.join(STORE_DIR).join(STORE_FILE);
    let copied_store = store.exists();
    if copied_store {
        let store_dir = dest.join(STORE_DIR);
        create_private_dir(&store_dir)?;
        let to = store_dir.join(STORE_FILE);
        snapshot_store(&store, &to)?;
        let mut entry = digest(&to)?;
        entry.path = relative(dest, &to);
        files.push(entry);
    }
    let manifest = Manifest {
        version: VERSION,
        created_at: now(),
        home: home.display().to_string(),
        store: copied_store,
        files,
    };
    write_json(&dest.join(MANIFEST_FILE), &manifest)?;
    let last = LastBackup {
        at: manifest.created_at,
        path: dest.display().to_string(),
        files: manifest.files.len(),
        bytes: manifest.files.iter().map(|f| f.bytes).sum(),
    };
    write_json(&home.join(LAST_BACKUP_FILE), &last)?;
    Ok(manifest)
}

/// Read the manifest at `dest` and check every listed file against its digest.
pub fn verify(dest: &Path) -> Result<Manifest, WalletError> {
    let path = dest.join(MANIFEST_FILE);
    let text = std::fs::read_to_string(&path).map_err(|error| setup(&path, error))?;
    let manifest: Manifest = serde_json::from_str(&text).map_err(|error| setup(&path, error))?;
    if manifest.version != VERSION {
        return Err(WalletError::Setup(format!(
            "{}: backup version {} is not {VERSION}",
            path.display(),
            manifest.version
        )));
    }
    for entry in &manifest.files {
        let file = dest.join(&entry.path);
        let found = digest(&file)?;
        if found.sha256 != entry.sha256 {
            return Err(WalletError::Setup(format!(
                "{}: digest {} does not match the manifest's {}",
                file.display(),
                found.sha256,
                entry.sha256
            )));
        }
    }
    Ok(manifest)
}

/// Restore the backup at `dest` into `home`, which must hold no seed yet.
/// Returns the verified manifest.
pub fn restore(dest: &Path, home: &Path) -> Result<Manifest, WalletError> {
    let manifest = verify(dest)?;
    let seed = home.join(SEED_FILE);
    if seed.exists() {
        return Err(WalletError::Invalid(format!(
            "{} already holds a seed; restore into an empty wallet home",
            home.display()
        )));
    }
    create_private_dir(home)?;
    for entry in &manifest.files {
        let to = home.join(&entry.path);
        if let Some(parent) = to.parent() {
            create_private_dir(parent)?;
        }
        copy_private(&dest.join(&entry.path), &to)?;
    }
    Ok(manifest)
}

/// The wallet home's record of its last backup, if any.
pub fn last(home: &Path) -> Option<LastBackup> {
    let text = std::fs::read_to_string(home.join(LAST_BACKUP_FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

fn relative(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), WalletError> {
    let text = serde_json::to_string_pretty(value).map_err(|error| setup(path, error))?;
    std::fs::write(path, text + "\n").map_err(|error| setup(path, error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Network, WalletConfig, load_or_create_seed};

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("openagents-wallet-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn make_store(home: &Path) {
        let dir = home.join(STORE_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        let connection = rusqlite::Connection::open(dir.join(STORE_FILE)).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE ldk_node_data (key TEXT PRIMARY KEY, value BLOB);
                 INSERT INTO ldk_node_data VALUES ('channel', x'0102');",
            )
            .unwrap();
    }

    #[test]
    fn backup_restores_the_same_files() {
        let home = temp("backup-home");
        let dest = temp("backup-dest");
        let restored = temp("backup-restored");
        WalletConfig::new(Network::Signet, None)
            .unwrap()
            .save(&home)
            .unwrap();
        let (seed, _) =
            load_or_create_seed(&home, true, || "abandon ".repeat(11) + "about").unwrap();
        make_store(&home);

        let manifest = write(&home, &dest).unwrap();
        assert!(manifest.store);
        assert_eq!(manifest.files.len(), 3);
        assert_eq!(last(&home).unwrap().files, 3);
        assert!(
            write(&home, &dest).is_err(),
            "a backup directory is not reused"
        );

        let verified = verify(&dest).unwrap();
        assert_eq!(verified, manifest);

        let again = restore(&dest, &restored).unwrap();
        assert_eq!(again, manifest);
        let (seed_again, created) = load_or_create_seed(&restored, false, String::new).unwrap();
        assert!(!created);
        assert_eq!(seed_again, seed);
        assert_eq!(
            WalletConfig::load(&restored).unwrap(),
            WalletConfig::load(&home).unwrap()
        );
        let connection =
            rusqlite::Connection::open(restored.join(STORE_DIR).join(STORE_FILE)).unwrap();
        let value: Vec<u8> = connection
            .query_row(
                "SELECT value FROM ldk_node_data WHERE key = 'channel'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(value, vec![1, 2]);
        assert!(
            restore(&dest, &restored).is_err(),
            "a seed is never overwritten"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&dest), 0o700);
            assert_eq!(mode(&dest.join(SEED_FILE)), 0o600);
            assert_eq!(mode(&restored.join(SEED_FILE)), 0o600);
        }
        for dir in [&home, &dest, &restored] {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn verify_notices_a_changed_file() {
        let home = temp("verify-home");
        let dest = temp("verify-dest");
        load_or_create_seed(&home, true, || "a b c".to_string()).unwrap();
        let manifest = write(&home, &dest).unwrap();
        assert!(!manifest.store);
        std::fs::write(dest.join(SEED_FILE), "x y z\n").unwrap();
        assert!(verify(&dest).is_err());
        for dir in [&home, &dest] {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}
