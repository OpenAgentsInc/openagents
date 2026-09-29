//! Where a served host's secret keys live.
//!
//! A CLI-only install keeps the host key in `host.key` in the access store,
//! as before, and the iroh key in a `0600` file under
//! `~/.openagents/connect` ([`FileKeySource`]). A host the desktop app runs
//! keeps the owner, host, and iroh keys in the login keychain
//! ([`Keychain`] on macOS, [`SecretService`] on Linux), read only by the
//! host process: never a file, an argument, or a log line.

use std::sync::Arc;

pub use openagents_connect::keys::{FileKeySource, KeyName, KeySource, Secret};
use secp256k1::SecretKey;

/// The keychain service the desktop app's secrets live under.
pub const KEYCHAIN_SERVICE: &str = "com.openagents.desktop";

/// The keychain account for each key the host holds, 64 lowercase hex
/// characters each. The host and owner names are the ones the desktop
/// app's adoption of an older host writes (`coder_service::adopt`).
#[must_use]
pub const fn keychain_account(name: KeyName) -> Option<&'static str> {
    match name {
        KeyName::Owner => Some("owner-key"),
        KeyName::Host => Some("host-key"),
        KeyName::HostIroh => Some("host-iroh-key"),
        KeyName::Device | KeyName::DeviceIroh => None,
    }
}

/// A key source as the host's configuration carries it.
#[derive(Clone)]
pub struct Keys(pub Arc<dyn KeySource>);

impl std::fmt::Debug for Keys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Keys(..)")
    }
}

/// The host key from a [`KeySource`], as `coder-access` reads it.
pub(crate) struct HostKey(pub(crate) Arc<dyn KeySource>);

impl coder_access::host::KeySource for HostKey {
    fn load(&self) -> coder_access::Result<Option<SecretKey>> {
        let secret = self.0.load(KeyName::Host).map_err(unavailable)?;
        secret
            .map(|secret| {
                SecretKey::from_byte_array(*secret.expose()).map_err(|_| {
                    coder_access::Error::new(
                        coder_access::Code::Malformed,
                        "the stored host key is not a key",
                    )
                })
            })
            .transpose()
    }

    fn store(&self, key: &SecretKey) -> coder_access::Result<()> {
        self.0
            .store(KeyName::Host, &Secret::from_bytes(key.secret_bytes()))
            .map_err(unavailable)
    }
}

fn unavailable(error: openagents_connect::Error) -> coder_access::Error {
    coder_access::Error::new(
        coder_access::Code::Unavailable,
        format!("the key source refused: {}", error.code.as_str()),
    )
}

/// The owner key from the key source, created on first use: the host of a
/// desktop app establishes its own owner, so there is no owner step.
///
/// # Errors
/// Refuses a key source that cannot be read or written.
pub fn owner(source: &dyn KeySource) -> openagents_connect::Result<SecretKey> {
    loop {
        let secret =
            openagents_connect::keys::load_or_create(source, KeyName::Owner, Secret::random)?;
        match SecretKey::from_byte_array(*secret.expose()) {
            Ok(key) => return Ok(key),
            // A random value outside the curve order; vanishingly rare.
            Err(_) => source.delete(KeyName::Owner)?,
        }
    }
}

/// The login keychain, under [`KEYCHAIN_SERVICE`], or another keychain a
/// test opens.
///
/// An item is readable without a prompt by the program that created it.
/// The host process creates the keys it holds, and `coder host adopt`
/// moves an older host's keys in from the same program, so the host reads
/// them silently; a key another program wrote asks once unless that
/// program granted the host access.
#[cfg(target_os = "macos")]
#[derive(Clone, Default)]
pub struct Keychain {
    /// `None` is the user's default keychain.
    keychain: Option<security_framework::os::macos::keychain::SecKeychain>,
}

#[cfg(target_os = "macos")]
impl std::fmt::Debug for Keychain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Keychain(..)")
    }
}

#[cfg(target_os = "macos")]
impl Keychain {
    /// The keys in `keychain` rather than the user's default keychain.
    #[must_use]
    pub fn in_keychain(keychain: security_framework::os::macos::keychain::SecKeychain) -> Self {
        Self {
            keychain: Some(keychain),
        }
    }

