//! Where this computer's vault keys come from.
//!
//! - The device key: 32 random bytes in the OS keychain (the login
//!   keychain on macOS, Secret Service on Linux, Credential Manager on
//!   Windows), under the desktop app's service `com.openagents.desktop`
//!   and the account `openagents.vault.<vault id>`. That account name is
//!   the device slot's `key_ref`. Behind [`DeviceKeys`] so tests use
//!   memory and never touch a real keychain.
//! - The person's Nostr key, for a `nostr` slot: a file named with
//!   `--nostr-key`, else the owner key the desktop app keeps in the
//!   keychain (`owner-key`), else the older `coder-owner/owner.key`.

use std::path::{Path, PathBuf};

use secp256k1::{Keypair, Secp256k1, SecretKey};
use zeroize::Zeroizing;

/// The keychain account (and slot `key_ref`) of a vault's device key.
pub(crate) fn key_ref(vault: &str) -> String {
    format!("openagents.vault.{vault}")
}

/// This operating system's word in a device slot.
pub(crate) const fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(windows) {
        "windows"
    } else {
        "linux"
    }
}

pub(crate) trait DeviceKeys {
    fn load(&self, key_ref: &str) -> Result<Option<Zeroizing<[u8; 32]>>, String>;
    fn store(&self, key_ref: &str, secret: &[u8; 32]) -> Result<(), String>;
    fn delete(&self, key_ref: &str) -> Result<(), String>;
}

/// The OS keychain.
pub(crate) struct Keychain(std::sync::Arc<dyn coder_host::serve::keys::AccountKeys>);

impl Keychain {
    pub(crate) fn open() -> Result<Self, String> {
        coder_host::serve::keys::platform_accounts()
            .map(Self)
            .map_err(|_| "This computer has no keychain this program can use.".to_owned())
    }
}

impl DeviceKeys for Keychain {
    fn load(&self, key_ref: &str) -> Result<Option<Zeroizing<[u8; 32]>>, String> {
        self.0
            .load_account(key_ref)
            .map(|secret| secret.map(|secret| Zeroizing::new(*secret.expose())))
            .map_err(|_| "The keychain couldn't be read. Unlock it and try again.".to_owned())
    }

    fn store(&self, key_ref: &str, secret: &[u8; 32]) -> Result<(), String> {
        self.0
            .store_account(
                key_ref,
                &coder_host::serve::keys::Secret::from_bytes(*secret),
            )
            .map_err(|_| "The keychain refused the key. Unlock it and try again.".to_owned())
    }

    fn delete(&self, key_ref: &str) -> Result<(), String> {
        self.0
            .delete_account(key_ref)
            .map_err(|_| "The keychain refused to remove the key.".to_owned())
    }
}

/// A Nostr secret key and where it was found.
pub(crate) struct NostrKey {
    pub secret: SecretKey,
    pub from: String,
}

impl NostrKey {
    /// The x-only public key, as 64 lowercase hex characters.
    pub(crate) fn pubkey(&self) -> String {
        let pair = Keypair::from_secret_key(&Secp256k1::new(), &self.secret);
        oa_vault::hex(&pair.x_only_public_key().0.serialize())
    }

    pub(crate) fn x_only(&self) -> secp256k1::XOnlyPublicKey {
        Keypair::from_secret_key(&Secp256k1::new(), &self.secret)
            .x_only_public_key()
            .0
    }
}

/// A key from text: `nsec1…` or 64 lowercase hex characters.
pub(crate) fn parse_key(text: &str) -> Option<SecretKey> {
    let text = text.trim();
    let bytes = if text.starts_with("nsec1") {
        nostr::nip19::decode_nsec(text).ok()?
    } else {
        coder_host::serve::keys::parse_hex(text)?
    };
    SecretKey::from_byte_array(bytes).ok()
}

/// The person's Nostr key: the file named, else the desktop app's owner
/// key in the keychain, else the older owner key file under `home`.
pub(crate) fn nostr_key(file: Option<&Path>, home: &Path) -> Result<NostrKey, String> {
    if let Some(file) = file {
        let text = Zeroizing::new(
            std::fs::read_to_string(file)
                .map_err(|error| format!("{}: {error}", file.display()))?,
        );
        let secret = parse_key(&text)
            .ok_or_else(|| format!("{} doesn't hold a Nostr key.", file.display()))?;
        return Ok(NostrKey {
            secret,
            from: file.display().to_string(),
        });
    }
    if let Ok(keys) = coder_host::cli::keychain_keys()
        && let Ok(Some(secret)) = coder_host::serve::keys::held_owner(keys.0.as_ref())
    {
        return Ok(NostrKey {
            secret,
            from: "the OpenAgents app's key in your keychain".into(),
        });
    }
    let legacy: PathBuf = home.join(".openagents/coder-owner/owner.key");
    if let Ok(text) = std::fs::read_to_string(&legacy) {
        let text = Zeroizing::new(text);
        if let Some(secret) = parse_key(&text) {
            return Ok(NostrKey {
                secret,
                from: legacy.display().to_string(),
            });
        }
    }
    Err("No Nostr key was found on this computer. Pass one with --nostr-key FILE.".into())
}

/// Seal a `nostr` slot's secret to the key's own public key: the NIP-44 v2
/// payload of its 64 hex characters.
pub(crate) fn seal_to_self(key: &NostrKey, secret: &[u8; 32]) -> Result<String, String> {
    let conversation = Zeroizing::new(nostr::nip44::conversation_key(&key.secret, &key.x_only()));
    let text = Zeroizing::new(oa_vault::hex(secret));
    let nonce = oa_vault::random::<32>().map_err(|error| error.to_string())?;
    nostr::nip44::encrypt(&text, &conversation, nonce)
}

/// Open a `nostr` slot's `sealed` value with the key.
pub(crate) fn open_from_self(key: &NostrKey, sealed: &str) -> Result<Zeroizing<[u8; 32]>, String> {
    let conversation = Zeroizing::new(nostr::nip44::conversation_key(&key.secret, &key.x_only()));
    let text = Zeroizing::new(
        nostr::nip44::decrypt(sealed, &conversation)
            .map_err(|_| "Your Nostr key doesn't open this vault.".to_owned())?,
    );
    oa_vault::unhex::<32>(&text)
        .map(Zeroizing::new)
        .map_err(|_| "Your Nostr key doesn't open this vault.".to_owned())
}
