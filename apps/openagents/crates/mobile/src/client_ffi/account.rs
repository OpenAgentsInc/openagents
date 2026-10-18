//! The same account controller used by the desktop, exposed to native controls.
use super::{
    AndroidIdentityVault, AndroidPairings, AndroidPasswords, AndroidSecretStorage,
    AndroidWalletSeed,
};
use super::{ClientListener, CoreClient, CoreConfig, CoreError, CoreResult, ListenerBridge};
use openagents_account::{
    Controller, Phase, PlatformStore,
    identity::{Identities, Identity},
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
impl From<openagents_account::Snapshot> for AccountSnapshot {
    fn from(snapshot: openagents_account::Snapshot) -> Self {
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
fn is_current_client<T>(clients: &[(u64, Weak<T>)], generation: u64, candidate: &Arc<T>) -> bool {
    clients.iter().any(|(client_generation, client)| {
        *client_generation == generation
            && client
                .upgrade()
                .is_some_and(|current| Arc::ptr_eq(&current, candidate))
    })
}
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

/// A server-checked invitation preview for the currently signed-in account.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ProjectInvitationPreview {
    pub project_name: String,
    pub role: String,
    pub expires_at: String,
    pub already_member: bool,
}

fn invitation_http() -> CoreResult<reqwest::Client> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| CoreError::Network {
            message: "Couldn't connect to the invitation service.".into(),
        })
}

