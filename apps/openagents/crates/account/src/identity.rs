//! The person's identity: 12 recovery words, made on first launch, and the
//! Nostr key NIP-06 derives from them (#11995).
//!
//! The words are the account. The key signs in silently
//! ([`crate::Controller::start_with_key`]); the same words open the wallet;
//! and restoring the words on another device brings the account back.
//!
//! Where the words come from, in order:
//!
//! 1. The identity's own Keychain item, once there is one.
//! 2. The wallet seed an earlier build stored (`com.openagents.app.spark`,
//!    `seed-v1`). When it exists, its words are the identity's words, so one
//!    backup covers both. That item is only read, never written or removed.
//! 3. New words, only when both are certainly absent. A store that can't be
//!    read now (a locked device) never leads to new words.
//!
//! The identity's item is on this device only by default; the person can
//! keep a copy in iCloud Keychain instead ([`Identities::set_icloud_copy`]).
//! Secret types here have no `Debug`.
use nostr::nip06::{self, Mnemonic};
use openagents::{EventTemplate, NostrSigner, SignerError, StorageError};
use secp256k1::Secp256k1;
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// A loaded identity: the words and the key derived from them.
pub struct Identity {
    mnemonic: Mnemonic,
    key: nip06::Identity,
}

impl Identity {
    /// The identity for checked BIP-39 entropy (16 or 32 bytes).
    pub fn from_entropy(entropy: &[u8]) -> Result<Self, IdentityError> {
        Self::from_mnemonic(Mnemonic::from_entropy(entropy).map_err(IdentityError::Words)?)
    }
    /// The identity for typed recovery words (12 or 24).
    pub fn from_words(words: &str) -> Result<Self, IdentityError> {
        Self::from_mnemonic(Mnemonic::parse(words).map_err(IdentityError::Words)?)
    }
    /// New 12 words.
    pub fn generate() -> Result<Self, IdentityError> {
        Self::from_mnemonic(Mnemonic::generate().map_err(IdentityError::Words)?)
    }
    fn from_mnemonic(mnemonic: Mnemonic) -> Result<Self, IdentityError> {
        let key = mnemonic.identity().map_err(IdentityError::Words)?;
        Ok(Self { mnemonic, key })
    }
    /// The public key as `npub1…`, the ID a person sees.
    pub fn npub(&self) -> String {
        self.key.npub()
    }
    /// The public key, 64 lowercase hex characters.
    pub fn pubkey_hex(&self) -> String {
        self.key.public_key_hex()
    }
    /// The recovery words, in order. Show them only after the person asked
    /// and passed the device's own check (Face ID or the passcode).
    pub fn words(&self) -> Vec<String> {
        self.mnemonic
            .words()
            .into_iter()
            .map(str::to_owned)
            .collect()
    }
    /// The secret key as `nsec1…`. It signs in to Nostr apps but can't
    /// recover the wallet; the words recover both.
    pub fn nsec(&self) -> String {
        self.key.nsec()
    }
    /// The words' BIP-39 entropy, for a wallet made from the same words.
    /// Keep it no longer than needed.
    pub fn entropy(&self) -> Vec<u8> {
        self.mnemonic.entropy()
    }
    /// Sign a NIP-01 event, answering it as JSON.
    pub fn sign(&self, template: &EventTemplate) -> String {
        let tags: Vec<nostr::domain::Tag> = template
            .tags
            .iter()
            .map(|t| nostr::domain::Tag(t.clone()))
            .collect();
        let mut event = nostr::domain::Event {
            id: String::new(),
            pubkey: self.pubkey_hex(),
            created_at: template.created_at,
            kind: template.kind,
            tags,
            content: template.content.clone(),
            sig: String::new(),
        };
        let id = event
            .computed_id_bytes()
            .expect("an owned event always serializes");
        event.id = hex(&id);
        event.sig = Secp256k1::signing_only()
            .sign_schnorr_no_aux_rand(&id, &self.key.keypair())
            .to_string();
        serde_json::to_string(&event).expect("an owned event always serializes")
    }
}