    fn find(
        &self,
        account: &str,
    ) -> std::result::Result<
        Option<(
            Vec<u8>,
            security_framework::os::macos::keychain_item::SecKeychainItem,
        )>,
        (),
    > {
        // errSecItemNotFound.
        const NOT_FOUND: i32 = -25_300;
        let keychains = self.keychain.clone().map(|k| vec![k]);
        match security_framework::os::macos::passwords::find_generic_password(
            keychains.as_deref(),
            KEYCHAIN_SERVICE,
            account,
        ) {
            Ok((password, item)) => Ok(Some((password.to_vec(), item))),
            Err(error) if error.code() == NOT_FOUND => Ok(None),
            Err(_) => Err(()),
        }
    }
}

#[cfg(target_os = "macos")]
impl KeySource for Keychain {
    fn load(&self, name: KeyName) -> openagents_connect::Result<Option<Secret>> {
        use openagents_connect::{Code, Error};
        let account = account(name)?;
        let Some((bytes, _)) = self
            .find(account)
            .map_err(|()| Error::new(Code::Unavailable, "read the keychain"))?
        else {
            return Ok(None);
        };
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| Error::new(Code::Malformed, "keychain item is not a key"))?;
        parse_hex(text.trim_end())
            .map(|bytes| Some(Secret::from_bytes(bytes)))
            .ok_or_else(|| Error::new(Code::Malformed, "keychain item is not a key"))
    }

    fn store(&self, name: KeyName, secret: &Secret) -> openagents_connect::Result<()> {
        use security_framework::os::macos::keychain::SecKeychain;
        let unavailable = |_| {
            openagents_connect::Error::new(
                openagents_connect::Code::Unavailable,
                "write the keychain",
            )
        };
        let account = account(name)?;
        let text: String = secret.expose().iter().map(|b| format!("{b:02x}")).collect();
        let keychain = match &self.keychain {
            Some(keychain) => keychain.clone(),
            None => SecKeychain::default().map_err(unavailable)?,
        };
        keychain
            .set_generic_password(KEYCHAIN_SERVICE, account, text.as_bytes())
            .map_err(unavailable)
    }

    fn delete(&self, name: KeyName) -> openagents_connect::Result<()> {
        let account = account(name)?;
        match self.find(account) {
            Ok(Some((_, item))) => {
                item.delete();
                Ok(())
            }
            Ok(None) => Ok(()),
            Err(()) => Err(openagents_connect::Error::new(
                openagents_connect::Code::Unavailable,
                "write the keychain",
            )),
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn account(name: KeyName) -> openagents_connect::Result<&'static str> {
    keychain_account(name).ok_or_else(|| {
        openagents_connect::Error::new(
            openagents_connect::Code::Unavailable,
            "the host keeps no device key",
        )
    })
}

/// The Linux keychain: the freedesktop Secret Service on the session bus
/// (GNOME Keyring, KWallet, KeePassXC), under [`KEYCHAIN_SERVICE`] and the
/// same accounts and hex values as the macOS keychain. The Secret Service
/// keeps items encrypted with the login keyring's password and unlocks it at
/// login. With no Secret Service on the bus every call refuses as
/// `unavailable`; the host never falls back to a file for these keys.
#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, Default)]
pub struct SecretService;

#[cfg(target_os = "linux")]
impl SecretService {
    fn entry(name: KeyName) -> openagents_connect::Result<keyring::Entry> {
        keyring::Entry::new(KEYCHAIN_SERVICE, account(name)?).map_err(|_| {
            openagents_connect::Error::new(
                openagents_connect::Code::Unavailable,
                "no Secret Service keyring answers on the session bus",
            )
        })
    }
}

#[cfg(target_os = "linux")]
impl KeySource for SecretService {
    fn load(&self, name: KeyName) -> openagents_connect::Result<Option<Secret>> {
        use openagents_connect::{Code, Error};
        match Self::entry(name)?.get_password() {
            Ok(text) => parse_hex(text.trim_end())
                .map(|bytes| Some(Secret::from_bytes(bytes)))
                .ok_or_else(|| Error::new(Code::Malformed, "keychain item is not a key")),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(Error::new(Code::Unavailable, "read the keychain")),
        }
    }

    fn store(&self, name: KeyName, secret: &Secret) -> openagents_connect::Result<()> {
        let text: String = secret.expose().iter().map(|b| format!("{b:02x}")).collect();
        Self::entry(name)?.set_password(&text).map_err(|_| {
            openagents_connect::Error::new(
                openagents_connect::Code::Unavailable,
                "write the keychain",
            )
        })
    }

    fn delete(&self, name: KeyName) -> openagents_connect::Result<()> {
        match Self::entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(openagents_connect::Error::new(
                openagents_connect::Code::Unavailable,
                "write the keychain",
            )),
        }
    }
}

