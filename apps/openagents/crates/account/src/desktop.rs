//! One app account shared by the desktop controls and their embedded engine.
use crate::{Controller, PlatformStore};
use openagents::{Client, CredentialRecord, CredentialStore, Deployment, Error, StorageError};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, Weak},
};
fn registry() -> &'static Mutex<HashMap<PathBuf, Weak<Controller>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<PathBuf, Weak<Controller>>>> = OnceLock::new();
    REGISTRY.get_or_init(Mutex::default)
}
pub fn register(data: &Path, controller: &Arc<Controller>) {
    let mut entries = registry().lock().unwrap();
    entries.retain(|_, weak| weak.strong_count() != 0);
    if let Some(old) = entries
        .insert(data.to_path_buf(), Arc::downgrade(controller))
        .and_then(|weak| weak.upgrade())
    {
        if !Arc::ptr_eq(&old, controller) {
            old.invalidate();
        }
    }
}
pub fn registered(data: &Path) -> Option<Arc<Controller>> {
    registry().lock().unwrap().get(data).and_then(Weak::upgrade)
}
/// Shared cache identity; neither a server nor a different account can reuse it.
pub fn sync_scope(deployment: &Deployment, edge: &str, account: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            format!(
                "{}\n{}\n{}\n{}",
                deployment.api_base(),
                deployment.website(),
                edge.trim_end_matches('/'),
                account
            )
            .as_bytes()
        )
    )
}
#[derive(Default)]
pub struct MemoryStore(Mutex<Option<CredentialRecord>>);
impl CredentialStore for MemoryStore {
    fn load(&self, deployment: &Deployment) -> Result<Option<CredentialRecord>, StorageError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .as_ref()
            .filter(|record| record.deployment() == deployment)
            .cloned())
    }
    fn save(&self, record: &CredentialRecord) -> Result<(), StorageError> {
        *self.0.lock().unwrap() = Some(record.clone());
        Ok(())
    }
    fn clear(&self, deployment: &Deployment) -> Result<(), StorageError> {
        let mut record = self.0.lock().unwrap();
        if record
            .as_ref()
            .is_some_and(|record| record.deployment() == deployment)
        {
            *record = None;
        }
        Ok(())
    }
}
fn startup_credential() -> &'static Mutex<Option<String>> {
    static CREDENTIAL: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    CREDENTIAL.get_or_init(Mutex::default)
}
/// The launcher removes this value from its environment before starting threads or agents.
pub fn set_startup_credential(credential: Option<String>) {
    *startup_credential().lock().unwrap() = credential;
}
/// Explicit environment credentials stay in memory. Normal app logins use Keychain.
/// The returned credential must be authenticated before it can enable sync.
pub fn configured(data: &Path) -> Result<(Arc<Controller>, Option<String>), Error> {
    let deployment = deployment()?;
    let credential = startup_credential()
        .lock()
        .unwrap()
        .take()
        .filter(|token| !token.is_empty());
    let store: Arc<dyn CredentialStore + Send + Sync> = if credential.is_some() {
        Arc::new(MemoryStore::default())
    } else {
        Arc::new(saved_store(data))
    };
    let marker = if credential.is_some() {
        "openagents-account/temporary-signed-out"
    } else {
        "openagents-account/signed-out"
    };
    let controller = Controller::new(Client::new(deployment)?, store, data.join(marker));
    register(data, &controller);
    Ok((controller, credential))
}

fn deployment() -> Result<Deployment, Error> {
    match (
        std::env::var("CODER_DESKTOP_API_URL").ok(),
        std::env::var("CODER_DESKTOP_WEBSITE_URL").ok(),
    ) {
        (None, None) => Ok(Deployment::production()),
        (Some(api), Some(site)) => Deployment::new(&api, &site),
        _ => Err(Error::InvalidDeployment),
    }
}

fn saved_store(data: &Path) -> PlatformStore {
    let (bundle, legacy_bundle) = if cfg!(debug_assertions) {
        (openagents_brand::DEV_BUNDLE_ID, openagents_brand::LEGACY_DEV_BUNDLE_ID)
    } else {
        (openagents_brand::BUNDLE_ID, openagents_brand::LEGACY_BUNDLE_ID)
    };
    let profile = data.to_string_lossy().into_owned();
    PlatformStore::new(format!("{bundle}.account"), profile)
        .with_legacy(legacy_entries(data, bundle, legacy_bundle))
}

/// Whether a process started now without a startup credential, such as the
/// background engine (#11931), finds a saved sign-in for `data`.
pub fn has_saved_sign_in(data: &Path) -> bool {
    let Ok(deployment) = deployment() else {
        return false;
    };
    saved_store(data).load(&deployment).ok().flatten().is_some()
}

/// Earlier Keychain entries to carry a sign-in over from: the pre-rename
/// bundle's entry for this data directory, then both bundles' entries for
/// each directory listed in `openagents-account/previous-profiles` (one
/// absolute path per line, written when a data directory is moved).
fn legacy_entries(data: &std::path::Path, bundle: &str, legacy_bundle: &str) -> Vec<(String, String)> {
    let current = data.to_string_lossy().into_owned();
    let mut profiles = vec![current.clone()];
    if let Ok(text) = std::fs::read_to_string(data.join("openagents-account/previous-profiles")) {
        profiles.extend(
            text.lines()
                .map(str::trim)
                .filter(|line| line.starts_with('/') && *line != current)
                .map(str::to_owned),
        );
    }
    let mut entries = Vec::new();
    for profile in profiles {
        for service in [bundle, legacy_bundle] {
            if service == bundle && profile == current {
                continue;
            }
            entries.push((format!("{service}.account"), profile.clone()));
        }
    }
    entries
}

#[cfg(test)]
mod legacy_tests {
    use super::legacy_entries;

    #[test]
    fn sign_in_carries_over_from_the_old_bundle_and_moved_data_directories() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        std::fs::create_dir_all(data.join("openagents-account")).unwrap();
        std::fs::write(
            data.join("openagents-account/previous-profiles"),
            "/old/checkout/target/macos-dev/data\nnot-absolute\n",
        )
        .unwrap();
        let current = data.to_string_lossy().into_owned();
        let entries = legacy_entries(&data, "new.bundle", "old.bundle");
        assert_eq!(
            entries,
            vec![
                ("old.bundle.account".to_owned(), current),
                ("new.bundle.account".to_owned(), "/old/checkout/target/macos-dev/data".to_owned()),
                ("old.bundle.account".to_owned(), "/old/checkout/target/macos-dev/data".to_owned()),
            ]
        );
    }
}
