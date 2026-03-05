//! Provider presence lifecycle primitives for Spacetime-backed online state.

use std::collections::{HashMap, HashSet};

use bitcoin::secp256k1::{
    Keypair, Message, Secp256k1, SecretKey, XOnlyPublicKey, schnorr::Signature,
};
use sha2::{Digest, Sha256};

pub const NOSTR_PRESENCE_BIND_DOMAIN: &str = "openagents:nostr-presence-bind:v1";
pub const OFFLINE_REASON_EXPLICIT: &str = "explicit_offline";
pub const OFFLINE_REASON_TTL_EXPIRED: &str = "ttl_expired";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresenceStatus {
    Online,
    Offline,
}

impl PresenceStatus {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Offline => "offline",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresenceCardinality {
    Device,
    Worker,
    Identity,
}

impl PresenceCardinality {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Device => "device",
            Self::Worker => "worker",
            Self::Identity => "identity",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresenceTtlPolicy {
    pub challenge_ttl_ms: u64,
    pub heartbeat_interval_ms: u64,
    pub stale_after_ms: u64,
}

impl Default for PresenceTtlPolicy {
    fn default() -> Self {
        Self {
            challenge_ttl_ms: 5 * 60 * 1_000,
            heartbeat_interval_ms: 5_000,
            stale_after_ms: 30_000,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NostrBindChallenge {
    pub node_id: String,
    pub challenge: String,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub consumed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderPresenceRow {
    pub node_id: String,
    pub session_id: String,
    pub worker_id: Option<String>,
    pub region: String,
    pub status: PresenceStatus,
    pub nostr_pubkey_hex: String,
    pub nostr_pubkey_npub: Option<String>,
    pub challenge_signature_hex: String,
    pub connected_at_unix_ms: u64,
    pub last_seen_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub offline_reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisterOnlineRequest {
    pub node_id: String,
    pub session_id: String,
    pub worker_id: Option<String>,
    pub region: String,
    pub nostr_pubkey_hex: String,
    pub nostr_pubkey_npub: Option<String>,
    pub challenge: String,
    pub challenge_signature_hex: String,
    pub now_unix_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresenceError {
    EmptyField(&'static str),
    ChallengeMissing { node_id: String },
    ChallengeMismatch,
    ChallengeExpired,
    ChallengeConsumed,
    InvalidPrivateKey(String),
    InvalidPublicKey(String),
    InvalidSignature(String),
    SignatureVerificationFailed(String),
    PresenceNotFound { node_id: String },
    PresenceNotOnline { node_id: String },
}

impl std::fmt::Display for PresenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyField(field) => write!(f, "{field} is required"),
            Self::ChallengeMissing { node_id } => {
                write!(f, "presence bind challenge missing for node_id={node_id}")
            }
            Self::ChallengeMismatch => write!(f, "presence bind challenge mismatch"),
            Self::ChallengeExpired => write!(f, "presence bind challenge expired"),
            Self::ChallengeConsumed => write!(f, "presence bind challenge already consumed"),
            Self::InvalidPrivateKey(reason) => write!(f, "invalid private key: {reason}"),
            Self::InvalidPublicKey(reason) => write!(f, "invalid nostr pubkey: {reason}"),
            Self::InvalidSignature(reason) => write!(f, "invalid schnorr signature: {reason}"),
            Self::SignatureVerificationFailed(reason) => {
                write!(f, "presence signature verification failed: {reason}")
            }
            Self::PresenceNotFound { node_id } => {
                write!(f, "presence row not found for node_id={node_id}")
            }
            Self::PresenceNotOnline { node_id } => {
                write!(f, "presence row is not online for node_id={node_id}")
            }
        }
    }
}

impl std::error::Error for PresenceError {}

#[derive(Clone, Debug)]
pub struct ProviderPresenceRegistry {
    policy: PresenceTtlPolicy,
    next_nonce: u64,
    challenges: HashMap<String, NostrBindChallenge>,
    rows: HashMap<String, ProviderPresenceRow>,
}

impl Default for ProviderPresenceRegistry {
    fn default() -> Self {
        Self::new(PresenceTtlPolicy::default())
    }
}

impl ProviderPresenceRegistry {
    #[must_use]
    pub fn new(policy: PresenceTtlPolicy) -> Self {
        Self {
            policy,
            next_nonce: 1,
            challenges: HashMap::new(),
            rows: HashMap::new(),
        }
    }

    #[must_use]
    pub fn policy(&self) -> PresenceTtlPolicy {
        self.policy
    }

    pub fn issue_nostr_bind_challenge(
        &mut self,
        node_id: &str,
        now_unix_ms: u64,
    ) -> Result<NostrBindChallenge, PresenceError> {
        require_non_empty(node_id, "node_id")?;
        let nonce = self.next_nonce;
        self.next_nonce = self.next_nonce.saturating_add(1);
        let challenge = format!("{NOSTR_PRESENCE_BIND_DOMAIN}:{node_id}:{now_unix_ms}:{nonce}");
        let row = NostrBindChallenge {
            node_id: node_id.to_string(),
            challenge,
            issued_at_unix_ms: now_unix_ms,
            expires_at_unix_ms: now_unix_ms.saturating_add(self.policy.challenge_ttl_ms),
            consumed: false,
        };
        self.challenges.insert(node_id.to_string(), row.clone());
        Ok(row)
    }

    pub fn register_online(
        &mut self,
        request: RegisterOnlineRequest,
    ) -> Result<ProviderPresenceRow, PresenceError> {
        require_non_empty(request.node_id.as_str(), "node_id")?;
        require_non_empty(request.session_id.as_str(), "session_id")?;
        require_non_empty(request.region.as_str(), "region")?;
        require_non_empty(request.nostr_pubkey_hex.as_str(), "nostr_pubkey_hex")?;
        require_non_empty(request.challenge.as_str(), "challenge")?;
        require_non_empty(
            request.challenge_signature_hex.as_str(),
            "challenge_signature_hex",
        )?;

        let Some(challenge_row) = self.challenges.get_mut(request.node_id.as_str()) else {
            return Err(PresenceError::ChallengeMissing {
                node_id: request.node_id,
            });
        };
        if challenge_row.consumed {
            return Err(PresenceError::ChallengeConsumed);
        }
        if request.now_unix_ms > challenge_row.expires_at_unix_ms {
            return Err(PresenceError::ChallengeExpired);
        }
        if request.challenge != challenge_row.challenge {
            return Err(PresenceError::ChallengeMismatch);
        }

        verify_nostr_presence_challenge_signature(
            request.nostr_pubkey_hex.as_str(),
            request.node_id.as_str(),
            request.challenge.as_str(),
            request.challenge_signature_hex.as_str(),
        )?;
        challenge_row.consumed = true;

        let connected_at = self
            .rows
            .get(request.node_id.as_str())
            .map(|existing| existing.connected_at_unix_ms)
            .unwrap_or(request.now_unix_ms);
        let row = ProviderPresenceRow {
            node_id: request.node_id.clone(),
            session_id: request.session_id,
            worker_id: request
                .worker_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
            region: request.region.trim().to_string(),
            status: PresenceStatus::Online,
            nostr_pubkey_hex: request.nostr_pubkey_hex.trim().to_string(),
            nostr_pubkey_npub: request
                .nostr_pubkey_npub
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
            challenge_signature_hex: request.challenge_signature_hex.trim().to_string(),
            connected_at_unix_ms: connected_at,
            last_seen_unix_ms: request.now_unix_ms,
            expires_at_unix_ms: request
                .now_unix_ms
                .saturating_add(self.policy.stale_after_ms),
            offline_reason: None,
        };
        self.rows.insert(request.node_id, row.clone());
        Ok(row)
    }

    pub fn heartbeat(
        &mut self,
        node_id: &str,
        now_unix_ms: u64,
    ) -> Result<ProviderPresenceRow, PresenceError> {
        require_non_empty(node_id, "node_id")?;
        let Some(row) = self.rows.get_mut(node_id) else {
            return Err(PresenceError::PresenceNotFound {
                node_id: node_id.to_string(),
            });
        };
        if row.status != PresenceStatus::Online {
            return Err(PresenceError::PresenceNotOnline {
                node_id: node_id.to_string(),
            });
        }
        row.last_seen_unix_ms = now_unix_ms;
        row.expires_at_unix_ms = now_unix_ms.saturating_add(self.policy.stale_after_ms);
        row.offline_reason = None;
        Ok(row.clone())
    }

    pub fn register_offline(
        &mut self,
        node_id: &str,
        now_unix_ms: u64,
    ) -> Result<ProviderPresenceRow, PresenceError> {
        require_non_empty(node_id, "node_id")?;
        let Some(row) = self.rows.get_mut(node_id) else {
            return Err(PresenceError::PresenceNotFound {
                node_id: node_id.to_string(),
            });
        };
        row.status = PresenceStatus::Offline;
        row.last_seen_unix_ms = now_unix_ms;
        row.expires_at_unix_ms = now_unix_ms;
        row.offline_reason = Some(OFFLINE_REASON_EXPLICIT.to_string());
        Ok(row.clone())
    }

    pub fn sweep_expired(&mut self, now_unix_ms: u64) -> Vec<String> {
        let mut expired = Vec::new();
        for row in self.rows.values_mut() {
            if row.status == PresenceStatus::Online && now_unix_ms > row.expires_at_unix_ms {
                row.status = PresenceStatus::Offline;
                row.offline_reason = Some(OFFLINE_REASON_TTL_EXPIRED.to_string());
                row.last_seen_unix_ms = now_unix_ms;
                row.expires_at_unix_ms = now_unix_ms;
                expired.push(row.node_id.clone());
            }
        }
        expired.sort();
        expired
    }

    #[must_use]
    pub fn row(&self, node_id: &str) -> Option<&ProviderPresenceRow> {
        self.rows.get(node_id)
    }

    #[must_use]
    pub fn rows(&self) -> Vec<ProviderPresenceRow> {
        let mut rows = self.rows.values().cloned().collect::<Vec<_>>();
        rows.sort_by(|left, right| left.node_id.cmp(&right.node_id));
        rows
    }

    #[must_use]
    pub fn providers_online(&self, cardinality: PresenceCardinality) -> u64 {
        match cardinality {
            PresenceCardinality::Device => self
                .rows
                .values()
                .filter(|row| row.status == PresenceStatus::Online)
                .count() as u64,
            PresenceCardinality::Worker => {
                let mut workers = HashSet::new();
                for row in self.rows.values() {
                    if row.status != PresenceStatus::Online {
                        continue;
                    }
                    workers.insert(worker_cardinality_key(row));
                }
                workers.len() as u64
            }
            PresenceCardinality::Identity => {
                let mut identities = HashSet::new();
                for row in self.rows.values() {
                    if row.status != PresenceStatus::Online {
                        continue;
                    }
                    identities.insert(row.nostr_pubkey_hex.clone());
                }
                identities.len() as u64
            }
        }
    }
}

#[must_use]
pub fn sign_nostr_presence_challenge(
    private_key_hex: &str,
    node_id: &str,
    challenge: &str,
) -> Result<String, PresenceError> {
    require_non_empty(private_key_hex, "private_key_hex")?;
    require_non_empty(node_id, "node_id")?;
    require_non_empty(challenge, "challenge")?;

    let private_key_bytes = hex::decode(private_key_hex.trim())
        .map_err(|error| PresenceError::InvalidPrivateKey(error.to_string()))?;
    let secret_key = SecretKey::from_slice(private_key_bytes.as_slice())
        .map_err(|error| PresenceError::InvalidPrivateKey(error.to_string()))?;
    let secp = Secp256k1::signing_only();
    let keypair = Keypair::from_secret_key(&secp, &secret_key);
    let message = Message::from_digest(challenge_digest(node_id, challenge));
    let signature = secp.sign_schnorr_no_aux_rand(&message, &keypair);
    Ok(hex::encode(signature.as_ref()))
}

pub fn verify_nostr_presence_challenge_signature(
    nostr_pubkey_hex: &str,
    node_id: &str,
    challenge: &str,
    signature_hex: &str,
) -> Result<(), PresenceError> {
    require_non_empty(nostr_pubkey_hex, "nostr_pubkey_hex")?;
    require_non_empty(node_id, "node_id")?;
    require_non_empty(challenge, "challenge")?;
    require_non_empty(signature_hex, "challenge_signature_hex")?;

    let pubkey_bytes = hex::decode(nostr_pubkey_hex.trim())
        .map_err(|error| PresenceError::InvalidPublicKey(error.to_string()))?;
    let pubkey = XOnlyPublicKey::from_slice(pubkey_bytes.as_slice())
        .map_err(|error| PresenceError::InvalidPublicKey(error.to_string()))?;
    let signature_bytes = hex::decode(signature_hex.trim())
        .map_err(|error| PresenceError::InvalidSignature(error.to_string()))?;
    let signature = Signature::from_slice(signature_bytes.as_slice())
        .map_err(|error| PresenceError::InvalidSignature(error.to_string()))?;
    let message = Message::from_digest(challenge_digest(node_id, challenge));
    let secp = Secp256k1::verification_only();
    secp.verify_schnorr(&signature, &message, &pubkey)
        .map_err(|error| PresenceError::SignatureVerificationFailed(error.to_string()))
}

fn worker_cardinality_key(row: &ProviderPresenceRow) -> String {
    let worker = row.worker_id.as_deref().unwrap_or(row.node_id.as_str());
    format!("{}:{worker}", row.nostr_pubkey_hex)
}

fn require_non_empty(value: &str, field: &'static str) -> Result<(), PresenceError> {
    if value.trim().is_empty() {
        return Err(PresenceError::EmptyField(field));
    }
    Ok(())
}

fn challenge_digest(node_id: &str, challenge: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(NOSTR_PRESENCE_BIND_DOMAIN.as_bytes());
    hasher.update(b":");
    hasher.update(node_id.as_bytes());
    hasher.update(b":");
    hasher.update(challenge.as_bytes());
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::{
        OFFLINE_REASON_EXPLICIT, OFFLINE_REASON_TTL_EXPIRED, PresenceCardinality, PresenceStatus,
        ProviderPresenceRegistry, RegisterOnlineRequest, sign_nostr_presence_challenge,
    };
    use bitcoin::secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};

    fn fixture_private_key_hex() -> String {
        "1111111111111111111111111111111111111111111111111111111111111111".to_string()
    }

    fn fixture_pubkey_hex(private_key_hex: &str) -> String {
        let bytes = hex::decode(private_key_hex).unwrap_or_default();
        let secret_key = SecretKey::from_slice(bytes.as_slice()).unwrap_or_else(|_| {
            SecretKey::from_slice(&[1u8; 32]).expect("fallback key should build")
        });
        let secp = Secp256k1::signing_only();
        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let (x_only, _) = XOnlyPublicKey::from_keypair(&keypair);
        hex::encode(x_only.serialize())
    }

    #[test]
    fn online_heartbeat_offline_lifecycle_is_deterministic() {
        let mut registry = ProviderPresenceRegistry::default();
        let private_key_hex = fixture_private_key_hex();
        let pubkey_hex = fixture_pubkey_hex(private_key_hex.as_str());

        let challenge = registry
            .issue_nostr_bind_challenge("node-a", 1_000)
            .expect("challenge should issue");
        let signature = sign_nostr_presence_challenge(
            private_key_hex.as_str(),
            "node-a",
            challenge.challenge.as_str(),
        )
        .expect("challenge should sign");

        let row = registry
            .register_online(RegisterOnlineRequest {
                node_id: "node-a".to_string(),
                session_id: "sess-a".to_string(),
                worker_id: Some("worker-a".to_string()),
                region: "us-central1".to_string(),
                nostr_pubkey_hex: pubkey_hex.clone(),
                nostr_pubkey_npub: Some("npub1fixture".to_string()),
                challenge: challenge.challenge,
                challenge_signature_hex: signature,
                now_unix_ms: 1_500,
            })
            .expect("online registration should pass");
        assert_eq!(row.status, PresenceStatus::Online);
        assert_eq!(registry.providers_online(PresenceCardinality::Identity), 1);

        let heartbeat = registry
            .heartbeat("node-a", 2_000)
            .expect("heartbeat should succeed");
        assert_eq!(heartbeat.status, PresenceStatus::Online);
        assert_eq!(heartbeat.offline_reason, None);

        let offline = registry
            .register_offline("node-a", 2_100)
            .expect("offline transition should succeed");
        assert_eq!(offline.status, PresenceStatus::Offline);
        assert_eq!(
            offline.offline_reason.as_deref(),
            Some(OFFLINE_REASON_EXPLICIT)
        );
        assert_eq!(registry.providers_online(PresenceCardinality::Identity), 0);
    }

    #[test]
    fn online_registration_requires_challenge() {
        let mut registry = ProviderPresenceRegistry::default();
        let private_key_hex = fixture_private_key_hex();
        let pubkey_hex = fixture_pubkey_hex(private_key_hex.as_str());
        let signature = sign_nostr_presence_challenge(private_key_hex.as_str(), "node-a", "nope")
            .expect("signature should build");

        let error = registry
            .register_online(RegisterOnlineRequest {
                node_id: "node-a".to_string(),
                session_id: "sess-a".to_string(),
                worker_id: Some("worker-a".to_string()),
                region: "us-central1".to_string(),
                nostr_pubkey_hex: pubkey_hex,
                nostr_pubkey_npub: None,
                challenge: "nope".to_string(),
                challenge_signature_hex: signature,
                now_unix_ms: 1_500,
            })
            .expect_err("missing challenge should fail");
        assert!(matches!(
            error,
            super::PresenceError::ChallengeMissing { .. }
        ));
    }

    #[test]
    fn ttl_expiry_marks_online_rows_offline() {
        let mut registry = ProviderPresenceRegistry::default();
        let private_key_hex = fixture_private_key_hex();
        let pubkey_hex = fixture_pubkey_hex(private_key_hex.as_str());

        let challenge = registry
            .issue_nostr_bind_challenge("node-ttl", 2_000)
            .expect("challenge should issue");
        let signature = sign_nostr_presence_challenge(
            private_key_hex.as_str(),
            "node-ttl",
            challenge.challenge.as_str(),
        )
        .expect("signature should build");
        let _ = registry
            .register_online(RegisterOnlineRequest {
                node_id: "node-ttl".to_string(),
                session_id: "sess-ttl".to_string(),
                worker_id: Some("worker-ttl".to_string()),
                region: "us-central1".to_string(),
                nostr_pubkey_hex: pubkey_hex,
                nostr_pubkey_npub: None,
                challenge: challenge.challenge,
                challenge_signature_hex: signature,
                now_unix_ms: 2_200,
            })
            .expect("online registration should pass");

        let policy = registry.policy();
        let expired = registry.sweep_expired(2_200 + policy.stale_after_ms + 1);
        assert_eq!(expired, vec!["node-ttl".to_string()]);

        let row = registry.row("node-ttl").expect("row should remain present");
        assert_eq!(row.status, PresenceStatus::Offline);
        assert_eq!(
            row.offline_reason.as_deref(),
            Some(OFFLINE_REASON_TTL_EXPIRED)
        );
    }

    #[test]
    fn multi_device_same_identity_cardinality_is_stable() {
        let mut registry = ProviderPresenceRegistry::default();
        let private_key_hex = fixture_private_key_hex();
        let pubkey_hex = fixture_pubkey_hex(private_key_hex.as_str());

        for (node, worker, now) in [("node-1", "worker-a", 5_000), ("node-2", "worker-b", 5_100)] {
            let challenge = registry
                .issue_nostr_bind_challenge(node, now)
                .expect("challenge should issue");
            let signature = sign_nostr_presence_challenge(
                private_key_hex.as_str(),
                node,
                challenge.challenge.as_str(),
            )
            .expect("signature should build");
            let _ = registry
                .register_online(RegisterOnlineRequest {
                    node_id: node.to_string(),
                    session_id: format!("sess-{node}"),
                    worker_id: Some(worker.to_string()),
                    region: "us-central1".to_string(),
                    nostr_pubkey_hex: pubkey_hex.clone(),
                    nostr_pubkey_npub: Some("npub1fixture".to_string()),
                    challenge: challenge.challenge,
                    challenge_signature_hex: signature,
                    now_unix_ms: now + 20,
                })
                .expect("online registration should pass");
        }

        assert_eq!(registry.providers_online(PresenceCardinality::Device), 2);
        assert_eq!(registry.providers_online(PresenceCardinality::Worker), 2);
        assert_eq!(registry.providers_online(PresenceCardinality::Identity), 1);
    }
}
