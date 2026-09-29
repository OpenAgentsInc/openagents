//! The OS keychain as the host's key source.
//!
//! Under the desktop app the host's Nostr key, the owner's Nostr key, and
//! the host's iroh key live in the login keychain under the service
//! `com.openagents.desktop` ([`coder_service::adopt::KEYCHAIN_SERVICE`]),
//! one item an account, each value 64 lowercase hexadecimal characters.
//! The accounts are the ones adoption writes (#9973): `host-key` and
//! `owner-key`, plus `host-iroh-key`. [`KeychainKeySource`] implements
//! `openagents-connect`'s [`KeySource`] over them for `coder host serve`.
//!
//! Only the host process and the short-lived `migrate` helper read or write
//! these items; the window process never links a call to this module (a
//! test in `tests.rs` checks its sources). Nothing here prints, logs, or
//! returns a secret, and no error echoes an item's value.

use coder_service::adopt::{self, HOST_KEY_ACCOUNT, KEYCHAIN_SERVICE, OWNER_KEY_ACCOUNT};
use openagents_connect::keys::{KeyName, KeySource, Secret};
use openagents_connect::{Code, Error};
use std::collections::BTreeMap;
use std::sync::Mutex;

/// The keychain account of the host's iroh secret key.
pub const HOST_IROH_KEY_ACCOUNT: &str = "host-iroh-key";

/// The keychain account for `name`. A computer's app holds no device keys.
pub const fn account(name: KeyName) -> Option<&'static str> {
    match name {
        KeyName::Host => Some(HOST_KEY_ACCOUNT),
        KeyName::Owner => Some(OWNER_KEY_ACCOUNT),
        KeyName::HostIroh => Some(HOST_IROH_KEY_ACCOUNT),
        KeyName::Device | KeyName::DeviceIroh => None,
    }
}

/// Where the items are kept: the OS keychain, or memory in tests.
pub trait SecretStore: Send {
    /// The item's value, `None` when there is no item.
    fn get(&mut self, account: &str) -> Result<Option<String>, String>;
    /// Creates or replaces the item.
    fn set(&mut self, account: &str, value: &str) -> Result<(), String>;
    /// Deletes the item; a missing item is not an error.
    fn delete(&mut self, account: &str) -> Result<(), String>;
}

/// The OS keychain through `keyring`: Keychain Services on macOS, the
/// Secret Service on Linux, and the Credential Manager on Windows.
#[derive(Debug, Default)]
pub struct OsKeychain;

fn entry(account: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, account)
        .map_err(|_| format!("the keychain cannot open the {account} item"))
}

impl SecretStore for OsKeychain {
    fn get(&mut self, account: &str) -> Result<Option<String>, String> {
        match entry(account)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(format!("the keychain cannot read the {account} item")),
        }
    }

    fn set(&mut self, account: &str, value: &str) -> Result<(), String> {
        entry(account)?
            .set_password(value)
            .map_err(|_| format!("the keychain cannot write the {account} item"))
    }

    fn delete(&mut self, account: &str) -> Result<(), String> {
        match entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(format!("the keychain cannot delete the {account} item")),
        }
    }
}

/// Items in memory, for tests.
#[derive(Debug, Default)]
pub struct MemoryStore(pub BTreeMap<String, String>);

impl SecretStore for MemoryStore {
    fn get(&mut self, account: &str) -> Result<Option<String>, String> {
        Ok(self.0.get(account).cloned())
    }

    fn set(&mut self, account: &str, value: &str) -> Result<(), String> {
        self.0.insert(account.into(), value.into());
        Ok(())
    }

    fn delete(&mut self, account: &str) -> Result<(), String> {
        self.0.remove(account);
        Ok(())
    }
}

fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Option<[u8; 32]> {
    let text = text.trim();
    if text.len() != 64 || !text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return None;
    }
    let mut out = [0u8; 32];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// The host's keys in a [`SecretStore`].
#[derive(Debug, Default)]
pub struct KeychainKeySource<S: SecretStore> {
    store: Mutex<S>,
}

impl<S: SecretStore> KeychainKeySource<S> {
    pub fn new(store: S) -> KeychainKeySource<S> {
        KeychainKeySource {
            store: Mutex::new(store),
        }
    }

