//! The wallet seed in the phone's key store, and the rules that keep it.
//!
//! The seed is BIP-39 entropy: 16 bytes (12 words) or 32 bytes (24 words).
//! On iOS it is the Keychain generic password with service
//! [`KEYCHAIN_SERVICE`] and account [`KEYCHAIN_ACCOUNT`], not synchronizable,
//! readable only while the device is unlocked, on this device only. The
//! earlier OpenAgents app (removed in #11901) wrote that same item, and this
//! app has the same bundle ID and team, so a person who had a balance there
//! finds the same wallet here.
//!
//! The rules, which the tests in `super::tests` hold:
//!
//! - A seed that is present is used exactly as stored. Nothing here can
//!   delete or replace it: [`KeyStore`] has no delete and no update, only an
//!   add that fails when the item exists.
//! - A new seed is made only after a read says the item is not there. A read
//!   that fails for any other reason (a locked device, a Keychain error) is
//!   [`SeedRead::Unavailable`]: the screen says so and offers to try again.
//! - A stored value of the wrong length is [`SeedRead::Unreadable`]. It is
//!   left alone and no new seed is made over it.
//! - Restoring from words is offered only while no seed is stored.

use openagents_spark::seed::{Seed, restore_entropy};

/// The Keychain item's service, as the earlier app wrote it.
pub const KEYCHAIN_SERVICE: &str = "com.openagents.app.spark";
/// The Keychain item's account, as the earlier app wrote it.
pub const KEYCHAIN_ACCOUNT: &str = "seed-v1";

/// What one read of the key store found.
#[derive(Clone, PartialEq, Eq, uniffi::Enum)]
pub enum KeyRead {
    /// The item exists; these are its bytes.
    Found { bytes: Vec<u8> },
    /// The key store answered that no such item exists.
    NotFound,
    /// The read failed for another reason, such as a locked device. `status`
    /// is the platform's code (an `OSStatus` on iOS), for the log only.
    Failed { status: i32 },
}

/// What one add to the key store did.
#[derive(Clone, PartialEq, Eq, uniffi::Enum)]
pub enum KeyAdd {
    Added,
    /// An item was already there; it was left as it was.
    AlreadyExists,
    Failed {
        status: i32,
    },
}

/// The platform key store that holds the seed. The iOS app implements it
/// over the Keychain item above. It deliberately has no way to delete or
/// overwrite the item.
#[uniffi::export(with_foreign)]
pub trait KeyStore: Send + Sync {
    /// Read the seed item.
    fn read_seed(&self) -> KeyRead;
    /// Add the seed item. It must fail with [`KeyAdd::AlreadyExists`], and
    /// change nothing, when an item is already stored.
    fn add_seed(&self, entropy: Vec<u8>) -> KeyAdd;
}

/// The seed as the wallet sees it after a read.
pub enum SeedRead {
    Present(Seed),
    /// Certainly absent: the key store said so.
    Absent,
    /// The key store couldn't be read now. Try again later; never make a
    /// seed in its place.
    Unavailable,
    /// A value is stored but isn't a seed. It is kept as it is.
    Unreadable,
}

/// Read the seed. Never writes.
pub fn read(store: &dyn KeyStore) -> SeedRead {
    match store.read_seed() {
        KeyRead::Found { bytes } => match Seed::from_entropy(bytes) {
            Ok(seed) => SeedRead::Present(seed),
            Err(_) => SeedRead::Unreadable,
        },
        KeyRead::NotFound => SeedRead::Absent,
        KeyRead::Failed { .. } => SeedRead::Unavailable,
    }
}

/// Why a seed could not be made, restored, or read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeedError {
    /// A seed is already stored; nothing was changed.
    Exists,
    Unavailable,
    Unreadable,
    /// The words were refused; the message says why, without repeating them.
    Words(String),
    /// The key store refused to save the seed.
    NotSaved,
}

