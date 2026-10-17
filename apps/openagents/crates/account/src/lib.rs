//! OpenAgents account state, independent of local workspace and provider logins.
pub use openagents;
use openagents::{
    Account, CancellationToken, Client, CredentialRecord, CredentialStore, Error, Session,
    StorageError,
};
use sha2::{Digest, Sha256};
use std::{
    future::Future,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
};
use tokio::sync::watch;
pub mod agent_lab;
pub mod chat_entries;
pub mod dashboard;
pub mod desktop;
pub mod history;
pub mod lab_cards;
mod platform;
pub mod reader;
pub mod turns;
pub mod video_editor;
pub use platform::PlatformStore;

#[derive(Clone, Debug)]
pub enum Phase {
    SignedOut { notice: Option<String> },
    Restoring,
    Starting,
    Waiting { code: String, url: String },
    SignedIn { account: Account },
    Failed { message: String, can_restore: bool },
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub generation: u64,
    pub phase: Phase,
}
struct State {
    generation: u64,
    cancel: CancellationToken,
    session: Option<Session>,
}
struct Ticket {
    generation: u64,
    cancel: CancellationToken,
    previous: Option<Session>,
}
/// Serialize credential operations so a late login cannot overwrite a newer login or logout.
pub struct Controller {
    client: Client,
    app_name: String,
    store: Arc<dyn CredentialStore + Send + Sync>,
    marker: PathBuf,
    state: Mutex<State>,
    io: tokio::sync::Mutex<()>,
    updates: watch::Sender<Snapshot>,
}
#[derive(Clone)]
pub struct Lease {
    controller: Weak<Controller>,
    pub generation: u64,
    session: Session,
    client: Client,
    pub cancel: CancellationToken,
}
impl Lease {
    pub fn client(&self) -> &Client {
        &self.client
    }
    pub fn session(&self) -> &Session {
        &self.session
    }
    pub fn is_current(&self) -> bool {
        self.controller
            .upgrade()
            .is_some_and(|c| c.authorized(self.generation))
    }
}
impl Controller {
    pub fn new(
        client: Client,
        store: Arc<dyn CredentialStore + Send + Sync>,
        marker: PathBuf,
    ) -> Arc<Self> {
        Self::new_named(client, store, marker, "OpenAgents".into())
    }
    pub fn new_named(
        client: Client,
        store: Arc<dyn CredentialStore + Send + Sync>,
        marker: PathBuf,
        app_name: String,
    ) -> Arc<Self> {
        // A sign-out on one site must never unseal another site's saved login.
        let marker = marker.with_extension(format!(
            "{:x}",
            Sha256::digest(
                format!(
                    "{}\n{}",
                    client.deployment().api_base(),
                    client.deployment().website()
                )
                .as_bytes()
            )
        ));
        let (updates, _) = watch::channel(Snapshot {
            generation: 0,
            phase: Phase::SignedOut { notice: None },
        });
        Arc::new(Self {
            client,
            app_name,
            store,
            marker,
            state: Mutex::new(State {
                generation: 0,
                cancel: CancellationToken::new(),
                session: None,
            }),
            io: tokio::sync::Mutex::new(()),
            updates,
        })
    }
    pub fn watch(&self) -> watch::Receiver<Snapshot> {
        self.updates.subscribe()
    }
    pub fn snapshot(&self) -> Snapshot {
        self.updates.borrow().clone()
    }
    pub fn lease(self: &Arc<Self>) -> Option<Lease> {
        let s = self.state.lock().unwrap();
        if expired(s.session.as_ref()?) {
            return None;
        }
        Some(Lease {
            controller: Arc::downgrade(self),
            generation: s.generation,
            session: s.session.clone()?,
            client: self.client.clone(),
            cancel: s.cancel.clone(),
        })
    }
    pub fn accepts(self: &Arc<Self>, lease: &Lease) -> bool {
        Weak::ptr_eq(&lease.controller, &Arc::downgrade(self)) && self.authorized(lease.generation)
    }
    fn authorized(&self, generation: u64) -> bool {
        let state = self.state.lock().unwrap();
        state.generation == generation && state.session.as_ref().is_some_and(|s| !expired(s))
    }
    fn current(&self, generation: u64) -> bool {
        self.state.lock().unwrap().generation == generation
    }
    fn begin(&self, phase: Phase) -> Ticket {
        let mut s = self.state.lock().unwrap();
        s.cancel.cancel();
        s.generation += 1;
        s.cancel = CancellationToken::new();
        let ticket = Ticket {
            generation: s.generation,
            cancel: s.cancel.clone(),
            previous: s.session.take(),
        };
        self.updates.send_replace(Snapshot {
            generation: s.generation,
            phase,
        });
        ticket
    }
    fn publish(&self, t: &Ticket, phase: Phase, session: Option<Session>) -> bool {
        let mut s = self.state.lock().unwrap();
        if s.generation != t.generation {
            return false;
        }
        s.session = session;
        self.updates.send_replace(Snapshot {
            generation: t.generation,
            phase,
        });
        true
    }
    /// Stop callbacks and reads when the owning app view or deployment is replaced.
    /// Stored credentials stay available for an explicit restore of that deployment.
    pub fn invalidate(&self) {
        self.begin(Phase::SignedOut { notice: None });
    }
    pub fn cancel(self: &Arc<Self>) -> impl Future<Output = ()> + Send + 'static {
        self.sign_out()
    }
    pub fn restore(self: &Arc<Self>) -> impl Future<Output = ()> + Send + 'static {
        let this = self.clone();
        let ticket = self.begin(Phase::Restoring);
        async move {
            this.restore_inner(ticket).await;
        }
    }
    async fn restore_inner(&self, t: Ticket) {
        let record = {
            let _io = self.io.lock().await;
            if !self.current(t.generation) {
                return;
            }
            let marker = self.marker.clone();
            let store = self.store.clone();
            let deployment = self.client.deployment().clone();
            tokio::task::spawn_blocking(move || {
                match std::fs::symlink_metadata(&marker) {
                    Ok(_) => return Ok(None),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => return Err(StorageError::Unavailable),
                }
                store.load(&deployment)
            })
            .await
            .unwrap_or(Err(StorageError::Unavailable))
        };
        let record = match record {
            Ok(Some(r)) => r,
            Ok(None) => {
                self.publish(&t, Phase::SignedOut { notice: None }, None);
                return;
            }
            Err(e) => {
                self.publish(
                    &t,
                    Phase::Failed {
                        message: storage_message(e).into(),
                        can_restore: true,
                    },
                    None,
                );
                return;
            }
        };
        let result = cancelled(&t.cancel, self.client.restore(&record)).await;
        match result {
            Ok(session) => {
                self.publish(
                    &t,
                    Phase::SignedIn {
                        account: session.account().clone(),
                    },
                    Some(session),
                );
            }
            Err(e) if invalid_login(&e) => {
                let _ = self.forget(&t).await;
                self.publish(
                    &t,
                    Phase::SignedOut {
                        notice: Some("Log in again to continue.".into()),
                    },
                    None,
                );
            }
            Err(Error::Cancelled) => {}
            Err(e) => {
                self.publish(
                    &t,
                    Phase::Failed {
                        message: e.to_string(),
                        can_restore: true,
                    },
                    None,
                );
            }
        }
    }
    /// Check a current login without treating an outage as a sign-out.
    pub async fn revalidate(self: Arc<Self>) {
        let Some(lease) = self.lease() else {
            if matches!(self.snapshot().phase, Phase::SignedIn { .. }) {
                self.restore().await;
            }
            return;
        };
        if let Err(error) = self.client.account(lease.session()).await {
            if invalid_login(&error) && self.accepts(&lease) {
                self.restore().await;
            }
        }
    }
    pub fn sign_in(
        self: &Arc<Self>,
        computer: String,
    ) -> impl Future<Output = ()> + Send + 'static {
        let this = self.clone();
        let ticket = self.begin(Phase::Starting);
        async move {
            this.login_inner(ticket, computer).await;
        }
    }
    async fn login_inner(&self, t: Ticket, computer: String) {
        if let Some(previous) = self.saved_for_sign_out(&t).await {
            let _ = self.client.sign_out_record(&previous).await;
        }
        if !self.current(t.generation) {
            return;
        }
        if self.forget(&t).await.is_err() {
            self.publish(
                &t,
                Phase::Failed {
                    message: "Couldn't remove the saved login. Unlock your device and try again."
                        .into(),
                    can_restore: false,
                },
                None,
            );
            return;
        }
        let mut login = match cancelled(
            &t.cancel,
            self.client.start_login(&self.app_name, &computer),
        )
        .await
        {
            Ok(l) => l,
            Err(Error::Cancelled) => return,
            Err(e) => {
                self.publish(
                    &t,
                    Phase::Failed {
                        message: e.to_string(),
                        can_restore: false,
                    },
                    None,
                );
                return;
            }
        };
        if !self.publish(
            &t,
            Phase::Waiting {
                code: login.user_code().to_owned(),
                url: login.verification_url().to_string(),
            },
            None,
        ) {
            return;
        }
        let session = match self.client.wait_for_login(&mut login, &t.cancel).await {
            Ok(s) => s,
            Err(Error::Cancelled) => return,
            Err(e) => {
                self.publish(
                    &t,
                    Phase::Failed {
                        message: e.to_string(),
                        can_restore: false,
                    },
                    None,
                );
                return;
            }
        };
        self.accept_session(t, session).await;
    }
    /// Import a caller-provided credential through the public SDK. No local token decoding.
    pub fn import_credential(
        self: &Arc<Self>,
        credential: String,
    ) -> impl Future<Output = ()> + Send + 'static {
        let this = self.clone();
        let ticket = self.begin(Phase::Starting);
        async move {
            if let Some(previous) = this.saved_for_sign_out(&ticket).await {
                let _ = this.client.sign_out_record(&previous).await;
            }
            if this.forget(&ticket).await.is_err() {
                this.publish(
                    &ticket,
                    Phase::Failed {
                        message:
                            "Couldn't remove the saved login. Unlock your device and try again."
                                .into(),
                        can_restore: false,
                    },
                    None,
                );
                return;
            }
            match cancelled(&ticket.cancel, this.client.authenticate(&credential)).await {
                Ok(session) => this.accept_session(ticket, session).await,
                Err(Error::Cancelled) => {}
                Err(error) => {
                    this.publish(
                        &ticket,
                        Phase::Failed {
                            message: error.to_string(),
                            can_restore: false,
                        },
                        None,
                    );
                }
            }
        }
    }
    async fn accept_session(&self, t: Ticket, session: Session) {
        // Keep the issued credential recoverable even if account discovery is offline.
        let saved = self.save(&t, session.record()).await;
        if !matches!(saved, Ok(true)) {
            let _ = self.client.sign_out(&session).await;
            if let Err(e) = saved {
                self.publish(
                    &t,
                    Phase::Failed {
                        message: storage_message(e).into(),
                        can_restore: false,
                    },
                    None,
                );
            }
            return;
        }
        match cancelled(&t.cancel, self.client.account(&session)).await {
            Ok(view) => {
                if !self.publish(
                    &t,
                    Phase::SignedIn {
                        account: view.account,
                    },
                    Some(session.clone()),
                ) {
                    let _ = self.client.sign_out(&session).await;
                }
            }
            Err(Error::Cancelled) => {
                let _ = self.client.sign_out(&session).await;
            }
            Err(e) if invalid_login(&e) => {
                let _ = self.forget(&t).await;
                let _ = self.client.sign_out(&session).await;
                self.publish(
                    &t,
                    Phase::Failed {
                        message: e.to_string(),
                        can_restore: false,
                    },
                    None,
                );
            }
            Err(e) => {
                self.publish(
                    &t,
                    Phase::Failed {
                        message: e.to_string(),
                        can_restore: true,
                    },
                    None,
                );
            }
        }
    }
    async fn save(&self, t: &Ticket, record: CredentialRecord) -> Result<bool, StorageError> {
        let _io = self.io.lock().await;
        if !self.current(t.generation) {
            return Ok(false);
        }
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || store.save(&record))
            .await
            .map_err(|_| StorageError::Unavailable)??;
        let current = {
            // Keep removing the marker ordered with a new sign-out generation.
            let state = self.state.lock().unwrap();
            if state.generation == t.generation {
                match std::fs::remove_file(&self.marker) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => return Err(StorageError::Unavailable),
                }
                true
            } else {
                false
            }
        };
        if !current {
            let _ = mark_signed_out(&self.marker);
            let store = self.store.clone();
            let deployment = self.client.deployment().clone();
            let _ = tokio::task::spawn_blocking(move || store.clear(&deployment)).await;
            return Ok(false);
        }
        Ok(true)
    }
    async fn saved_for_sign_out(&self, t: &Ticket) -> Option<CredentialRecord> {
        let _io = self.io.lock().await;
        if !self.current(t.generation) {
            return None;
        }
        let store = self.store.clone();
        let deployment = self.client.deployment().clone();
        tokio::task::spawn_blocking(move || store.load(&deployment))
            .await
            .ok()
            .and_then(Result::ok)
            .flatten()
            .or_else(|| t.previous.as_ref().map(Session::record))
    }
    async fn forget(&self, t: &Ticket) -> Result<(), StorageError> {
        let _io = self.io.lock().await;
        if !self.current(t.generation) {
            return Ok(());
        }
        let store = self.store.clone();
        let deployment = self.client.deployment().clone();
        let marker = self.marker.clone();
        tokio::task::spawn_blocking(move || {
            let marked = mark_signed_out(&marker);
            let cleared = store.clear(&deployment);
            // The marker prevents automatic restore even when Keychain deletion is refused.
            marked.and(cleared)
        })
        .await
        .map_err(|_| StorageError::Unavailable)?
    }
    pub fn sign_out(self: &Arc<Self>) -> impl Future<Output = ()> + Send + 'static {
        let this = self.clone();
        let t = self.begin(Phase::SignedOut { notice: None });
        {
            // Seal restore before waiting for an OS credential prompt or a network request.
            let state = self.state.lock().unwrap();
            if state.generation == t.generation {
                let _ = mark_signed_out(&self.marker);
            }
        }
        async move {
            let saved = this.saved_for_sign_out(&t).await;
            let local = this.forget(&t).await;
            let remote = match &saved {
                Some(record) => this.client.sign_out_record(record).await,
                None => Ok(()),
            };
            let notice = if local.is_err() {
                Some("Couldn't remove the saved login. Unlock your device and try again.".into())
            } else if remote.is_err() {
                Some("You're signed out here. Remove this app's login on the website to sign it out there too.".into())
            } else {
                None
            };
            this.publish(&t, Phase::SignedOut { notice }, None);
        }
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.state.lock().unwrap().cancel.cancel();
    }
}
fn mark_signed_out(path: &Path) -> Result<(), StorageError> {
    let parent = path.parent().ok_or(StorageError::Unavailable)?;
    std::fs::create_dir_all(parent).map_err(|_| StorageError::Unavailable)?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(_) => Err(StorageError::Unavailable),
    }
}
async fn cancelled<T>(
    cancel: &CancellationToken,
    f: impl Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    tokio::select! {biased;_=cancel.cancelled()=>Err(Error::Cancelled),result=f=>result}
}
fn invalid_login(error: &Error) -> bool {
    matches!(
        error,
        Error::Unauthorized
            | Error::Forbidden
            | Error::ExpiredSession
            | Error::InvalidCredential
            | Error::WrongAccount
            | Error::WrongDeployment
    )
}
fn storage_message(error: StorageError) -> &'static str {
    match error {
        StorageError::Denied => "Unlock your device to save or restore your login.",
        StorageError::Unavailable => "Couldn't save or restore your login. Try again.",
    }
}

fn expired(session: &Session) -> bool {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    session.expires_at().is_some_and(|expiry| expiry <= now)
}
