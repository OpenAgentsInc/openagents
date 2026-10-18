//! Android's encrypted-file provider boundary. Rust assigns every namespace
//! and key and decides when a secret may be created or replaced.

use openagents_account::{
    Passwords,
    identity::{
        IdentityVault, PHONE_SERVICE, WALLET_ACCOUNT, WALLET_SERVICE, WalletRead, WalletSeed,
    },
    openagents::StorageError,
};
use openagents_client::PairingStore;
use std::sync::{Arc, Mutex};

#[derive(Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SecretRead {
    Absent,
    Present { bytes: Vec<u8> },
    Locked,
    Corrupt,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum SecretMutation {
    Stored,
    AlreadyExists,
    Absent,
    Locked,
    Failed,
}

/// Platform operations over opaque encrypted values. The provider must use
/// Android Keystore backed encryption, atomic writes, and private app files.
#[uniffi::export(with_foreign)]
pub trait AndroidSecretStorage: Send + Sync {
    fn read(&self, namespace: String, key: String) -> SecretRead;
    /// Add only when no value exists. A racing writer must get AlreadyExists.
    fn create(&self, namespace: String, key: String, bytes: Vec<u8>) -> SecretMutation;
    /// Atomically replace or create one value after Rust authorizes the change.
    fn replace(&self, namespace: String, key: String, bytes: Vec<u8>) -> SecretMutation;
    fn delete(&self, namespace: String, key: String) -> SecretMutation;
}

fn read(
    storage: &dyn AndroidSecretStorage,
    namespace: &str,
    key: &str,
) -> Result<Option<Vec<u8>>, StorageError> {
    match storage.read(namespace.into(), key.into()) {
        SecretRead::Absent => Ok(None),
        SecretRead::Present { bytes } => Ok(Some(bytes)),
        SecretRead::Locked => Err(StorageError::Denied),
        SecretRead::Corrupt | SecretRead::Failed => Err(StorageError::Unavailable),
    }
}

fn mutation(result: SecretMutation) -> Result<(), StorageError> {
    match result {
        SecretMutation::Stored => Ok(()),
        SecretMutation::Locked => Err(StorageError::Denied),
        SecretMutation::AlreadyExists | SecretMutation::Absent | SecretMutation::Failed => {
            Err(StorageError::Unavailable)
        }
    }
}

pub struct AndroidPasswords(pub Arc<dyn AndroidSecretStorage>);

impl Passwords for AndroidPasswords {
    fn get(&self, service: &str, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        read(self.0.as_ref(), service, key)
    }

    fn set(&self, service: &str, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        let mut payload = bytes.to_vec();
        let result = self.0.replace(service.into(), key.into(), payload.clone());
        payload.fill(0);
        mutation(result)
    }

    fn delete(&self, service: &str, key: &str) -> Result<(), StorageError> {
        match self.0.delete(service.into(), key.into()) {
            SecretMutation::Stored | SecretMutation::Absent => Ok(()),
            SecretMutation::Locked => Err(StorageError::Denied),
            SecretMutation::AlreadyExists | SecretMutation::Failed => {
                Err(StorageError::Unavailable)
            }
        }
    }

    // Android has no earlier Keychain-backed account entries to migrate.
    fn list(&self, _service: &str) -> Result<Vec<Vec<u8>>, StorageError> {
        Ok(Vec::new())
    }
}

pub struct AndroidIdentityVault(pub Arc<dyn AndroidSecretStorage>);

impl IdentityVault for AndroidIdentityVault {
    fn load(&self) -> Result<Option<(Vec<u8>, bool)>, StorageError> {
        read(self.0.as_ref(), PHONE_SERVICE, "words-v1")
            .map(|value| value.map(|bytes| (bytes, false)))
    }

    fn create_if_absent(&self, entropy: &[u8], icloud: bool) -> Result<bool, StorageError> {
        if icloud {
            return Err(StorageError::Unavailable);
        }
        let mut payload = entropy.to_vec();
        let result = self
            .0
            .create(PHONE_SERVICE.into(), "words-v1".into(), payload.clone());
        payload.fill(0);
        match result {
            SecretMutation::Stored => Ok(true),
            SecretMutation::AlreadyExists => Ok(false),
            SecretMutation::Locked => Err(StorageError::Denied),
            SecretMutation::Absent | SecretMutation::Failed => Err(StorageError::Unavailable),
        }
    }