impl NostrSigner for Identity {
    fn sign_event(&self, template: &EventTemplate) -> Result<String, SignerError> {
        Ok(self.sign(template))
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Why the identity couldn't be opened, made, or restored. The messages are
/// for the person; none repeats the words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentityError {
    /// The device's key store can't be read now, usually because it is
    /// locked. Nothing new is made in its place.
    Locked,
    /// A wallet is saved on this device, but it can't be read. It is left
    /// alone, and no new words are made over it.
    WalletUnreadable,
    /// The words were refused.
    Words(nip06::Nip06Error),
    /// The key store refused to save the words.
    NotSaved,
    /// This device's wallet uses other words; restoring different words
    /// would split the account from the wallet.
    WalletDiffers,
    /// The account on this device isn't backed up yet, and restoring other
    /// words would lose it.
    Unsaved,
}

impl IdentityError {
    pub fn message(&self) -> String {
        match self {
            Self::Locked => "Unlock your device to open your account.".into(),
            Self::WalletUnreadable => {
                "The wallet saved on this device can't be read, so your account can't start. It has been left as it is.".into()
            }
            Self::Words(e) => e.to_string(),
            Self::NotSaved => "Couldn't save your account on this device. Try again.".into(),
            Self::WalletDiffers => {
                "This device's wallet uses different words. Restore those words instead.".into()
            }
            Self::Unsaved => {
                "Back up this device's 12 words first. Restoring other words replaces this account here.".into()
            }
        }
    }
}

impl std::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}
impl std::error::Error for IdentityError {}

/// Where the identity's words are kept. The Keychain implementation is
/// [`KeychainVault`]; tests use memory.
pub trait IdentityVault: Send + Sync {
    /// The stored entropy, and whether it is the iCloud Keychain copy.
    fn load(&self) -> Result<Option<(Vec<u8>, bool)>, StorageError>;
    /// Add the first identity without replacing a value created by a racer.
    /// Stores with an atomic insert should override this method.
    fn create_if_absent(&self, entropy: &[u8], icloud: bool) -> Result<bool, StorageError> {
        self.store(entropy, icloud)?;
        Ok(true)
    }
    /// Store `entropy` on this device only (`icloud == false`) or as an
    /// iCloud Keychain copy, then remove it from the other place.
    fn store(&self, entropy: &[u8], icloud: bool) -> Result<(), StorageError>;
}

/// What one read of the wallet seed found.
#[derive(Clone, PartialEq, Eq)]
pub enum WalletRead {
    /// The stored BIP-39 entropy (16 or 32 bytes).
    Found(Vec<u8>),
    /// The key store answered that no wallet is stored.
    Absent,
    /// The key store couldn't be read now.
    Unavailable,
    /// A value is stored but isn't a seed.
    Unreadable,
}

/// A read-only view of the wallet seed. It has no way to write.
pub trait WalletSeed: Send + Sync {
    fn read(&self) -> WalletRead;
}

/// No wallet on this device, as on the desktop today.
pub struct NoWallet;
impl WalletSeed for NoWallet {
    fn read(&self) -> WalletRead {
        WalletRead::Absent
    }
}

/// One answer in the backup check: the word at a 1-based position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WordAnswer {
    pub position: u32,
    pub word: String,
}

/// The identity's storage and the records beside it: whether the words
/// were confirmed, and the iCloud copy setting.
pub struct Identities {
    vault: Arc<dyn IdentityVault>,
    wallet: Arc<dyn WalletSeed>,
    dir: PathBuf,
}

impl Identities {
    /// `dir` holds the small non-secret records (the words-saved marker).
    pub fn new(vault: Arc<dyn IdentityVault>, wallet: Arc<dyn WalletSeed>, dir: &Path) -> Self {
        Self {
            vault,
            wallet,
            dir: dir.to_owned(),
        }
    }

    /// The phone's identity: its own Keychain item, and the earlier app's
    /// wallet seed read as the words when it exists.
    pub fn phone(data_dir: &Path) -> Self {
        Self::new(
            Arc::new(KeychainVault::new(PHONE_SERVICE)),
            Arc::new(KeychainWallet),
            &data_dir.join("identity"),
        )
    }

