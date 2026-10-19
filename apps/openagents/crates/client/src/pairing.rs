//! Connect a computer by its code (#12000): pair with no prior registry
//! entry, keep the grant in the platform's store, connect again on launch,
//! and link the computer to this account when its code asks.
use super::*;
use openagents_sync::{
    account_link,
    paired_device::{self, PairedComputer, PairingStore},
};

/// A computer the phone just connected to. Link it to this account next
/// with [`Client::link_computer`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectedComputer {
    pub device_id: String,
    pub label: String,
}

const SIGNED_OUT: &str = "You're signed out.";

fn row(computer: &PairedComputer) -> openagents_proto::Device {
    openagents_proto::Device {
        id: computer.device_id.clone(),
        name: computer.label.clone(),
        platform: "desktop".into(),
        last_seen_at: None,
        created_at: None,
        version: None,
        cursor_sdk_version: None,
        capabilities: vec![],
        origin: openagents_proto::DeviceOrigin::Unknown,
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Client {
    fn pairing_store(&self) -> Option<Arc<dyn PairingStore>> {
        lock(&self.inner.pairings).clone()
    }

    /// Keep pairings made by code in `store` (the Keychain on iOS), and
    /// connect to every computer already saved there for this account.
    pub fn set_pairing_store(&self, store: Arc<dyn PairingStore>) {
        *lock(&self.inner.pairings) = Some(store.clone());
        if self.inner.live().is_none() {
            return;
        }
        let saved = match store.load(self.inner.credentials.user_id()) {
            Ok(saved) => saved,
            Err(error) => {
                tracing::warn!(%error, "saved pairings unreadable");
                return;
            }
        };
        for computer in saved {
            self.inner.workspace.add_paired(row(&computer));
            let client = self.clone();
            crate::runtime::handle().spawn(async move {
                match paired_device::open(&computer, coder_access_policy()).await {
                    Ok((channel, _)) => client.install_channel(&computer.device_id, channel),
                    Err(error) => {
                        tracing::debug!(%error, "saved pairing didn't connect");
                    }
                }
            });
        }
        self.inner.recompute_workspace();
    }

    fn install_channel(
        &self,
        device_id: &str,
        channel: openagents_sync::device_channel::DeviceChannel,
    ) {
        let Some(live) = self.inner.live() else {
            return;
        };
        if self.inner.cancel.is_cancelled() {
            return;
        }
        lock(&live.relay.device_channels).insert(device_id.into(), Arc::new(channel));
        live.relay.channel_changed(device_id);
        for core in self.inner.cores() {
            self.inner.kick_room(&core.chat_id);
        }
    }

    /// Pair with the computer a scanned or pasted code names (connect
    /// code, connect link, or host invitation). The computer needs no
    /// entry in this account's list first: it appears from the pairing,
    /// and the grant is saved in the pairing store.
    ///
    /// # Errors
    /// Why the computer wasn't added, in words for the screen.
    pub async fn connect_computer(&self, code: &str) -> Result<ConnectedComputer> {
        if self.inner.cancel.is_cancelled() {
            return Err(ClientError::HostError(SIGNED_OUT.into()));
        }
        if self.inner.live().is_none() {
            return Err(ClientError::Unsupported(
                "Connecting a computer needs your account.".into(),
            ));
        }
        let account = self.inner.credentials.user_id().to_owned();
        let scan = tokio::select! {
            _ = self.inner.cancel.cancelled() => return Err(ClientError::HostError(SIGNED_OUT.into())),
            result = paired_device::pair_scanned(code, &account, coder_access_policy()) => {
                result.map_err(|error| ClientError::HostError(error.0))?
            }
        };
        if scan.computer.device_id == self.inner.config.device_id {
            return Err(ClientError::HostError(
                "That's this phone's own code. Scan the code on your computer.".into(),
            ));
        }
        if let Some(store) = self.pairing_store()
            && store.save(&scan.computer).is_err()
        {
            // The pairing still works until the app closes.
            tracing::warn!("pairing not saved");
        }
        let connected = ConnectedComputer {
            device_id: scan.computer.device_id.clone(),
            label: scan.computer.label.clone(),
        };
        self.inner.workspace.add_paired(row(&scan.computer));
        self.install_channel(&connected.device_id, scan.channel);
        self.inner.recompute_workspace();
        Ok(connected)
    }

    /// Add the just-paired computer's account key to this account, so the
    /// computer signs in as the same person (NIP-HOST "Account key link by
    /// QR"): get a challenge for this account from `api_base`
    /// (`https://api.openagents.com/v1/`), have the computer sign it over
    /// its device connection, check the proof, and send it with this
    /// account's session, which is the phone's approval, then tell the
    /// computer it landed so it signs in. `own` is this
    /// phone's account key, which is never linked again. Answers the
    /// computer's key.
    ///
    /// # Errors
    /// Why the computer wasn't linked, in words for the screen. Pairing
    /// stays in place either way.
    pub async fn link_computer(
        &self,
        device_id: &str,
        api_base: &str,
        own: Option<&str>,
    ) -> Result<String> {
        let channel = self
            .inner
            .live()
            .and_then(|live| lock(&live.relay.device_channels).get(device_id).cloned())
            .ok_or_else(|| ClientError::HostError("Connect this computer first.".into()))?;
        let token = self.inner.tokens.bearer().await?;
        let failed = || {
            ClientError::HostUnavailable(
                "Couldn't sign your computer in as you. Try again in a minute.".into(),
            )
        };
        let challenge: serde_json::Value = crate::auth::http()
            .get(account_link::challenge_url(api_base))
            .bearer_auth(&token)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|_| failed())?
            .error_for_status()
            .map_err(|_| failed())?
            .json()
            .await
            .map_err(|_| failed())?;
        let challenge = challenge["challenge"]
            .as_str()
            .filter(|c| !c.is_empty() && c.len() <= 256)
            .ok_or_else(failed)?;
        let url = account_link::link_url(api_base);
        let proof = account_link::request(&channel, &url, challenge, own, now())
            .await
            .map_err(|error| ClientError::HostError(error.to_string()))?;
        let response = crate::auth::http()
            .post(&url)
            .bearer_auth(&token)
            .timeout(Duration::from_secs(10))
            .json(&serde_json::json!({ "proof": proof.proof }))
            .send()
            .await
            .map_err(|_| failed())?;
        if response.status().is_success() {
            // Tell the computer the link landed so it signs in now. If it
            // doesn't hear, it offers its key again; the link stands.
            if let Err(error) = account_link::confirm(&channel, &proof.key).await {
                tracing::debug!(%error, "computer didn't take the link confirmation");
            }
            return Ok(proof.key);
        }
        let status = response.status().as_u16();
        let body: serde_json::Value = response.json().await.unwrap_or_default();
        match (status, body["error"]["code"].as_str()) {
            (409, Some("identity_taken")) => Err(ClientError::HostError(
                "This computer already signs in to another account. It's connected, but it stays on that account.".into(),
            )),
            (409, Some("too_many_keys")) => Err(ClientError::HostError(
                "Your account has as many devices as it can hold. Remove one in Settings first."
                    .into(),
            )),
            (401, _) => Err(ClientError::Auth("We couldn't connect to your account. Try again.".into())),
            _ => Err(failed()),
        }
    }

    /// Drop a pairing made by code from the store and the devices list.
    pub(crate) fn forget_saved_pairing(&self, device_id: &str) {
        if let Some(store) = self.pairing_store()
            && store
                .delete(self.inner.credentials.user_id(), device_id)
                .is_err()
        {
            tracing::warn!("saved pairing not removed");
        }
        self.inner.workspace.remove_paired(device_id);
        self.inner.recompute_workspace();
    }
}

fn coder_access_policy() -> paired_device::RelayPolicy {
    paired_device::RelayPolicy::Production
}
