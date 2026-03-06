use std::time::{Duration, Instant};

use autopilot_spacetime::live::{
    DEFAULT_LIVE_PRESENCE_HEARTBEAT_INTERVAL_MS, DEFAULT_LIVE_PRESENCE_STALE_AFTER_MS,
    LivePresenceSummary, LiveSpacetimeClient,
};
use autopilot_spacetime::presence::{
    OFFLINE_REASON_EXPLICIT, OFFLINE_REASON_TTL_EXPIRED, PresenceCardinality,
    ProviderPresenceRegistry, RegisterOnlineRequest, sign_nostr_presence_challenge,
};
use nostr::NostrIdentity;

use crate::state::provider_runtime::ProviderMode;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpacetimePresenceSnapshot {
    pub providers_online: u64,
    pub counter_source: String,
    pub counter_cardinality: String,
    pub node_id: String,
    pub session_id: String,
    pub node_status: String,
    pub node_last_seen_unix_ms: Option<u64>,
    pub node_offline_reason: Option<String>,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
}

impl Default for SpacetimePresenceSnapshot {
    fn default() -> Self {
        Self {
            providers_online: 0,
            counter_source: "spacetime.presence".to_string(),
            counter_cardinality: PresenceCardinality::Identity.label().to_string(),
            node_id: "uninitialized".to_string(),
            session_id: "uninitialized".to_string(),
            node_status: "unregistered".to_string(),
            node_last_seen_unix_ms: None,
            node_offline_reason: None,
            last_error: None,
            last_action: Some("Spacetime presence runtime initialized".to_string()),
        }
    }
}

pub struct SpacetimePresenceRuntime {
    registry: ProviderPresenceRegistry,
    live_client: Option<LiveSpacetimeClient>,
    node_id: String,
    session_id: String,
    region: String,
    cardinality: PresenceCardinality,
    heartbeat_interval: Duration,
    stale_after: Duration,
    next_heartbeat_due: Option<Instant>,
    next_refresh_due: Option<Instant>,
    providers_online: u64,
    node_status: String,
    node_last_seen_unix_ms: Option<u64>,
    node_offline_reason: Option<String>,
    last_error: Option<String>,
    last_action: Option<String>,
}

