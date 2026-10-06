//! A device that holds a terminal share and no host grant.
//!
//! A guest reaches the host only through sealed NIP-TERM artifacts on a
//! relay: a direct channel needs a NIP-HOST grant, and NIP-HOST operations
//! need one too, so a share opens neither. The host answers a guest's
//! terminal requests while the guest holds a current share and admits each
//! one against that share alone.

use std::sync::Mutex;
use std::time::Duration;

use coder_access::RelayPolicy;
use coder_pty::share::ShareGrant;
use coder_pty::wire::{TerminalResult, Value as TermValue};
use secp256k1::SecretKey;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::link::{Incoming, relay_terminal, subscribe};
use crate::message::TermRequest;
use crate::{Error, Result};

/// A share holder's relay route to one host.
pub struct Guest {
    secret: SecretKey,
    host: String,
    relay: String,
    policy: RelayPolicy,
    grant: ShareGrant,
    frames_in: mpsc::UnboundedSender<Incoming>,
    frames: tokio::sync::Mutex<mpsc::UnboundedReceiver<Incoming>>,
    subscriptions: Mutex<Vec<JoinHandle<()>>>,
}

impl std::fmt::Debug for Guest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The secret key stays out of debug output.
        f.debug_struct("Guest")
            .field("host", &self.host)
            .field("share", &self.grant.share)
            .finish_non_exhaustive()
    }
}

impl Drop for Guest {
    fn drop(&mut self) {
        for task in std::mem::take(&mut *lock(&self.subscriptions)) {
            task.abort();
        }
    }
}

impl Guest {
    /// A guest from the share envelope `host` sealed to `secret`'s key,
    /// reaching the host through `relay`.
    ///
    /// # Errors
    /// Refuses an envelope another key signed or sealed.
    pub fn new(
        authorization: &serde_json::Value,
        secret: SecretKey,
        host: impl Into<String>,
        relay: impl Into<String>,
        policy: RelayPolicy,
    ) -> Result<Self> {
        let host = host.into();
        let relay = relay.into();
        policy
            .validate(&relay)
            .map_err(|error| Error::Transport(error.message))?;
        let grant = crate::share::open(authorization, &secret, &host)?;
        let (frames_in, frames) = mpsc::unbounded_channel();
        Ok(Self {
            secret,
            host,
            relay,
            policy,
            grant,
            frames_in,
            frames: tokio::sync::Mutex::new(frames),
            subscriptions: Mutex::default(),
        })
    }

    /// The share's terms, as the host signed them.
    #[must_use]
    pub fn grant(&self) -> &ShareGrant {
        &self.grant
    }

    /// Send one NIP-TERM request. An accepted attach starts delivering that
    /// attachment's frames to [`Guest::next_incoming`].
    ///
    /// # Errors
    /// Reports transport failures. A refusal is a result, not an error.
    pub async fn terminal(&self, request: TermRequest) -> Result<TerminalResult> {
        let result =
            relay_terminal(&self.secret, &self.host, self.policy, &self.relay, &request).await?;
        if let Some(TermValue::Attached { attachment, .. }) = &result.value {
            let task = subscribe(
                self.secret,
                self.host.clone(),
                self.relay.clone(),
                attachment.clone(),
                self.frames_in.clone(),
            );
            lock(&self.subscriptions).push(task);
        }
        Ok(result)
    }

    /// The next frame or record-stream part from any attachment.
    pub async fn next_incoming(&self, timeout: Duration) -> Option<Incoming> {
        tokio::time::timeout(timeout, self.frames.lock().await.recv())
            .await
            .ok()
            .flatten()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