    /// Open this device's identity, or make one: see the module notes for
    /// the order. Blocking: call it off the main thread.
    pub fn load_or_create(&self) -> Result<Identity, IdentityError> {
        match self.vault.load() {
            Ok(Some((entropy, _))) => return Identity::from_entropy(&entropy),
            Ok(None) => {}
            Err(_) => return Err(IdentityError::Locked),
        }
        let identity = match self.wallet.read() {
            WalletRead::Found(entropy) => {
                Identity::from_entropy(&entropy).map_err(|_| IdentityError::WalletUnreadable)?
            }
            WalletRead::Absent => Identity::generate()?,
            WalletRead::Unavailable => return Err(IdentityError::Locked),
            WalletRead::Unreadable => return Err(IdentityError::WalletUnreadable),
        };
        if self.create_if_absent(&identity, false)? {
            return Ok(identity);
        }
        self.load()?.ok_or(IdentityError::NotSaved)
    }

    /// The stored identity, if there is one. Never makes one.
    pub fn load(&self) -> Result<Option<Identity>, IdentityError> {
        match self.vault.load() {
            Ok(Some((entropy, _))) => Identity::from_entropy(&entropy).map(Some),
            Ok(None) => Ok(None),
            Err(_) => Err(IdentityError::Locked),
        }
    }

    /// Replace this device's identity with typed words. Refused when the
    /// wallet here uses other words, and, unless `replace_unsaved`, when the
    /// current identity differs and wasn't backed up.
    pub fn restore(&self, words: &str, replace_unsaved: bool) -> Result<Identity, IdentityError> {
        let restored = Identity::from_words(words)?;
        match self.wallet.read() {
            WalletRead::Found(entropy) if entropy != restored.entropy() => {
                return Err(IdentityError::WalletDiffers);
            }
            WalletRead::Found(_) | WalletRead::Absent => {}
            WalletRead::Unavailable => return Err(IdentityError::Locked),
            WalletRead::Unreadable => return Err(IdentityError::WalletUnreadable),
        }
        let (current, icloud) = match self.vault.load() {
            Ok(Some((entropy, icloud))) => (Some(entropy), icloud),
            Ok(None) => (None, false),
            Err(_) => return Err(IdentityError::Locked),
        };
        if let Some(current) = &current {
            if *current == restored.entropy() {
                return Ok(restored);
            }
            let saved = Identity::from_entropy(current)
                .map(|c| self.words_saved(&c))
                .unwrap_or(false);
            if !saved && !replace_unsaved {
                return Err(IdentityError::Unsaved);
            }
        }
        self.save(&restored, icloud)?;
        // Typed words are backed up by definition.
        self.mark_saved(&restored);
        Ok(restored)
    }

    fn save(&self, identity: &Identity, icloud: bool) -> Result<(), IdentityError> {
        let mut entropy = identity.entropy();
        let stored = self.vault.store(&entropy, icloud);
        entropy.fill(0);
        stored.map_err(|_| IdentityError::NotSaved)
    }

    fn create_if_absent(&self, identity: &Identity, icloud: bool) -> Result<bool, IdentityError> {
        let mut entropy = identity.entropy();
        let stored = self.vault.create_if_absent(&entropy, icloud);
        entropy.fill(0);
        stored.map_err(|_| IdentityError::NotSaved)
    }

    /// Whether the person has an iCloud Keychain copy of the words.
    pub fn icloud_copy(&self) -> bool {
        matches!(self.vault.load(), Ok(Some((_, true))))
    }

    /// Keep the words in iCloud Keychain (`true`) or on this device only.
    pub fn set_icloud_copy(&self, on: bool) -> Result<(), IdentityError> {
        let Some((entropy, current)) = self.vault.load().map_err(|_| IdentityError::Locked)? else {
            return Err(IdentityError::NotSaved);
        };
        if current == on {
            return Ok(());
        }
        self.vault
            .store(&entropy, on)
            .map_err(|_| IdentityError::NotSaved)
    }

