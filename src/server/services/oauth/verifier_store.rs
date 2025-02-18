use oauth2::PkceCodeVerifier;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};
use tracing::{error, info};

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
        info!("Creating new VerifierStore");
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn store_verifier(&self, state: &str, verifier: PkceCodeVerifier) {
        info!("Storing verifier for state: {}", state);
        info!("Verifier secret length: {}", verifier.secret().len());

        let mut store = match self.store.write() {
            Ok(store) => store,
            Err(e) => {
                error!("Failed to acquire write lock: {}", e);
                return;
            }
        };

        let current_count = store.len();
        store.insert(
            state.to_string(),
            StoredVerifier {
                verifier,
                created_at: Instant::now(),
            },
        );
        info!(
            "Stored verifier. Store size before: {}, after: {}",
            current_count,
            store.len()
        );

        self.cleanup_old_verifiers();
    }

    pub fn get_verifier(&self, state: &str) -> Option<PkceCodeVerifier> {
        info!("Attempting to get verifier for state: {}", state);

        let mut store = match self.store.write() {
            Ok(store) => store,
            Err(e) => {
                error!("Failed to acquire write lock: {}", e);
                return None;
            }
        };

        let current_keys: Vec<String> = store.keys().cloned().collect();
        info!("Current states in store: {:?}", current_keys);

        let result = store.remove(state).map(|stored| {
            info!(
                "Found and removed verifier for state: {}. Age: {:?}",
                state,
                stored.created_at.elapsed()
            );
            PkceCodeVerifier::new(stored.verifier.secret().to_string())
        });

        if result.is_none() {
            error!("No verifier found for state: {}", state);
        }

        result
    }

    fn cleanup_old_verifiers(&self) {
        let mut store = match self.store.write() {
            Ok(store) => store,
            Err(e) => {
                error!("Failed to acquire write lock for cleanup: {}", e);
                return;
            }
        };

        let before_count = store.len();
        let now = Instant::now();
        store.retain(|state, stored| {
            let age = now.duration_since(stored.created_at);
            let keep = age < Duration::from_secs(300); // 5 minutes
            if !keep {
                info!(
                    "Removing expired verifier for state: {}. Age: {:?}",
                    state, age
                );
            }
            keep
        });

        info!(
            "Cleaned up verifiers. Before: {}, After: {}",
            before_count,
            store.len()
        );
    }
}

impl Default for VerifierStore {
    fn default() -> Self {
        Self::new()
    }
}
