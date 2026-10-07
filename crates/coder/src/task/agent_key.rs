//! Custody of a workshop agent's own key
//! (`docs/verse/agent-identity-and-engrams.md`, "Identity").
//!
//! A [`KeyStore`] keeps one secret key per agent. [`FileKeys`] keeps it in
//! `agents/NAME/key`, mode `0600`, beside her record: a scratch host, a
//! CLI-only host, and every test use it. A host that runs with
//! `--keychain` installs [`Migrating`] over the host's keychain
//! ([`install_for_host`]), which keeps her key under the account
//! `agent:NAME` in the host's keychain service and moves a file key in,
//! deleting the file only after the keychain reads the same key back.
//!
//! Custody fails closed. When a key she had can't be read, she refuses
//! requests and journals why (`Store::custody`), and the host never makes
//! her a new key in its place.
//!
//! Key custody follows Buzz's secret store design (a keychain item with a
//! file fallback, read back before migrating, and an identity that fails
//! closed), reimplemented here.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use secp256k1::SecretKey;

use super::agent::{self, Entry, Kind};

/// Where one agent's key lives: her name and her directory.
#[derive(Clone, Copy, Debug)]
pub struct Slot<'a> {
    pub name: &'a str,
    /// `agents/NAME` under the host root.
    pub dir: &'a Path,
}

impl Slot<'_> {
    /// Her keychain account, `agent:NAME`.
    #[must_use]
    pub fn account(&self) -> String {
        format!("agent:{}", self.name)
    }

    /// Her key file, `agents/NAME/key`.
    #[must_use]
    pub fn file(&self) -> PathBuf {
        self.dir.join("key")
    }
}

/// Where the host keeps each agent's secret key.
pub trait KeyStore: Send + Sync + std::fmt::Debug {
    /// `file` or `keychain`, for her journal and `openagents agent show`.
    fn custody(&self) -> &'static str;

    /// Her key, or `None` when this store holds none.
    ///
    /// # Errors
    /// When the store can't be read or holds something that is not a key.
    fn load(&self, slot: Slot<'_>) -> Result<Option<SecretKey>, String>;

    /// Keeps `key` as hers, replacing what was there.
    ///
    /// # Errors
    /// When the store can't be written.
    fn store(&self, slot: Slot<'_>, key: &SecretKey) -> Result<(), String>;

    /// Deletes her key. Returns whether there was one.
    ///
    /// # Errors
    /// When the store can't be written.
    fn delete(&self, slot: Slot<'_>) -> Result<bool, String>;
}

/// Her key in `agents/NAME/key`, 64 hex characters, mode `0600`.
#[derive(Clone, Copy, Debug, Default)]
pub struct FileKeys;

impl KeyStore for FileKeys {
    fn custody(&self) -> &'static str {
        "file"
    }

    fn load(&self, slot: Slot<'_>) -> Result<Option<SecretKey>, String> {
        let path = slot.file();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("can't read {}: {e}", path.display())),
        };
        agent::parse_secret(&text)
            .map(Some)
            .map_err(|_| "the agent's key file holds no key".to_string())
    }

    fn store(&self, slot: Slot<'_>, key: &SecretKey) -> Result<(), String> {
        agent::private_dir(slot.dir)?;
        agent::write_private(&slot.file(), hex(&key.secret_bytes()).as_bytes())
    }

    fn delete(&self, slot: Slot<'_>) -> Result<bool, String> {
        let path = slot.file();
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(format!("can't remove {}: {e}", path.display())),
        }
    }
}

/// A keychain in memory, by account: the fake keychain tests use, so no
/// test touches the real one. [`MemoryKeys::fail`] makes every call refuse,
/// as a locked keychain does.
#[derive(Debug, Default)]
pub struct MemoryKeys {
    items: Mutex<BTreeMap<String, [u8; 32]>>,
    failing: AtomicBool,
}

impl MemoryKeys {
    /// Makes every later call refuse (`true`) or answer again (`false`).
    pub fn fail(&self, failing: bool) {
        self.failing.store(failing, Ordering::SeqCst);
    }

    /// Whether it holds an item for `account`.
    #[must_use]
    pub fn holds(&self, account: &str) -> bool {
        self.items().contains_key(account)
    }

    fn items(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, [u8; 32]>> {
        self.items
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn check(&self) -> Result<(), String> {
        if self.failing.load(Ordering::SeqCst) {
            Err("the keychain is locked".into())
        } else {
            Ok(())
        }
    }
}

impl KeyStore for MemoryKeys {
    fn custody(&self) -> &'static str {
        "keychain"
    }

    fn load(&self, slot: Slot<'_>) -> Result<Option<SecretKey>, String> {
        self.check()?;
        self.items()
            .get(&slot.account())
            .map(|bytes| {
                SecretKey::from_byte_array(*bytes).map_err(|_| "keychain item is not a key".into())
            })
            .transpose()
    }

    fn store(&self, slot: Slot<'_>, key: &SecretKey) -> Result<(), String> {
        self.check()?;
        self.items().insert(slot.account(), key.secret_bytes());
        Ok(())
    }

    fn delete(&self, slot: Slot<'_>) -> Result<bool, String> {
        self.check()?;
        Ok(self.items().remove(&slot.account()).is_some())
    }
}

/// The host's keychain (`coder host serve --keychain`) under its keychain
/// service, by account `agent:NAME`.
pub struct HostKeychain(pub Arc<dyn coder_host::serve::keys::AccountKeys>);

impl std::fmt::Debug for HostKeychain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HostKeychain(..)")
    }
}