impl SeedError {
    /// What the screen says.
    pub fn message(&self) -> String {
        match self {
            Self::Exists => "This phone already has a wallet.".into(),
            Self::Unavailable => UNAVAILABLE.into(),
            Self::Unreadable => UNREADABLE.into(),
            Self::Words(message) => message.clone(),
            Self::NotSaved => "Couldn't save the wallet on this phone. Try again.".into(),
        }
    }
}

/// What the screen says when the key store can't be read now.
pub const UNAVAILABLE: &str = "Your wallet is locked. Unlock your phone, then try again.";
/// What the screen says when the stored seed can't be read.
pub const UNREADABLE: &str =
    "The wallet saved on this phone can't be read. It has been left as it is.";

/// What the identity's own Keychain item holds (#11995), read only.
#[derive(Clone, PartialEq, Eq)]
pub enum IdentityEntropy {
    /// The identity's BIP-39 entropy.
    Found(Vec<u8>),
    /// No identity is stored yet. When one is made later, it takes the
    /// wallet's words, so there is still one backup.
    Absent,
    /// The item couldn't be read now (a locked device).
    Unavailable,
}

/// Where the wallet finds the identity's words.
pub type IdentitySource = std::sync::Arc<dyn Fn() -> IdentityEntropy + Send + Sync>;

/// The phone's identity item, through `openagents_account::identity`.
pub fn phone_identity() -> IdentitySource {
    std::sync::Arc::new(|| match openagents_account::identity::phone_entropy() {
        Ok(Some(entropy)) => IdentityEntropy::Found(entropy),
        Ok(None) => IdentityEntropy::Absent,
        Err(_) => IdentityEntropy::Unavailable,
    })
}

/// Make a new 12-word seed and store it, but only when the key store says
/// none exists. When one appeared in the meantime, that one is used.
pub fn create(store: &dyn KeyStore) -> Result<Seed, SeedError> {
    create_with(store, None)
}

/// Make the wallet's seed, only when the key store says none exists: from
/// the identity's words when the phone has an identity, so the person has
/// one set of words for both, and new words only when it has none. Neither
/// item is ever overwritten: the seed is added only where none is stored,
/// and the identity's item is only read.
pub fn create_with(
    store: &dyn KeyStore,
    identity: Option<&IdentitySource>,
) -> Result<Seed, SeedError> {
    match read(store) {
        SeedRead::Present(_) => return Err(SeedError::Exists),
        SeedRead::Unavailable => return Err(SeedError::Unavailable),
        SeedRead::Unreadable => return Err(SeedError::Unreadable),
        SeedRead::Absent => {}
    }
    let seed = match identity.map(|source| source()) {
        Some(IdentityEntropy::Found(entropy)) => {
            Seed::from_entropy(entropy).map_err(|_| SeedError::NotSaved)?
        }
        Some(IdentityEntropy::Unavailable) => return Err(SeedError::Unavailable),
        Some(IdentityEntropy::Absent) | None => {
            Seed::generate().map_err(|_| SeedError::NotSaved)?
        }
    };
    save(store, seed)
}

/// Store the seed for recovery words, but only when none exists.
pub fn restore(store: &dyn KeyStore, words: &str) -> Result<Seed, SeedError> {
    let entropy = restore_entropy(words).map_err(SeedError::Words)?;
    let entropy = decode_hex(&entropy).ok_or(SeedError::NotSaved)?;
    let seed = Seed::from_entropy(entropy).map_err(SeedError::Words)?;
    match read(store) {
        SeedRead::Present(_) => return Err(SeedError::Exists),
        SeedRead::Unavailable => return Err(SeedError::Unavailable),
        SeedRead::Unreadable => return Err(SeedError::Unreadable),
        SeedRead::Absent => {}
    }
    save(store, seed)
}