    fn store(&self, entropy: &[u8], icloud: bool) -> Result<(), StorageError> {
        if icloud {
            return Err(StorageError::Unavailable);
        }
        let mut payload = entropy.to_vec();
        let result = self
            .0
            .replace(PHONE_SERVICE.into(), "words-v1".into(), payload.clone());
        payload.fill(0);
        mutation(result)
    }
}

pub struct AndroidWalletSeed(pub Arc<dyn AndroidSecretStorage>);

impl WalletSeed for AndroidWalletSeed {
    fn read(&self) -> WalletRead {
        match self.0.read(WALLET_SERVICE.into(), WALLET_ACCOUNT.into()) {
            SecretRead::Absent => WalletRead::Absent,
            SecretRead::Present { bytes } => WalletRead::Found(bytes),
            SecretRead::Locked | SecretRead::Failed => WalletRead::Unavailable,
            SecretRead::Corrupt => WalletRead::Unreadable,
        }
    }
}

pub struct AndroidWalletKeyStore(pub Arc<dyn AndroidSecretStorage>);

impl super::super::wallet::seed::KeyStore for AndroidWalletKeyStore {
    fn read_seed(&self) -> super::super::wallet::seed::KeyRead {
        use super::super::wallet::seed::KeyRead;
        match self.0.read(WALLET_SERVICE.into(), WALLET_ACCOUNT.into()) {
            SecretRead::Absent => KeyRead::NotFound,
            SecretRead::Present { bytes } => KeyRead::Found { bytes },
            SecretRead::Locked => KeyRead::Failed { status: -25293 },
            SecretRead::Corrupt => KeyRead::Unreadable,
            SecretRead::Failed => KeyRead::Failed { status: -1 },
        }
    }

    fn add_seed(&self, entropy: Vec<u8>) -> super::super::wallet::seed::KeyAdd {
        use super::super::wallet::seed::KeyAdd;
        match self
            .0
            .create(WALLET_SERVICE.into(), WALLET_ACCOUNT.into(), entropy)
        {
            SecretMutation::Stored => KeyAdd::Added,
            SecretMutation::AlreadyExists => KeyAdd::AlreadyExists,
            SecretMutation::Locked => KeyAdd::Failed { status: -25293 },
            SecretMutation::Absent | SecretMutation::Failed => KeyAdd::Failed { status: -1 },
        }
    }
}

const PAIRING_SERVICE: &str = "com.openagents.app.pairings";
const MAX_PAIRINGS: usize = 32;

/// Existing paired grants stay in Rust's encoded record shape and are stored
/// together under the account-scoped encrypted value.
pub struct AndroidPairings {
    storage: Arc<dyn AndroidSecretStorage>,
    write_lock: Mutex<()>,
}

impl AndroidPairings {
    pub fn new(storage: Arc<dyn AndroidSecretStorage>) -> Self {
        Self {
            storage,
            write_lock: Mutex::new(()),
        }
    }

    fn load_values(
        &self,
        account: &str,
    ) -> Result<Vec<serde_json::Value>, openagents_client::StoreError> {
        let bytes = match self.storage.read(PAIRING_SERVICE.into(), account.into()) {
            SecretRead::Absent => return Ok(Vec::new()),
            SecretRead::Present { bytes } => bytes,
            SecretRead::Locked | SecretRead::Corrupt | SecretRead::Failed => {
                return Err(openagents_client::StoreError);
            }
        };
        let result = serde_json::from_slice(&bytes).map_err(|_| openagents_client::StoreError);
        let mut bytes = bytes;
        bytes.fill(0);
        result
    }

    fn save_values(
        &self,
        account: &str,
        values: &[serde_json::Value],
    ) -> Result<(), openagents_client::StoreError> {
        let mut bytes = serde_json::to_vec(values).map_err(|_| openagents_client::StoreError)?;
        let result = (|| match self.storage.read(PAIRING_SERVICE.into(), account.into()) {
            SecretRead::Absent => {
                self.storage
                    .create(PAIRING_SERVICE.into(), account.into(), bytes.clone())
            }
            SecretRead::Present {
                bytes: mut existing,
            } => {
                existing.fill(0);
                self.storage
                    .replace(PAIRING_SERVICE.into(), account.into(), bytes.clone())
            }
            SecretRead::Locked | SecretRead::Corrupt | SecretRead::Failed => SecretMutation::Failed,
        })();
        bytes.fill(0);
        if result == SecretMutation::Stored {
            Ok(())
        } else {
            Err(openagents_client::StoreError)
        }
    }
}