impl SpacetimePresenceRuntime {
    #[must_use]
    pub fn new(identity: Option<&NostrIdentity>) -> Self {
        let now_unix_ms = now_unix_ms();
        let hostname = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .ok()
            .map(|value| sanitize_component(value.as_str()))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "desktop".to_string());
        let identity_hint = identity
            .map(|value| value.public_key_hex.chars().take(12).collect::<String>())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "anon".to_string());
        let node_id = format!("device:{hostname}:{identity_hint}:{}", std::process::id());
        let session_id = format!("sess:{hostname}:{now_unix_ms}");
        let region = std::env::var("OPENAGENTS_REGION")
            .or_else(|_| std::env::var("OA_REGION"))
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "local".to_string());
        let registry = ProviderPresenceRegistry::default();
        let heartbeat_interval = Duration::from_millis(
            registry
                .policy()
                .heartbeat_interval_ms
                .max(DEFAULT_LIVE_PRESENCE_HEARTBEAT_INTERVAL_MS),
        );
        let stale_after = Duration::from_millis(
            registry
                .policy()
                .stale_after_ms
                .max(DEFAULT_LIVE_PRESENCE_STALE_AFTER_MS),
        );

        let mut runtime = Self {
            registry,
            live_client: None,
            node_id,
            session_id,
            region,
            cardinality: PresenceCardinality::Identity,
            heartbeat_interval,
            stale_after,
            next_heartbeat_due: None,
            next_refresh_due: None,
            providers_online: 0,
            node_status: "unregistered".to_string(),
            node_last_seen_unix_ms: None,
            node_offline_reason: None,
            last_error: None,
            last_action: Some("Spacetime presence runtime initialized".to_string()),
        };
        runtime.refresh_local_snapshot();
        runtime
    }

    pub fn configure_live_client(&mut self, client: LiveSpacetimeClient) {
        self.live_client = Some(client);
        self.next_refresh_due = Some(Instant::now());
        self.last_error = None;
        self.last_action = Some("Configured live Spacetime presence client".to_string());
        let _ = self.refresh_live_presence();
    }

    pub fn clear_live_client(&mut self) {
        self.live_client = None;
        self.next_refresh_due = None;
        self.next_heartbeat_due = None;
        self.last_error = None;
        self.last_action = Some("Using local Spacetime presence registry fallback".to_string());
        self.refresh_local_snapshot();
    }

    #[must_use]
    pub fn live_client(&self) -> Option<LiveSpacetimeClient> {
        self.live_client.clone()
    }

    pub fn register_online(&mut self, identity: Option<&NostrIdentity>) -> Result<(), String> {
        let Some(identity) = identity else {
            let error = "cannot register online: nostr identity missing".to_string();
            self.last_error = Some(error.clone());
            return Err(error);
        };
        let now_unix_ms = now_unix_ms();

        if let Some(client) = self.live_client.as_ref() {
            let challenge = client.request_presence_challenge(
                self.node_id.as_str(),
                self.session_id.as_str(),
                Some(self.node_id.as_str()),
                self.region.as_str(),
            )?;
            let signature = sign_nostr_presence_challenge(
                identity.private_key_hex.as_str(),
                self.node_id.as_str(),
                challenge.as_str(),
            )
            .map_err(|error| error.to_string())?;
            client.bind_presence_identity(
                self.node_id.as_str(),
                identity.public_key_hex.as_str(),
                Some(identity.npub.as_str()),
                signature.as_str(),
            )?;
            self.node_offline_reason = None;
            self.next_heartbeat_due = Some(Instant::now() + self.heartbeat_interval);
            self.next_refresh_due = Some(Instant::now() + self.heartbeat_interval);
            self.last_error = None;
            self.last_action = Some("Registered provider presence as online".to_string());
            self.refresh_live_presence()?;
            return Ok(());
        }

        let challenge = self
            .registry
            .issue_nostr_bind_challenge(self.node_id.as_str(), now_unix_ms)
            .map_err(|error| error.to_string())?;
        let signature = sign_nostr_presence_challenge(
            identity.private_key_hex.as_str(),
            self.node_id.as_str(),
            challenge.challenge.as_str(),
        )
        .map_err(|error| error.to_string())?;

        self.registry
            .register_online(RegisterOnlineRequest {
                node_id: self.node_id.clone(),
                session_id: self.session_id.clone(),
                worker_id: Some(self.node_id.clone()),
                region: self.region.clone(),
                nostr_pubkey_hex: identity.public_key_hex.clone(),
                nostr_pubkey_npub: Some(identity.npub.clone()),
                challenge: challenge.challenge,
                challenge_signature_hex: signature,
                now_unix_ms,
            })
            .map_err(|error| error.to_string())?;

        self.node_offline_reason = None;
        self.next_heartbeat_due = Some(Instant::now() + self.heartbeat_interval);
        self.last_error = None;
        self.last_action = Some("Registered provider presence as online".to_string());
        self.refresh_local_snapshot();
        Ok(())
    }

    pub fn register_offline(&mut self) -> Result<(), String> {
        if let Some(client) = self.live_client.as_ref() {
            match client.register_offline(self.node_id.as_str()) {
                Ok(()) => {}
                Err(error)
                    if error.contains("active connection missing")
                        || error.contains("identity is not connected") => {}
                Err(error) => {
                    self.last_error = Some(error.clone());
                    return Err(error);
                }
            }
            self.next_heartbeat_due = None;
            self.next_refresh_due = Some(Instant::now() + self.heartbeat_interval);
            self.node_status = "offline".to_string();
            self.node_offline_reason = Some(OFFLINE_REASON_EXPLICIT.to_string());
            self.last_error = None;
            self.last_action = Some("Registered provider presence as offline".to_string());
            let _ = self.refresh_live_presence();
            return Ok(());
        }

        if self.registry.row(self.node_id.as_str()).is_none() {
            self.next_heartbeat_due = None;
            self.last_error = None;
            self.last_action = Some("Provider presence already offline".to_string());
            self.node_status = "offline".to_string();
            self.node_offline_reason = Some(OFFLINE_REASON_EXPLICIT.to_string());
            self.providers_online = 0;
            return Ok(());
        }
        let now_unix_ms = now_unix_ms();
        self.registry
            .register_offline(self.node_id.as_str(), now_unix_ms)
            .map_err(|error| error.to_string())?;
        self.next_heartbeat_due = None;
        self.last_error = None;
        self.last_action = Some("Registered provider presence as offline".to_string());
        self.refresh_local_snapshot();
        Ok(())
    }

    pub fn tick(&mut self, provider_mode: ProviderMode) -> bool {
        if self.live_client.is_some() {
            return self.tick_live(provider_mode);
        }

        let now_unix_ms = now_unix_ms();
        let mut changed = false;
        let expired = self.registry.sweep_expired(now_unix_ms);
        if expired.iter().any(|node| node == self.node_id.as_str()) {
            self.last_error = Some("Provider presence expired by TTL".to_string());
            self.last_action = Some("Provider presence TTL expired".to_string());
            self.next_heartbeat_due = None;
            changed = true;
        }

        if provider_mode == ProviderMode::Offline {
            if changed {
                self.refresh_local_snapshot();
            }
            return changed;
        }

        let due = self
            .next_heartbeat_due
            .is_some_and(|instant| Instant::now() >= instant);
        if !due {
            if changed {
                self.refresh_local_snapshot();
            }
            return changed;
        }
        if self.registry.row(self.node_id.as_str()).is_some() {
            match self.registry.heartbeat(self.node_id.as_str(), now_unix_ms) {
                Ok(_) => {
                    self.last_error = None;
                    self.last_action = Some("Registered provider presence heartbeat".to_string());
                    changed = true;
                }
                Err(error) => {
                    self.last_error = Some(error.to_string());
                    changed = true;
                }
            }
        }
        self.next_heartbeat_due = Some(Instant::now() + self.heartbeat_interval);
        self.refresh_local_snapshot();
        changed
    }

    #[must_use]
    pub fn snapshot(&self) -> SpacetimePresenceSnapshot {
        SpacetimePresenceSnapshot {
            providers_online: self.providers_online,
            counter_source: if self.live_client.is_some() {
                "spacetime.presence.live".to_string()
            } else {
                "spacetime.presence".to_string()
            },
            counter_cardinality: self.cardinality.label().to_string(),
            node_id: self.node_id.clone(),
            session_id: self.session_id.clone(),
            node_status: self.node_status.clone(),
            node_last_seen_unix_ms: self.node_last_seen_unix_ms,
            node_offline_reason: self.node_offline_reason.clone(),
            last_error: self.last_error.clone(),
            last_action: self.last_action.clone(),
        }
    }

    #[must_use]
    pub fn registry(&self) -> &ProviderPresenceRegistry {
        &self.registry
    }

    fn tick_live(&mut self, provider_mode: ProviderMode) -> bool {
        let mut changed = false;
        let now = Instant::now();
        let refresh_due = self.next_refresh_due.is_none_or(|deadline| now >= deadline);
        let heartbeat_due = provider_mode != ProviderMode::Offline
            && self
                .next_heartbeat_due
                .is_some_and(|deadline| now >= deadline);

        if heartbeat_due {
            let Some(client) = self.live_client.as_ref() else {
                return false;
            };
            match client.heartbeat(self.node_id.as_str()) {
                Ok(()) => {
                    self.last_error = None;
                    self.last_action = Some("Registered provider presence heartbeat".to_string());
                    self.next_heartbeat_due = Some(now + self.heartbeat_interval);
                    changed = true;
                }
                Err(error) => {
                    self.last_error = Some(error);
                    self.next_heartbeat_due = Some(now + self.heartbeat_interval);
                    changed = true;
                }
            }
        }

        if refresh_due || heartbeat_due {
            let refreshed = self.refresh_live_presence().is_ok();
            self.next_refresh_due = Some(now + self.heartbeat_interval);
            changed = changed || refreshed;
        }

        if provider_mode != ProviderMode::Offline
            && self.node_status == "online"
            && self.node_last_seen_unix_ms.is_some_and(|last_seen| {
                now_unix_ms().saturating_sub(last_seen) > self.stale_after.as_millis() as u64
            })
        {
            self.node_status = "offline".to_string();
            self.node_offline_reason = Some(OFFLINE_REASON_TTL_EXPIRED.to_string());
            self.last_action = Some("Provider presence TTL expired".to_string());
            changed = true;
        }

        changed
    }

    fn refresh_local_snapshot(&mut self) {
        let row = self.registry.row(self.node_id.as_str());
        self.providers_online = self.registry.providers_online(self.cardinality);
        self.node_status = row
            .map(|value| value.status.label().to_string())
            .unwrap_or_else(|| {
                self.node_offline_reason
                    .as_ref()
                    .map_or_else(|| "unregistered".to_string(), |_| "offline".to_string())
            });
        self.node_last_seen_unix_ms = row.map(|value| value.last_seen_unix_ms);
        self.node_offline_reason = row
            .and_then(|value| value.offline_reason.clone())
            .or_else(|| self.node_offline_reason.clone());
    }

    fn refresh_live_presence(&mut self) -> Result<(), String> {
        let Some(client) = self.live_client.as_ref() else {
            return Ok(());
        };
        let summary = client.presence_summary(
            self.node_id.as_str(),
            now_unix_ms(),
            self.stale_after.as_millis() as u64,
        )?;
        self.apply_live_summary(summary);
        Ok(())
    }

    fn apply_live_summary(&mut self, summary: LivePresenceSummary) {
        self.providers_online = summary.providers_online;
        self.node_last_seen_unix_ms = summary.node_last_seen_unix_ms;
        if summary.node_online {
            self.node_status = "online".to_string();
            self.node_offline_reason = None;
        } else if self.node_offline_reason.is_some() {
            self.node_status = "offline".to_string();
        } else {
            self.node_status = "unregistered".to_string();
        }
    }
}