async fn invitation_post(
    http: &reqwest::Client,
    website: &reqwest::Url,
    bearer: &str,
    path: &str,
    code: &str,
) -> CoreResult<serde_json::Value> {
    let url = website.join(path).map_err(|_| CoreError::Internal {
        message: "The account service address is invalid.".into(),
    })?;
    let response = http
        .post(url)
        .bearer_auth(bearer)
        .json(&serde_json::json!({ "code": code }))
        .send()
        .await
        .map_err(|_| CoreError::Network {
            message: "Couldn't reach the invitation service.".into(),
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(if status == reqwest::StatusCode::UNAUTHORIZED {
            CoreError::Auth {
                message: "Log in again to continue.".into(),
            }
        } else {
            CoreError::HostError {
                message: "This invitation is unavailable for this account or has expired.".into(),
            }
        });
    }
    response.json().await.map_err(|_| CoreError::HostError {
        message: "The invitation service returned an unreadable response.".into(),
    })
}

fn invitation_preview(body: &serde_json::Value) -> CoreResult<ProjectInvitationPreview> {
    let invalid = || CoreError::HostError {
        message: "The invitation service returned an invalid preview.".into(),
    };
    let project_name = body["project"]["name"]
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(invalid)?
        .to_owned();
    let role = body["role"]
        .as_str()
        .filter(|value| matches!(*value, "viewer" | "editor"))
        .ok_or_else(invalid)?
        .to_owned();
    let expires_at = body["expires_at"]
        .as_str()
        .filter(|expiry| !expiry.trim().is_empty())
        .ok_or_else(invalid)?;
    let already_member = body["member"].as_bool().ok_or_else(invalid)?;
    Ok(ProjectInvitationPreview {
        project_name,
        role,
        expires_at: expires_at.to_owned(),
        already_member,
    })
}

fn invitation_code(link: &str) -> CoreResult<String> {
    const PROJECT_INVITE_PREFIX: &str = "https://openagents.com/projects/join/";
    super::project_invitation::validated_project_invitation_link(link)
        .and_then(|url| url.strip_prefix(PROJECT_INVITE_PREFIX))
        .map(str::to_owned)
        .ok_or_else(|| CoreError::InvalidArgument {
            message: "This project invitation link is invalid.".into(),
        })
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
    /// Where the 12 words live (#11995).
    pub(crate) identities: Arc<Identities>,
    /// The identity once loaded or made on launch.
    pub(crate) identity: Arc<Mutex<Option<Arc<Identity>>>>,
    /// This device's name, sent with each key sign-in.
    pub(crate) device_name: Arc<Mutex<String>>,
    pairings: Option<Arc<dyn openagents_client::PairingStore>>,
}
impl CoreAccount {
    /// An account with no identity store, for tests: the words are never
    /// read from or written to a real Keychain.
    pub(crate) fn from_store(
        client: Client,
        store: Arc<dyn CredentialStore + Send + Sync>,
        data_dir: PathBuf,
        listener: Arc<dyn AccountListener>,
    ) -> Arc<Self> {
        let identities = Identities::new(
            Arc::new(super::identity::NoVault),
            Arc::new(openagents_account::identity::NoWallet),
            &data_dir.join("identity"),
        );
        Self::from_parts(client, store, identities, data_dir, listener)
    }
    pub(crate) fn from_parts(
        client: Client,
        store: Arc<dyn CredentialStore + Send + Sync>,
        identities: Identities,
        data_dir: PathBuf,
        listener: Arc<dyn AccountListener>,
    ) -> Arc<Self> {
        Self::from_parts_with_pairings(client, store, identities, data_dir, listener, None)
    }

    fn from_parts_with_pairings(
        client: Client,
        store: Arc<dyn CredentialStore + Send + Sync>,
        identities: Identities,
        data_dir: PathBuf,
        listener: Arc<dyn AccountListener>,
        pairings: Option<Arc<dyn openagents_client::PairingStore>>,
    ) -> Arc<Self> {
        let deployment = client.deployment().clone();
        let controller = Controller::new_named(
            client,
            store.clone(),
            data_dir.join("account/signed-out"),
            "OpenAgents".into(),
        );
        let cancel = CancellationToken::new();
        let clients: Clients = Arc::default();
        let identity: Arc<Mutex<Option<Arc<Identity>>>> = Arc::default();
        let device_name = Arc::new(Mutex::new(String::from("Phone")));
        let this = Arc::new(Self {
            controller: controller.clone(),
            store,
            deployment,
            data_dir,
            clients: clients.clone(),
            cache: Mutex::new(None),
            cancel: cancel.clone(),
            identities: Arc::new(identities),
            identity: identity.clone(),
            device_name: device_name.clone(),
            pairings,
        });
        let renewing = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut updates = controller.watch();
        openagents_client::runtime::shared().spawn(async move {
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
                            let key = identity.lock().unwrap().clone();
                            // Sign in again with the key before the session
                            // ends (#11995); otherwise check it still works.
                            if let Some(key) = key.filter(|_| controller.renewal_due()) {
                                if !renewing.swap(true, std::sync::atomic::Ordering::SeqCst) {
                                    let renew = controller
                                        .renew_with_key(key, device_name.lock().unwrap().clone());
                                    let renewing = renewing.clone();
                                    openagents_client::runtime::shared().spawn(async move {
                                        renew.await;
                                        renewing.store(false, std::sync::atomic::Ordering::SeqCst);
                                    });
                                }
                            } else {
                                let controller = controller.clone();
                                openagents_client::runtime::shared().spawn(controller.revalidate());
                            }
                        }
                    }
                }
            }
        });
        this
    }
    pub(crate) fn controller(&self) -> &Arc<Controller> {
        &self.controller
    }
    pub(crate) fn data_dir(&self) -> &std::path::Path {
        &self.data_dir
    }
    /// Close this account's open clients before a new sign-in.
    pub(crate) fn close_clients(&self) {
        close_clients(&self.clients, None);
    }
    fn profile(&self, account: &str, edge: &str) -> PathBuf {
        self.data_dir.join(format!(
            "account-{}",
            openagents_account::desktop::sync_scope(&self.deployment, edge, account)
        ))
    }
}
/// Computers paired by code keep their grants in the Keychain (#12000).
fn pairing_store() -> Arc<dyn openagents_client::PairingStore> {
    if cfg!(any(target_os = "ios", target_os = "macos")) && !cfg!(test) {
        Arc::new(openagents_client::KeychainPairings::new(
            "com.openagents.app.pairings",
        ))
    } else {
        Arc::new(openagents_client::MemoryPairings::default())
    }
}

