//! Where the host's secret key lives.
//!
//! By default the key is `host.key` in the private store directory, beside
//! the grant book. A host that another program manages, such as the
//! OpenAgents desktop app, keeps it in the OS keychain instead and passes a
//! [`KeySource`] to [`Host::with_keys`](super::Host::with_keys). The book,
//! its lock, and the grants stay in the store directory either way; only
//! the secret moves.
use super::*;
use std::sync::{Arc, Mutex};

/// A place that holds the host's secret key. Implementations never log,
/// print, or return the key in an error.
pub trait KeySource: Send + Sync {
    /// The stored host key, or `None` when none is stored yet.
    ///
    /// # Errors
    /// Refuses a source that cannot be read or holds an invalid key.
    fn load(&self) -> Result<Option<SecretKey>>;
    /// Store a newly created host key. The host calls this once, while it
    /// initializes a store that has no book yet.
    ///
    /// # Errors
    /// Refuses a source that cannot be written.
    fn store(&self, key: &SecretKey) -> Result<()>;
}

/// The host key from a [`KeySource`], read once and then held in memory,
/// as the resident host already holds it to sign.
pub(super) struct Keys {
    source: Arc<dyn KeySource>,
    cached: Mutex<Option<SecretKey>>,
}

impl Keys {
    pub(super) fn new(source: Arc<dyn KeySource>) -> Self {
        Self {
            source,
            cached: Mutex::new(None),
        }
    }

    fn cache(&self) -> std::sync::MutexGuard<'_, Option<SecretKey>> {
        self.cached
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The host key. With `create`, a missing key is made and stored; the
    /// caller allows that only for a store without a book.
    pub(super) fn key(&self, create: bool) -> Result<SecretKey> {
        let mut cached = self.cache();
        if let Some(key) = *cached {
            return Ok(key);
        }
        let key = match self.source.load()? {
            Some(key) => key,
            None if create => {
                let key = SecretKey::new(&mut secp256k1::rand::rng());
                self.source.store(&key)?;
                // Read it back, so a source that dropped the write fails now
                // rather than after the book names a key nobody holds.
                match self.source.load()? {
                    Some(read) if read == key => key,
                    _ => {
                        return fail(
                            Code::Unavailable,
                            "the key source did not keep the new host key",
                        );
                    }
                }
            }
            None => {
                return fail(
                    Code::Unavailable,
                    "the host key is missing from its key source",
                );
            }
        };
        *cached = Some(key);
        Ok(key)
    }
}

/// A key source that holds the key in memory only, for tests and for a
/// caller that supplies the key some other way.
#[derive(Default)]
pub struct MemoryKeys(Mutex<Option<SecretKey>>);

impl MemoryKeys {
    /// A source that already holds `key`.
    #[must_use]
    pub fn holding(key: SecretKey) -> Self {
        Self(Mutex::new(Some(key)))
    }
}

impl KeySource for MemoryKeys {
    fn load(&self) -> Result<Option<SecretKey>> {
        Ok(*self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner))
    }
    fn store(&self, key: &SecretKey) -> Result<()> {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(*key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_host_under_a_key_source_writes_no_key_file_and_reopens_with_the_same_key() {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("access");
        let source = Arc::new(MemoryKeys::default());
        let owner = pubkey(&SecretKey::new(&mut secp256k1::rand::rng()));
        let host = Host::with_keys(&state, RelayPolicy::LoopbackTest, source.clone());
        let key = host.init(&owner).unwrap();
        assert!(!state.join("host.key").exists());
        assert_eq!(pubkey(&source.load().unwrap().unwrap()), key);
        // A second process opening the same store and source sees the same host.
        let again = Host::with_keys(&state, RelayPolicy::LoopbackTest, source);
        assert_eq!(again.public_key().unwrap(), key);
        assert_eq!(again.owner().unwrap(), owner);
    }

    #[test]
    fn a_missing_key_refuses_once_the_book_exists() {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("access");
        let owner = pubkey(&SecretKey::new(&mut secp256k1::rand::rng()));
        let first = Arc::new(MemoryKeys::default());
        Host::with_keys(&state, RelayPolicy::LoopbackTest, first)
            .init(&owner)
            .unwrap();
        // An empty source must not mint a second identity for the book.
        let empty = Host::with_keys(
            &state,
            RelayPolicy::LoopbackTest,
            Arc::new(MemoryKeys::default()),
        );
        assert_eq!(empty.init(&owner).unwrap_err().code, Code::Unavailable);
        assert_eq!(empty.owner().unwrap_err().code, Code::Unavailable);
        // Another key for the same book is refused as a different identity.
        let other = Host::with_keys(
            &state,
            RelayPolicy::LoopbackTest,
            Arc::new(MemoryKeys::holding(SecretKey::new(
                &mut secp256k1::rand::rng(),
            ))),
        );
        assert_eq!(other.owner().unwrap_err().code, Code::Malformed);
    }
}
