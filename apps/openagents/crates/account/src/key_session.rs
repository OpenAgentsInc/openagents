//! Silent sign-in with the identity's key (#11995).
//!
//! On launch the app signs in with the key and nothing for the person to do:
//! [`Controller::start_with_key`]. A saved session is reused while it has
//! more than [`RENEW_WITHIN`] left, and replaced by signing again after that
//! ([`Controller::renew_with_key`] does it in place while the app runs).
//!
//! A device that was signed in before the identity existed (a device-code or
//! Apple session from an earlier build) keeps its account: the first launch
//! adds the new key to that account with the old session
//! (`POST /v1/device/keys`), so the key's own sign-ins reach the same account
//! instead of making a new one. A small marker per account and key records
//! that the link is done.
use crate::identity::Identity;
use crate::{Controller, Phase, Ticket, cancelled, invalid_login, storage_message};
use openagents::{CredentialRecord, Error, Session, StorageError};
use sha2::{Digest, Sha256};
use std::{future::Future, path::PathBuf, sync::Arc};

/// Sign in again once a session has less than this left. Sessions last 30
/// days.
pub const RENEW_WITHIN: u64 = 7 * 86_400;

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn due(session: &Session) -> bool {
    session
        .expires_at()
        .is_some_and(|expiry| expiry <= now().saturating_add(RENEW_WITHIN))
}

impl Controller {
    fn link_marker(&self, account: &str, pubkey: &str) -> PathBuf {
        let digest = Sha256::digest(
            format!(
                "{}\n{}\n{account}\n{pubkey}",
                self.client.deployment().api_base(),
                self.client.deployment().website()
            )
            .as_bytes(),
        );
        self.marker
            .with_file_name(format!("key-linked-{:x}", digest))
    }

    /// The saved session record, unless the person signed out here.
    async fn saved_record(&self, t: &Ticket) -> Result<Option<CredentialRecord>, StorageError> {
        let _io = self.io.lock().await;
        if !self.current(t.generation) {
            return Ok(None);
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
    }

    /// Whether `pubkey` is known to sign in to `account` here.
    pub fn key_linked(&self, account: &str, pubkey: &str) -> bool {
        self.link_marker(account, pubkey).exists()
    }

    fn mark_linked(&self, account: &str, pubkey: &str) {
        let path = self.link_marker(account, pubkey);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, b"1\n");
    }

