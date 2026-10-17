use std::fmt;
use std::time::Duration;

use openagents_login::Started;
use openagents_login::protocol::{DeviceError, device_error};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::time::Instant;
use url::Url;

use crate::{CancellationToken, Client, Deployment, Error};

mod apple;
pub use apple::AppleSignIn;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Account {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub tenant: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AccountView {
    pub account: Account,
    pub workspaces: Vec<Workspace>,
}

/// Explicit persistence payload. Serialize only into the application's credential store.
#[derive(Clone, Serialize, Deserialize)]
pub struct CredentialRecord {
    deployment: Deployment,
    account: Account,
    expires_at: Option<u64>,
    credential: String,
}

impl fmt::Debug for CredentialRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CredentialRecord")
            .field("expires_at", &self.expires_at)
            .field("credential", &"[redacted]")
            .finish()
    }
}

impl CredentialRecord {
    pub fn deployment(&self) -> &Deployment {
        &self.deployment
    }
    pub fn account(&self) -> &Account {
        &self.account
    }
    pub fn expires_at(&self) -> Option<u64> {
        self.expires_at
    }
    pub fn expired(&self, now: u64) -> bool {
        self.expires_at.is_some_and(|expiry| expiry <= now)
    }
}

/// An authenticated account, bound to its exact API/website pair.
#[derive(Clone, Debug)]
pub struct Session {
    record: CredentialRecord,
}

impl Session {
    pub fn account(&self) -> &Account {
        &self.record.account
    }
    pub fn expires_at(&self) -> Option<u64> {
        self.record.expires_at
    }
    pub fn deployment(&self) -> &Deployment {
        &self.record.deployment
    }
    /// Export for explicit storage; the client never saves it by itself.
    pub fn record(&self) -> CredentialRecord {
        self.record.clone()
    }
    /// Bearer for an explicitly configured OpenAgents service, such as Coder sync.
    /// Keep it out of logs and ordinary application files.
    pub fn bearer_token(&self) -> &str {
        &self.record.credential
    }
    pub(crate) fn token(&self) -> &str {
        &self.record.credential
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageError {
    Denied,
    Unavailable,
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Denied => "Allow access to your saved login, then try again.",
            Self::Unavailable => "Your saved login couldn't be opened. Try again.",
        })
    }
}
impl std::error::Error for StorageError {}

/// Caller-owned persistence. Implement with Keychain, another OS store, or memory.
pub trait CredentialStore {
    fn load(&self, deployment: &Deployment) -> Result<Option<CredentialRecord>, StorageError>;
    fn save(&self, record: &CredentialRecord) -> Result<(), StorageError>;
    fn clear(&self, deployment: &Deployment) -> Result<(), StorageError>;
}

/// A single login attempt. Drop or cancel it to stop waiting; no credential is saved.
pub struct Login {
    deployment: Deployment,
    started: Started,
    verification: Url,
    deadline: Instant,
    next_poll: Instant,
    interval: Duration,
    finished: bool,
}

