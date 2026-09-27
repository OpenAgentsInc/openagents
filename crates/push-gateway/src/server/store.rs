//! Durable gateway state in one file.
//!
//! The state file holds installations, delivery capabilities, and finished
//! delivery outcomes. Native tokens are sealed with AES-256-GCM under the
//! operator's state key, with the installation handle as associated data, so
//! the file alone discloses no token. Capabilities are stored as HMAC
//! digests, never as the bearer string. Every write replaces the file
//! atomically: write a sibling, flush it to disk, and rename it over.

use std::{collections::BTreeMap, io::Write as _, path::PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ring::{
    aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
    hmac,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};

use crate::wire::Transport;

const STATE_FILE: &str = "state.json";
const STATE_VERSION: u32 = 1;
/// How long a lapsed installation's record stays for its watermarks.
pub const LAPSED_RETENTION_SECONDS: u64 = 86_400;

/// Everything the gateway persists.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Data {
    /// File format version.
    pub version: u32,
    /// Installations by handle.
    pub installations: BTreeMap<String, Installation>,
    /// Delivery capabilities by the HMAC of the grant string.
    pub delegations: BTreeMap<String, Delegation>,
    /// Terminal delivery outcomes by `relay_pubkey:request_id`.
    pub finished: BTreeMap<String, Finished>,
}

/// One native token held for one owner key.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Installation {
    /// The Nostr key that registered it and alone may change it.
    pub owner: String,
    /// The profile it was registered under.
    pub app_profile: String,
    /// The transport of that profile.
    pub transport: Transport,
    /// Rotation counter.
    pub endpoint_epoch: u64,
    /// Sealed native token; empty once revoked.
    pub token_sealed: String,
    /// Keyed digest of `(app_profile, token)` for uniqueness.
    pub token_digest: String,
    /// When the installation lapses.
    pub expires_at: u64,
    /// Set by an installation revocation.
    pub revoked: bool,
    /// Set when a provider reports the token permanently invalid.
    pub invalid_at: Option<u64>,
    /// Highest delegation generation per relay.
    pub generations: BTreeMap<String, u64>,
    /// Start of the current delivery quota window.
    pub window_start: u64,
    /// Deliveries admitted in the current window.
    pub window_count: u32,
}

impl Installation {
    /// Whether the installation can still receive or change.
    #[must_use]
    pub fn live(&self, now: u64) -> bool {
        !self.revoked && self.expires_at > now
    }
}

/// One delivery capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delegation {
    /// The installation it reaches.
    pub installation_handle: String,
    /// The relay key that alone may present it.
    pub relay_pubkey: String,
    /// Its delegation generation.
    pub generation: u64,
    /// The installation epoch it was issued under.
    pub endpoint_epoch: u64,
    /// Not valid before.
    pub not_before: u64,
    /// Not valid after.
    pub expires_at: u64,
}

/// A terminal delivery outcome kept for idempotent replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finished {
    /// HTTP status returned.
    pub status: u16,
    /// Body returned.
    pub body: serde_json::Value,
    /// When the record may be dropped.
    pub retain_until: u64,
}

/// The state file, its keys, and the in-memory copy.
pub struct Store {
    path: PathBuf,
    seal_key: LessSafeKey,
    digest_key: hmac::Key,
    rng: SystemRandom,
    /// The current state.
    pub data: Data,
}

impl Store {
    /// Open or create the state in `dir` under the 32-byte `key`.
    ///
    /// # Errors
    ///
    /// Returns a reason when the directory or file cannot be read or parsed.
    pub fn open(dir: &std::path::Path, key: &[u8; 32]) -> Result<Self, String> {
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("cannot create the state directory: {error}"))?;
        let path = dir.join(STATE_FILE);
        let data = match std::fs::read(&path) {
            Ok(bytes) => {
                let data: Data = serde_json::from_slice(&bytes)
                    .map_err(|error| format!("the state file is not valid: {error}"))?;
                if data.version != STATE_VERSION {
                    return Err(format!(
                        "the state file has version {}, expected {STATE_VERSION}",
                        data.version
                    ));
                }
                data
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Data {
                version: STATE_VERSION,
                ..Data::default()
            },
            Err(error) => return Err(format!("cannot read the state file: {error}")),
        };
        let seal_key = LessSafeKey::new(
            UnboundKey::new(&AES_256_GCM, key).map_err(|_| "the state key is not valid")?,
        );
        // Separate keys for sealing and digests, both derived from the one
        // operator key.
        let digest_key = hmac::Key::new(
            hmac::HMAC_SHA256,
            hmac::sign(
                &hmac::Key::new(hmac::HMAC_SHA256, key),
                b"push-gateway digest",
            )
            .as_ref(),
        );
        Ok(Self {
            path,
            seal_key,
            digest_key,
            rng: SystemRandom::new(),
            data,
        })
    }