impl PairingStore for AndroidPairings {
    fn load(
        &self,
        account: &str,
    ) -> Result<Vec<openagents_client::PairedComputer>, openagents_client::StoreError> {
        let mut records = Vec::new();
        for value in self.load_values(account)? {
            let bytes = serde_json::to_vec(&value).map_err(|_| openagents_client::StoreError)?;
            if let Some(record) = openagents_client::PairedComputer::decode(&bytes) {
                if record.account == account {
                    records.push(record);
                }
            }
        }
        records.truncate(MAX_PAIRINGS);
        Ok(records)
    }

    fn save(
        &self,
        computer: &openagents_client::PairedComputer,
    ) -> Result<(), openagents_client::StoreError> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| openagents_client::StoreError)?;
        let mut values = self.load_values(&computer.account)?;
        let entry: serde_json::Value = serde_json::from_slice(&computer.encode())
            .map_err(|_| openagents_client::StoreError)?;
        values.retain(|value| {
            value.get("device_id").and_then(serde_json::Value::as_str) != Some(&computer.device_id)
        });
        if values.len() >= MAX_PAIRINGS {
            return Err(openagents_client::StoreError);
        }
        values.push(entry);
        self.save_values(&computer.account, &values)
    }

    fn delete(&self, account: &str, device_id: &str) -> Result<(), openagents_client::StoreError> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| openagents_client::StoreError)?;
        let mut values = self.load_values(account)?;
        values.retain(|value| {
            value.get("device_id").and_then(serde_json::Value::as_str) != Some(device_id)
        });
        if values.is_empty() {
            match self.storage.delete(PAIRING_SERVICE.into(), account.into()) {
                SecretMutation::Stored | SecretMutation::Absent => Ok(()),
                _ => Err(openagents_client::StoreError),
            }
        } else {
            self.save_values(account, &values)
        }
    }
}