/// A key source seen as the keychain that adopting an older host writes
/// (`coder_service::adopt`): items by account name, values 64 lowercase
/// hex characters.
pub struct AdoptInto<'a>(pub &'a dyn KeySource);

impl coder_service::adopt::Keychain for AdoptInto<'_> {
    fn read(&mut self, account: &str) -> coder_service::Result<Option<String>> {
        let name = named(account)?;
        let secret = self
            .0
            .load(name)
            .map_err(|_| coder_service::Error::Refused("the keychain cannot be read".into()))?;
        Ok(secret.map(|secret| secret.expose().iter().map(|b| format!("{b:02x}")).collect()))
    }

    fn write(&mut self, account: &str, value: &str) -> coder_service::Result<()> {
        let name = named(account)?;
        let bytes = parse_hex(value.trim()).ok_or_else(|| {
            coder_service::Error::Refused("the key to keep is not 64 hex characters".into())
        })?;
        self.0
            .store(name, &Secret::from_bytes(bytes))
            .map_err(|_| coder_service::Error::Refused("the keychain cannot be written".into()))
    }
}

/// The key an account names.
fn named(account: &str) -> coder_service::Result<KeyName> {
    [KeyName::Owner, KeyName::Host, KeyName::HostIroh]
        .into_iter()
        .find(|name| keychain_account(*name) == Some(account))
        .ok_or_else(|| coder_service::Error::Refused(format!("no key is kept as `{account}`")))
}

/// 64 lowercase hex characters as 32 bytes.
#[must_use]
pub fn parse_hex(text: &str) -> Option<[u8; 32]> {
    let bytes = text.as_bytes();
    if bytes.len() != 64 {
        return None;
    }
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };
    let mut out = [0; 32];
    for (index, pair) in bytes.chunks_exact(2).enumerate() {
        out[index] = digit(pair[0])? << 4 | digit(pair[1])?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keychain_accounts_match_the_adoption_step_and_name_no_device_key() {
        assert_eq!(keychain_account(KeyName::Host), Some("host-key"));
        assert_eq!(keychain_account(KeyName::Owner), Some("owner-key"));
        assert_eq!(keychain_account(KeyName::HostIroh), Some("host-iroh-key"));
        assert_eq!(keychain_account(KeyName::Device), None);
        assert_eq!(parse_hex(&"0a".repeat(32)), Some([10; 32]));
        assert_eq!(parse_hex(&"0A".repeat(32)), None);
    }

    #[test]
    fn the_host_key_through_a_file_source_round_trips_and_the_owner_is_made_once() {
        let dir = tempfile::tempdir().unwrap();
        let source: Arc<dyn KeySource> = Arc::new(FileKeySource::new(dir.path().join("keys")));
        let host = HostKey(source.clone());
        use coder_access::host::KeySource as _;
        assert!(host.load().unwrap().is_none());
        let key = SecretKey::new(&mut secp256k1::rand::rng());
        host.store(&key).unwrap();
        assert_eq!(host.load().unwrap(), Some(key));
        let first = owner(source.as_ref()).unwrap();
        assert_eq!(owner(source.as_ref()).unwrap(), first);
    }

    /// The real Secret Service: every key the host keeps round-trips as the
    /// same hex item the macOS keychain holds. Run inside `dbus-run-session`
    /// with an unlocked `gnome-keyring-daemon --components=secrets`:
    /// `cargo test -p coder-host --lib -- --ignored secret_service`.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "needs a Secret Service on the session bus; writes real keyring items"]
    fn the_secret_service_keeps_the_host_keys_as_hex() {
        let source = SecretService;
        for name in [KeyName::Owner, KeyName::Host, KeyName::HostIroh] {
            assert_eq!(source.load(name).unwrap(), None, "{} exists", name.as_str());
        }
        let host = HostKey(Arc::new(source));
        use coder_access::host::KeySource as _;
        let key = SecretKey::new(&mut secp256k1::rand::rng());
        host.store(&key).unwrap();
        assert_eq!(host.load().unwrap(), Some(key));
        let entry = keyring::Entry::new(KEYCHAIN_SERVICE, "host-key").unwrap();
        assert_eq!(entry.get_password().unwrap(), hex_of(&key.secret_bytes()));
        let first = owner(&source).unwrap();
        assert_eq!(owner(&source).unwrap(), first);
        for name in [KeyName::Owner, KeyName::Host, KeyName::HostIroh] {
            source.delete(name).unwrap();
            assert_eq!(source.load(name).unwrap(), None);
        }
        assert!(source.load(KeyName::Device).is_err());
    }

    #[cfg(target_os = "linux")]
    fn hex_of(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}