    /// Start this device's account on launch with nothing for the person to
    /// do: reuse the saved session (adding the key to its account the first
    /// time), or sign in with the key.
    pub fn start_with_key(
        self: &Arc<Self>,
        identity: Arc<Identity>,
        computer: String,
    ) -> impl Future<Output = ()> + Send + 'static {
        let this = self.clone();
        let ticket = self.begin(Phase::Restoring);
        async move { this.start_inner(ticket, identity, computer, false).await }
    }

    /// Sign in with the key now, replacing any saved session, as after the
    /// person restores their words.
    pub fn sign_in_with_key(
        self: &Arc<Self>,
        identity: Arc<Identity>,
        computer: String,
    ) -> impl Future<Output = ()> + Send + 'static {
        let this = self.clone();
        let ticket = self.begin(Phase::Starting);
        async move { this.start_inner(ticket, identity, computer, true).await }
    }

    async fn start_inner(
        &self,
        t: Ticket,
        identity: Arc<Identity>,
        computer: String,
        replace: bool,
    ) {
        let pubkey = identity.pubkey_hex();
        let mut existing = None;
        if !replace {
            match self.saved_record(&t).await {
                Ok(Some(record)) => {
                    match cancelled(&t.cancel, self.client.restore(&record)).await {
                        Ok(session) => existing = Some(session),
                        Err(Error::Cancelled) => return,
                        // Expired or revoked: the key signs in below.
                        Err(e) if invalid_login(&e) => {}
                        Err(e) => {
                            // Offline: keep the saved session for a retry.
                            self.publish(
                                &t,
                                Phase::Failed {
                                    message: e.to_string(),
                                    can_restore: true,
                                },
                                None,
                            );
                            return;
                        }
                    }
                }
                Ok(None) => {}
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
            }
        }
        if let Some(session) = &existing {
            let account = session.account().id.clone();
            if !self.key_linked(&account, &pubkey) {
                match cancelled(&t.cancel, self.client.link_key(session, identity.as_ref())).await {
                    Ok(_) => self.mark_linked(&account, &pubkey),
                    Err(Error::Cancelled) => return,
                    Err(e) if invalid_login(&e) => existing = None,
                    // The key already signs in to another account, or the
                    // server couldn't be reached: keep this session, and try
                    // again on the next launch.
                    Err(_) => {}
                }
            }
        }
        if let Some(session) = existing.take() {
            if !due(&session) {
                self.publish(
                    &t,
                    Phase::SignedIn {
                        account: session.account().clone(),
                    },
                    Some(session),
                );
                return;
            }
            existing = Some(session);
        }
        let signed = cancelled(
            &t.cancel,
            self.client
                .sign_in_with_key(identity.as_ref(), &self.app_name, &computer),
        )
        .await;
        match signed {
            Ok(signed) => {
                self.mark_linked(&signed.session.account().id, &pubkey);
                let previous = existing.or_else(|| t.previous.clone());
                let generation = t.generation;
                self.accept_session(t, signed.session).await;
                let accepted = {
                    let s = self.state.lock().unwrap();
                    s.generation == generation && s.session.is_some()
                };
                // Revoke the session this one replaces once the new one is
                // saved, never before.
                if accepted {
                    if let Some(previous) = previous {
                        let _ = self.client.sign_out(&previous).await;
                    }
                }
            }
            Err(Error::Cancelled) => {}
            Err(e) => match existing {
                // A session that is only due for renewal still works.
                Some(session) => {
                    self.publish(
                        &t,
                        Phase::SignedIn {
                            account: session.account().clone(),
                        },
                        Some(session),
                    );
                }
                None => {
                    self.publish(
                        &t,
                        Phase::Failed {
                            message: e.to_string(),
                            can_restore: true,
                        },
                        None,
                    );
                }
            },
        }
    }

    /// While signed in, sign in again with the key once the session is due,
    /// and swap the new session in without closing anything: open clients
    /// keep the old one, which still works until it expires. A key that now
    /// reaches another account starts over instead.
    pub fn renew_with_key(
        self: &Arc<Self>,
        identity: Arc<Identity>,
        computer: String,
    ) -> impl Future<Output = ()> + Send + 'static {
        let this = self.clone();
        async move {
            let (generation, current) = {
                let s = this.state.lock().unwrap();
                match &s.session {
                    Some(session) if due(session) => (s.generation, session.clone()),
                    _ => return,
                }
            };
            let Ok(signed) = this
                .client
                .sign_in_with_key(identity.as_ref(), &this.app_name, &computer)
                .await
            else {
                return;
            };
            if signed.session.account().id != current.account().id {
                let _ = this.client.sign_out(&signed.session).await;
                this.start_with_key(identity, computer).await;
                return;
            }
            let record = signed.session.record();
            {
                let _io = this.io.lock().await;
                if !this.current(generation) {
                    drop(_io);
                    let _ = this.client.sign_out(&signed.session).await;
                    return;
                }
                let store = this.store.clone();
                let saved = tokio::task::spawn_blocking(move || store.save(&record))
                    .await
                    .map_err(|_| StorageError::Unavailable)
                    .and_then(|r| r);
                if saved.is_err() {
                    drop(_io);
                    let _ = this.client.sign_out(&signed.session).await;
                    return;
                }
                let mut s = this.state.lock().unwrap();
                if s.generation != generation {
                    return;
                }
                s.session = Some(signed.session);
            }
        }
    }

    /// The identity couldn't be opened or made (a locked device, say):
    /// show `message`, and let the app try again.
    pub fn identity_failed(&self, message: String) {
        self.begin(Phase::Failed {
            message,
            can_restore: true,
        });
    }

    /// Whether the signed-in session is due to be renewed.
    pub fn renewal_due(&self) -> bool {
        self.state.lock().unwrap().session.as_ref().is_some_and(due)
    }
}