#[uniffi::export]
pub fn android_wallet_key_store(
    storage: Arc<dyn AndroidSecretStorage>,
) -> Arc<dyn super::super::wallet::seed::KeyStore> {
    Arc::new(AndroidWalletKeyStore(storage))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct MemoryProvider {
        values: Mutex<HashMap<(String, String), Vec<u8>>>,
        next_read: Mutex<Option<SecretRead>>,
        creates: Mutex<usize>,
    }

    impl AndroidSecretStorage for MemoryProvider {
        fn read(&self, namespace: String, key: String) -> SecretRead {
            if let Some(result) = self.next_read.lock().unwrap().take() {
                return result;
            }
            self.values
                .lock()
                .unwrap()
                .get(&(namespace, key))
                .cloned()
                .map_or(SecretRead::Absent, |bytes| SecretRead::Present { bytes })
        }
        fn create(&self, namespace: String, key: String, bytes: Vec<u8>) -> SecretMutation {
            *self.creates.lock().unwrap() += 1;
            let mut values = self.values.lock().unwrap();
            if values.contains_key(&(namespace.clone(), key.clone())) {
                SecretMutation::AlreadyExists
            } else {
                values.insert((namespace, key), bytes);
                SecretMutation::Stored
            }
        }
        fn replace(&self, namespace: String, key: String, bytes: Vec<u8>) -> SecretMutation {
            self.values.lock().unwrap().insert((namespace, key), bytes);
            SecretMutation::Stored
        }
        fn delete(&self, namespace: String, key: String) -> SecretMutation {
            if self
                .values
                .lock()
                .unwrap()
                .remove(&(namespace, key))
                .is_some()
            {
                SecretMutation::Stored
            } else {
                SecretMutation::Absent
            }
        }
    }

    #[test]
    fn wallet_read_before_create_and_racing_add_preserve_the_first_seed() {
        use super::super::wallet::seed::{KeyAdd, KeyRead, KeyStore};
        let provider = Arc::new(MemoryProvider::default());
        let store = AndroidWalletKeyStore(provider.clone());
        assert_eq!(store.read_seed(), KeyRead::NotFound);
        assert_eq!(store.add_seed(vec![1, 2, 3]), KeyAdd::Added);
        assert_eq!(store.add_seed(vec![4, 5, 6]), KeyAdd::AlreadyExists);
        assert_eq!(
            store.read_seed(),
            KeyRead::Found {
                bytes: vec![1, 2, 3]
            }
        );
        assert_eq!(
            AndroidWalletKeyStore(provider).read_seed(),
            KeyRead::Found {
                bytes: vec![1, 2, 3]
            }
        );
    }

    #[test]
    fn locked_or_corrupt_identity_reads_fail_without_creating_a_secret() {
        let provider = Arc::new(MemoryProvider::default());
        *provider.next_read.lock().unwrap() = Some(SecretRead::Locked);
        assert!(matches!(
            AndroidIdentityVault(provider.clone()).load(),
            Err(StorageError::Denied)
        ));
        *provider.next_read.lock().unwrap() = Some(SecretRead::Corrupt);
        assert!(matches!(
            AndroidIdentityVault(provider.clone()).load(),
            Err(StorageError::Unavailable)
        ));
        assert_eq!(*provider.creates.lock().unwrap(), 0);
    }

    #[test]
    fn identity_lifecycle_does_not_replace_a_locked_existing_identity() {
        use openagents_account::identity::{Identities, IdentityError};

        let provider = Arc::new(MemoryProvider::default());
        *provider.next_read.lock().unwrap() = Some(SecretRead::Locked);
        let directory = tempfile::tempdir().unwrap();
        let identities = Identities::new(
            Arc::new(AndroidIdentityVault(provider.clone())),
            Arc::new(AndroidWalletSeed(provider.clone())),
            directory.path(),
        );

        assert!(matches!(
            identities.load_or_create(),
            Err(IdentityError::Locked)
        ));
        assert!(provider.values.lock().unwrap().is_empty());
        assert_eq!(*provider.creates.lock().unwrap(), 0);
    }

    #[test]
    fn racing_identity_creation_keeps_the_first_value() {
        use openagents_account::identity::Identities;

        let provider = Arc::new(MemoryProvider::default());
        let stored = vec![1; 16];
        provider
            .values
            .lock()
            .unwrap()
            .insert((PHONE_SERVICE.into(), "words-v1".into()), stored.clone());
        *provider.next_read.lock().unwrap() = Some(SecretRead::Absent);
        let directory = tempfile::tempdir().unwrap();
        let identities = Identities::new(
            Arc::new(AndroidIdentityVault(provider.clone())),
            Arc::new(AndroidWalletSeed(provider.clone())),
            directory.path(),
        );

        let opened = identities.load_or_create().unwrap();
        assert_eq!(opened.entropy(), stored);
        assert_eq!(
            AndroidIdentityVault(provider.clone()).load().unwrap(),
            Some((stored, false))
        );
        assert_eq!(*provider.creates.lock().unwrap(), 1);
    }

    #[test]
    fn corrupt_wallet_key_is_unreadable_and_is_not_replaced() {
        use super::super::wallet::seed::{KeyRead, KeyStore};
        let provider = Arc::new(MemoryProvider::default());
        *provider.next_read.lock().unwrap() = Some(SecretRead::Corrupt);

        assert_eq!(
            AndroidWalletKeyStore(provider.clone()).read_seed(),
            KeyRead::Unreadable
        );
        assert_eq!(*provider.creates.lock().unwrap(), 0);
    }

    #[test]
    fn wallet_creation_fails_closed_on_locked_or_corrupt_reads() {
        use super::super::wallet::seed::{SeedError, create};
        let provider = Arc::new(MemoryProvider::default());
        let store = AndroidWalletKeyStore(provider.clone());

        *provider.next_read.lock().unwrap() = Some(SecretRead::Locked);
        assert!(matches!(create(&store), Err(SeedError::Unavailable)));
        assert_eq!(*provider.creates.lock().unwrap(), 0);

        *provider.next_read.lock().unwrap() = Some(SecretRead::Corrupt);
        assert!(matches!(create(&store), Err(SeedError::Unreadable)));
        assert_eq!(*provider.creates.lock().unwrap(), 0);
    }
}