#[uniffi::export]
impl CoreAccount {
    /// Build chooser content from a validated project invitation link.
    pub fn project_invitation_share(
        &self,
        link: String,
    ) -> CoreResult<super::ProjectInvitationShare> {
        super::project_invitation::project_invitation_share(&link).ok_or_else(|| {
            CoreError::InvalidArgument {
                message: "This project invitation link isn't valid.".into(),
            }
        })
    }

    /// Preview a project invitation with the active account. The invitation
    /// code stays in Rust and is sent only to the account API.
    pub async fn preview_project_invitation(
        &self,
        link: String,
    ) -> CoreResult<ProjectInvitationPreview> {
        let code = invitation_code(&link)?;
        let lease = self.controller.lease().ok_or_else(auth_error)?;
        let bearer = lease.session().bearer_token().to_owned();
        let website = self.deployment.website().clone();
        let http = invitation_http()?;
        super::on_runtime(async move {
            let body = invitation_post(
                &http,
                &website,
                &bearer,
                "v1/projects/invitations/preview",
                &code,
            )
            .await?;
            invitation_preview(&body)
        })
        .await
    }

    /// Accept a project invitation for the active account after the person
    /// confirms the preview. The server checks the invitation, expiry, and
    /// recipient against this account's bearer token.
    pub async fn accept_project_invitation(&self, link: String) -> CoreResult<String> {
        let code = invitation_code(&link)?;
        let lease = self.controller.lease().ok_or_else(auth_error)?;
        let bearer = lease.session().bearer_token().to_owned();
        let website = self.deployment.website().clone();
        let http = invitation_http()?;
        super::on_runtime(async move {
            let body = invitation_post(
                &http,
                &website,
                &bearer,
                "v1/projects/invitations/accept",
                &code,
            )
            .await?;
            body["id"]
                .as_str()
                .or_else(|| body["project"]["id"].as_str())
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| CoreError::HostError {
                    message: "The invitation service returned an invalid project.".into(),
                })
        })
        .await
    }

    /// Have the computer `device_id`, just connected by code, sign in as
    /// this account (#12000): its account key is added to the account.
    /// Call it right after `CoreClient::connect_computer`; the computer
    /// answers only shortly after the scan. Answers the computer's key.
    pub async fn link_computer(
        &self,
        client: Arc<CoreClient>,
        device_id: String,
    ) -> CoreResult<String> {
        let api_base = {
            let lease = self.controller.lease().ok_or_else(auth_error)?;
            if !is_current_client(&self.clients.lock().unwrap(), lease.generation, &client) {
                return Err(auth_error());
            }
            lease.client().deployment().api_base().to_string()
        };
        let client = client.client.clone();
        // The phone's own identity key (#11995) is never a computer's.
        let own = self
            .identity
            .lock()
            .unwrap()
            .as_ref()
            .map(|identity| identity.pubkey_hex());
        super::on_runtime(async move {
            client
                .link_computer(&device_id, &api_base, own.as_deref())
                .await
        })
        .await
    }
}