fn sanitize_component(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::SpacetimePresenceRuntime;
    use crate::state::provider_runtime::ProviderMode;
    use autopilot_spacetime::presence::PresenceStatus;
    use bitcoin::secp256k1::{Keypair, Secp256k1, SecretKey};
    use nostr::NostrIdentity;
    use std::path::PathBuf;

    fn fixture_identity() -> NostrIdentity {
        let secret_key =
            SecretKey::from_slice(&[0x11_u8; 32]).expect("fixture secret key should be valid");
        let secp = Secp256k1::signing_only();
        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let (x_only, _) = keypair.x_only_public_key();
        NostrIdentity {
            identity_path: PathBuf::from("/tmp/identity.mnemonic"),
            mnemonic: "fixture".to_string(),
            npub: "npub1fixture".to_string(),
            nsec: "nsec1fixture".to_string(),
            public_key_hex: hex::encode(x_only.serialize()),
            private_key_hex: "1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
        }
    }

    #[test]
    fn runtime_registers_online_and_offline() {
        let identity = fixture_identity();
        let mut runtime = SpacetimePresenceRuntime::new(Some(&identity));
        let online = runtime.register_online(Some(&identity));
        assert!(online.is_ok());

        let snapshot_online = runtime.snapshot();
        assert_eq!(snapshot_online.providers_online, 1);
        assert_eq!(snapshot_online.node_status, "online");
        assert_eq!(runtime.registry().providers_online(runtime.cardinality), 1);

        let offline = runtime.register_offline();
        assert!(offline.is_ok());

        let snapshot_offline = runtime.snapshot();
        assert_eq!(snapshot_offline.providers_online, 0);
        assert_eq!(snapshot_offline.node_status, "offline");
    }

    #[test]
    fn runtime_heartbeat_tick_updates_presence_when_online() {
        let identity = fixture_identity();
        let mut runtime = SpacetimePresenceRuntime::new(Some(&identity));
        let online = runtime.register_online(Some(&identity));
        assert!(online.is_ok());

        let pre = runtime
            .registry()
            .row(runtime.node_id.as_str())
            .expect("presence row should exist")
            .last_seen_unix_ms;

        std::thread::sleep(std::time::Duration::from_millis(
            runtime.registry().policy().heartbeat_interval_ms + 10,
        ));
        let changed = runtime.tick(ProviderMode::Online);
        assert!(changed);

        let row = runtime
            .registry()
            .row(runtime.node_id.as_str())
            .expect("presence row should still exist");
        assert_eq!(row.status, PresenceStatus::Online);
        assert!(row.last_seen_unix_ms >= pre);
    }

    #[test]
    fn runtime_requires_identity_to_go_online() {
        let mut runtime = SpacetimePresenceRuntime::new(None);
        let error = runtime
            .register_online(None)
            .expect_err("missing identity should fail");
        assert!(error.contains("nostr identity missing"));
        assert_eq!(runtime.snapshot().node_status, "unregistered");
    }
}
