//! Reads the retired Android Spark seed before the current wallet can create one.

use super::{AndroidSecretStorage, SecretMutation, SecretRead};
use crate::wallet::{
    CoreWallet, WalletListener,
    seed::{KeyAdd, KeyRead, KeyStore},
};
use std::sync::Arc;

const WALLET_NAMESPACE: &str = "com.openagents.app.spark";
const WALLET_KEY: &str = "seed-v1";

#[derive(Clone, PartialEq, Eq, uniffi::Enum)]
pub enum RetiredSeedRead {
    Absent,
    Present { bytes: Vec<u8> },
    Locked,
    Corrupt,
    Failed,
}

/// Reads the old encrypted file. This interface cannot create or delete it.
#[uniffi::export(with_foreign)]
pub trait AndroidRetiredWalletSeed: Send + Sync {
    fn read_retired_seed(&self) -> RetiredSeedRead;
}

struct MigratingWalletKeyStore {
    current: Arc<dyn AndroidSecretStorage>,
    retired: Arc<dyn AndroidRetiredWalletSeed>,
}

impl MigratingWalletKeyStore {
    fn read_current(&self) -> KeyRead {
        match self
            .current
            .read(WALLET_NAMESPACE.into(), WALLET_KEY.into())
        {
            SecretRead::Absent => KeyRead::NotFound,
            SecretRead::Present { bytes } => KeyRead::Found { bytes },
            SecretRead::Locked => KeyRead::Failed { status: -25293 },
            SecretRead::Corrupt | SecretRead::Failed => KeyRead::Failed { status: -1 },
        }
    }

    fn migrate_retired(&self) -> KeyRead {
        let mut bytes = match self.retired.read_retired_seed() {
            RetiredSeedRead::Absent => return KeyRead::NotFound,
            RetiredSeedRead::Present { bytes } => bytes,
            RetiredSeedRead::Locked => return KeyRead::Failed { status: -25293 },
            RetiredSeedRead::Corrupt => return KeyRead::Found { bytes: Vec::new() },
            RetiredSeedRead::Failed => return KeyRead::Failed { status: -1 },
        };

        if bytes.len() != 16 && bytes.len() != 32 {
            bytes.fill(0);
            return KeyRead::Found { bytes: Vec::new() };
        }

        let write = self
            .current
            .create(WALLET_NAMESPACE.into(), WALLET_KEY.into(), bytes.clone());
        let result = match write {
            SecretMutation::Stored | SecretMutation::AlreadyExists => {
                match self
                    .current
                    .read(WALLET_NAMESPACE.into(), WALLET_KEY.into())
                {
                    SecretRead::Present { bytes: mut stored } => {
                        let same_seed = stored == bytes;
                        stored.fill(0);
                        if same_seed {
                            KeyRead::Found {
                                bytes: bytes.clone(),
                            }
                        } else {
                            KeyRead::Failed { status: -1 }
                        }
                    }
                    SecretRead::Locked => KeyRead::Failed { status: -25293 },
                    SecretRead::Absent | SecretRead::Corrupt | SecretRead::Failed => {
                        KeyRead::Failed { status: -1 }
                    }
                }
            }
            SecretMutation::Locked => KeyRead::Failed { status: -25293 },
            SecretMutation::Absent | SecretMutation::Failed => KeyRead::Failed { status: -1 },
        };
        bytes.fill(0);
        result
    }
}

impl KeyStore for MigratingWalletKeyStore {
    fn read_seed(&self) -> KeyRead {
        match self.read_current() {
            KeyRead::NotFound => self.migrate_retired(),
            current => current,
        }
    }

    fn add_seed(&self, entropy: Vec<u8>) -> KeyAdd {
        match self
            .current
            .create(WALLET_NAMESPACE.into(), WALLET_KEY.into(), entropy)
        {
            SecretMutation::Stored => KeyAdd::Added,
            SecretMutation::AlreadyExists => KeyAdd::AlreadyExists,
            SecretMutation::Locked => KeyAdd::Failed { status: -25293 },
            SecretMutation::Absent | SecretMutation::Failed => KeyAdd::Failed { status: -1 },
        }
    }
}