#[uniffi::export]
impl CoreAccount {
    pub fn project_sharing(
        &self,
        listener: Arc<dyn super::SharingListener>,
    ) -> CoreResult<Arc<super::CoreSharing>> {
        let lease = self.controller.lease().ok_or_else(auth_error)?;
        let directory = self
            .profile(&lease.session().account().id, "project-sharing")
            .join("sharing");
        super::CoreSharing::new(lease, &directory, listener)
    }
    pub fn voice_agent(
        &self,
        listener: Arc<dyn super::VoiceAgentListener>,
    ) -> Arc<super::CoreVoiceAgent> {
        super::CoreVoiceAgent::new(self.controller.clone(), listener)
    }
    pub fn agent_lab(
        &self,
        listener: Arc<dyn super::AgentLabListener>,
    ) -> Arc<super::CoreAgentLab> {
        super::CoreAgentLab::new(
            self.controller.clone(),
            self.data_dir.join("agent-lab-requests"),
            listener,
        )
    }
    pub fn custom_agents(
        &self,
        listener: Arc<dyn super::CustomAgentsListener>,
    ) -> Arc<super::CoreAgents> {
        super::CoreAgents::new(
            self.controller.clone(),
            self.data_dir.join("agent-requests"),
            listener,
        )
    }

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
        // Saved-history pages of 200 can take the server over ten seconds.
        let client = Client::with_timeout(deployment, Duration::from_secs(30)).map_err(|e| {
            CoreError::InvalidArgument {
                message: e.to_string(),
            }
        })?;
        #[cfg(target_os = "ios")]
        let store = PlatformStore::mobile("com.openagents.app.account".into(), data_dir.clone());
        #[cfg(not(target_os = "ios"))]
        let store = PlatformStore::new("com.openagents.app.account".into(), data_dir.clone());
        let store = Arc::new(store);
        let data_dir = PathBuf::from(data_dir);
        let identities = Identities::phone(&data_dir);
        Ok(Self::from_parts(
            client, store, identities, data_dir, listener,
        ))
    }

    /// Construct an account whose credentials, identity, wallet seed, and
    /// paired grants use Rust policies over Android's encrypted byte store.
    #[uniffi::constructor]
    pub fn new_with_secure_storage(
        data_dir: String,
        api_url: String,
        website_url: String,
        listener: Arc<dyn AccountListener>,
        storage: Arc<dyn AndroidSecretStorage>,
    ) -> CoreResult<Arc<Self>> {
        let deployment =
            Deployment::new(&api_url, &website_url).map_err(|e| CoreError::InvalidArgument {
                message: e.to_string(),
            })?;
        let client = Client::with_timeout(deployment, Duration::from_secs(30)).map_err(|e| {
            CoreError::InvalidArgument {
                message: e.to_string(),
            }
        })?;
        let passwords = Arc::new(AndroidPasswords(storage.clone()));
        let store = Arc::new(
            PlatformStore::new("com.openagents.app.account".into(), data_dir.clone())
                .with_passwords(passwords),
        );
        let data_dir = PathBuf::from(data_dir);
        let identities = Identities::new(
            Arc::new(AndroidIdentityVault(storage.clone())),
            Arc::new(AndroidWalletSeed(storage.clone())),
            &data_dir.join("identity"),
        );
        let pairings: Arc<dyn openagents_client::PairingStore> =
            Arc::new(AndroidPairings::new(storage));
        Ok(Self::from_parts_with_pairings(
            client,
            store,
            identities,
            data_dir,
            listener,
            Some(pairings),
        ))
    }

    pub fn snapshot(&self) -> AccountSnapshot {
        self.controller.snapshot().into()
    }
    pub fn restore(&self) {
        let task = self.controller.restore();
        close_clients(&self.clients, None);
        openagents_client::runtime::shared().spawn(task);
    }
    /// Resume account checks without cancelling a device-code approval in progress.
    pub fn revalidate(&self) {
        openagents_client::runtime::shared().spawn(self.controller.clone().revalidate());
    }
    /// Explicit credentials are checked by the same SDK and stored by the same controller.
    pub fn sign_in_with_credential(&self, credential: String) {
        let task = self.controller.import_credential(credential);
        close_clients(&self.clients, None);
        openagents_client::runtime::shared().spawn(task);
    }
    pub fn cancel(&self) {
        self.sign_out();
    }
    pub fn sign_out(&self) {
        let task = self.controller.sign_out();
        close_clients(&self.clients, None);
        openagents_client::runtime::shared().spawn(task);
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
        crate::android_bootstrap::ensure_initialized()?;
        let mut cache = self.cache.lock().unwrap();
        let lease = self.controller.lease().ok_or_else(auth_error)?;
        config.data_dir = self
            .profile(&lease.session().account().id, &config.edge_url)
            .to_string_lossy()
            .into_owned();
        let chosen_cache = PathBuf::from(&config.data_dir);
        let credentials = openagents_client::Credentials::OpenAgents {
            account_id: lease.session().account().id.clone(),
            bearer: lease.session().bearer_token().into(),
            expires_at: lease.session().expires_at(),
        };
        let client = openagents_client::Client::new(
            config.into(),
            credentials,
            Arc::new(ListenerBridge(listener)),
        )?;
        client.set_pairing_store(self.pairings.clone().unwrap_or_else(pairing_store));
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
    /// Test-only device-code sign-in for the account fixture's tests. The
    /// app itself signs in with its key and never shows a code (#11995).
    pub fn sign_in(&self, device_name: String) {
        let task = self.controller.sign_in(device_name);
        close_clients(&self.clients, None);
        openagents_client::runtime::shared().spawn(task);
    }
    /// Test-only loopback delivery keeps the credential out of Swift and Xcode logs.
    pub fn sign_in_from_fixture(&self, url: String) {
        let controller = self.controller.clone();
        close_clients(&self.clients, None);
        openagents_client::runtime::shared().spawn(async move {
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
    use openagents_account::openagents::{CredentialRecord, StorageError};

    #[test]
    fn computer_link_accepts_only_the_current_accounts_client() {
        let current = Arc::new(());
        let other_account = Arc::new(());
        let clients = vec![(7, Arc::downgrade(&current))];

        assert!(is_current_client(&clients, 7, &current));
        assert!(!is_current_client(&clients, 8, &current));
        assert!(!is_current_client(&clients, 7, &other_account));
    }

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
    async fn signing_out_cancels_a_pending_cloud_start() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let server_entered = entered.clone();
        let server_release = release.clone();
        let server = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let entered = server_entered.clone();
                let release = server_release.clone();
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    let mut buf = [0; 1024];
                    while !request.windows(4).any(|v| v == b"\r\n\r\n") {
                        let n = socket.read(&mut buf).await.unwrap();
                        if n == 0 {
                            return;
                        }
                        request.extend_from_slice(&buf[..n]);
                    }
                    let body = if request.starts_with(b"POST /v1/cloud-host ") {
                        entered.notify_one();
                        release.notified().await;
                        r#"{"computer":null,"engine":"running","device_name":"Our cloud","until_unix":null,"started_unix":null}"#
                    } else {
                        r#"{"account":{"id":"acct_fixture","label":"Fixture"},"workspaces":[]}"#
                    };
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });
        let root = tempfile::tempdir().unwrap();
        let account = CoreAccount::from_store(
            Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap(),
            Arc::new(Memory),
            root.path().into(),
            Arc::new(Silent),
        );
        account
            .controller
            .clone()
            .import_credential(format!("oa_sk_{}", "1".repeat(64)))
            .await;
        assert!(account.controller.lease().is_some());
        let calling = account.clone();
        let start = tokio::spawn(async move { calling.start_cloud_host().await });
        tokio::time::timeout(Duration::from_secs(5), entered.notified())
            .await
            .unwrap();
        account.controller.invalidate();
        let result = tokio::time::timeout(Duration::from_secs(2), start).await;
        release.notify_one();
        server.abort();
        assert!(matches!(
            result.unwrap().unwrap(),
            Err(CoreError::Auth { .. })
        ));
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
    #[tokio::test]
    async fn project_sharing_rejects_old_callbacks_after_logout_and_another_login() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        struct Changes;
        impl super::super::SharingListener for Changes {
            fn on_sharing_changed(&self) {}
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let second = Arc::new(AtomicBool::new(false));
        let signed_in = second.clone();
        let server = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let signed_in = signed_in.clone();
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    let mut buffer = [0; 1024];
                    while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                        let n = socket.read(&mut buffer).await.unwrap();
                        if n == 0 {
                            return;
                        }
                        request.extend_from_slice(&buffer[..n]);
                    }
                    let id = if signed_in.load(Ordering::Relaxed) {
                        "acct_b"
                    } else {
                        "acct_a"
                    };
                    let body =
                        serde_json::json!({"account":{"id":id,"label":"Fixture"},"workspaces":[]})
                            .to_string();
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });
        let root = tempfile::tempdir().unwrap();
        let account = CoreAccount::from_store(
            Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap(),
            Arc::new(Memory),
            root.path().into(),
            Arc::new(Silent),
        );
        account
            .controller
            .clone()
            .import_credential(format!("oa_sk_{}", "1".repeat(64)))
            .await;
        let sharing = account.project_sharing(Arc::new(Changes)).unwrap();
        let view: serde_json::Value = serde_json::from_str(&sharing.snapshot().unwrap()).unwrap();
        let instance = view["instance"].as_str().unwrap().to_owned();
        let revision = view["revision"].as_u64().unwrap();
        let copied = sharing
            .activate(instance.clone(), revision, "recipient".into())
            .await
            .unwrap()
            .unwrap();
        let first = coder_sharing::Recipient::parse(&copied).unwrap();
        assert_eq!(first.account, "acct_a");
        struct ClientChanges;
        impl super::super::ClientListener for ClientChanges {
            fn on_event(&self, _: super::super::ClientEvent) {}
        }
        let core = account
            .connect(
                CoreConfig {
                    edge_url: origin.clone(),
                    data_dir: String::new(),
                    device_id: "fixture-phone".into(),
                    device_name: "Fixture phone".into(),
                    platform: "ios".into(),
                    app_version: "fixture".into(),
                },
                Arc::new(ClientChanges),
            )
            .unwrap();
        let homes = account
            .chat_homes(core.clone(), "a1000000-0000-4000-8000-000000000001".into())
            .unwrap();
        account.controller.invalidate();
        assert!(matches!(homes.choices(), Err(CoreError::Auth { .. })));
        assert!(matches!(
            homes
                .activate("old-view".into(), 1, "home-choice-1".into())
                .await,
            Err(CoreError::Auth { .. })
        ));
        assert!(matches!(sharing.snapshot(), Err(CoreError::Auth { .. })));
        assert!(matches!(
            sharing
                .activate(instance.clone(), revision, "recipient".into())
                .await,
            Err(CoreError::Auth { .. })
        ));
        second.store(true, Ordering::Relaxed);
        account
            .controller
            .clone()
            .import_credential(format!("oa_sk_{}", "2".repeat(64)))
            .await;
        assert_eq!(
            account.controller.lease().unwrap().session().account().id,
            "acct_b"
        );
        assert!(matches!(
            account.chat_homes(core, "a1000000-0000-4000-8000-000000000001".into()),
            Err(CoreError::Auth { .. })
        ));
        let other = account.project_sharing(Arc::new(Changes)).unwrap();
        let view: serde_json::Value = serde_json::from_str(&other.snapshot().unwrap()).unwrap();
        let copied = other
            .activate(
                view["instance"].as_str().unwrap().into(),
                view["revision"].as_u64().unwrap(),
                "recipient".into(),
            )
            .await
            .unwrap()
            .unwrap();
        let recipient = coder_sharing::Recipient::parse(&copied).unwrap();
        assert_eq!(recipient.account, "acct_b");
        assert_ne!(recipient.device, first.device);
        assert!(sharing.snapshot().is_err());
        assert!(
            sharing
                .activate(instance, revision, "recipient".into())
                .await
                .is_err()
        );
        server.abort();
    }
}