    /// Three different word positions (1-based) to ask for in the backup
    /// check.
    pub fn backup_check(&self, identity: &Identity) -> Vec<u32> {
        let count = identity.words().len() as u32;
        let mut seed = [0u8; 16];
        if getrandom::fill(&mut seed).is_err() {
            return vec![2, 5, count.min(11)];
        }
        let mut picked = Vec::new();
        let mut i = 0;
        while picked.len() < 3 {
            let digest = Sha256::digest([&seed[..], &[i][..]].concat());
            let position =
                (u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]) % count) + 1;
            if !picked.contains(&position) {
                picked.push(position);
            }
            i = i.wrapping_add(1);
        }
        picked.sort_unstable();
        picked
    }

    /// Check the person's answers; when every one matches, record that the
    /// words are saved.
    pub fn confirm_backup(&self, identity: &Identity, answers: &[WordAnswer]) -> bool {
        let words = identity.words();
        let right = !answers.is_empty()
            && answers.iter().all(|a| {
                (a.position as usize)
                    .checked_sub(1)
                    .and_then(|i| words.get(i))
                    .is_some_and(|w| w.eq_ignore_ascii_case(a.word.trim()))
            });
        if right {
            self.mark_saved(identity);
        }
        right
    }

    /// Whether this identity's words were confirmed as saved.
    pub fn words_saved(&self, identity: &Identity) -> bool {
        std::fs::read_to_string(self.saved_marker())
            .is_ok_and(|text| text.trim() == identity.pubkey_hex())
    }

    fn mark_saved(&self, identity: &Identity) {
        let _ = std::fs::create_dir_all(&self.dir);
        let _ = std::fs::write(self.saved_marker(), identity.pubkey_hex());
    }

    fn saved_marker(&self) -> PathBuf {
        self.dir.join("words-saved")
    }
}

/// The phone identity's Keychain service.
pub const PHONE_SERVICE: &str = "com.openagents.app.identity";

/// The phone identity's stored entropy, read only: the wallet makes its
/// seed from it so one set of words backs up both.
pub fn phone_entropy() -> Result<Option<Vec<u8>>, StorageError> {
    Ok(KeychainVault::new(PHONE_SERVICE)
        .load()?
        .map(|(entropy, _)| entropy))
}

/// The identity's Keychain item: a generic password with `service` and the
/// account `words-v1`, holding the BIP-39 entropy. On iOS it is readable
/// after the first unlock, so a background renewal can sign in; on this
/// device only unless the person chose the iCloud copy.
pub struct KeychainVault {
    service: String,
}

const VAULT_ACCOUNT: &str = "words-v1";

impl KeychainVault {
    pub fn new(service: &str) -> Self {
        Self {
            service: service.into(),
        }
    }
}

#[cfg(target_os = "ios")]
mod keychain {
    use super::*;
    use core_foundation::{
        base::TCFType,
        string::{CFString, CFStringRef},
    };
    use security_framework::passwords::{
        PasswordOptions, delete_generic_password_options, generic_password,
        set_generic_password_options,
    };
    use security_framework_sys::access_control::{
        kSecAttrAccessibleAfterFirstUnlock, kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
    };

    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        /// The item attribute key for accessibility; the sys crate has
        /// only its values.
        static kSecAttrAccessible: CFStringRef;
    }

    fn options(service: &str, icloud: bool) -> PasswordOptions {
        let mut options = PasswordOptions::new_generic_password(service, VAULT_ACCOUNT);
        options.set_access_synchronized(Some(icloud));
        options
    }

    fn push(options: &mut PasswordOptions, key: CFStringRef, value: CFStringRef) {
        // SAFETY: both are static Security framework constants.
        let (key, value) = unsafe {
            (
                CFString::wrap_under_get_rule(key),
                CFString::wrap_under_get_rule(value),
            )
        };
        #[allow(deprecated)]
        options.query.push((key, value.into_CFType()));
    }

    pub(super) fn read(service: &str, icloud: bool) -> Result<Option<Vec<u8>>, StorageError> {
        match generic_password(options(service, icloud)) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.code() == -25300 => Ok(None),
            Err(e) => Err(mapped(e.code())),
        }
    }

    pub(super) fn write(service: &str, bytes: &[u8], icloud: bool) -> Result<(), StorageError> {
        let mut add = options(service, icloud);
        // SAFETY: static Security framework constants.
        let accessible = unsafe {
            if icloud {
                kSecAttrAccessibleAfterFirstUnlock
            } else {
                kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
            }
        };
        push(&mut add, unsafe { kSecAttrAccessible }, accessible);
        // An existing item with other attributes would not match the update
        // that follows a duplicate add, so remove this store's copy first.
        remove(service, icloud)?;
        set_generic_password_options(bytes, add).map_err(|e| mapped(e.code()))
    }

    pub(super) fn remove(service: &str, icloud: bool) -> Result<(), StorageError> {
        match delete_generic_password_options(options(service, icloud)) {
            Ok(()) => Ok(()),
            Err(e) if e.code() == -25300 => Ok(()),
            Err(e) => Err(mapped(e.code())),
        }
    }

    pub(super) fn wallet() -> WalletRead {
        let mut query =
            PasswordOptions::new_generic_password(super::WALLET_SERVICE, super::WALLET_ACCOUNT);
        query.set_access_synchronized(Some(false));
        match generic_password(query) {
            Ok(bytes) => super::wallet_bytes(bytes),
            Err(e) if e.code() == -25300 => WalletRead::Absent,
            Err(_) => WalletRead::Unavailable,
        }
    }

    fn mapped(code: i32) -> StorageError {
        match code {
            -25293 | -25308 | -128 => StorageError::Denied,
            _ => StorageError::Unavailable,
        }
    }
}