#[uniffi::export]
impl CoreWallet {
    /// Open the Android wallet after checking the retired encrypted seed.
    #[uniffi::constructor]
    pub fn android(
        home: String,
        current: Arc<dyn AndroidSecretStorage>,
        retired: Arc<dyn AndroidRetiredWalletSeed>,
        listener: Arc<dyn WalletListener>,
    ) -> Arc<Self> {
        Self::new(
            home,
            Arc::new(MigratingWalletKeyStore { current, retired }),
            listener,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Clone)]
    struct MemoryStorage {
        value: Arc<Mutex<SecretRead>>,
        next_create: Arc<Mutex<SecretMutation>>,
        creates: Arc<Mutex<usize>>,
    }

    impl MemoryStorage {
        fn absent() -> Self {
            Self {
                value: Arc::new(Mutex::new(SecretRead::Absent)),
                next_create: Arc::new(Mutex::new(SecretMutation::Stored)),
                creates: Arc::new(Mutex::new(0)),
            }
        }
    }

    impl AndroidSecretStorage for MemoryStorage {
        fn read(&self, _namespace: String, _key: String) -> SecretRead {
            self.value.lock().unwrap().clone()
        }

        fn create(&self, _namespace: String, _key: String, bytes: Vec<u8>) -> SecretMutation {
            *self.creates.lock().unwrap() += 1;
            let outcome = *self.next_create.lock().unwrap();
            if outcome == SecretMutation::Stored {
                let mut value = self.value.lock().unwrap();
                if *value == SecretRead::Absent {
                    *value = SecretRead::Present { bytes };
                } else {
                    return SecretMutation::AlreadyExists;
                }
            }
            outcome
        }

        fn replace(&self, _namespace: String, _key: String, _bytes: Vec<u8>) -> SecretMutation {
            SecretMutation::Failed
        }

        fn delete(&self, _namespace: String, _key: String) -> SecretMutation {
            SecretMutation::Failed
        }
    }

    #[derive(Clone)]
    struct Retired(Arc<Mutex<RetiredSeedRead>>);

    impl AndroidRetiredWalletSeed for Retired {
        fn read_retired_seed(&self) -> RetiredSeedRead {
            self.0.lock().unwrap().clone()
        }
    }

    fn store(current: &MemoryStorage, retired: &Retired) -> MigratingWalletKeyStore {
        MigratingWalletKeyStore {
            current: Arc::new(current.clone()),
            retired: Arc::new(retired.clone()),
        }
    }

    fn entropy() -> Vec<u8> {
        (0..16).collect()
    }

    #[test]
    fn migrates_the_exact_legacy_seed_once_and_reuses_it() {
        let current = MemoryStorage::absent();
        let retired = Retired(Arc::new(Mutex::new(RetiredSeedRead::Present {
            bytes: entropy(),
        })));
        let key_store = store(&current, &retired);

        assert!(key_store.read_seed() == KeyRead::Found { bytes: entropy() });
        assert!(key_store.read_seed() == KeyRead::Found { bytes: entropy() });
        assert_eq!(*current.creates.lock().unwrap(), 1);
        assert!(*current.value.lock().unwrap() == SecretRead::Present { bytes: entropy() });
        assert!(matches!(
            *retired.0.lock().unwrap(),
            RetiredSeedRead::Present { .. }
        ));
    }

    #[test]
    fn migrates_both_supported_entropy_sizes_without_changing_bytes() {
        for bytes in [(0u8..16).collect::<Vec<_>>(), (0u8..32).collect::<Vec<_>>()] {
            let current = MemoryStorage::absent();
            let retired = Retired(Arc::new(Mutex::new(RetiredSeedRead::Present {
                bytes: bytes.clone(),
            })));

            assert!(
                store(&current, &retired).read_seed()
                    == KeyRead::Found {
                        bytes: bytes.clone()
                    }
            );
            assert!(*current.value.lock().unwrap() == SecretRead::Present { bytes });
        }
    }