#[cfg(test)]
mod project_invitation_tests {
    use super::{CoreError, invitation_code, invitation_http, invitation_post, invitation_preview};
    use serde_json::Value;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        task::JoinHandle,
    };

    struct CapturedRequest {
        path: String,
        authorization: String,
        body: Value,
    }

    async fn read_request(stream: &mut TcpStream) -> CapturedRequest {
        let mut bytes = Vec::new();
        let mut chunk = [0; 4096];
        let (header_end, body_len) = loop {
            let count = stream.read(&mut chunk).await.unwrap();
            assert_ne!(count, 0, "fake client closed before sending a request");
            bytes.extend_from_slice(&chunk[..count]);
            if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                if bytes.len() >= end + 4 + length {
                    break (end, length);
                }
            }
        };
        let headers = String::from_utf8_lossy(&bytes[..header_end]);
        let mut lines = headers.lines();
        let path = lines
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .to_owned();
        let authorization = lines
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("authorization")
                    .then(|| value.trim().to_owned())
            })
            .unwrap_or_default();
        let body_start = header_end + 4;
        let body = serde_json::from_slice(&bytes[body_start..body_start + body_len]).unwrap();
        CapturedRequest {
            path,
            authorization,
            body,
        }
    }

    async fn fake_server(
        responses: Vec<(u16, &'static str)>,
    ) -> (reqwest::Url, JoinHandle<Vec<CapturedRequest>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                requests.push(read_request(&mut stream).await);
                let response = format!(
                    "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
            requests
        });
        (
            reqwest::Url::parse(&format!("http://{address}/")).unwrap(),
            task,
        )
    }

    #[tokio::test]
    async fn preview_and_accept_use_the_active_bearer_and_keep_the_code_in_rust() {
        let (website, server) = fake_server(vec![
            (
                200,
                include_str!("../../fixtures/android-project-invitation-preview.json"),
            ),
            (
                200,
                include_str!("../../fixtures/android-project-invitation-accepted.json"),
            ),
        ])
        .await;
        let http = invitation_http().unwrap();
        let link = "https://openagents.com/projects/join/prjinv_fixture-token_01";
        let code = invitation_code(link).unwrap();
        let preview_body = invitation_post(
            &http,
            &website,
            "synthetic-account-token",
            "v1/projects/invitations/preview",
            &code,
        )
        .await
        .unwrap();
        let preview = invitation_preview(&preview_body).unwrap();
        assert_eq!(preview.project_name, "Synthetic project");
        assert_eq!(preview.role, "editor");
        assert_eq!(preview.expires_at, "2030-01-01T00:00:00Z");
        assert!(!preview.already_member);

        let accepted = invitation_post(
            &http,
            &website,
            "synthetic-account-token",
            "v1/projects/invitations/accept",
            &code,
        )
        .await
        .unwrap();
        assert_eq!(accepted["id"], "prj_fixture_01");
        let requests = server.await.unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].path, "/v1/projects/invitations/preview");
        assert_eq!(requests[1].path, "/v1/projects/invitations/accept");
        for request in requests {
            assert_eq!(request.authorization, "Bearer synthetic-account-token");
            assert_eq!(request.body["code"], "prjinv_fixture-token_01");
        }
    }

    #[tokio::test]
    async fn expired_wrong_account_and_replayed_invitations_fail_closed_once() {
        for (status, path, body) in [
            (
                410,
                "v1/projects/invitations/preview",
                include_str!("../../fixtures/android-project-invitation-expired.json"),
            ),
            (
                403,
                "v1/projects/invitations/accept",
                include_str!("../../fixtures/android-project-invitation-wrong-account.json"),
            ),
            (
                409,
                "v1/projects/invitations/accept",
                include_str!("../../fixtures/android-project-invitation-replayed.json"),
            ),
        ] {
            let (website, server) = fake_server(vec![(status, body)]).await;
            let http = invitation_http().unwrap();
            let result = invitation_post(
                &http,
                &website,
                "synthetic-account-token",
                path,
                "prjinv_fixture-token_01",
            )
            .await;
            assert!(matches!(result, Err(CoreError::HostError { .. })));
            assert_eq!(server.await.unwrap().len(), 1, "requests are not retried");
        }
    }

    #[test]
    fn malformed_project_links_are_rejected_before_network_use() {
        for link in [
            "https://openagents.com.evil/projects/join/prjinv_fixture-token_01",
            "https://openagents.com/projects/join/prjinv_",
            "https://openagents.com/projects/join/prjinv_fixture-token_01?next=evil",
            "https://openagents.com/projects/join/prjinv_fixture-token_01/extra",
        ] {
            assert!(invitation_code(link).is_err(), "{link}");
        }
    }
}