impl fmt::Debug for Login {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Login")
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

impl Login {
    pub fn user_code(&self) -> &str {
        &self.started.user_code
    }
    /// A validated URL on this deployment's website, suitable for a system browser.
    pub fn verification_url(&self) -> &Url {
        &self.verification
    }
    pub fn expires_in(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }
    pub fn next_poll_in(&self) -> Duration {
        self.next_poll.saturating_duration_since(Instant::now())
    }
    pub fn cancel(&mut self) {
        self.finished = true;
    }
}

#[derive(Debug)]
pub enum Poll {
    Pending { retry_after: Duration },
    SlowDown { retry_after: Duration },
    SignedIn(Session),
}

#[derive(Deserialize)]
struct TokenAnswer {
    access_token: String,
    token_type: String,
    expires_in: u64,
    account: Account,
}

impl TokenAnswer {
    /// The answer, when it is a bounded app session for a named account.
    fn checked(&self) -> Result<&Self, Error> {
        if self.token_type != "Bearer"
            || !openagents_login::is_sign_in_token(&self.access_token)
            || !self.access_token.starts_with("sess_")
            || self.account.id.is_empty()
            || self.expires_in == 0
            || self.expires_in > 30 * 86400
        {
            return Err(Error::InvalidResponse);
        }
        Ok(self)
    }
}

impl Client {
    /// Import a job credential issued by the work service. This credential
    /// authorizes only that job's computer operations, not account or chat access.
    pub async fn authenticate_computer_job(&self, credential: &str) -> Result<Session, Error> {
        self.authenticate_job(credential, "scj_", &["work", "computer-session"])
            .await
    }
    /// Import an Environment attempt's computer credential. The grant excludes
    /// account reads and every other attempt's computers and artifacts.
    pub async fn authenticate_environment_job(&self, credential: &str) -> Result<Session, Error> {
        self.authenticate_job(credential, "ecj_", &["environment-computers", "session"])
            .await
    }
    async fn authenticate_job(
        &self,
        credential: &str,
        prefix: &str,
        path: &[&str],
    ) -> Result<Session, Error> {
        if !credential.starts_with(prefix) || credential.len() > 256 {
            return Err(Error::InvalidCredential);
        }
        let view: AccountView = self
            .request(Method::GET, path, &[], None, Some(credential))
            .await?;
        if view.account.id.is_empty() {
            return Err(Error::InvalidResponse);
        }
        Ok(Session {
            record: CredentialRecord {
                deployment: self.deployment.clone(),
                account: view.account,
                expires_at: None,
                credential: credential.into(),
            },
        })
    }
    /// Start an existing OpenAgents device grant. Does not launch a browser.
    pub async fn start_login(&self, app: &str, computer: &str) -> Result<Login, Error> {
        let started: Started = self
            .request(
                Method::POST,
                &["device", "code"],
                &[],
                Some(json!({"app": app, "computer": computer})),
                None,
            )
            .await?;
        if started.expires_in == 0
            || started.expires_in > 600
            || !(1..=60).contains(&started.interval)
            || !started.poll_request()["device_code"]
                .as_str()
                .is_some_and(|code| code.starts_with("dvc_") && code.len() <= 128)
        {
            return Err(Error::InvalidResponse);
        }
        let verification = self
            .deployment
            .verification(&started.verification_uri, &started.user_code)?;
        if let Some(complete) = &started.verification_uri_complete {
            if Url::parse(complete).map_err(|_| Error::InvalidResponse)? != verification {
                return Err(Error::InvalidResponse);
            }
        }
        let at = Instant::now();
        let interval = Duration::from_secs(started.interval);
        Ok(Login {
            deployment: self.deployment.clone(),
            deadline: at + Duration::from_secs(started.expires_in),
            next_poll: at + interval,
            interval,
            started,
            verification,
            finished: false,
        })
    }

    /// Poll no faster than the server permits. An interrupted redemption ends this attempt.
    pub async fn poll_login(&self, login: &mut Login) -> Result<Poll, Error> {
        if login.deployment != self.deployment {
            return Err(Error::WrongDeployment);
        }
        if login.finished {
            return Err(Error::FinishedLogin);
        }
        if Instant::now() >= login.deadline {
            login.finished = true;
            return Err(Error::ExpiredLogin);
        }
        if Instant::now() < login.next_poll {
            return Ok(Poll::Pending {
                retry_after: login.next_poll_in(),
            });
        }
        // Dropping this future after dispatch cannot make the grant safe to replay.
        login.finished = true;
        let request = self.request::<Value>(
            Method::POST,
            &["device", "token"],
            &[],
            Some(login.started.poll_request()),
            None,
        );
        let body = match tokio::time::timeout_at(login.deadline, request).await {
            Ok(Ok(value)) => value,
            result => {
                login.finished = true;
                let error = match result {
                    Ok(Err(error)) => error,
                    _ => Error::Network { timeout: true },
                };
                return Err(Error::UncertainLogin(Box::new(error)));
            }
        };
        login.next_poll = Instant::now() + login.interval;
        match device_error(&body, login.interval.as_secs()) {
            DeviceError::Pending => {
                login.finished = false;
                Ok(Poll::Pending {
                    retry_after: login.interval,
                })
            }
            DeviceError::SlowDown(seconds) => {
                login.finished = false;
                login.interval = Duration::from_secs(seconds.clamp(login.interval.as_secs(), 60));
                login.next_poll = Instant::now() + login.interval;
                Ok(Poll::SlowDown {
                    retry_after: login.interval,
                })
            }
            DeviceError::Denied => {
                login.finished = true;
                Err(Error::Denied)
            }
            DeviceError::Expired => {
                login.finished = true;
                Err(Error::ExpiredLogin)
            }
            DeviceError::InvalidGrant => {
                login.finished = true;
                Err(Error::FinishedLogin)
            }
            DeviceError::Other => {
                login.finished = true;
                let answer: TokenAnswer =
                    serde_json::from_value(body).map_err(|_| Error::InvalidResponse)?;
                answer.checked()?;
                let session = Session {
                    record: CredentialRecord {
                        deployment: self.deployment.clone(),
                        account: answer.account,
                        expires_at: Some(crate::now().saturating_add(answer.expires_in)),
                        credential: answer.access_token,
                    },
                };
                // Keep the issued credential even if a later account/workspace read is offline.
                // The application calls `account` before showing account resources.
                Ok(Poll::SignedIn(session))
            }
        }
    }