    #[test]
    fn absence_does_not_create_a_wallet() {
        let current = MemoryStorage::absent();
        let retired = Retired(Arc::new(Mutex::new(RetiredSeedRead::Absent)));
        assert!(store(&current, &retired).read_seed() == KeyRead::NotFound);
        assert_eq!(*current.creates.lock().unwrap(), 0);
    }

    #[test]
    fn locked_failed_or_corrupt_legacy_storage_fails_closed() {
        for read in [
            RetiredSeedRead::Locked,
            RetiredSeedRead::Failed,
            RetiredSeedRead::Corrupt,
        ] {
            let current = MemoryStorage::absent();
            let retired = Retired(Arc::new(Mutex::new(read)));
            let result = store(&current, &retired).read_seed();
            assert!(!matches!(result, KeyRead::NotFound));
            assert_eq!(*current.creates.lock().unwrap(), 0);
        }
    }

    #[test]
    fn unreadable_or_invalid_legacy_seed_is_never_copied() {
        for bytes in [vec![0; 15], vec![0; 33]] {
            let current = MemoryStorage::absent();
            let retired = Retired(Arc::new(Mutex::new(RetiredSeedRead::Present { bytes })));
            assert!(matches!(
                store(&current, &retired).read_seed(),
                KeyRead::Found { bytes } if bytes.is_empty()
            ));
            assert!(*current.value.lock().unwrap() == SecretRead::Absent);
            assert_eq!(*current.creates.lock().unwrap(), 0);
        }
    }

    #[test]
    fn current_store_errors_do_not_fall_back_to_legacy_storage() {
        for read in [SecretRead::Locked, SecretRead::Corrupt, SecretRead::Failed] {
            let current = MemoryStorage::absent();
            *current.value.lock().unwrap() = read;
            let retired = Retired(Arc::new(Mutex::new(RetiredSeedRead::Present {
                bytes: entropy(),
            })));
            assert!(matches!(
                store(&current, &retired).read_seed(),
                KeyRead::Failed { .. }
            ));
            assert_eq!(*current.creates.lock().unwrap(), 0);
        }
    }

    #[test]
    fn interrupted_migration_retries_without_changing_the_legacy_seed() {
        let current = MemoryStorage::absent();
        *current.next_create.lock().unwrap() = SecretMutation::Failed;
        let retired = Retired(Arc::new(Mutex::new(RetiredSeedRead::Present {
            bytes: entropy(),
        })));
        let key_store = store(&current, &retired);

        assert!(matches!(key_store.read_seed(), KeyRead::Failed { .. }));
        assert!(*current.value.lock().unwrap() == SecretRead::Absent);
        *current.next_create.lock().unwrap() = SecretMutation::Stored;
        assert!(key_store.read_seed() == KeyRead::Found { bytes: entropy() });
        assert!(matches!(
            *retired.0.lock().unwrap(),
            RetiredSeedRead::Present { .. }
        ));
    }

    #[test]
    fn a_racing_different_seed_is_never_replaced() {
        let current = MemoryStorage::absent();
        *current.next_create.lock().unwrap() = SecretMutation::AlreadyExists;
        *current.value.lock().unwrap() = SecretRead::Present { bytes: vec![7; 16] };
        let retired = Retired(Arc::new(Mutex::new(RetiredSeedRead::Present {
            bytes: entropy(),
        })));

        assert!(matches!(
            store(&current, &retired).read_seed(),
            KeyRead::Failed { .. }
        ));
        assert!(*current.value.lock().unwrap() == SecretRead::Present { bytes: vec![7; 16] });
    }
}
