//! The same account controller used by the desktop, exposed to native controls.
use super::{ClientListener, CoreClient, CoreConfig, CoreError, CoreResult, ListenerBridge};
use coder_account::{
    Controller, Phase, PlatformStore,
    openagents::{Client, CredentialStore, Deployment},
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;
#[derive(Clone, Debug, uniffi::Enum)]
pub enum AccountState {
    SignedOut { notice: Option<String> },
    Restoring,
    Starting,
    Waiting { code: String, url: String },
    SignedIn { account_id: String, name: String },
    Failed { message: String, can_restore: bool },
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct AccountSnapshot {
    pub generation: u64,
    pub state: AccountState,
}
impl From<coder_account::Snapshot> for AccountSnapshot {
    fn from(snapshot: coder_account::Snapshot) -> Self {
        Self {
            generation: snapshot.generation,
            state: match snapshot.phase {
                Phase::SignedOut { notice } => AccountState::SignedOut { notice },
                Phase::Restoring => AccountState::Restoring,
                Phase::Starting => AccountState::Starting,
                Phase::Waiting { code, url } => AccountState::Waiting { code, url },
                Phase::SignedIn { account } => AccountState::SignedIn {
                    account_id: account.id,
                    name: account.label,
                },
                Phase::Failed {
                    message,
                    can_restore,
                } => AccountState::Failed {
                    message,
                    can_restore,
                },
            },
        }
    }
}
#[uniffi::export(with_foreign)]
pub trait AccountListener: Send + Sync {
    fn on_account_changed(&self, snapshot: AccountSnapshot);
}
type Clients = Arc<Mutex<Vec<(u64, Weak<CoreClient>)>>>;
fn close_clients(clients: &Clients, keep: Option<u64>) {
    clients.lock().unwrap().retain(|(generation, client)| {
        let Some(client) = client.upgrade() else {
            return false;
        };
        if keep == Some(*generation) {
            true
        } else {
            client.client.shutdown();
            false
        }
    });
}
fn auth_error() -> CoreError {
    CoreError::Auth {
        message: "Log in again to continue.".into(),
    }
}
#[derive(uniffi::Object)]
pub struct CoreAccount {
    controller: Arc<Controller>,
    store: Arc<dyn CredentialStore + Send + Sync>,
    deployment: Deployment,
    data_dir: PathBuf,
    clients: Clients,
    cache: Mutex<Option<PathBuf>>,
    cancel: CancellationToken,
}
impl CoreAccount {
    fn from_store(
        client: Client,
        store: Arc<dyn CredentialStore + Send + Sync>,
        data_dir: PathBuf,
        listener: Arc<dyn AccountListener>,
    ) -> Arc<Self> {
        let deployment = client.deployment().clone();
        let controller = Controller::new_named(
            client,
            store.clone(),
            data_dir.join("account/signed-out"),
            "OpenAgents Coder".into(),
        );
        let cancel = CancellationToken::new();
        let clients: Clients = Arc::default();
        let this = Arc::new(Self {
            controller: controller.clone(),
            store,
            deployment,
            data_dir,
            clients: clients.clone(),
            cache: Mutex::new(None),
            cancel: cancel.clone(),
        });
        let mut updates = controller.watch();
        zeron_client::runtime::shared().spawn(async move {
            listener.on_account_changed(controller.snapshot().into());
            let mut timer = tokio::time::interval(Duration::from_secs(1));
            let mut checked = Instant::now();
            loop {
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => break,
                    changed = updates.changed() => {
                        if changed.is_err() { break; }
                        let snapshot = updates.borrow_and_update().clone();
                        let keep = matches!(snapshot.phase, Phase::SignedIn { .. })
                            .then_some(snapshot.generation);
                        close_clients(&clients, keep);
                        listener.on_account_changed(snapshot.into());
                    }
                    _ = timer.tick() => {
                        if matches!(controller.snapshot().phase, Phase::SignedIn { .. })
                            && (controller.lease().is_none()
                                || checked.elapsed() >= Duration::from_secs(15))
                        {
                            checked = Instant::now();
                            let controller = controller.clone();
                            zeron_client::runtime::shared().spawn(controller.revalidate());
                        }
                    }
                }
            }
        });
        this
    }
    fn profile(&self, account: &str, edge: &str) -> PathBuf {
        self.data_dir.join(format!(
            "account-{}",
            coder_account::desktop::sync_scope(&self.deployment, edge, account)
        ))
    }
}
#[uniffi::export]
impl CoreAccount {
    /// Read the same account history as the desktop without starting an engine.
    pub fn saved_history(
        &self,
        listener: Arc<dyn super::SavedHistoryListener>,
    ) -> CoreResult<Arc<super::CoreSavedHistory>> {
        let lease = self.controller.lease().ok_or_else(auth_error)?;
        Ok(super::CoreSavedHistory::new(
            lease,
            self.controller.watch(),
            listener,
        ))
    }
    #[uniffi::constructor]
    pub fn new(
        data_dir: String,
        api_url: String,
        website_url: String,
        listener: Arc<dyn AccountListener>,
    ) -> CoreResult<Arc<Self>> {
        let deployment =
            Deployment::new(&api_url, &website_url).map_err(|e| CoreError::InvalidArgument {
                message: e.to_string(),
            })?;
        let client = Client::with_timeout(deployment, Duration::from_secs(10)).map_err(|e| {
            CoreError::InvalidArgument {
                message: e.to_string(),
            }
        })?;
        let store = Arc::new(PlatformStore::new(
            "com.openagents.coder.account".into(),
            data_dir.clone(),
        ));
        Ok(Self::from_store(
            client,
            store,
            PathBuf::from(data_dir),
            listener,
        ))
    }
    pub fn snapshot(&self) -> AccountSnapshot {
        self.controller.snapshot().into()
    }
    pub fn restore(&self) {
        let task = self.controller.restore();
        close_clients(&self.clients, None);
        zeron_client::runtime::shared().spawn(task);
    }
    /// Resume account checks without cancelling a device-code approval in progress.
    pub fn revalidate(&self) {
        zeron_client::runtime::shared().spawn(self.controller.clone().revalidate());
    }
    pub fn sign_in(&self, device_name: String) {
        let task = self.controller.sign_in(device_name);
        close_clients(&self.clients, None);
        zeron_client::runtime::shared().spawn(task);
    }
    /// Explicit credentials are checked by the same SDK and stored by the same controller.
    pub fn sign_in_with_credential(&self, credential: String) {
        let task = self.controller.import_credential(credential);
        close_clients(&self.clients, None);
        zeron_client::runtime::shared().spawn(task);
    }
    pub fn cancel(&self) {
        self.sign_out();
    }
    pub fn sign_out(&self) {
        let task = self.controller.sign_out();
        close_clients(&self.clients, None);
        zeron_client::runtime::shared().spawn(task);
    }
    /// A native diagnostic checks only this app/profile/deployment's own entry.
    pub fn stored_login_present(&self) -> CoreResult<bool> {
        self.store
            .load(&self.deployment)
            .map(|record| record.is_some())
            .map_err(|_| CoreError::Storage {
                message: "Couldn't check the saved login.".into(),
            })
    }
    /// Delete only the last connected account cache after an explicit sign-out.
    pub fn clear_local_data(&self) -> CoreResult<()> {
        let mut cache = self.cache.lock().unwrap();
        if self.controller.lease().is_some() {
            return Err(auth_error());
        }
        if let Some(path) = cache.as_ref() {
            match std::fs::remove_dir_all(path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => {
                    return Err(CoreError::Storage {
                        message: "Couldn't remove the local chats.".into(),
                    });
                }
            }
        }
        *cache = None;
        Ok(())
    }
    /// Open only the current account's registry and document cache.
    pub fn connect(
        &self,
        mut config: CoreConfig,
        listener: Arc<dyn ClientListener>,
    ) -> CoreResult<Arc<CoreClient>> {
        let mut cache = self.cache.lock().unwrap();
        let lease = self.controller.lease().ok_or_else(auth_error)?;
        config.data_dir = self
            .profile(&lease.session().account().id, &config.edge_url)
            .to_string_lossy()
            .into_owned();
        let chosen_cache = PathBuf::from(&config.data_dir);
        let credentials = zeron_client::Credentials::OpenAgents {
            account_id: lease.session().account().id.clone(),
            bearer: lease.session().bearer_token().into(),
            expires_at: lease.session().expires_at(),
        };
        let client = zeron_client::Client::new(
            config.into(),
            credentials,
            Arc::new(ListenerBridge(listener)),
        )?;
        let core = Arc::new(CoreClient { client });
        let mut clients = self.clients.lock().unwrap();
        if !self.controller.accepts(&lease) {
            core.client.shutdown();
            return Err(auth_error());
        }
        *cache = Some(chosen_cache);
        clients.push((lease.generation, Arc::downgrade(&core)));
        Ok(core)
    }
}
impl Drop for CoreAccount {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.controller.invalidate();
        close_clients(&self.clients, None);
    }
}