    /// Wait with explicit cancellation. A dropped/uncertain redemption is never automatically retried.
    pub async fn wait_for_login(
        &self,
        login: &mut Login,
        cancellation: &CancellationToken,
    ) -> Result<Session, Error> {
        loop {
            let result = tokio::select! {
                biased;
                _ = cancellation.cancelled() => { login.cancel(); return Err(Error::Cancelled); },
                result = async {
                    tokio::time::sleep_until(login.next_poll.min(login.deadline)).await;
                    self.poll_login(login).await
                } => result,
            };
            match result? {
                Poll::SignedIn(session) => {
                    if cancellation.is_cancelled() {
                        let _ = self.sign_out(&session).await;
                        return Err(Error::Cancelled);
                    }
                    return Ok(session);
                }
                Poll::Pending { .. } | Poll::SlowDown { .. } => {}
            }
        }
    }

    pub(crate) fn check_session(&self, session: &Session) -> Result<(), Error> {
        if session.record.deployment != self.deployment {
            return Err(Error::WrongDeployment);
        }
        if session.record.expired(crate::now()) {
            return Err(Error::ExpiredSession);
        }
        Ok(())
    }

    /// Read the existing account and current workspaces; never trusts saved membership data.
    pub async fn account(&self, session: &Session) -> Result<AccountView, Error> {
        self.check_session(session)?;
        let view: AccountView = self
            .request(Method::GET, &["account"], &[], None, Some(session.token()))
            .await?;
        if view.account.id != session.account().id {
            return Err(Error::WrongAccount);
        }
        Ok(view)
    }

    /// Explicitly import an app session or sign-in key. Does not read environment variables.
    pub async fn authenticate(&self, credential: &str) -> Result<Session, Error> {
        if !openagents_login::is_sign_in_token(credential) {
            return Err(Error::InvalidCredential);
        }
        let view: AccountView = self
            .request(Method::GET, &["account"], &[], None, Some(credential))
            .await?;
        if view.account.id.is_empty() {
            return Err(Error::InvalidResponse);
        }
        Ok(Session {
            record: CredentialRecord {
                deployment: self.deployment.clone(),
                account: view.account,
                expires_at: None,
                credential: credential.into(),
            },
        })
    }

    /// Revalidate a record loaded by the caller. Failure does not erase caller storage.
    pub async fn restore(&self, record: &CredentialRecord) -> Result<Session, Error> {
        let session = Session {
            record: record.clone(),
        };
        self.check_session(&session)?;
        if !openagents_login::is_sign_in_token(session.token()) {
            return Err(Error::InvalidCredential);
        }
        let view = self.account(&session).await?;
        Ok(Session {
            record: CredentialRecord {
                account: view.account,
                ..record.clone()
            },
        })
    }

    /// Revoke a saved app session without restoring account authority first.
    /// This also works when account discovery is unavailable or local expiry has passed.
    /// Always clear local credentials and views, even when revocation fails.
    pub async fn sign_out_record(&self, record: &CredentialRecord) -> Result<(), Error> {
        if record.deployment != self.deployment {
            return Err(Error::WrongDeployment);
        }
        if !openagents_login::is_sign_in_token(&record.credential) {
            return Err(Error::InvalidCredential);
        }
        self.sign_out(&Session {
            record: record.clone(),
        })
        .await
    }

    /// Revoke this app session. Sign-in keys are only forgotten by the caller, as in the CLI.
    /// Clear the caller's local storage and views even if this request fails offline.
    pub async fn sign_out(&self, session: &Session) -> Result<(), Error> {
        if session.record.deployment != self.deployment {
            return Err(Error::WrongDeployment);
        }
        if openagents_login::is_sign_in_key(session.token()) {
            return Ok(());
        }
        match self
            .request::<Value>(
                Method::POST,
                &["device", "sign-out"],
                &[],
                Some(json!({})),
                Some(session.token()),
            )
            .await
        {
            Ok(value) if value["signed_out"] == true => Ok(()),
            Err(Error::Unauthorized) => Ok(()),
            Ok(_) => Err(Error::InvalidResponse),
            Err(error) => Err(error),
        }
    }
}