#[cfg(target_os = "macos")]
mod keychain {
    //! The desktop's login keychain has no iCloud copy; both settings keep
    //! the words in the one item.
    use super::*;
    use security_framework::passwords::{
        delete_generic_password, get_generic_password, set_generic_password,
    };

    pub(super) fn read(service: &str, icloud: bool) -> Result<Option<Vec<u8>>, StorageError> {
        if icloud {
            return Ok(None);
        }
        match get_generic_password(service, VAULT_ACCOUNT) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.code() == -25300 => Ok(None),
            Err(e) if matches!(e.code(), -25293 | -25308 | -128) => Err(StorageError::Denied),
            Err(_) => Err(StorageError::Unavailable),
        }
    }
    pub(super) fn write(service: &str, bytes: &[u8], icloud: bool) -> Result<(), StorageError> {
        if icloud {
            return Ok(());
        }
        set_generic_password(service, VAULT_ACCOUNT, bytes).map_err(|_| StorageError::Unavailable)
    }
    pub(super) fn remove(service: &str, icloud: bool) -> Result<(), StorageError> {
        if icloud {
            return Ok(());
        }
        match delete_generic_password(service, VAULT_ACCOUNT) {
            Ok(()) => Ok(()),
            Err(e) if e.code() == -25300 => Ok(()),
            Err(_) => Err(StorageError::Unavailable),
        }
    }
    pub(super) fn wallet() -> WalletRead {
        WalletRead::Absent
    }
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
mod keychain {
    use super::*;
    pub(super) fn read(_: &str, _: bool) -> Result<Option<Vec<u8>>, StorageError> {
        Err(StorageError::Unavailable)
    }
    pub(super) fn write(_: &str, _: &[u8], _: bool) -> Result<(), StorageError> {
        Err(StorageError::Unavailable)
    }
    pub(super) fn remove(_: &str, _: bool) -> Result<(), StorageError> {
        Err(StorageError::Unavailable)
    }
    pub(super) fn wallet() -> WalletRead {
        WalletRead::Absent
    }
}

impl IdentityVault for KeychainVault {
    fn load(&self) -> Result<Option<(Vec<u8>, bool)>, StorageError> {
        if let Some(bytes) = keychain::read(&self.service, false)? {
            return Ok(Some((bytes, false)));
        }
        Ok(keychain::read(&self.service, true)?.map(|bytes| (bytes, true)))
    }
    fn store(&self, entropy: &[u8], icloud: bool) -> Result<(), StorageError> {
        // Write the new place before removing the old, so the words are
        // never in neither.
        keychain::write(&self.service, entropy, icloud)?;
        keychain::remove(&self.service, !icloud)
    }
}

/// The wallet seed's Keychain item, as the earlier OpenAgents app wrote it.
pub const WALLET_SERVICE: &str = "com.openagents.app.spark";
/// The wallet seed item's account.
pub const WALLET_ACCOUNT: &str = "seed-v1";

/// The wallet seed in the phone's Keychain, read only.
pub struct KeychainWallet;
impl WalletSeed for KeychainWallet {
    fn read(&self) -> WalletRead {
        keychain::wallet()
    }
}