/// Add `seed` and read it back. If the add found an item already there,
/// the stored item wins and `seed` is dropped.
fn save(store: &dyn KeyStore, seed: Seed) -> Result<Seed, SeedError> {
    match store.add_seed(seed.entropy.clone()) {
        KeyAdd::Added => match read(store) {
            SeedRead::Present(stored) if stored.entropy == seed.entropy => Ok(stored),
            SeedRead::Present(_) => Err(SeedError::Exists),
            SeedRead::Unavailable => Err(SeedError::Unavailable),
            SeedRead::Unreadable => Err(SeedError::Unreadable),
            SeedRead::Absent => Err(SeedError::NotSaved),
        },
        KeyAdd::AlreadyExists => Err(SeedError::Exists),
        KeyAdd::Failed { .. } => Err(SeedError::NotSaved),
    }
}

fn decode_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len() / 2)
        .map(|i| u8::from_str_radix(text.get(i * 2..i * 2 + 2)?, 16).ok())
        .collect()
}

/// The stored wallet's recovery words, for the identity (#11995).
#[derive(Clone, PartialEq, Eq, uniffi::Enum)]
pub enum WalletWords {
    /// The stored seed's words, in order: 12 or 24 BIP-39 English words.
    Words { words: Vec<String> },
    /// No wallet is stored on this phone.
    NoWallet,
    /// The key store couldn't be read now (for example, the phone is
    /// locked). Try again; don't make new words in its place.
    Unavailable,
    /// A value is stored but isn't a seed. Leave it alone.
    Unreadable,
}

/// The recovery words of the wallet stored on this phone, if there is one.
///
/// This is how the identity (#11995) uses the same words as the wallet, so
/// one backup covers the person's npub and their bitcoin. It only reads:
/// it never makes, changes, or deletes a seed. Ask for it only when the words
/// are needed (to derive the identity key, or after the person chose to see
/// them), and keep the result no longer than that.
#[uniffi::export]
pub fn wallet_seed_words(store: std::sync::Arc<dyn KeyStore>) -> WalletWords {
    words(store.as_ref())
}

pub(crate) fn words(store: &dyn KeyStore) -> WalletWords {
    match read(store) {
        SeedRead::Present(seed) => WalletWords::Words {
            words: seed.words().into_iter().map(str::to_owned).collect(),
        },
        SeedRead::Absent => WalletWords::NoWallet,
        SeedRead::Unavailable => WalletWords::Unavailable,
        SeedRead::Unreadable => WalletWords::Unreadable,
    }
}

/// A key store held in memory: the offline fixture, and the tests.
#[derive(Default)]
pub struct MemoryStore {
    pub item: std::sync::Mutex<Option<Vec<u8>>>,
    /// While set, every read fails as a locked device's does.
    pub locked: std::sync::atomic::AtomicBool,
    /// Every add that was attempted, in order.
    pub adds: std::sync::Mutex<Vec<Vec<u8>>>,
}

impl MemoryStore {
    pub fn holding(entropy: Vec<u8>) -> Self {
        Self {
            item: std::sync::Mutex::new(Some(entropy)),
            ..Self::default()
        }
    }
    pub fn stored(&self) -> Option<Vec<u8>> {
        self.item
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }
}

impl KeyStore for MemoryStore {
    fn read_seed(&self) -> KeyRead {
        if self.locked.load(std::sync::atomic::Ordering::SeqCst) {
            // errSecInteractionNotAllowed
            return KeyRead::Failed { status: -25308 };
        }
        match self.stored() {
            Some(bytes) => KeyRead::Found { bytes },
            None => KeyRead::NotFound,
        }
    }
    fn add_seed(&self, entropy: Vec<u8>) -> KeyAdd {
        self.adds
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .push(entropy.clone());
        if self.locked.load(std::sync::atomic::Ordering::SeqCst) {
            return KeyAdd::Failed { status: -25308 };
        }
        let mut item = self
            .item
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if item.is_some() {
            return KeyAdd::AlreadyExists;
        }
        *item = Some(entropy);
        KeyAdd::Added
    }
}
