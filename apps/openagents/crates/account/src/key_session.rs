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
//!
//! A new key's first sign-in may need a sign-up proof (#11994): an App
//! Attest attestation over the challenge on the phone ([`Attester`]), or a
//! proof of work everywhere else and when App Attest isn't available. The
//! Android may supply Standard Play Integrity over a challenge-and-key
//! digest instead. Provider refusal or unavailability still uses proof of
//! work. The controller tries the plain sign-in first, so a key that already has an
//! account never pays for a proof.
mod recovery;

use crate::identity::Identity;
use crate::{Controller, Phase, Ticket, cancelled, invalid_login, storage_message};
use openagents::{CredentialRecord, Error, KeySignIn, Session, StorageError};
use sha2::{Digest, Sha256};
use std::{future::Future, path::PathBuf, sync::Arc};

/// Sign in again once a session has less than this left. Sessions last 30
/// days.
pub const RENEW_WITHIN: u64 = 7 * 86_400;

/// An App Attest attestation, base64, as the server takes it.
#[derive(Clone)]
pub struct Attestation {
    /// The key id `DCAppAttestService.generateKey` returned.
    pub key_id: String,
    /// `attestKey(_:clientDataHash:)` over `SHA-256(challenge)`.
    pub attestation: String,
}

/// The phone's App Attest. `attest` is called off the main thread with
/// `SHA-256(challenge)` and may block while the system answers; `None` when
/// App Attest isn't supported (the simulator) or refused, and the sign-in
/// falls back to a proof of work.
pub trait Attester: Send + Sync {
    fn attest(&self, client_data_hash: Vec<u8>) -> Option<Attestation>;
}

/// The Android platform call's result. Unavailable and refused requests
/// both use proof of work; neither grants a sign-in on its own.
#[derive(Clone)]
pub enum PlayIntegrityOutcome {
    Token(String),
    Unavailable,
    Refused,
}

/// Standard Play Integrity on Android. Called off the UI thread with a
/// digest containing no recovery words or session credential.
pub trait PlayIntegrityAttester: Send + Sync {
    fn attest(&self, request_hash: String) -> PlayIntegrityOutcome;
}

fn refused(error: &Error, code: &str) -> bool {
    matches!(error, Error::Refused { code: Some(c), .. } if *c == code)
}

/// The sign-up proof's body fields over `challenge`.
fn prove(
    attester: Option<Arc<dyn Attester>>,
    challenge: &str,
    pubkey: &str,
    play_integrity: Option<Arc<dyn PlayIntegrityAttester>>,
    attest: bool,
) -> serde_json::Map<String, serde_json::Value> {
    let mut fields = serde_json::Map::new();
    if attest {
        if let Some(a) =
            attester.and_then(|a| a.attest(Sha256::digest(challenge.as_bytes()).to_vec()))
        {
            fields.insert(
                "app_attest".into(),
                serde_json::json!({"key_id": a.key_id, "attestation": a.attestation}),
            );
            return fields;
        }
    }
    if attest {
        if let Some(PlayIntegrityOutcome::Token(token)) = play_integrity
            .map(|a| a.attest(openagents::play_integrity_request_hash(challenge, pubkey)))
        {
            if !token.is_empty()
                && token.len() <= 12 * 1024
                && token.bytes().all(|b| b.is_ascii_graphic())
            {
                fields.insert("play_integrity".into(), serde_json::json!({"token": token}));
                return fields;
            }
        }
    }
    fields.insert(
        "pow".into(),
        serde_json::Value::String(openagents::solve_pow(challenge, openagents::POW_BITS)),
    );
    fields
}

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
    /// Use App Attest for a new key's first sign-in on this phone.
    pub fn set_attester(&self, attester: Arc<dyn Attester>) {
        *self.attester.lock().unwrap() = Some(attester);
    }

    /// Set Android's Standard Play Integrity provider before identity startup.
    pub fn set_play_integrity_attester(&self, attester: Arc<dyn PlayIntegrityAttester>) {
        *self.play_integrity_attester.lock().unwrap() = Some(attester);
    }

    /// Sign in with the key; when the server asks a new key for a sign-up
    /// proof, make one off this thread (App Attest when there is one, or
    /// else a proof of work) over a fresh challenge and send it.
    async fn key_sign_in(&self, identity: &Identity, computer: &str) -> Result<KeySignIn, Error> {
        match self
            .client
            .sign_in_with_key(identity, &self.app_name, computer)
            .await
        {
            Err(e) if refused(&e, "proof_required") => {}
            other => return other,
        }
        let mut attest = true;
        loop {
            let challenge = self.client.key_challenge().await?;
            let attester = self.attester.lock().unwrap().clone();
            let over = challenge.clone();
            let play_integrity = self.play_integrity_attester.lock().unwrap().clone();
            let pubkey = identity.pubkey_hex();
            let proof = tokio::task::spawn_blocking(move || {
                prove(attester, &over, &pubkey, play_integrity, attest)
            })
            .await
            .map_err(|_| Error::InvalidCredential)?;
            let sent_play = proof.contains_key("play_integrity");
            let sent_app_attest = proof.contains_key("app_attest");
            match self
                .client
                .sign_in_with_key_on(identity, &challenge, &self.app_name, computer, proof)
                .await
            {
                // An attestation the server won't take: once more with a
                // proof of work.
                Err(e)
                    if (sent_app_attest
                        && (refused(&e, "invalid_attestation")
                            || refused(&e, "attestation_in_use")))
                        || (sent_play
                            && (refused(&e, "invalid_play_integrity")
                                || refused(&e, "play_integrity_unavailable")
                                || refused(&e, "proof_used")
                                || refused(&e, "proof_required"))) =>
                {
                    attest = false;
                }
                other => return other,
            }
        }
    }

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
        let record = tokio::task::spawn_blocking(move || {
            match std::fs::symlink_metadata(&marker) {
                Ok(_) => return Ok(None),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(StorageError::Unavailable),
            }
            store.load(&deployment)
        })
        .await
        .unwrap_or(Err(StorageError::Unavailable));
        if let Ok(Some(_)) = &record {
            self.mark_checking_saved(t);
        }
        record
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
                    // The session was just restored, so it works: a refused
                    // link (an older server without the route, a key on
                    // another account, no network) never throws it away.
                    // Try again on the next launch.
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
        let signed = cancelled(&t.cancel, self.key_sign_in(&identity, &computer)).await;
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
            let Ok(signed) = this.key_sign_in(&identity, &computer).await else {
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
