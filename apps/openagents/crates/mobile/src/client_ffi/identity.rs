//! The phone's identity (#11995): 12 words made on first launch, the Nostr
//! key derived from them, and the silent sign-in with that key. The words,
//! the key and the rules live in `openagents_account::identity`; this is the
//! phone's surface over them: start, Your ID, back up, confirm, restore,
//! and the iCloud Keychain copy.
//!
//! The app gates [`CoreAccount::backup_words`] and
//! [`CoreAccount::reveal_nsec`] behind its own device check (Face ID or the
//! passcode) before calling them, and never logs what they return.
use super::{CoreAccount, CoreError, CoreResult};
use openagents_account::{
    identity::{IdentityError, IdentityVault, WordAnswer},
    openagents::StorageError,
};
use std::sync::Arc;

/// The person's own account identity, for Settings' "Your ID" row.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct IdentityView {
    /// The public key as `npub1…`.
    pub npub: String,
    /// Whether the 12 words were backed up and confirmed.
    pub words_saved: bool,
    /// Whether the words are kept in iCloud Keychain rather than on this
    /// phone only.
    pub icloud_copy: bool,
}

/// One answer in the backup check: the word at a 1-based position.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct WordCheck {
    pub position: u32,
    pub word: String,
}

/// What the phone's App Attest step made: base64 of the key id and of the
/// attestation.
#[derive(uniffi::Record, Clone)]
pub struct AppAttestation {
    pub key_id: String,
    pub attestation: String,
}

/// The phone's App Attest (#11994), implemented in Swift over
/// `DCAppAttestService`: make or reuse this install's key (its id kept in
/// the Keychain) and attest it over `client_data_hash`. Called on a
/// background thread; it may wait for the system. Answer `None` when App
/// Attest isn't supported, as in the simulator, and the core sends a proof
/// of work instead.
#[uniffi::export(with_foreign)]
pub trait AppAttester: Send + Sync {
    fn attest(&self, client_data_hash: Vec<u8>) -> Option<AppAttestation>;
}

struct Bridge(Arc<dyn AppAttester>);
impl openagents_account::Attester for Bridge {
    fn attest(&self, client_data_hash: Vec<u8>) -> Option<openagents_account::Attestation> {
        self.0
            .attest(client_data_hash)
            .map(|a| openagents_account::Attestation {
                key_id: a.key_id,
                attestation: a.attestation,
            })
    }
}

/// An identity store that is never there: accounts made for tests use it,
/// so they never reach the real Keychain.
pub(crate) struct NoVault;
impl IdentityVault for NoVault {
    fn load(&self) -> Result<Option<(Vec<u8>, bool)>, StorageError> {
        Err(StorageError::Unavailable)
    }
    fn store(&self, _: &[u8], _: bool) -> Result<(), StorageError> {
        Err(StorageError::Unavailable)
    }
}

fn error(e: IdentityError) -> CoreError {
    match e {
        IdentityError::Words(_) => CoreError::InvalidArgument {
            message: e.message(),
        },
        _ => CoreError::Storage {
            message: e.message(),
        },
    }
}

const NOT_READY: &str = "Your account isn't ready yet. Try again in a moment.";

fn no_identity() -> CoreError {
    CoreError::Storage {
        message: NOT_READY.into(),
    }
}

impl CoreAccount {
    fn loaded(&self) -> CoreResult<Arc<openagents_account::identity::Identity>> {
        self.identity
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(no_identity)
    }
    fn view(&self, identity: &openagents_account::identity::Identity) -> IdentityView {
        IdentityView {
            npub: identity.npub(),
            words_saved: self.identities.words_saved(identity),
            icloud_copy: self.identities.icloud_copy(),
        }
    }
}

#[uniffi::export]
impl CoreAccount {
    /// Use App Attest for this phone's first sign-in. Set it before
    /// `start_identity`.
    pub fn set_app_attester(&self, attester: Arc<dyn AppAttester>) {
        self.controller().set_attester(Arc::new(Bridge(attester)));
    }