    /// Write the state atomically.
    ///
    /// # Errors
    ///
    /// Returns a reason when the file cannot be written.
    pub fn save(&self) -> Result<(), String> {
        let bytes = serde_json::to_vec(&self.data).map_err(|error| error.to_string())?;
        let temporary = self.path.with_extension("json.tmp");
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|error| format!("cannot write the state file: {error}"))?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("cannot write the state file: {error}"))?;
        std::fs::rename(&temporary, &self.path)
            .map_err(|error| format!("cannot replace the state file: {error}"))?;
        if let Some(parent) = self.path.parent()
            && let Ok(directory) = std::fs::File::open(parent)
        {
            let _ = directory.sync_all();
        }
        Ok(())
    }

    /// Seal `token` for `handle`.
    ///
    /// # Errors
    ///
    /// Returns a reason when randomness or sealing fails.
    pub fn seal(&self, handle: &str, token: &str) -> Result<String, String> {
        let mut nonce = [0_u8; NONCE_LEN];
        self.rng
            .fill(&mut nonce)
            .map_err(|_| "randomness is unavailable")?;
        let mut sealed = token.as_bytes().to_vec();
        self.seal_key
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(handle.as_bytes()),
                &mut sealed,
            )
            .map_err(|_| "sealing failed")?;
        let mut out = nonce.to_vec();
        out.extend_from_slice(&sealed);
        Ok(STANDARD.encode(out))
    }

    /// Open a token sealed for `handle`.
    #[must_use]
    pub fn unseal(&self, handle: &str, sealed: &str) -> Option<String> {
        let bytes = STANDARD.decode(sealed).ok()?;
        if bytes.len() < NONCE_LEN {
            return None;
        }
        let (nonce, rest) = bytes.split_at(NONCE_LEN);
        let mut rest = rest.to_vec();
        let plain = self
            .seal_key
            .open_in_place(
                Nonce::try_assume_unique_for_key(nonce).ok()?,
                Aad::from(handle.as_bytes()),
                &mut rest,
            )
            .ok()?;
        String::from_utf8(plain.to_vec()).ok()
    }

    /// The keyed digest of a value.
    #[must_use]
    pub fn digest(&self, parts: &[&str]) -> String {
        let mut context = hmac::Context::with_key(&self.digest_key);
        for part in parts {
            context.update(&(part.len() as u64).to_be_bytes());
            context.update(part.as_bytes());
        }
        crate::hex(context.sign().as_ref())
    }

    /// Fill `bytes` with randomness.
    ///
    /// # Errors
    ///
    /// Returns a reason when randomness is unavailable.
    pub fn random(&self, bytes: &mut [u8]) -> Result<(), String> {
        self.rng
            .fill(bytes)
            .map_err(|_| "randomness is unavailable".to_owned())
    }

    /// Drop lapsed installations, expired capabilities, and old outcomes.
    pub fn prune(&mut self, now: u64) {
        self.data
            .finished
            .retain(|_, finished| finished.retain_until > now);
        self.data.installations.retain(|_, installation| {
            installation
                .expires_at
                .saturating_add(LAPSED_RETENTION_SECONDS)
                > now
        });
        let installations = &self.data.installations;
        self.data.delegations.retain(|_, delegation| {
            delegation.expires_at > now
                && installations
                    .get(&delegation.installation_handle)
                    .is_some_and(|installation| installation.live(now))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_sealed_bound_to_their_handle_and_the_file_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let key = [7_u8; 32];
        let mut store = Store::open(dir.path(), &key).unwrap();
        let sealed = store.seal("handle-a", "device-token-material").unwrap();
        assert!(!sealed.contains("device-token-material"));
        assert_eq!(
            store.unseal("handle-a", &sealed).as_deref(),
            Some("device-token-material")
        );
        assert_eq!(store.unseal("handle-b", &sealed), None);
        assert_ne!(store.digest(&["a", "bc"]), store.digest(&["ab", "c"]));
        store.data.installations.insert(
            "handle-a".into(),
            Installation {
                owner: "o".into(),
                app_profile: "p".into(),
                transport: Transport::Apns,
                endpoint_epoch: 1,
                token_sealed: sealed,
                token_digest: store.digest(&["p", "device-token-material"]),
                expires_at: 100,
                revoked: false,
                invalid_at: None,
                generations: BTreeMap::new(),
                window_start: 0,
                window_count: 0,
            },
        );
        store.save().unwrap();
        let text = std::fs::read_to_string(dir.path().join(STATE_FILE)).unwrap();
        assert!(!text.contains("device-token-material"));
        let reopened = Store::open(dir.path(), &key).unwrap();
        assert_eq!(reopened.data.installations.len(), 1);
        let wrong = Store::open(dir.path(), &[8_u8; 32]).unwrap();
        let installation = &wrong.data.installations["handle-a"];
        assert_eq!(wrong.unseal("handle-a", &installation.token_sealed), None);
        let mut pruned = reopened;
        pruned.prune(100 + LAPSED_RETENTION_SECONDS);
        assert!(pruned.data.installations.is_empty());
    }
}
