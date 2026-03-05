use std::time::{Duration, Instant};

use autopilot_spacetime::presence::{
    PresenceCardinality, ProviderPresenceRegistry, RegisterOnlineRequest,
    sign_nostr_presence_challenge,
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
    node_id: String,
    session_id: String,
    region: String,
    cardinality: PresenceCardinality,
    heartbeat_interval: Duration,
    next_heartbeat_due: Option<Instant>,
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
        let heartbeat_interval =
            Duration::from_millis(registry.policy().heartbeat_interval_ms.max(1_000));

        Self {
            registry,
            node_id,
            session_id,
            region,
            cardinality: PresenceCardinality::Identity,
            heartbeat_interval,
            next_heartbeat_due: None,
            last_error: None,
            last_action: Some("Spacetime presence runtime initialized".to_string()),
        }
    }

    pub fn register_online(&mut self, identity: Option<&NostrIdentity>) -> Result<(), String> {
        let Some(identity) = identity else {
            let error = "cannot register online: nostr identity missing".to_string();
            self.last_error = Some(error.clone());
            return Err(error);
        };
        let now_unix_ms = now_unix_ms();
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

        self.next_heartbeat_due = Some(Instant::now() + self.heartbeat_interval);
        self.last_error = None;
        self.last_action = Some("Registered provider presence as online".to_string());
        Ok(())
    }

    pub fn register_offline(&mut self) -> Result<(), String> {
        if self.registry.row(self.node_id.as_str()).is_none() {
            self.next_heartbeat_due = None;
            self.last_error = None;
            self.last_action = Some("Provider presence already offline".to_string());
            return Ok(());
        }
        let now_unix_ms = now_unix_ms();
        self.registry
            .register_offline(self.node_id.as_str(), now_unix_ms)
            .map_err(|error| error.to_string())?;
        self.next_heartbeat_due = None;
        self.last_error = None;
        self.last_action = Some("Registered provider presence as offline".to_string());
        Ok(())
    }

    pub fn tick(&mut self, provider_mode: ProviderMode) -> bool {
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
            return changed;
        }

        let due = self
            .next_heartbeat_due
            .is_some_and(|instant| Instant::now() >= instant);
        if !due {
            return changed;
        }
        if self.registry.row(self.node_id.as_str()).is_some() {
            match self.registry.heartbeat(self.node_id.as_str(), now_unix_ms) {
                Ok(_) => {
                    self.last_error = None;
                    self.last_action =
                        Some("Registered provider presence heartbeat".to_string());
                    changed = true;
                }
                Err(error) => {
                    self.last_error = Some(error.to_string());
                    changed = true;
                }
            }
        }
        self.next_heartbeat_due = Some(Instant::now() + self.heartbeat_interval);
        changed
    }

    #[must_use]
    pub fn snapshot(&self) -> SpacetimePresenceSnapshot {
        let row = self.registry.row(self.node_id.as_str());
        SpacetimePresenceSnapshot {
            providers_online: self.registry.providers_online(self.cardinality),
            counter_source: "spacetime.presence".to_string(),
            counter_cardinality: self.cardinality.label().to_string(),
            node_id: self.node_id.clone(),
            session_id: self.session_id.clone(),
            node_status: row
                .map(|value| value.status.label().to_string())
                .unwrap_or_else(|| "unregistered".to_string()),
            node_last_seen_unix_ms: row.map(|value| value.last_seen_unix_ms),
            node_offline_reason: row.and_then(|value| value.offline_reason.clone()),
            last_error: self.last_error.clone(),
            last_action: self.last_action.clone(),
        }
    }

    #[must_use]
    pub fn registry(&self) -> &ProviderPresenceRegistry {
        &self.registry
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
    use autopilot_spacetime::presence::PresenceStatus;
    use bitcoin::secp256k1::{Keypair, Secp256k1, SecretKey};
    use crate::state::provider_runtime::ProviderMode;
    use nostr::NostrIdentity;
    use std::path::PathBuf;

    fn fixture_identity() -> NostrIdentity {
        let secret_key = SecretKey::from_slice(&[0x11_u8; 32])
            .expect("fixture secret key should be valid");
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
        assert_eq!(snapshot_online.node_status, PresenceStatus::Online.label());
        assert_eq!(snapshot_online.providers_online, 1);

        let offline = runtime.register_offline();
        assert!(offline.is_ok());
        let snapshot_offline = runtime.snapshot();
        assert_eq!(snapshot_offline.node_status, PresenceStatus::Offline.label());
        assert_eq!(snapshot_offline.providers_online, 0);
    }

    #[test]
    fn runtime_heartbeat_tick_updates_presence_when_online() {
        let identity = fixture_identity();
        let mut runtime = SpacetimePresenceRuntime::new(Some(&identity));
        let online = runtime.register_online(Some(&identity));
        assert!(online.is_ok());

        std::thread::sleep(std::time::Duration::from_millis(
            runtime.registry().policy().heartbeat_interval_ms + 10,
        ));
        let changed = runtime.tick(ProviderMode::Online);
        assert!(changed);
        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.node_status, PresenceStatus::Online.label());
        assert!(snapshot.node_last_seen_unix_ms.is_some());
    }
}