    fn with<T>(&self, run: impl FnOnce(&mut S) -> T) -> T {
        let mut store = self
            .store
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        run(&mut store)
    }
}

fn unavailable(message: String) -> Error {
    Error::new(Code::Unavailable, message)
}

fn no_account(name: KeyName) -> Error {
    Error::new(
        Code::Malformed,
        format!("the desktop app keeps no {} key", name.as_str()),
    )
}

impl<S: SecretStore> KeySource for KeychainKeySource<S> {
    fn load(&self, name: KeyName) -> openagents_connect::Result<Option<Secret>> {
        let account = account(name).ok_or_else(|| no_account(name))?;
        match self.with(|store| store.get(account)).map_err(unavailable)? {
            None => Ok(None),
            Some(value) => unhex(&value)
                .map(|bytes| Some(Secret::from_bytes(bytes)))
                .ok_or_else(|| {
                    Error::new(
                        Code::Malformed,
                        format!("the keychain's {account} item is not a key"),
                    )
                }),
        }
    }

    fn store(&self, name: KeyName, secret: &Secret) -> openagents_connect::Result<()> {
        let account = account(name).ok_or_else(|| no_account(name))?;
        self.with(|store| store.set(account, &hex(secret.expose())))
            .map_err(unavailable)
    }

    fn delete(&self, name: KeyName) -> openagents_connect::Result<()> {
        let account = account(name).ok_or_else(|| no_account(name))?;
        self.with(|store| store.delete(account))
            .map_err(unavailable)
    }
}

/// Adoption of an old-style setup reaches the keychain through here,
/// under the same accounts the host reads.
impl<S: SecretStore> adopt::Keychain for KeychainKeySource<S> {
    fn read(&mut self, account: &str) -> coder_service::Result<Option<String>> {
        self.with(|store| store.get(account))
            .map_err(coder_service::Error::Refused)
    }

    fn write(&mut self, account: &str, value: &str) -> coder_service::Result<()> {
        self.with(|store| store.set(account, value))
            .map_err(coder_service::Error::Refused)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_connect::keys::load_or_create;

    #[test]
    fn keys_are_lowercase_hex_under_the_adoption_accounts() {
        let source = KeychainKeySource::new(MemoryStore::default());
        assert_eq!(source.load(KeyName::Host).unwrap(), None);
        let made = load_or_create(&source, KeyName::Host, || Secret::from_bytes([0xab; 32]))
            .expect("a key");
        assert_eq!(made.expose(), &[0xab; 32]);
        // A second call reads the stored key instead of making another.
        let again =
            load_or_create(&source, KeyName::Host, || Secret::from_bytes([1; 32])).expect("a key");
        assert_eq!(again, made);
        assert_eq!(
            source.with(|store| store.0.get(HOST_KEY_ACCOUNT).cloned()),
            Some("ab".repeat(32))
        );
        assert_eq!(account(KeyName::Owner), Some("owner-key"));
        assert_eq!(account(KeyName::HostIroh), Some("host-iroh-key"));
        assert_eq!(account(KeyName::Device), None);
        source.delete(KeyName::Host).unwrap();
        assert_eq!(source.load(KeyName::Host).unwrap(), None);
    }

    #[test]
    fn a_malformed_item_is_an_error_that_does_not_echo_it() {
        let mut store = MemoryStore::default();
        store.set(OWNER_KEY_ACCOUNT, "nsec1notakey").unwrap();
        let source = KeychainKeySource::new(store);
        let error = source.load(KeyName::Owner).expect_err("refused");
        assert!(!format!("{error:?} {error}").contains("nsec1notakey"));
    }

    /// Adoption's writes land where the host reads.
    #[test]
    fn adoption_writes_what_the_host_reads() {
        use coder_service::adopt::Keychain as _;
        let mut source = KeychainKeySource::new(MemoryStore::default());
        source.write(HOST_KEY_ACCOUNT, &"cd".repeat(32)).unwrap();
        let key = source.load(KeyName::Host).unwrap().expect("a key");
        assert_eq!(key.expose(), &[0xcd; 32]);
    }
}