impl KeyStore for HostKeychain {
    fn custody(&self) -> &'static str {
        "keychain"
    }

    fn load(&self, slot: Slot<'_>) -> Result<Option<SecretKey>, String> {
        let secret = self
            .0
            .load_account(&slot.account())
            .map_err(|e| format!("can't read the keychain: {}", e.code.as_str()))?;
        secret
            .map(|secret| {
                SecretKey::from_byte_array(*secret.expose())
                    .map_err(|_| "her keychain item is not a key".to_string())
            })
            .transpose()
    }

    fn store(&self, slot: Slot<'_>, key: &SecretKey) -> Result<(), String> {
        let secret = coder_host::serve::keys::Secret::from_bytes(key.secret_bytes());
        self.0
            .store_account(&slot.account(), &secret)
            .map_err(|e| format!("can't write the keychain: {}", e.code.as_str()))
    }

    fn delete(&self, slot: Slot<'_>) -> Result<bool, String> {
        let had = self.load(slot).ok().flatten().is_some();
        self.0
            .delete_account(&slot.account())
            .map_err(|e| format!("can't write the keychain: {}", e.code.as_str()))?;
        Ok(had)
    }
}

/// A keychain with the file store behind it: her key lives in `keychain`,
/// and a key still in her file moves in on first read, the file deleted
/// only after the keychain reads the same key back. A keychain that
/// refuses is an error, never a reason to read or make a file key.
#[derive(Debug)]
pub struct Migrating {
    keychain: Arc<dyn KeyStore>,
    file: FileKeys,
}

impl Migrating {
    #[must_use]
    pub fn new(keychain: Arc<dyn KeyStore>) -> Self {
        Self {
            keychain,
            file: FileKeys,
        }
    }

    /// Moves a file key into the keychain. Returns whether the file went;
    /// it stays whenever the keychain didn't keep the same key.
    fn migrate(&self, slot: Slot<'_>, key: &SecretKey) -> bool {
        if self.keychain.store(slot, key).is_err() {
            return false;
        }
        match self.keychain.load(slot) {
            Ok(Some(back)) if back == *key => self.file.delete(slot).is_ok(),
            _ => false,
        }
    }
}

impl KeyStore for Migrating {
    fn custody(&self) -> &'static str {
        "keychain"
    }

    fn load(&self, slot: Slot<'_>) -> Result<Option<SecretKey>, String> {
        if let Some(key) = self.keychain.load(slot)? {
            // A file left behind by a move cut short holds the same key.
            if self.file.load(slot).ok().flatten() == Some(key) {
                let _ = self.file.delete(slot);
            }
            return Ok(Some(key));
        }
        let Some(key) = self.file.load(slot)? else {
            return Ok(None);
        };
        let text = if self.migrate(slot, &key) {
            "her key moved from its file into the host's keychain"
        } else {
            "her key stays in its file: the keychain didn't keep it"
        };
        journal(slot, text);
        Ok(Some(key))
    }

    fn store(&self, slot: Slot<'_>, key: &SecretKey) -> Result<(), String> {
        self.keychain.store(slot, key)?;
        match self.keychain.load(slot)? {
            Some(back) if back == *key => Ok(()),
            _ => Err("the keychain didn't keep her key".into()),
        }
    }

    fn delete(&self, slot: Slot<'_>) -> Result<bool, String> {
        let file = self.file.delete(slot)?;
        let keychain = self.keychain.delete(slot)?;
        Ok(file || keychain)
    }
}

fn journal(slot: Slot<'_>, text: &str) {
    let Some(root) = slot.dir.parent().and_then(Path::parent) else {
        return;
    };
    if let Ok(store) = agent::Store::with_keys(root, slot.name, Arc::new(FileKeys)) {
        let _ = store.append(&Entry::new(unix_now(), Kind::Keyed, text));
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

static INSTALLED: OnceLock<Arc<dyn KeyStore>> = OnceLock::new();

/// Makes `keys` the store every agent [`agent::Store::new`] opens in this
/// process keeps her key in. The first call wins.
///
/// # Errors
/// When another store was installed first.
pub fn install(keys: Arc<dyn KeyStore>) -> Result<(), String> {
    INSTALLED
        .set(keys)
        .map_err(|_| "an agent key store is installed already".into())
}

/// The store [`install`] installed, else [`FileKeys`].
#[must_use]
pub fn installed() -> Arc<dyn KeyStore> {
    INSTALLED
        .get()
        .cloned()
        .unwrap_or_else(|| Arc::new(FileKeys))
}

/// Installs the host's keychain as the agents' key store when `arguments`
/// serve a host with `--keychain`, as the desktop app runs it. Returns
/// whether it did.
///
/// # Errors
/// When the platform has no keychain this host reads.
pub fn install_for_host(arguments: &[String]) -> Result<bool, String> {
    let serves = arguments.iter().any(|a| a == "serve");
    let keychain = arguments.iter().any(|a| a == "--keychain");
    if !(serves && keychain) {
        return Ok(false);
    }
    let accounts = coder_host::serve::keys::platform_accounts()
        .map_err(|e| format!("no keychain for the agents' keys: {}", e.code.as_str()))?;
    install(Arc::new(Migrating::new(Arc::new(HostKeychain(accounts)))))?;
    Ok(true)
}

#[cfg(test)]
#[path = "agent_key_tests.rs"]
mod tests;