#[allow(dead_code)]
fn wallet_bytes(mut bytes: Vec<u8>) -> WalletRead {
    let read = if matches!(bytes.len(), 16 | 32) {
        WalletRead::Found(bytes.clone())
    } else {
        WalletRead::Unreadable
    };
    bytes.fill(0);
    read
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use base64::Engine as _;
    use std::sync::Mutex;

    /// The identity's words in memory: (local, iCloud).
    #[derive(Default)]
    pub(crate) struct MemoryVault {
        pub items: Mutex<(Option<Vec<u8>>, Option<Vec<u8>>)>,
        pub locked: std::sync::atomic::AtomicBool,
    }
    impl IdentityVault for MemoryVault {
        fn load(&self) -> Result<Option<(Vec<u8>, bool)>, StorageError> {
            if self.locked.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(StorageError::Denied);
            }
            let items = self.items.lock().unwrap();
            Ok(items
                .0
                .clone()
                .map(|e| (e, false))
                .or_else(|| items.1.clone().map(|e| (e, true))))
        }
        fn store(&self, entropy: &[u8], icloud: bool) -> Result<(), StorageError> {
            let mut items = self.items.lock().unwrap();
            if icloud {
                items.1 = Some(entropy.to_vec());
                items.0 = None;
            } else {
                items.0 = Some(entropy.to_vec());
                items.1 = None;
            }
            Ok(())
        }
    }

    /// A wallet seed that only answers reads, and counts them.
    pub(crate) struct FixedWallet(pub WalletRead);
    impl WalletSeed for FixedWallet {
        fn read(&self) -> WalletRead {
            self.0.clone()
        }
    }

    pub(crate) fn identities(
        vault: &Arc<MemoryVault>,
        wallet: WalletRead,
        dir: &Path,
    ) -> Identities {
        Identities::new(vault.clone(), Arc::new(FixedWallet(wallet)), dir)
    }

    // NIP-06's first test vector.
    const VECTOR_WORDS: &str =
        "leader monkey parrot ring guide accident before fence cannon height naive bean";
    const VECTOR_NPUB: &str = "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu";

    #[test]
    fn first_launch_makes_12_words_and_relaunch_reuses_them() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(MemoryVault::default());
        let first = identities(&vault, WalletRead::Absent, dir.path())
            .load_or_create()
            .unwrap();
        assert_eq!(first.words().len(), 12);
        assert!(first.npub().starts_with("npub1"));
        // Stored on this device only.
        assert!(vault.items.lock().unwrap().0.is_some());
        let again = identities(&vault, WalletRead::Absent, dir.path())
            .load_or_create()
            .unwrap();
        assert_eq!(again.npub(), first.npub());
    }

    #[test]
    fn the_wallet_words_are_the_identity_words_and_the_wallet_is_never_written() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(MemoryVault::default());
        let wallet = Identity::from_words(VECTOR_WORDS).unwrap().entropy();
        let identity = identities(&vault, WalletRead::Found(wallet.clone()), dir.path())
            .load_or_create()
            .unwrap();
        assert_eq!(identity.npub(), VECTOR_NPUB);
        assert_eq!(identity.words().join(" "), VECTOR_WORDS);
        // FixedWallet has no way to write; the identity keeps its own copy.
        assert_eq!(vault.items.lock().unwrap().0.as_deref(), Some(&wallet[..]));
    }

    #[test]
    fn a_locked_or_unreadable_store_never_leads_to_new_words() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(MemoryVault::default());
        for (wallet, error) in [
            (WalletRead::Unavailable, IdentityError::Locked),
            (WalletRead::Unreadable, IdentityError::WalletUnreadable),
        ] {
            let result = identities(&vault, wallet, dir.path()).load_or_create();
            assert_eq!(result.err(), Some(error));
            assert!(vault.items.lock().unwrap().0.is_none());
        }
        vault
            .locked
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let result = identities(&vault, WalletRead::Absent, dir.path()).load_or_create();
        assert_eq!(result.err(), Some(IdentityError::Locked));
    }

    #[test]
    fn restoring_words_gives_the_same_npub() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(MemoryVault::default());
        let ids = identities(&vault, WalletRead::Absent, dir.path());
        let restored = ids.restore(&VECTOR_WORDS.to_uppercase(), false).unwrap();
        assert_eq!(restored.npub(), VECTOR_NPUB);
        assert!(ids.words_saved(&restored));
        assert_eq!(ids.load_or_create().unwrap().npub(), VECTOR_NPUB);
        // A different device restoring the same words gets the same key.
        let other = tempfile::tempdir().unwrap();
        let elsewhere = identities(&Arc::default(), WalletRead::Absent, other.path());
        assert_eq!(
            elsewhere.restore(VECTOR_WORDS, false).unwrap().pubkey_hex(),
            restored.pubkey_hex()
        );
    }

    #[test]
    fn restore_keeps_an_unsaved_account_and_a_different_wallet() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(MemoryVault::default());
        let ids = identities(&vault, WalletRead::Absent, dir.path());
        let first = ids.load_or_create().unwrap();
        assert_eq!(
            ids.restore(VECTOR_WORDS, false).err(),
            Some(IdentityError::Unsaved)
        );
        assert_eq!(ids.load().unwrap().unwrap().npub(), first.npub());
        assert_eq!(ids.restore(VECTOR_WORDS, true).unwrap().npub(), VECTOR_NPUB);

        let other = Identity::generate().unwrap().entropy();
        let walleted = identities(&vault, WalletRead::Found(other), dir.path());
        assert_eq!(
            walleted.restore(VECTOR_WORDS, true).err(),
            Some(IdentityError::WalletDiffers)
        );
        assert!(matches!(
            ids.restore("leader monkey", false).err(),
            Some(IdentityError::Words(_))
        ));
    }

    #[test]
    fn the_backup_check_records_saved_words_only_when_every_answer_matches() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(MemoryVault::default());
        let ids = identities(&vault, WalletRead::Absent, dir.path());
        let identity = ids.load_or_create().unwrap();
        let positions = ids.backup_check(&identity);
        assert_eq!(positions.len(), 3);
        assert!(positions.iter().all(|p| (1..=12).contains(p)));
        assert!(positions.windows(2).all(|w| w[0] < w[1]));
        let words = identity.words();
        let answer = |p: u32, w: &str| WordAnswer {
            position: p,
            word: w.into(),
        };
        let mut answers: Vec<_> = positions
            .iter()
            .map(|&p| answer(p, &words[p as usize - 1]))
            .collect();
        answers[0].word = "wrong".into();
        assert!(!ids.confirm_backup(&identity, &answers));
        assert!(!ids.words_saved(&identity));
        assert!(!ids.confirm_backup(&identity, &[]));
        answers[0].word = format!(" {} ", words[positions[0] as usize - 1].to_uppercase());
        assert!(ids.confirm_backup(&identity, &answers));
        assert!(ids.words_saved(&identity));
        assert!(!ids.confirm_backup(&identity, &[answer(13, "x")]));
    }

    #[test]
    fn the_icloud_copy_moves_the_words_between_stores() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(MemoryVault::default());
        let ids = identities(&vault, WalletRead::Absent, dir.path());
        let identity = ids.load_or_create().unwrap();
        assert!(!ids.icloud_copy());
        ids.set_icloud_copy(true).unwrap();
        assert!(ids.icloud_copy());
        assert!(vault.items.lock().unwrap().0.is_none());
        assert_eq!(ids.load().unwrap().unwrap().npub(), identity.npub());
        ids.set_icloud_copy(false).unwrap();
        assert!(!ids.icloud_copy());
    }

    #[test]
    fn signed_events_verify_as_nip98() {
        let identity = Identity::from_words(VECTOR_WORDS).unwrap();
        let event = identity.sign(&EventTemplate {
            created_at: 1_700_000_000,
            kind: 27_235,
            tags: vec![
                vec!["u".into(), "https://api.example/v1/device/nostr".into()],
                vec!["method".into(), "POST".into()],
            ],
            content: String::new(),
        });
        let event: nostr::domain::Event = serde_json::from_str(&event).unwrap();
        assert_eq!(event.pubkey, identity.pubkey_hex());
        assert_eq!(event.id, event.computed_id().unwrap());
        event.validate_nip01_structure().unwrap();
        let header = format!(
            "Nostr {}",
            base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&event).unwrap())
        );
        let claim = nostr::domain::parse_http_authorization_claim(
            &header,
            "POST",
            "https://api.example/v1/device/nostr",
            1_700_000_000,
        )
        .unwrap();
        assert_eq!(claim.auth.pubkey, identity.pubkey_hex());
    }

    #[test]
    fn errors_read_plainly_and_never_repeat_the_words() {
        for e in [
            IdentityError::Locked,
            IdentityError::WalletUnreadable,
            IdentityError::NotSaved,
            IdentityError::WalletDiffers,
            IdentityError::Unsaved,
        ] {
            let text = e.message();
            assert!(oa_copy::violations(&text, &[]).is_empty(), "{text}");
            assert!(!text.to_lowercase().contains("sign in"), "{text}");
        }
    }
}