#[cfg(feature = "account-fixture")]
#[uniffi::export]
impl CoreAccount {
    /// Test-only loopback delivery keeps the credential out of Swift and Xcode logs.
    pub fn sign_in_from_fixture(&self, url: String) {
        let controller = self.controller.clone();
        close_clients(&self.clients, None);
        zeron_client::runtime::shared().spawn(async move {
            let Ok(url) = reqwest::Url::parse(&url) else {
                return;
            };
            if url.scheme() != "http" || url.host_str() != Some("127.0.0.1") {
                return;
            }
            let Ok(client) = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(10))
                .build()
            else {
                return;
            };
            let Ok(response) = client.get(url).send().await else {
                return;
            };
            if !response.status().is_success()
                || response.content_length().is_none_or(|len| len > 4096)
            {
                return;
            }
            let Ok(body) = response.json::<serde_json::Value>().await else {
                return;
            };
            let Some(credential) = body["credential"]
                .as_str()
                .filter(|token| token.len() < 4096)
            else {
                return;
            };
            controller.import_credential(credential.to_owned()).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_account::openagents::{CredentialRecord, StorageError};
    struct Memory;
    impl CredentialStore for Memory {
        fn load(&self, _: &Deployment) -> Result<Option<CredentialRecord>, StorageError> {
            Ok(None)
        }
        fn save(&self, _: &CredentialRecord) -> Result<(), StorageError> {
            Ok(())
        }
        fn clear(&self, _: &Deployment) -> Result<(), StorageError> {
            Ok(())
        }
    }
    struct Silent;
    impl AccountListener for Silent {
        fn on_account_changed(&self, _: AccountSnapshot) {}
    }
    #[tokio::test]
    async fn caches_are_scoped_and_logout_keeps_the_restore_fence() {
        let root = tempfile::tempdir().unwrap();
        let account = CoreAccount::from_store(
            Client::new(Deployment::production()).unwrap(),
            Arc::new(Memory),
            root.path().into(),
            Arc::new(Silent),
        );
        let edge = "https://sync.openagents.com";
        let cache = account.profile("acct_a", edge);
        assert_ne!(cache, account.profile("acct_b", edge));
        assert_ne!(cache, account.profile("acct_a", "https://other.example"));
        assert_eq!(
            cache,
            account.profile("acct_a", "https://sync.openagents.com/")
        );
        let other = CoreAccount::from_store(
            Client::new(
                Deployment::new("https://api.other.example/v1", "https://other.example").unwrap(),
            )
            .unwrap(),
            Arc::new(Memory),
            root.path().into(),
            Arc::new(Silent),
        );
        assert_ne!(cache, other.profile("acct_a", edge));
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(cache.join("fixture"), b"private fixture").unwrap();
        *account.cache.lock().unwrap() = Some(cache.clone());
        account.controller.sign_out().await;
        account.clear_local_data().unwrap();
        assert!(!cache.exists());
        assert!(root.path().join("account").is_dir());
        assert!(
            std::fs::read_dir(root.path().join("account"))
                .unwrap()
                .next()
                .is_some()
        );
    }
}
