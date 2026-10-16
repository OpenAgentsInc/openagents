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
    let deployment = match (
        std::env::var("CODER_DESKTOP_API_URL").ok(),
        std::env::var("CODER_DESKTOP_WEBSITE_URL").ok(),
    ) {
        (None, None) => Deployment::production(),
        (Some(api), Some(site)) => Deployment::new(&api, &site)?,
        _ => return Err(Error::InvalidDeployment),
    };
    let credential = startup_credential()
        .lock()
        .unwrap()
        .take()
        .filter(|token| !token.is_empty());
    let store: Arc<dyn CredentialStore + Send + Sync> = if credential.is_some() {
        Arc::new(MemoryStore::default())
    } else {
        let bundle = if cfg!(debug_assertions) {
            coder_brand::DEV_BUNDLE_ID
        } else {
            coder_brand::BUNDLE_ID
        };
        Arc::new(PlatformStore::new(
            format!("{bundle}.account"),
            data.to_string_lossy().into_owned(),
        ))
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