    /// Starts this phone's account on launch, with nothing for the person
    /// to do: open the 12 words (or reuse the wallet's, or make new ones),
    /// then sign in with the key. A phone that was signed in before keeps
    /// its account; the new key is added to it.
    pub fn start_identity(&self, device_name: String) {
        if !device_name.trim().is_empty() {
            *self.device_name.lock().unwrap() = device_name.clone();
        }
        let device_name = self.device_name.lock().unwrap().clone();
        let identities = self.identities.clone();
        let slot = self.identity.clone();
        let controller = self.controller().clone();
        self.close_clients();
        openagents_client::runtime::shared().spawn(async move {
            let loaded = tokio::task::spawn_blocking(move || identities.load_or_create())
                .await
                .unwrap_or(Err(IdentityError::Locked));
            match loaded {
                Ok(identity) => {
                    let identity = Arc::new(identity);
                    *slot.lock().unwrap() = Some(identity.clone());
                    controller.start_with_key(identity, device_name).await;
                }
                Err(e) => controller.identity_failed(e.message()),
            }
        });
    }

    /// The account's identity, or `None` until it is loaded on launch.
    pub fn identity(&self) -> Option<IdentityView> {
        let identity = self.identity.lock().unwrap().clone()?;
        Some(self.view(&identity))
    }

    /// The 12 words, in order, for Settings > Back up your account. Call
    /// only after the person passed the app's Face ID or passcode check.
    pub fn backup_words(&self) -> CoreResult<Vec<String>> {
        Ok(self.loaded()?.words())
    }

    /// Three word positions (1-based) to ask for after the person wrote the
    /// words down.
    pub fn backup_check(&self) -> CoreResult<Vec<u32>> {
        let identity = self.loaded()?;
        Ok(self.identities.backup_check(&identity))
    }

    /// Check the backup answers; when every one matches, the words are
    /// recorded as saved and Settings stops asking.
    pub fn confirm_backup(&self, answers: Vec<WordCheck>) -> CoreResult<bool> {
        let answers: Vec<WordAnswer> = answers
            .into_iter()
            .map(|a| WordAnswer {
                position: a.position,
                word: a.word,
            })
            .collect();
        let identity = self.loaded()?;
        Ok(self.identities.confirm_backup(&identity, &answers))
    }

    /// Restore from 12 or 24 words, then sign in with their key, which
    /// brings back the account they belong to. Refused when this phone's
    /// own words aren't backed up yet, unless `replace_unsaved`.
    pub fn restore_from_words(
        &self,
        words: String,
        replace_unsaved: bool,
    ) -> CoreResult<IdentityView> {
        let identity = Arc::new(
            self.identities
                .restore(&words, replace_unsaved)
                .map_err(error)?,
        );
        *self.identity.lock().unwrap() = Some(identity.clone());
        let task = self
            .controller()
            .sign_in_with_key(identity.clone(), self.device_name.lock().unwrap().clone());
        self.close_clients();
        openagents_client::runtime::shared().spawn(task);
        Ok(self.view(&identity))
    }

    /// Keep the words in iCloud Keychain (`true`), so a new iPhone on the
    /// same Apple account gets them back, or on this phone only (`false`,
    /// the default).
    pub fn set_icloud_copy(&self, on: bool) -> CoreResult<()> {
        self.identities.set_icloud_copy(on).map_err(error)
    }

    /// The key alone as `nsec1…`, for other Nostr apps. It can't recover
    /// the wallet; the words recover both. Gate it as `backup_words`.
    pub fn reveal_nsec(&self) -> CoreResult<String> {
        Ok(self.loaded()?.nsec())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_phone_copy_reads_plainly() {
        assert!(oa_copy::violations(super::NOT_READY, &[]).is_empty());
    }
}
