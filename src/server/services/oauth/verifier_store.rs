use oauth2::PkceCodeVerifier;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

#[derive(Debug)]
struct StoredVerifier {
    verifier: PkceCodeVerifier,
    created_at: Instant,
}

impl Clone for StoredVerifier {
    fn clone(&self) -> Self {
        Self {
            verifier: PkceCodeVerifier::new(self.verifier.secret().to_string()),
            created_at: self.created_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VerifierStore {
    store: Arc<RwLock<HashMap<String, StoredVerifier>>>,
}

impl VerifierStore {
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn store_verifier(&self, state: &str, verifier: PkceCodeVerifier) {
        let mut store = self.store.write().unwrap();
        store.insert(
            state.to_string(),
            StoredVerifier {
                verifier,
                created_at: Instant::now(),
            },
        );
        self.cleanup_old_verifiers();
    }

    pub fn get_verifier(&self, state: &str) -> Option<PkceCodeVerifier> {
        let mut store = self.store.write().unwrap();
        store
            .remove(state)
            .map(|stored| PkceCodeVerifier::new(stored.verifier.secret().to_string()))
    }

    fn cleanup_old_verifiers(&self) {
        let mut store = self.store.write().unwrap();
        let now = Instant::now();
        store.retain(|_, stored| {
            now.duration_since(stored.created_at) < Duration::from_secs(300) // 5 minutes
        });
    }
}

impl Default for VerifierStore {
    fn default() -> Self {
        Self::new()
    }
}
