//! Trusted-LAN cluster control-plane substrate for Psionic.

mod layer_sharded;
mod operator_manifest;
mod ordered_state;
mod replicated_serving;
mod scheduler;
mod serving_policy;
mod tensor_sharded;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use psionic_runtime::{ClusterEvidenceBundleVerificationError, SignedClusterEvidenceBundle};
use rand::random;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{
    net::UdpSocket,
    sync::{Mutex, oneshot},
    task::JoinHandle,
    time::{MissedTickBehavior, interval},
};

pub use layer_sharded::*;
pub use operator_manifest::*;
pub use ordered_state::*;
pub use replicated_serving::*;
pub use scheduler::*;
pub use serving_policy::*;
pub use tensor_sharded::*;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "trusted-lan cluster control-plane substrate";

const HELLO_INTERVAL: Duration = Duration::from_millis(100);
const PING_INTERVAL: Duration = Duration::from_millis(75);
const MAX_DATAGRAM_BYTES: usize = 8 * 1024;
const DEFAULT_REPLAY_WINDOW_SIZE: u64 = 64;
const SIGNING_KEY_BYTES: usize = 32;
const VERIFYING_KEY_BYTES: usize = 32;
const SIGNATURE_BYTES: usize = 64;

/// Errors returned by the local cluster transport.
#[derive(Debug, Error)]
pub enum ClusterError {
    /// The local cluster socket could not be bound.
    #[error("failed to bind local cluster socket: {0}")]
    Bind(#[source] std::io::Error),
    /// The local cluster socket address could not be read.
    #[error("failed to read local cluster socket address: {0}")]
    LocalAddr(#[source] std::io::Error),
    /// The background cluster task failed while running.
    #[error("cluster task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
    /// The background cluster task exited with a runtime failure.
    #[error("cluster runtime error: {0}")]
    Runtime(String),
    /// The configured identity policy could not be loaded or stored.
    #[error("failed to read or write local cluster identity: {0}")]
    IdentityIo(#[source] std::io::Error),
    /// The configured identity file contained invalid data.
    #[error("failed to parse local cluster identity: {0}")]
    IdentityFormat(#[source] serde_json::Error),
    /// The configured identity file contained an invalid signing key.
    #[error("failed to decode local cluster signing key: {0}")]
    IdentityKey(String),
    /// The configured operator manifest could not be read or written.
    #[error("failed to read or write cluster operator manifest: {0}")]
    ManifestIo(#[source] std::io::Error),
    /// The configured operator manifest contained invalid data.
    #[error("failed to parse cluster operator manifest: {0}")]
    ManifestFormat(#[source] serde_json::Error),
    /// The configured operator manifest schema version is unsupported.
    #[error(
        "unsupported cluster operator manifest schema version: expected {expected}, found {actual}"
    )]
    ManifestSchemaVersion { expected: u32, actual: u32 },
}

/// Namespace that scopes one trusted local cluster.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClusterNamespace(String);

impl ClusterNamespace {
    /// Creates a cluster namespace from the supplied string.
    #[must_use]
    pub fn new(namespace: impl Into<String>) -> Self {
        Self(namespace.into())
    }

    /// Returns the namespace as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Shared admission token for the first trusted-LAN seam.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionToken(String);

impl AdmissionToken {
    /// Creates an admission token from the supplied string.
    #[must_use]
    pub fn new(admission_token: impl Into<String>) -> Self {
        Self(admission_token.into())
    }

    /// Returns the token as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for AdmissionToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AdmissionToken(<redacted>)")
    }
}

/// Explicit namespace and admission configuration for the local cluster seam.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterAdmissionConfig {
    /// Namespace that scopes the cluster.
    pub namespace: ClusterNamespace,
    /// Shared admission token for the first trusted-LAN seam.
    pub admission_token: AdmissionToken,
}

impl ClusterAdmissionConfig {
    /// Creates explicit namespace and admission configuration.
    #[must_use]
    pub fn new(namespace: impl Into<String>, admission_token: impl Into<String>) -> Self {
        Self {
            namespace: ClusterNamespace::new(namespace),
            admission_token: AdmissionToken::new(admission_token),
        }
    }
}

/// Trust posture for one cluster transport configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterTrustPosture {
    /// Shared-admission trusted-LAN posture used for the first shipped scope.
    TrustedLanSharedAdmission,
    /// Authenticated configured-peer posture suitable for operator-managed wider networks.
    AuthenticatedConfiguredPeers,
}

/// One explicitly configured peer for the authenticated cluster posture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfiguredClusterPeer {
    /// Stable node identity expected at the remote address.
    pub node_id: NodeId,
    /// Explicit socket address for the configured peer.
    pub remote_addr: SocketAddr,
    /// Expected message-signing public key for the peer.
    pub auth_public_key: String,
    /// Previously accepted keys during one explicit rotation overlap.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub previous_auth_public_keys: Vec<String>,
}

impl ConfiguredClusterPeer {
    /// Creates one configured authenticated peer entry.
    #[must_use]
    pub fn new(
        node_id: NodeId,
        remote_addr: SocketAddr,
        auth_public_key: impl Into<String>,
    ) -> Self {
        Self {
            node_id,
            remote_addr,
            auth_public_key: auth_public_key.into(),
            previous_auth_public_keys: Vec::new(),
        }
    }

    /// Allows one explicit previous-key overlap during operator rollout.
    #[must_use]
    pub fn with_previous_auth_public_keys(
        mut self,
        previous_auth_public_keys: Vec<String>,
    ) -> Self {
        self.previous_auth_public_keys = previous_auth_public_keys;
        self
    }

    fn key_match(&self, auth_public_key: &str) -> Option<ConfiguredPeerKeyMatch> {
        if self.auth_public_key == auth_public_key {
            return Some(ConfiguredPeerKeyMatch::Current);
        }
        if self
            .previous_auth_public_keys
            .iter()
            .any(|candidate| candidate == auth_public_key)
        {
            return Some(ConfiguredPeerKeyMatch::Previous);
        }
        None
    }
}

/// Which configured-peer key matched during authentication.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfiguredPeerKeyMatch {
    /// The peer authenticated with the currently expected key.
    Current,
    /// The peer authenticated with an explicitly overlapped previous key.
    Previous,
}

/// Dial and retry policy for configured peers in wider-network clusters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfiguredPeerDialPolicy {
    /// Base hello backoff measured in hello ticks.
    pub base_backoff_ticks: u32,
    /// Maximum hello backoff measured in hello ticks.
    pub max_backoff_ticks: u32,
    /// Unanswered hello attempts after which the peer is considered degraded.
    pub degraded_after_unanswered_hellos: u32,
    /// Unanswered hello attempts after which the peer is considered unreachable.
    pub unreachable_after_unanswered_hellos: u32,
}

impl ConfiguredPeerDialPolicy {
    /// Default dial policy for operator-managed multi-subnet configured peers.
    #[must_use]
    pub const fn operator_managed_default() -> Self {
        Self {
            base_backoff_ticks: 1,
            max_backoff_ticks: 8,
            degraded_after_unanswered_hellos: 2,
            unreachable_after_unanswered_hellos: 4,
        }
    }
}

impl Default for ConfiguredPeerDialPolicy {
    fn default() -> Self {
        Self::operator_managed_default()
    }
}

/// Explicit configured-peer reachability posture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfiguredPeerReachability {
    /// Peer is configured but not yet proven healthy.
    Pending,
    /// Peer has been reached successfully.
    Reachable,
    /// Peer is still configured but repeated unanswered attempts have degraded confidence.
    Degraded,
    /// Peer remains configured but is currently considered unreachable.
    Unreachable,
}

/// Machine-checkable health snapshot for one configured peer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfiguredPeerHealthSnapshot {
    /// Configured peer node ID.
    pub node_id: NodeId,
    /// Configured peer remote address.
    pub remote_addr: SocketAddr,
    /// Effective reachability posture.
    pub reachability: ConfiguredPeerReachability,
    /// Consecutive unanswered hello attempts.
    pub unanswered_hello_attempts: u32,
    /// Remaining hello ticks before the next dial attempt.
    pub remaining_backoff_ticks: u32,
    /// Count of successful hello/ping handshakes observed.
    pub successful_handshakes: u32,
}

impl ConfiguredPeerHealthSnapshot {
    fn new(peer: &ConfiguredClusterPeer) -> Self {
        Self {
            node_id: peer.node_id.clone(),
            remote_addr: peer.remote_addr,
            reachability: ConfiguredPeerReachability::Pending,
            unanswered_hello_attempts: 0,
            remaining_backoff_ticks: 0,
            successful_handshakes: 0,
        }
    }
}

/// Rollout disposition observed while authenticating one configured peer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterTrustRolloutDisposition {
    /// Peer was accepted under an explicit overlap instead of the current bundle.
    AcceptedOverlap,
    /// Peer was refused because its trust bundle version is not currently accepted.
    RefusedVersionMismatch,
}

/// Machine-checkable rollout diagnostic for configured-peer authentication.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterTrustRolloutDiagnostic {
    /// Remote node participating in rollout.
    pub remote_node_id: NodeId,
    /// Remote socket address used for transport.
    pub remote_addr: SocketAddr,
    /// Current local trust-bundle version.
    pub expected_trust_bundle_version: u64,
    /// Remote trust-bundle version, when one was observed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_trust_bundle_version: Option<u64>,
    /// Which key matched during authentication, when one matched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_match: Option<ConfiguredPeerKeyMatch>,
    /// Rollout disposition associated with this observation.
    pub disposition: ClusterTrustRolloutDisposition,
}

/// Machine-checkable trust policy for one local cluster transport.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterTrustPolicy {
    /// Trust posture active for this configuration.
    pub posture: ClusterTrustPosture,
    /// Whether wire messages must carry verifiable signatures.
    pub require_message_authentication: bool,
    /// Sliding replay window size per authenticated peer.
    pub replay_window_size: u64,
    /// Current trust-bundle version for this cluster config.
    pub trust_bundle_version: u64,
    /// Additional trust-bundle versions accepted during explicit rollout overlap.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accepted_trust_bundle_versions: Vec<u64>,
    /// Explicit authenticated peers when configured-peer posture is active.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub configured_peers: Vec<ConfiguredClusterPeer>,
    /// Dial and retry policy for configured peers.
    #[serde(default)]
    pub configured_peer_dial_policy: ConfiguredPeerDialPolicy,
}

impl ClusterTrustPolicy {
    /// Trust policy for the first trusted-LAN shipped scope.
    #[must_use]
    pub const fn trusted_lan() -> Self {
        Self {
            posture: ClusterTrustPosture::TrustedLanSharedAdmission,
            require_message_authentication: false,
            replay_window_size: 0,
            trust_bundle_version: 1,
            accepted_trust_bundle_versions: Vec::new(),
            configured_peers: Vec::new(),
            configured_peer_dial_policy: ConfiguredPeerDialPolicy::operator_managed_default(),
        }
    }

    /// Trust policy for authenticated configured peers across wider networks.
    #[must_use]
    pub fn authenticated_configured_peers(configured_peers: Vec<ConfiguredClusterPeer>) -> Self {
        Self {
            posture: ClusterTrustPosture::AuthenticatedConfiguredPeers,
            require_message_authentication: true,
            replay_window_size: DEFAULT_REPLAY_WINDOW_SIZE,
            trust_bundle_version: 1,
            accepted_trust_bundle_versions: Vec::new(),
            configured_peers,
            configured_peer_dial_policy: ConfiguredPeerDialPolicy::operator_managed_default(),
        }
    }

    /// Overrides the current trust-bundle version for this cluster config.
    #[must_use]
    pub fn with_trust_bundle_version(mut self, trust_bundle_version: u64) -> Self {
        self.trust_bundle_version = trust_bundle_version;
        self
    }

    /// Declares additional trust-bundle versions accepted during rollout overlap.
    #[must_use]
    pub fn with_accepted_trust_bundle_versions(
        mut self,
        mut accepted_trust_bundle_versions: Vec<u64>,
    ) -> Self {
        accepted_trust_bundle_versions.sort_unstable();
        accepted_trust_bundle_versions.dedup();
        self.accepted_trust_bundle_versions = accepted_trust_bundle_versions;
        self
    }

    /// Overrides the dial policy for configured peers.
    #[must_use]
    pub fn with_configured_peer_dial_policy(
        mut self,
        configured_peer_dial_policy: ConfiguredPeerDialPolicy,
    ) -> Self {
        self.configured_peer_dial_policy = configured_peer_dial_policy;
        self
    }

    /// Returns a stable digest for the effective trust policy.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"cluster_trust_policy|");
        hasher.update(match self.posture {
            ClusterTrustPosture::TrustedLanSharedAdmission => {
                b"trusted_lan_shared_admission".as_slice()
            }
            ClusterTrustPosture::AuthenticatedConfiguredPeers => {
                b"authenticated_configured_peers".as_slice()
            }
        });
        hasher.update(b"|");
        hasher.update(if self.require_message_authentication {
            b"signed".as_slice()
        } else {
            b"unsigned".as_slice()
        });
        hasher.update(b"|");
        hasher.update(self.replay_window_size.to_string().as_bytes());
        hasher.update(b"|trust_bundle_version|");
        hasher.update(self.trust_bundle_version.to_string().as_bytes());
        for accepted_version in &self.accepted_trust_bundle_versions {
            hasher.update(b"|accepted_version|");
            hasher.update(accepted_version.to_string().as_bytes());
        }
        for peer in &self.configured_peers {
            hasher.update(b"|peer|");
            hasher.update(peer.node_id.as_str().as_bytes());
            hasher.update(b"|");
            hasher.update(peer.remote_addr.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(peer.auth_public_key.as_bytes());
            for previous_key in &peer.previous_auth_public_keys {
                hasher.update(b"|previous_key|");
                hasher.update(previous_key.as_bytes());
            }
        }
        hasher.update(b"|dial_policy|");
        hasher.update(
            self.configured_peer_dial_policy
                .base_backoff_ticks
                .to_string()
                .as_bytes(),
        );
        hasher.update(b"|");
        hasher.update(
            self.configured_peer_dial_policy
                .max_backoff_ticks
                .to_string()
                .as_bytes(),
        );
        hasher.update(b"|");
        hasher.update(
            self.configured_peer_dial_policy
                .degraded_after_unanswered_hellos
                .to_string()
                .as_bytes(),
        );
        hasher.update(b"|");
        hasher.update(
            self.configured_peer_dial_policy
                .unreachable_after_unanswered_hellos
                .to_string()
                .as_bytes(),
        );
        hex::encode(hasher.finalize())
    }

    /// Derives the current compute-market trust posture from the shipped cluster policy.
    #[must_use]
    pub fn compute_market_trust_assessment(&self) -> ClusterComputeMarketTrustAssessment {
        let mut refusal_reasons = vec![
            ClusterComputeMarketTrustRefusalReason::MissingSignedEvidenceBundleExport,
            ClusterComputeMarketTrustRefusalReason::MissingAttestedNodeIdentityAdmission,
            ClusterComputeMarketTrustRefusalReason::MissingNonLanDiscoveryPosture,
        ];
        match self.posture {
            ClusterTrustPosture::TrustedLanSharedAdmission => {
                refusal_reasons.insert(
                    0,
                    ClusterComputeMarketTrustRefusalReason::TrustedLanSharedAdmissionOnly,
                );
            }
            ClusterTrustPosture::AuthenticatedConfiguredPeers => {
                refusal_reasons.insert(
                    0,
                    ClusterComputeMarketTrustRefusalReason::OperatorManagedConfiguredPeersOnly,
                );
            }
        }
        if !self.require_message_authentication {
            refusal_reasons.insert(
                1,
                ClusterComputeMarketTrustRefusalReason::MissingAuthenticatedTransport,
            );
        }
        ClusterComputeMarketTrustAssessment {
            posture: self.posture,
            trust_policy_digest: self.stable_digest(),
            disposition: if refusal_reasons.is_empty() {
                ClusterComputeMarketTrustDisposition::Eligible
            } else {
                ClusterComputeMarketTrustDisposition::Refused
            },
            refusal_reasons,
        }
    }

    fn configured_peer(&self, node_id: &NodeId) -> Option<&ConfiguredClusterPeer> {
        self.configured_peers
            .iter()
            .find(|peer| peer.node_id == *node_id)
    }

    fn accepts_trust_bundle_version(&self, trust_bundle_version: u64) -> bool {
        self.trust_bundle_version == trust_bundle_version
            || self
                .accepted_trust_bundle_versions
                .contains(&trust_bundle_version)
    }
}

impl Default for ClusterTrustPolicy {
    fn default() -> Self {
        Self::trusted_lan()
    }
}

/// Current compute-market trust disposition derived from cluster policy truth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterComputeMarketTrustDisposition {
    /// The current cluster trust policy is not sufficient for compute-market claims.
    Refused,
    /// The current cluster trust policy is sufficient for compute-market claims.
    Eligible,
}

/// Explicit refusal reasons for compute-market trust claims.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterComputeMarketTrustRefusalReason {
    /// Shared admission on a trusted LAN is not a market-safe posture.
    TrustedLanSharedAdmissionOnly,
    /// Wider-network transport still lacks mandatory authenticated messaging.
    MissingAuthenticatedTransport,
    /// Configured peers remain operator-managed allowlist entries instead of market admission.
    OperatorManagedConfiguredPeersOnly,
    /// Cluster evidence is not yet exported as one signed immutable bundle.
    MissingSignedEvidenceBundleExport,
    /// Node admission is not yet backed by attested node identity.
    MissingAttestedNodeIdentityAdmission,
    /// Discovery posture is not yet explicit for wider non-LAN environments.
    MissingNonLanDiscoveryPosture,
}

/// Machine-checkable assessment for whether a cluster trust policy supports compute-market claims.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterComputeMarketTrustAssessment {
    /// Trust posture active for the underlying cluster policy.
    pub posture: ClusterTrustPosture,
    /// Stable digest of the underlying trust policy.
    pub trust_policy_digest: String,
    /// Effective compute-market disposition for the current policy.
    pub disposition: ClusterComputeMarketTrustDisposition,
    /// Explicit reasons why wider compute-market claims remain refused.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refusal_reasons: Vec<ClusterComputeMarketTrustRefusalReason>,
}

impl ClusterComputeMarketTrustAssessment {
    /// Returns a stable digest for the current compute-market trust assessment.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"cluster_compute_market_trust_assessment|");
        hasher.update(match self.posture {
            ClusterTrustPosture::TrustedLanSharedAdmission => {
                b"trusted_lan_shared_admission".as_slice()
            }
            ClusterTrustPosture::AuthenticatedConfiguredPeers => {
                b"authenticated_configured_peers".as_slice()
            }
        });
        hasher.update(b"|");
        hasher.update(self.trust_policy_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(match self.disposition {
            ClusterComputeMarketTrustDisposition::Refused => b"refused".as_slice(),
            ClusterComputeMarketTrustDisposition::Eligible => b"eligible".as_slice(),
        });
        for refusal_reason in &self.refusal_reasons {
            hasher.update(b"|refusal|");
            hasher.update(match refusal_reason {
                ClusterComputeMarketTrustRefusalReason::TrustedLanSharedAdmissionOnly => {
                    b"trusted_lan_shared_admission_only".as_slice()
                }
                ClusterComputeMarketTrustRefusalReason::MissingAuthenticatedTransport => {
                    b"missing_authenticated_transport".as_slice()
                }
                ClusterComputeMarketTrustRefusalReason::OperatorManagedConfiguredPeersOnly => {
                    b"operator_managed_configured_peers_only".as_slice()
                }
                ClusterComputeMarketTrustRefusalReason::MissingSignedEvidenceBundleExport => {
                    b"missing_signed_evidence_bundle_export".as_slice()
                }
                ClusterComputeMarketTrustRefusalReason::MissingAttestedNodeIdentityAdmission => {
                    b"missing_attested_node_identity_admission".as_slice()
                }
                ClusterComputeMarketTrustRefusalReason::MissingNonLanDiscoveryPosture => {
                    b"missing_non_lan_discovery_posture".as_slice()
                }
            });
        }
        hex::encode(hasher.finalize())
    }
}

/// Node role visible to the local cluster transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeRole {
    /// Participates only in cluster control.
    CoordinatorOnly,
    /// Participates only in execution.
    ExecutorOnly,
    /// Participates in both control and execution.
    Mixed,
}

/// Stable cluster identity for one trusted local namespace/admission pair.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClusterId(String);

impl ClusterId {
    #[must_use]
    pub fn new(namespace: &ClusterNamespace, admission_token: &AdmissionToken) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(namespace.as_str().as_bytes());
        hasher.update([0]);
        hasher.update(admission_token.as_str().as_bytes());
        Self(hex::encode(hasher.finalize()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable node identity for one local cluster participant.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId(String);

impl NodeId {
    #[must_use]
    pub fn new(node_id: impl Into<String>) -> Self {
        Self(node_id.into())
    }

    #[must_use]
    pub fn random() -> Self {
        let raw: [u8; 16] = random();
        Self(hex::encode(raw))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Monotonic node incarnation counter used to distinguish restarts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeEpoch(u64);

impl NodeEpoch {
    /// Initial node epoch for a never-before-seen node identity.
    #[must_use]
    pub const fn initial() -> Self {
        Self(1)
    }

    /// Next node epoch after one already-persisted incarnation.
    #[must_use]
    pub const fn next(previous: Self) -> Self {
        Self(previous.0.saturating_add(1))
    }

    /// Returns the raw epoch value.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// Identity persistence policy for the first local cluster seam.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeIdentityPersistence {
    /// Generate a fresh node identity on every start.
    Ephemeral,
    /// Persist node identity and epoch state in one local JSON file.
    FileBacked { path: PathBuf },
}

/// Cluster/node identity facts surfaced by the transport.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterNodeIdentity {
    /// Shared cluster identity.
    pub cluster_id: ClusterId,
    /// Stable local node identity.
    pub node_id: NodeId,
    /// Monotonic node epoch for this running instance.
    pub node_epoch: NodeEpoch,
    /// Declared node role.
    pub role: NodeRole,
    /// Message-signing public key advertised by this node.
    pub auth_public_key: String,
}

impl ClusterNodeIdentity {
    /// Verifies that a signed cluster evidence bundle was exported by this node identity.
    pub fn verify_signed_evidence_bundle(
        &self,
        bundle: &SignedClusterEvidenceBundle,
    ) -> Result<(), ClusterEvidenceBundleIdentityError> {
        if bundle.signature.signer_node_id != self.node_id.as_str() {
            return Err(ClusterEvidenceBundleIdentityError::SignerNodeMismatch {
                expected: self.node_id.clone(),
                actual: bundle.signature.signer_node_id.clone(),
            });
        }
        if bundle.signature.signer_public_key != self.auth_public_key {
            return Err(ClusterEvidenceBundleIdentityError::SignerKeyMismatch {
                expected: self.auth_public_key.clone(),
                actual: bundle.signature.signer_public_key.clone(),
            });
        }
        bundle
            .verify()
            .map_err(ClusterEvidenceBundleIdentityError::Verification)
    }
}

/// Verification failure while binding one signed evidence bundle to a cluster node identity.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ClusterEvidenceBundleIdentityError {
    /// The bundle was signed by a different node ID than the current identity.
    #[error("cluster evidence bundle signer node mismatch: expected {expected:?}, found {actual}")]
    SignerNodeMismatch {
        /// Node ID surfaced by the current cluster identity.
        expected: NodeId,
        /// Signer node ID carried by the bundle.
        actual: String,
    },
    /// The bundle was signed by a different public key than the current identity.
    #[error("cluster evidence bundle signer key mismatch: expected {expected}, found {actual}")]
    SignerKeyMismatch {
        /// Public key surfaced by the current cluster identity.
        expected: String,
        /// Public key carried by the bundle.
        actual: String,
    },
    /// The bundle signature or digest was invalid.
    #[error(transparent)]
    Verification(#[from] ClusterEvidenceBundleVerificationError),
}

/// Minimal local-cluster transport configuration for the first trusted-LAN seam.
#[derive(Clone, Debug)]
pub struct LocalClusterConfig {
    /// Explicit namespace and admission configuration.
    pub admission: ClusterAdmissionConfig,
    /// Local socket address to bind.
    pub bind_addr: SocketAddr,
    /// Explicit seed peers for the first discovery mode.
    pub seed_peers: Vec<SocketAddr>,
    /// Declared role for this node.
    pub role: NodeRole,
    /// Identity persistence policy for this node.
    pub identity_persistence: NodeIdentityPersistence,
    /// Machine-checkable trust policy for this node's transport.
    pub trust_policy: ClusterTrustPolicy,
}

impl LocalClusterConfig {
    /// Creates a minimal local-cluster configuration.
    #[must_use]
    pub fn new(
        namespace: impl Into<String>,
        admission_token: impl Into<String>,
        bind_addr: SocketAddr,
        role: NodeRole,
    ) -> Self {
        Self {
            admission: ClusterAdmissionConfig::new(namespace, admission_token),
            bind_addr,
            seed_peers: Vec::new(),
            role,
            identity_persistence: NodeIdentityPersistence::Ephemeral,
            trust_policy: ClusterTrustPolicy::trusted_lan(),
        }
    }

    /// Attaches explicit seed peers for the first discovery mode.
    #[must_use]
    pub fn with_seed_peers(mut self, seed_peers: Vec<SocketAddr>) -> Self {
        self.seed_peers = seed_peers;
        self
    }

    /// Attaches a file-backed identity policy for stable restart identity.
    #[must_use]
    pub fn with_file_backed_identity(mut self, path: PathBuf) -> Self {
        self.identity_persistence = NodeIdentityPersistence::FileBacked { path };
        self
    }

    /// Attaches an explicit trust policy.
    #[must_use]
    pub fn with_trust_policy(mut self, trust_policy: ClusterTrustPolicy) -> Self {
        self.trust_policy = trust_policy;
        self
    }

    /// Attaches authenticated configured peers and seeds discovery from them.
    #[must_use]
    pub fn with_authenticated_configured_peers(
        mut self,
        configured_peers: Vec<ConfiguredClusterPeer>,
    ) -> Self {
        self.seed_peers = configured_peers
            .iter()
            .map(|peer| peer.remote_addr)
            .collect();
        self.trust_policy = ClusterTrustPolicy::authenticated_configured_peers(configured_peers);
        self
    }

    /// Overrides the configured-peer dial policy used for authenticated clusters.
    #[must_use]
    pub fn with_configured_peer_dial_policy(
        mut self,
        configured_peer_dial_policy: ConfiguredPeerDialPolicy,
    ) -> Self {
        self.trust_policy = self
            .trust_policy
            .clone()
            .with_configured_peer_dial_policy(configured_peer_dial_policy);
        self
    }

    /// Builds local cluster config from one persisted operator manifest.
    #[must_use]
    pub fn from_operator_manifest(manifest: ClusterOperatorManifest) -> Self {
        manifest.into()
    }

    /// Loads local cluster config from one persisted operator manifest file.
    pub fn load_operator_manifest(path: impl AsRef<std::path::Path>) -> Result<Self, ClusterError> {
        ClusterOperatorManifest::load_json(path).map(Into::into)
    }
}

/// Machine-checkable cluster-join refusal reason for the first local seam.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClusterJoinRefusalReason {
    /// The remote namespace did not match this node's namespace.
    NamespaceMismatch {
        /// Expected namespace.
        expected: ClusterNamespace,
        /// Observed namespace.
        actual: ClusterNamespace,
    },
    /// The remote admission token digest did not match.
    AdmissionMismatch,
    /// The remote cluster identity did not match the local cluster.
    ClusterIdMismatch {
        /// Expected local cluster identity.
        expected: ClusterId,
        /// Observed remote cluster identity.
        actual: ClusterId,
    },
    /// The remote node reused an older node epoch than the current known epoch.
    StaleNodeEpoch {
        /// Highest epoch already observed for this node ID.
        current: NodeEpoch,
        /// Epoch attempted by the stale peer.
        attempted: NodeEpoch,
    },
    /// The remote node is not part of the configured authenticated peer set.
    ConfiguredPeerUnknown,
    /// The remote peer address does not match the configured authenticated peer entry.
    ConfiguredPeerAddressMismatch {
        /// Expected configured peer address.
        expected: SocketAddr,
        /// Observed source address.
        actual: SocketAddr,
    },
    /// The remote peer key does not match the configured authenticated peer entry.
    ConfiguredPeerKeyMismatch {
        /// Expected configured peer key.
        expected: String,
        /// Observed peer key.
        actual: String,
    },
    /// The remote message lacked a valid signature or authentication payload.
    MessageAuthenticationFailed,
    /// The remote peer used an unexpected trust-bundle version.
    TrustBundleVersionMismatch {
        /// Current local trust-bundle version.
        expected: u64,
        /// Observed remote trust-bundle version, when present.
        actual: Option<u64>,
        /// Additional bundle versions explicitly accepted during rollout.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        accepted: Vec<u64>,
    },
    /// The remote message replayed an already-observed or expired authenticated counter.
    ReplayDetected {
        /// Highest authenticated counter already observed for this peer.
        highest_seen: u64,
        /// Counter attempted by the replayed message.
        attempted: u64,
    },
}

/// One refused cluster-join observation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterJoinRefusal {
    /// Remote socket address that attempted the join.
    pub remote_addr: SocketAddr,
    /// Remote node identity, when one could be parsed.
    pub remote_node_id: Option<NodeId>,
    /// Remote cluster identity, when one could be parsed.
    pub remote_cluster_id: Option<ClusterId>,
    /// Remote node epoch, when one could be parsed.
    pub remote_node_epoch: Option<NodeEpoch>,
    /// Machine-checkable refusal reason.
    pub reason: ClusterJoinRefusalReason,
}

/// Peer handshake observations surfaced by the first transport path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerHandshakeState {
    /// Whether a typed hello was observed from this peer.
    pub saw_hello: bool,
    /// Highest ping sequence observed from this peer.
    pub last_ping_sequence: Option<u64>,
}

/// Snapshot of one discovered peer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerSnapshot {
    /// Remote socket address used for the current local-cluster transport.
    pub remote_addr: SocketAddr,
    /// Cluster/node identity surfaced by the peer.
    pub identity: ClusterNodeIdentity,
    /// Hello/ping handshake facts observed so far.
    pub handshake: PeerHandshakeState,
}

/// Running local-cluster node for the first hello/ping seam.
pub struct LocalClusterNode {
    local_addr: SocketAddr,
    local_identity: ClusterNodeIdentity,
    trust_policy: ClusterTrustPolicy,
    state: Arc<Mutex<SharedState>>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), String>>>,
}

impl LocalClusterNode {
    /// Starts the first local-cluster hello/ping transport.
    pub async fn spawn(config: LocalClusterConfig) -> Result<Self, ClusterError> {
        let loaded_identity = load_or_create_local_identity(&config)?;
        let local_identity = loaded_identity.identity.clone();
        let transport_config = TransportConfig::from_config(config, loaded_identity);
        let trust_policy = transport_config.trust_policy.clone();
        let socket = Arc::new(
            UdpSocket::bind(transport_config.bind_addr)
                .await
                .map_err(ClusterError::Bind)?,
        );
        let local_addr = socket.local_addr().map_err(ClusterError::LocalAddr)?;
        let state = Arc::new(Mutex::new(SharedState::new(
            transport_config.seed_peers.clone(),
            &transport_config.trust_policy,
        )));
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(run_transport(
            socket,
            state.clone(),
            transport_config,
            shutdown_rx,
        ));
        Ok(Self {
            local_addr,
            local_identity,
            trust_policy,
            state,
            shutdown_tx: Some(shutdown_tx),
            task: Some(task),
        })
    }

    /// Returns the bound local transport address.
    #[must_use]
    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Returns the local cluster/node identity.
    #[must_use]
    pub fn local_identity(&self) -> &ClusterNodeIdentity {
        &self.local_identity
    }

    /// Returns the machine-checkable trust policy for this node.
    #[must_use]
    pub fn trust_policy(&self) -> &ClusterTrustPolicy {
        &self.trust_policy
    }

    /// Returns the currently discovered peers.
    pub async fn peer_snapshots(&self) -> Vec<PeerSnapshot> {
        self.state.lock().await.peer_snapshots()
    }

    /// Returns health snapshots for explicitly configured peers.
    pub async fn configured_peer_health_snapshots(&self) -> Vec<ConfiguredPeerHealthSnapshot> {
        self.state.lock().await.configured_peer_health_snapshots()
    }

    /// Returns machine-checkable join refusals observed by this node.
    pub async fn join_refusals(&self) -> Vec<ClusterJoinRefusal> {
        self.state.lock().await.join_refusals()
    }

    /// Returns machine-checkable trust-rollout diagnostics observed by this node.
    pub async fn trust_rollout_diagnostics(&self) -> Vec<ClusterTrustRolloutDiagnostic> {
        self.state.lock().await.trust_rollout_diagnostics()
    }

    /// Shuts the local-cluster node down and waits for the background task.
    pub async fn shutdown(mut self) -> Result<(), ClusterError> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        if let Some(task) = self.task.take() {
            let outcome = task.await?;
            return outcome.map_err(ClusterError::Runtime);
        }
        Ok(())
    }
}

impl Drop for LocalClusterNode {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

#[derive(Clone)]
struct TransportConfig {
    namespace: ClusterNamespace,
    admission_digest: String,
    bind_addr: SocketAddr,
    seed_peers: BTreeSet<SocketAddr>,
    local_identity: ClusterNodeIdentity,
    local_signing_key: SigningKey,
    trust_policy: ClusterTrustPolicy,
}

impl TransportConfig {
    fn from_config(config: LocalClusterConfig, local_identity: LoadedLocalIdentity) -> Self {
        Self {
            namespace: config.admission.namespace,
            admission_digest: admission_digest(&config.admission.admission_token),
            bind_addr: config.bind_addr,
            seed_peers: config.seed_peers.into_iter().collect(),
            local_identity: local_identity.identity,
            local_signing_key: local_identity.signing_key,
            trust_policy: config.trust_policy,
        }
    }
}

#[derive(Default)]
struct SharedState {
    peers: BTreeMap<NodeId, PeerSnapshot>,
    configured_peer_health: BTreeMap<NodeId, ConfiguredPeerHealthSnapshot>,
    trust_rollout_diagnostics: BTreeMap<NodeId, ClusterTrustRolloutDiagnostic>,
    peer_replay_windows: BTreeMap<NodeId, PeerReplayWindow>,
    join_refusals: Vec<ClusterJoinRefusal>,
    seed_peers: BTreeSet<SocketAddr>,
    next_ping_sequence: u64,
    next_authenticated_message_counter: u64,
}

impl SharedState {
    fn new(seed_peers: BTreeSet<SocketAddr>, trust_policy: &ClusterTrustPolicy) -> Self {
        let configured_peer_health = trust_policy
            .configured_peers
            .iter()
            .map(|peer| {
                (
                    peer.node_id.clone(),
                    ConfiguredPeerHealthSnapshot::new(peer),
                )
            })
            .collect();
        Self {
            peers: BTreeMap::new(),
            configured_peer_health,
            trust_rollout_diagnostics: BTreeMap::new(),
            peer_replay_windows: BTreeMap::new(),
            join_refusals: Vec::new(),
            seed_peers,
            next_ping_sequence: 0,
            next_authenticated_message_counter: 1,
        }
    }

    fn peer_snapshots(&self) -> Vec<PeerSnapshot> {
        self.peers.values().cloned().collect()
    }

    fn configured_peer_health_snapshots(&self) -> Vec<ConfiguredPeerHealthSnapshot> {
        self.configured_peer_health.values().cloned().collect()
    }

    fn join_refusals(&self) -> Vec<ClusterJoinRefusal> {
        self.join_refusals.clone()
    }

    fn trust_rollout_diagnostics(&self) -> Vec<ClusterTrustRolloutDiagnostic> {
        self.trust_rollout_diagnostics.values().cloned().collect()
    }

    fn push_join_refusal(&mut self, refusal: ClusterJoinRefusal) {
        self.join_refusals.push(refusal);
    }

    fn push_trust_rollout_diagnostic(&mut self, diagnostic: ClusterTrustRolloutDiagnostic) {
        self.trust_rollout_diagnostics
            .insert(diagnostic.remote_node_id.clone(), diagnostic);
    }

    fn next_ping_sequence(&mut self) -> u64 {
        let sequence = self.next_ping_sequence;
        self.next_ping_sequence = self.next_ping_sequence.saturating_add(1);
        sequence
    }

    fn next_authenticated_message_counter(&mut self) -> u64 {
        let counter = self.next_authenticated_message_counter;
        self.next_authenticated_message_counter =
            self.next_authenticated_message_counter.saturating_add(1);
        counter
    }

    fn undiscovered_seed_peers(&self) -> Vec<SocketAddr> {
        self.seed_peers
            .iter()
            .copied()
            .filter(|addr| self.peers.values().all(|peer| peer.remote_addr != *addr))
            .collect()
    }

    fn configured_peers_due_for_dial(
        &mut self,
        trust_policy: &ClusterTrustPolicy,
    ) -> Vec<SocketAddr> {
        let mut due_peers = Vec::new();
        for peer in &trust_policy.configured_peers {
            if self.peers.contains_key(&peer.node_id) {
                self.mark_configured_peer_reachable(&peer.node_id);
                continue;
            }
            let Some(health) = self.configured_peer_health.get_mut(&peer.node_id) else {
                continue;
            };
            if health.remaining_backoff_ticks > 0 {
                health.remaining_backoff_ticks -= 1;
                continue;
            }
            health.unanswered_hello_attempts = health.unanswered_hello_attempts.saturating_add(1);
            health.reachability = classify_configured_peer_reachability(
                health.unanswered_hello_attempts,
                trust_policy.configured_peer_dial_policy,
            );
            health.remaining_backoff_ticks = next_configured_peer_backoff_ticks(
                health.unanswered_hello_attempts,
                trust_policy.configured_peer_dial_policy,
            );
            due_peers.push(peer.remote_addr);
        }
        due_peers
    }

    fn discovered_peer_addrs(&self) -> Vec<SocketAddr> {
        self.peers.values().map(|peer| peer.remote_addr).collect()
    }

    fn record_hello(
        &mut self,
        remote_addr: SocketAddr,
        identity: ClusterNodeIdentity,
        authenticated_counter: Option<u64>,
        replay_window_size: u64,
    ) -> Result<bool, Box<ClusterJoinRefusal>> {
        let outcome = self.validate_peer_epoch(remote_addr, &identity)?;
        if let Some(counter) = authenticated_counter {
            self.record_authenticated_counter(remote_addr, &identity, counter, replay_window_size)?;
        }
        self.mark_configured_peer_reachable(&identity.node_id);
        let snapshot = self.ensure_peer_snapshot(remote_addr, identity);
        snapshot.handshake.saw_hello = true;
        Ok(outcome.should_reply_hello)
    }

    fn record_ping(
        &mut self,
        remote_addr: SocketAddr,
        identity: ClusterNodeIdentity,
        sequence: u64,
        authenticated_counter: Option<u64>,
        replay_window_size: u64,
    ) -> Result<(), Box<ClusterJoinRefusal>> {
        let _ = self.validate_peer_epoch(remote_addr, &identity)?;
        if let Some(counter) = authenticated_counter {
            self.record_authenticated_counter(remote_addr, &identity, counter, replay_window_size)?;
        }
        self.mark_configured_peer_reachable(&identity.node_id);
        let snapshot = self.ensure_peer_snapshot(remote_addr, identity);
        snapshot.handshake.last_ping_sequence = Some(sequence);
        Ok(())
    }

    fn ensure_peer_snapshot(
        &mut self,
        remote_addr: SocketAddr,
        identity: ClusterNodeIdentity,
    ) -> &mut PeerSnapshot {
        let entry = self
            .peers
            .entry(identity.node_id.clone())
            .or_insert_with(|| PeerSnapshot {
                remote_addr,
                identity: identity.clone(),
                handshake: PeerHandshakeState {
                    saw_hello: false,
                    last_ping_sequence: None,
                },
            });
        if identity.node_epoch > entry.identity.node_epoch {
            entry.handshake = PeerHandshakeState {
                saw_hello: false,
                last_ping_sequence: None,
            };
        }
        entry.remote_addr = remote_addr;
        entry.identity = identity;
        entry
    }

    fn mark_configured_peer_reachable(&mut self, node_id: &NodeId) {
        if let Some(health) = self.configured_peer_health.get_mut(node_id) {
            let was_reachable =
                matches!(health.reachability, ConfiguredPeerReachability::Reachable);
            health.reachability = ConfiguredPeerReachability::Reachable;
            health.unanswered_hello_attempts = 0;
            health.remaining_backoff_ticks = 0;
            if !was_reachable {
                health.successful_handshakes = health.successful_handshakes.saturating_add(1);
            }
        }
    }

    fn validate_peer_epoch(
        &self,
        remote_addr: SocketAddr,
        identity: &ClusterNodeIdentity,
    ) -> Result<PeerEpochOutcome, Box<ClusterJoinRefusal>> {
        if let Some(existing) = self.peers.get(&identity.node_id) {
            if identity.node_epoch < existing.identity.node_epoch {
                return Err(Box::new(ClusterJoinRefusal {
                    remote_addr,
                    remote_node_id: Some(identity.node_id.clone()),
                    remote_cluster_id: Some(identity.cluster_id.clone()),
                    remote_node_epoch: Some(identity.node_epoch),
                    reason: ClusterJoinRefusalReason::StaleNodeEpoch {
                        current: existing.identity.node_epoch,
                        attempted: identity.node_epoch,
                    },
                }));
            }
            let should_reply_hello =
                identity.node_epoch > existing.identity.node_epoch || !existing.handshake.saw_hello;
            return Ok(PeerEpochOutcome { should_reply_hello });
        }
        Ok(PeerEpochOutcome {
            should_reply_hello: true,
        })
    }

    fn record_authenticated_counter(
        &mut self,
        remote_addr: SocketAddr,
        identity: &ClusterNodeIdentity,
        counter: u64,
        replay_window_size: u64,
    ) -> Result<(), Box<ClusterJoinRefusal>> {
        let replay_window = self
            .peer_replay_windows
            .entry(identity.node_id.clone())
            .or_insert_with(|| PeerReplayWindow::new(identity.node_epoch));
        if identity.node_epoch > replay_window.node_epoch {
            *replay_window = PeerReplayWindow::new(identity.node_epoch);
        }
        replay_window
            .record(counter, replay_window_size)
            .map_err(|reason| {
                Box::new(ClusterJoinRefusal {
                    remote_addr,
                    remote_node_id: Some(identity.node_id.clone()),
                    remote_cluster_id: Some(identity.cluster_id.clone()),
                    remote_node_epoch: Some(identity.node_epoch),
                    reason,
                })
            })
    }
}

struct PeerEpochOutcome {
    should_reply_hello: bool,
}

struct PeerReplayWindow {
    node_epoch: NodeEpoch,
    highest_seen: u64,
    accepted_counters: BTreeSet<u64>,
}

impl PeerReplayWindow {
    fn new(node_epoch: NodeEpoch) -> Self {
        Self {
            node_epoch,
            highest_seen: 0,
            accepted_counters: BTreeSet::new(),
        }
    }

    fn record(
        &mut self,
        counter: u64,
        replay_window_size: u64,
    ) -> Result<(), ClusterJoinRefusalReason> {
        if replay_window_size == 0 {
            return Ok(());
        }
        let minimum_allowed = self.highest_seen.saturating_sub(replay_window_size);
        if counter <= minimum_allowed || self.accepted_counters.contains(&counter) {
            return Err(ClusterJoinRefusalReason::ReplayDetected {
                highest_seen: self.highest_seen,
                attempted: counter,
            });
        }
        self.accepted_counters.insert(counter);
        self.highest_seen = self.highest_seen.max(counter);
        while self.accepted_counters.len() as u64 > replay_window_size {
            let _ = self.accepted_counters.pop_first();
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct HelloMessage {
    sender: ClusterNodeIdentity,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PingMessage {
    sender: ClusterNodeIdentity,
    sequence: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WireMessage {
    Hello(HelloMessage),
    Ping(PingMessage),
}

impl WireMessage {
    fn sender(&self) -> &ClusterNodeIdentity {
        match self {
            Self::Hello(message) => &message.sender,
            Self::Ping(message) => &message.sender,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WireEnvelope {
    namespace: ClusterNamespace,
    admission_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    trust_bundle_version: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    authenticated_counter: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signature_hex: Option<String>,
    message: WireMessage,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedNodeIdentityRecord {
    cluster_id: ClusterId,
    node_id: NodeId,
    last_epoch: NodeEpoch,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    auth_secret_key_hex: Option<String>,
}

struct LoadedLocalIdentity {
    identity: ClusterNodeIdentity,
    signing_key: SigningKey,
}

fn load_or_create_local_identity(
    config: &LocalClusterConfig,
) -> Result<LoadedLocalIdentity, ClusterError> {
    let cluster_id = ClusterId::new(
        &config.admission.namespace,
        &config.admission.admission_token,
    );
    match &config.identity_persistence {
        NodeIdentityPersistence::Ephemeral => {
            let signing_key = SigningKey::from_bytes(&random::<[u8; SIGNING_KEY_BYTES]>());
            Ok(LoadedLocalIdentity {
                identity: ClusterNodeIdentity {
                    cluster_id,
                    node_id: NodeId::random(),
                    node_epoch: NodeEpoch::initial(),
                    role: config.role,
                    auth_public_key: encode_auth_public_key(&signing_key.verifying_key()),
                },
                signing_key,
            })
        }
        NodeIdentityPersistence::FileBacked { path } => {
            let mut node_id = NodeId::random();
            let mut node_epoch = NodeEpoch::initial();
            let mut signing_key = SigningKey::from_bytes(&random::<[u8; SIGNING_KEY_BYTES]>());
            if path.exists() {
                let bytes = fs::read(path).map_err(ClusterError::IdentityIo)?;
                let record: PersistedNodeIdentityRecord =
                    serde_json::from_slice(&bytes).map_err(ClusterError::IdentityFormat)?;
                if record.cluster_id == cluster_id {
                    node_id = record.node_id;
                    node_epoch = NodeEpoch::next(record.last_epoch);
                    if let Some(auth_secret_key_hex) = record.auth_secret_key_hex {
                        signing_key = decode_signing_key(&auth_secret_key_hex)?;
                    }
                }
            } else if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(ClusterError::IdentityIo)?;
            }
            let record = PersistedNodeIdentityRecord {
                cluster_id: cluster_id.clone(),
                node_id: node_id.clone(),
                last_epoch: node_epoch,
                auth_secret_key_hex: Some(hex::encode(signing_key.to_bytes())),
            };
            let encoded =
                serde_json::to_vec_pretty(&record).map_err(ClusterError::IdentityFormat)?;
            fs::write(path, encoded).map_err(ClusterError::IdentityIo)?;
            Ok(LoadedLocalIdentity {
                identity: ClusterNodeIdentity {
                    cluster_id,
                    node_id,
                    node_epoch,
                    role: config.role,
                    auth_public_key: encode_auth_public_key(&signing_key.verifying_key()),
                },
                signing_key,
            })
        }
    }
}

fn decode_signing_key(auth_secret_key_hex: &str) -> Result<SigningKey, ClusterError> {
    let bytes = hex::decode(auth_secret_key_hex)
        .map_err(|error| ClusterError::IdentityKey(error.to_string()))?;
    let secret_key_bytes: [u8; SIGNING_KEY_BYTES] = bytes
        .try_into()
        .map_err(|_| ClusterError::IdentityKey(String::from("invalid signing key length")))?;
    Ok(SigningKey::from_bytes(&secret_key_bytes))
}

fn decode_verifying_key(auth_public_key: &str) -> Result<VerifyingKey, ClusterJoinRefusalReason> {
    let bytes = hex::decode(auth_public_key)
        .map_err(|_| ClusterJoinRefusalReason::MessageAuthenticationFailed)?;
    let verifying_key_bytes: [u8; VERIFYING_KEY_BYTES] = bytes
        .try_into()
        .map_err(|_| ClusterJoinRefusalReason::MessageAuthenticationFailed)?;
    VerifyingKey::from_bytes(&verifying_key_bytes)
        .map_err(|_| ClusterJoinRefusalReason::MessageAuthenticationFailed)
}

fn encode_auth_public_key(verifying_key: &VerifyingKey) -> String {
    hex::encode(verifying_key.to_bytes())
}

fn decode_signature(signature_hex: &str) -> Result<Signature, ClusterJoinRefusalReason> {
    let bytes = hex::decode(signature_hex)
        .map_err(|_| ClusterJoinRefusalReason::MessageAuthenticationFailed)?;
    let signature_bytes: [u8; SIGNATURE_BYTES] = bytes
        .try_into()
        .map_err(|_| ClusterJoinRefusalReason::MessageAuthenticationFailed)?;
    Ok(Signature::from_bytes(&signature_bytes))
}

async fn run_transport(
    socket: Arc<UdpSocket>,
    state: Arc<Mutex<SharedState>>,
    config: TransportConfig,
    mut shutdown_rx: oneshot::Receiver<()>,
) -> Result<(), String> {
    send_hello_to_seed_peers(&socket, &config, &state).await?;

    let mut hello_tick = interval(HELLO_INTERVAL);
    hello_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut ping_tick = interval(PING_INTERVAL);
    ping_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut recv_buf = vec![0_u8; MAX_DATAGRAM_BYTES];

    loop {
        tokio::select! {
            _ = &mut shutdown_rx => return Ok(()),
            _ = hello_tick.tick() => {
                send_hello_to_seed_peers(&socket, &config, &state).await?;
            }
            _ = ping_tick.tick() => {
                send_ping_to_discovered_peers(&socket, &config, &state).await?;
            }
            received = socket.recv_from(&mut recv_buf) => {
                let (len, remote_addr) = received.map_err(|error| error.to_string())?;
                handle_incoming_message(
                    &socket,
                    &state,
                    &config,
                    remote_addr,
                    &recv_buf[..len],
                )
                .await?;
            }
        }
    }
}

async fn send_hello_to_seed_peers(
    socket: &Arc<UdpSocket>,
    config: &TransportConfig,
    state: &Arc<Mutex<SharedState>>,
) -> Result<(), String> {
    let remote_addrs = {
        let mut guard = state.lock().await;
        if matches!(
            config.trust_policy.posture,
            ClusterTrustPosture::AuthenticatedConfiguredPeers
        ) {
            guard.configured_peers_due_for_dial(&config.trust_policy)
        } else {
            guard.undiscovered_seed_peers()
        }
    };
    for remote_addr in remote_addrs {
        let envelope = outbound_envelope(
            state,
            config,
            WireMessage::Hello(HelloMessage {
                sender: config.local_identity.clone(),
            }),
        )
        .await?;
        send_message(socket, remote_addr, &envelope).await?;
    }
    Ok(())
}

async fn send_ping_to_discovered_peers(
    socket: &Arc<UdpSocket>,
    config: &TransportConfig,
    state: &Arc<Mutex<SharedState>>,
) -> Result<(), String> {
    let (peer_addrs, sequence) = {
        let mut guard = state.lock().await;
        (guard.discovered_peer_addrs(), guard.next_ping_sequence())
    };
    for remote_addr in peer_addrs {
        let envelope = outbound_envelope(
            state,
            config,
            WireMessage::Ping(PingMessage {
                sender: config.local_identity.clone(),
                sequence,
            }),
        )
        .await?;
        send_message(socket, remote_addr, &envelope).await?;
    }
    Ok(())
}

async fn handle_incoming_message(
    socket: &Arc<UdpSocket>,
    state: &Arc<Mutex<SharedState>>,
    config: &TransportConfig,
    remote_addr: SocketAddr,
    payload: &[u8],
) -> Result<(), String> {
    let envelope = match serde_json::from_slice::<WireEnvelope>(payload) {
        Ok(envelope) => envelope,
        Err(_) => return Ok(()),
    };

    if envelope.namespace != config.namespace {
        state.lock().await.push_join_refusal(ClusterJoinRefusal {
            remote_addr,
            remote_node_id: Some(envelope.message.sender().node_id.clone()),
            remote_cluster_id: Some(envelope.message.sender().cluster_id.clone()),
            remote_node_epoch: Some(envelope.message.sender().node_epoch),
            reason: ClusterJoinRefusalReason::NamespaceMismatch {
                expected: config.namespace.clone(),
                actual: envelope.namespace.clone(),
            },
        });
        return Ok(());
    }
    if envelope.admission_digest != config.admission_digest {
        state.lock().await.push_join_refusal(ClusterJoinRefusal {
            remote_addr,
            remote_node_id: Some(envelope.message.sender().node_id.clone()),
            remote_cluster_id: Some(envelope.message.sender().cluster_id.clone()),
            remote_node_epoch: Some(envelope.message.sender().node_epoch),
            reason: ClusterJoinRefusalReason::AdmissionMismatch,
        });
        return Ok(());
    }

    let trust_rollout_diagnostic =
        match authenticate_incoming_envelope(&envelope, remote_addr, config) {
            Ok(trust_rollout_diagnostic) => trust_rollout_diagnostic,
            Err(reason) => {
                let rollout_diagnostic =
                    trust_rollout_diagnostic_from_refusal(&envelope, remote_addr, &reason, config);
                let mut guard = state.lock().await;
                if let Some(rollout_diagnostic) = rollout_diagnostic {
                    guard.push_trust_rollout_diagnostic(rollout_diagnostic);
                }
                guard.push_join_refusal(ClusterJoinRefusal {
                    remote_addr,
                    remote_node_id: Some(envelope.message.sender().node_id.clone()),
                    remote_cluster_id: Some(envelope.message.sender().cluster_id.clone()),
                    remote_node_epoch: Some(envelope.message.sender().node_epoch),
                    reason,
                });
                return Ok(());
            }
        };
    if let Some(trust_rollout_diagnostic) = trust_rollout_diagnostic {
        state
            .lock()
            .await
            .push_trust_rollout_diagnostic(trust_rollout_diagnostic);
    }

    match envelope.message {
        WireMessage::Hello(hello) => {
            if hello.sender.node_id == config.local_identity.node_id {
                return Ok(());
            }
            if hello.sender.cluster_id != config.local_identity.cluster_id {
                state.lock().await.push_join_refusal(ClusterJoinRefusal {
                    remote_addr,
                    remote_node_id: Some(hello.sender.node_id),
                    remote_cluster_id: Some(hello.sender.cluster_id.clone()),
                    remote_node_epoch: Some(hello.sender.node_epoch),
                    reason: ClusterJoinRefusalReason::ClusterIdMismatch {
                        expected: config.local_identity.cluster_id.clone(),
                        actual: hello.sender.cluster_id,
                    },
                });
                return Ok(());
            }

            let should_reply_hello = {
                let mut guard = state.lock().await;
                match guard.record_hello(
                    remote_addr,
                    hello.sender,
                    envelope.authenticated_counter,
                    config.trust_policy.replay_window_size,
                ) {
                    Ok(should_reply) => should_reply,
                    Err(refusal) => {
                        guard.push_join_refusal(*refusal);
                        return Ok(());
                    }
                }
            };

            if should_reply_hello {
                let envelope = outbound_envelope(
                    state,
                    config,
                    WireMessage::Hello(HelloMessage {
                        sender: config.local_identity.clone(),
                    }),
                )
                .await?;
                send_message(socket, remote_addr, &envelope).await?;
            }

            let sequence = state.lock().await.next_ping_sequence();
            let envelope = outbound_envelope(
                state,
                config,
                WireMessage::Ping(PingMessage {
                    sender: config.local_identity.clone(),
                    sequence,
                }),
            )
            .await?;
            send_message(socket, remote_addr, &envelope).await?;
        }
        WireMessage::Ping(ping) => {
            if ping.sender.node_id == config.local_identity.node_id {
                return Ok(());
            }
            if ping.sender.cluster_id != config.local_identity.cluster_id {
                state.lock().await.push_join_refusal(ClusterJoinRefusal {
                    remote_addr,
                    remote_node_id: Some(ping.sender.node_id),
                    remote_cluster_id: Some(ping.sender.cluster_id.clone()),
                    remote_node_epoch: Some(ping.sender.node_epoch),
                    reason: ClusterJoinRefusalReason::ClusterIdMismatch {
                        expected: config.local_identity.cluster_id.clone(),
                        actual: ping.sender.cluster_id,
                    },
                });
                return Ok(());
            }

            let mut guard = state.lock().await;
            if let Err(refusal) = guard.record_ping(
                remote_addr,
                ping.sender,
                ping.sequence,
                envelope.authenticated_counter,
                config.trust_policy.replay_window_size,
            ) {
                guard.push_join_refusal(*refusal);
            }
        }
    }
    Ok(())
}

async fn outbound_envelope(
    state: &Arc<Mutex<SharedState>>,
    config: &TransportConfig,
    message: WireMessage,
) -> Result<WireEnvelope, String> {
    let authenticated_counter = if config.trust_policy.require_message_authentication {
        Some(state.lock().await.next_authenticated_message_counter())
    } else {
        None
    };
    let mut envelope = WireEnvelope {
        namespace: config.namespace.clone(),
        admission_digest: config.admission_digest.clone(),
        trust_bundle_version: config
            .trust_policy
            .require_message_authentication
            .then_some(config.trust_policy.trust_bundle_version),
        authenticated_counter,
        signature_hex: None,
        message,
    };
    if config.trust_policy.require_message_authentication {
        let signature = config
            .local_signing_key
            .sign(&wire_signing_payload(&envelope).map_err(|error| error.to_string())?);
        envelope.signature_hex = Some(hex::encode(signature.to_bytes()));
    }
    Ok(envelope)
}

fn authenticate_incoming_envelope(
    envelope: &WireEnvelope,
    remote_addr: SocketAddr,
    config: &TransportConfig,
) -> Result<Option<ClusterTrustRolloutDiagnostic>, ClusterJoinRefusalReason> {
    let mut trust_rollout_diagnostic = None;
    if matches!(
        config.trust_policy.posture,
        ClusterTrustPosture::AuthenticatedConfiguredPeers
    ) {
        let Some(configured_peer) = config
            .trust_policy
            .configured_peer(&envelope.message.sender().node_id)
        else {
            return Err(ClusterJoinRefusalReason::ConfiguredPeerUnknown);
        };
        if configured_peer.remote_addr != remote_addr {
            return Err(ClusterJoinRefusalReason::ConfiguredPeerAddressMismatch {
                expected: configured_peer.remote_addr,
                actual: remote_addr,
            });
        }
        let key_match = configured_peer.key_match(&envelope.message.sender().auth_public_key);
        let Some(key_match) = key_match else {
            return Err(ClusterJoinRefusalReason::ConfiguredPeerKeyMismatch {
                expected: configured_peer.auth_public_key.clone(),
                actual: envelope.message.sender().auth_public_key.clone(),
            });
        };
        let actual_trust_bundle_version = envelope.trust_bundle_version;
        let Some(actual_trust_bundle_version) = actual_trust_bundle_version else {
            return Err(ClusterJoinRefusalReason::TrustBundleVersionMismatch {
                expected: config.trust_policy.trust_bundle_version,
                actual: None,
                accepted: config.trust_policy.accepted_trust_bundle_versions.clone(),
            });
        };
        if !config
            .trust_policy
            .accepts_trust_bundle_version(actual_trust_bundle_version)
        {
            return Err(ClusterJoinRefusalReason::TrustBundleVersionMismatch {
                expected: config.trust_policy.trust_bundle_version,
                actual: Some(actual_trust_bundle_version),
                accepted: config.trust_policy.accepted_trust_bundle_versions.clone(),
            });
        }
        if actual_trust_bundle_version != config.trust_policy.trust_bundle_version
            || matches!(key_match, ConfiguredPeerKeyMatch::Previous)
        {
            trust_rollout_diagnostic = Some(ClusterTrustRolloutDiagnostic {
                remote_node_id: envelope.message.sender().node_id.clone(),
                remote_addr,
                expected_trust_bundle_version: config.trust_policy.trust_bundle_version,
                actual_trust_bundle_version: Some(actual_trust_bundle_version),
                key_match: Some(key_match),
                disposition: ClusterTrustRolloutDisposition::AcceptedOverlap,
            });
        }
    }
    if !config.trust_policy.require_message_authentication {
        return Ok(trust_rollout_diagnostic);
    }
    let counter = envelope
        .authenticated_counter
        .ok_or(ClusterJoinRefusalReason::MessageAuthenticationFailed)?;
    let signature_hex = envelope
        .signature_hex
        .as_deref()
        .ok_or(ClusterJoinRefusalReason::MessageAuthenticationFailed)?;
    let verifying_key = decode_verifying_key(&envelope.message.sender().auth_public_key)?;
    let signature = decode_signature(signature_hex)?;
    verifying_key
        .verify(
            &wire_signing_payload(envelope)
                .map_err(|_| ClusterJoinRefusalReason::MessageAuthenticationFailed)?,
            &signature,
        )
        .map_err(|_| ClusterJoinRefusalReason::MessageAuthenticationFailed)?;
    let _ = counter;
    Ok(trust_rollout_diagnostic)
}

async fn send_message(
    socket: &Arc<UdpSocket>,
    remote_addr: SocketAddr,
    envelope: &WireEnvelope,
) -> Result<(), String> {
    let encoded = serde_json::to_vec(envelope).map_err(|error| error.to_string())?;
    socket
        .send_to(&encoded, remote_addr)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn admission_digest(admission_token: &AdmissionToken) -> String {
    let mut hasher = Sha256::new();
    hasher.update(admission_token.as_str().as_bytes());
    hex::encode(hasher.finalize())
}

fn wire_signing_payload(envelope: &WireEnvelope) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(&(
        &envelope.namespace,
        &envelope.admission_digest,
        envelope.trust_bundle_version,
        envelope.authenticated_counter,
        &envelope.message,
    ))
}

fn classify_configured_peer_reachability(
    unanswered_hello_attempts: u32,
    dial_policy: ConfiguredPeerDialPolicy,
) -> ConfiguredPeerReachability {
    if unanswered_hello_attempts >= dial_policy.unreachable_after_unanswered_hellos {
        return ConfiguredPeerReachability::Unreachable;
    }
    if unanswered_hello_attempts >= dial_policy.degraded_after_unanswered_hellos {
        return ConfiguredPeerReachability::Degraded;
    }
    ConfiguredPeerReachability::Pending
}

fn next_configured_peer_backoff_ticks(
    unanswered_hello_attempts: u32,
    dial_policy: ConfiguredPeerDialPolicy,
) -> u32 {
    if unanswered_hello_attempts == 0 {
        return 0;
    }
    let exponent = unanswered_hello_attempts.saturating_sub(1).min(31);
    let multiplier = 1_u32.checked_shl(exponent).unwrap_or(u32::MAX);
    dial_policy
        .base_backoff_ticks
        .saturating_mul(multiplier)
        .min(dial_policy.max_backoff_ticks)
}

fn trust_rollout_diagnostic_from_refusal(
    envelope: &WireEnvelope,
    remote_addr: SocketAddr,
    reason: &ClusterJoinRefusalReason,
    config: &TransportConfig,
) -> Option<ClusterTrustRolloutDiagnostic> {
    match reason {
        ClusterJoinRefusalReason::TrustBundleVersionMismatch { actual, .. } => {
            Some(ClusterTrustRolloutDiagnostic {
                remote_node_id: envelope.message.sender().node_id.clone(),
                remote_addr,
                expected_trust_bundle_version: config.trust_policy.trust_bundle_version,
                actual_trust_bundle_version: *actual,
                key_match: config
                    .trust_policy
                    .configured_peer(&envelope.message.sender().node_id)
                    .and_then(|peer| peer.key_match(&envelope.message.sender().auth_public_key)),
                disposition: ClusterTrustRolloutDisposition::RefusedVersionMismatch,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loopback_addr(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }

    fn sample_admission() -> ClusterAdmissionConfig {
        ClusterAdmissionConfig::new("lan-alpha", "shared-secret")
    }

    fn sample_signing_key(byte: u8) -> SigningKey {
        SigningKey::from_bytes(&[byte; SIGNING_KEY_BYTES])
    }

    fn sample_identity(
        admission: &ClusterAdmissionConfig,
        node_id: &str,
        role: NodeRole,
        signing_key: &SigningKey,
    ) -> ClusterNodeIdentity {
        ClusterNodeIdentity {
            cluster_id: ClusterId::new(&admission.namespace, &admission.admission_token),
            node_id: NodeId::new(node_id),
            node_epoch: NodeEpoch::initial(),
            role,
            auth_public_key: encode_auth_public_key(&signing_key.verifying_key()),
        }
    }

    fn sample_transport_config(
        local_identity: ClusterNodeIdentity,
        local_signing_key: SigningKey,
        trust_policy: ClusterTrustPolicy,
    ) -> TransportConfig {
        let admission = sample_admission();
        TransportConfig {
            namespace: admission.namespace.clone(),
            admission_digest: admission_digest(&admission.admission_token),
            bind_addr: loopback_addr(31000),
            seed_peers: BTreeSet::new(),
            local_identity,
            local_signing_key,
            trust_policy,
        }
    }

    fn signed_ping_envelope(
        namespace: &ClusterNamespace,
        admission_digest: &str,
        trust_bundle_version: Option<u64>,
        sender: ClusterNodeIdentity,
        signing_key: &SigningKey,
        authenticated_counter: u64,
        sequence: u64,
    ) -> WireEnvelope {
        let mut envelope = WireEnvelope {
            namespace: namespace.clone(),
            admission_digest: admission_digest.to_owned(),
            trust_bundle_version,
            authenticated_counter: Some(authenticated_counter),
            signature_hex: None,
            message: WireMessage::Ping(PingMessage { sender, sequence }),
        };
        let signature = signing_key.sign(
            &wire_signing_payload(&envelope)
                .unwrap_or_else(|_| unreachable!("test envelope should serialize")),
        );
        envelope.signature_hex = Some(hex::encode(signature.to_bytes()));
        envelope
    }

    #[test]
    fn authenticated_trust_policy_digest_changes_with_posture_and_peers() {
        let trusted_lan = ClusterTrustPolicy::trusted_lan();
        let configured =
            ClusterTrustPolicy::authenticated_configured_peers(vec![ConfiguredClusterPeer::new(
                NodeId::new("node-b"),
                loopback_addr(31001),
                "peer-key-a",
            )]);
        let configured_other_version = configured.clone().with_trust_bundle_version(2);
        let configured_other_policy =
            configured
                .clone()
                .with_configured_peer_dial_policy(ConfiguredPeerDialPolicy {
                    base_backoff_ticks: 2,
                    max_backoff_ticks: 4,
                    degraded_after_unanswered_hellos: 2,
                    unreachable_after_unanswered_hellos: 3,
                });
        let configured_other_addr =
            ClusterTrustPolicy::authenticated_configured_peers(vec![ConfiguredClusterPeer::new(
                NodeId::new("node-b"),
                loopback_addr(31002),
                "peer-key-a",
            )]);

        assert_ne!(trusted_lan.stable_digest(), configured.stable_digest());
        assert_ne!(
            configured.stable_digest(),
            configured_other_addr.stable_digest()
        );
        assert_ne!(
            configured.stable_digest(),
            configured_other_policy.stable_digest()
        );
        assert_ne!(
            configured.stable_digest(),
            configured_other_version.stable_digest()
        );
    }

    #[test]
    fn compute_market_assessment_refuses_trusted_lan_policy() {
        let trusted_lan = ClusterTrustPolicy::trusted_lan();

        let assessment = trusted_lan.compute_market_trust_assessment();

        assert_eq!(
            assessment.disposition,
            ClusterComputeMarketTrustDisposition::Refused
        );
        assert_eq!(
            assessment.posture,
            ClusterTrustPosture::TrustedLanSharedAdmission
        );
        assert_eq!(assessment.trust_policy_digest, trusted_lan.stable_digest());
        assert_eq!(
            assessment.refusal_reasons,
            vec![
                ClusterComputeMarketTrustRefusalReason::TrustedLanSharedAdmissionOnly,
                ClusterComputeMarketTrustRefusalReason::MissingAuthenticatedTransport,
                ClusterComputeMarketTrustRefusalReason::MissingSignedEvidenceBundleExport,
                ClusterComputeMarketTrustRefusalReason::MissingAttestedNodeIdentityAdmission,
                ClusterComputeMarketTrustRefusalReason::MissingNonLanDiscoveryPosture,
            ]
        );
    }

    #[test]
    fn compute_market_assessment_refuses_operator_managed_configured_peers() {
        let configured =
            ClusterTrustPolicy::authenticated_configured_peers(vec![ConfiguredClusterPeer::new(
                NodeId::new("node-b"),
                loopback_addr(31001),
                "peer-key-a",
            )]);

        let assessment = configured.compute_market_trust_assessment();

        assert_eq!(
            assessment.disposition,
            ClusterComputeMarketTrustDisposition::Refused
        );
        assert_eq!(
            assessment.posture,
            ClusterTrustPosture::AuthenticatedConfiguredPeers
        );
        assert_eq!(assessment.trust_policy_digest, configured.stable_digest());
        assert_eq!(
            assessment.refusal_reasons,
            vec![
                ClusterComputeMarketTrustRefusalReason::OperatorManagedConfiguredPeersOnly,
                ClusterComputeMarketTrustRefusalReason::MissingSignedEvidenceBundleExport,
                ClusterComputeMarketTrustRefusalReason::MissingAttestedNodeIdentityAdmission,
                ClusterComputeMarketTrustRefusalReason::MissingNonLanDiscoveryPosture,
            ]
        );
    }

    #[test]
    fn compute_market_assessment_digest_changes_with_refusal_shape() {
        let trusted_lan = ClusterTrustPolicy::trusted_lan().compute_market_trust_assessment();
        let configured =
            ClusterTrustPolicy::authenticated_configured_peers(vec![ConfiguredClusterPeer::new(
                NodeId::new("node-b"),
                loopback_addr(31002),
                "peer-key-a",
            )])
            .compute_market_trust_assessment();

        assert_ne!(trusted_lan.stable_digest(), configured.stable_digest());
    }

    #[test]
    fn cluster_node_identity_verifies_signed_evidence_bundle() {
        let admission = sample_admission();
        let signing_key = sample_signing_key(13);
        let identity = sample_identity(&admission, "exporter", NodeRole::Mixed, &signing_key);
        let bundle = psionic_runtime::ClusterEvidenceBundlePayload::new(
            "text_generation",
            "request-1",
            "request-digest",
            "fixture-decoder-v0",
            "v0",
            "cuda",
            "served-artifact-digest",
            "weight-bundle-digest",
            psionic_runtime::ClusterEvidenceBundleStatus::Succeeded,
            psionic_runtime::ClusterExecutionContext::new(
                identity.cluster_id.as_str(),
                "cluster-state-digest",
                "cluster-topology-digest",
                identity.node_id.as_str(),
                psionic_runtime::ClusterTransportClass::TrustedLanDatagram,
                psionic_runtime::ClusterExecutionDisposition::RemoteWholeRequest,
            ),
        )
        .sign(identity.node_id.as_str(), &signing_key);

        let verification = identity.verify_signed_evidence_bundle(&bundle);

        assert!(
            verification.is_ok(),
            "bundle should verify against node identity"
        );
    }

    #[test]
    fn cluster_node_identity_refuses_bundle_with_wrong_signer_key() {
        let admission = sample_admission();
        let identity_signing_key = sample_signing_key(14);
        let identity = sample_identity(
            &admission,
            "exporter",
            NodeRole::Mixed,
            &identity_signing_key,
        );
        let other_signing_key = sample_signing_key(15);
        let bundle = psionic_runtime::ClusterEvidenceBundlePayload::new(
            "text_generation",
            "request-1",
            "request-digest",
            "fixture-decoder-v0",
            "v0",
            "cuda",
            "served-artifact-digest",
            "weight-bundle-digest",
            psionic_runtime::ClusterEvidenceBundleStatus::Succeeded,
            psionic_runtime::ClusterExecutionContext::new(
                identity.cluster_id.as_str(),
                "cluster-state-digest",
                "cluster-topology-digest",
                identity.node_id.as_str(),
                psionic_runtime::ClusterTransportClass::TrustedLanDatagram,
                psionic_runtime::ClusterExecutionDisposition::RemoteWholeRequest,
            ),
        )
        .sign(identity.node_id.as_str(), &other_signing_key);

        let verification = identity.verify_signed_evidence_bundle(&bundle);

        assert!(
            matches!(
                verification,
                Err(ClusterEvidenceBundleIdentityError::SignerKeyMismatch { .. })
            ),
            "bundle with a different control-plane key should be refused"
        );
    }

    #[test]
    fn tampered_authenticated_message_is_refused() {
        let admission = sample_admission();
        let local_signing_key = sample_signing_key(7);
        let local_identity = sample_identity(
            &admission,
            "local",
            NodeRole::CoordinatorOnly,
            &local_signing_key,
        );
        let remote_signing_key = sample_signing_key(9);
        let remote_identity = sample_identity(
            &admission,
            "remote",
            NodeRole::ExecutorOnly,
            &remote_signing_key,
        );
        let remote_addr = loopback_addr(31002);
        let config = sample_transport_config(
            local_identity,
            local_signing_key,
            ClusterTrustPolicy::authenticated_configured_peers(vec![ConfiguredClusterPeer::new(
                remote_identity.node_id.clone(),
                remote_addr,
                remote_identity.auth_public_key.clone(),
            )]),
        );
        let mut envelope = signed_ping_envelope(
            &config.namespace,
            &config.admission_digest,
            Some(config.trust_policy.trust_bundle_version),
            remote_identity,
            &remote_signing_key,
            1,
            7,
        );
        if let WireMessage::Ping(message) = &mut envelope.message {
            message.sequence = 8;
        }

        let refusal = authenticate_incoming_envelope(&envelope, remote_addr, &config);
        assert_eq!(
            refusal,
            Err(ClusterJoinRefusalReason::MessageAuthenticationFailed)
        );
    }

    #[test]
    fn replay_protection_rejects_duplicate_authenticated_counters() {
        let admission = sample_admission();
        let remote_signing_key = sample_signing_key(11);
        let remote_identity = sample_identity(
            &admission,
            "remote",
            NodeRole::ExecutorOnly,
            &remote_signing_key,
        );
        let remote_addr = loopback_addr(31003);
        let mut state = SharedState::new(BTreeSet::new(), &ClusterTrustPolicy::trusted_lan());

        let hello = state.record_hello(remote_addr, remote_identity.clone(), Some(12), 8);
        assert!(
            hello.is_ok(),
            "first authenticated hello should be accepted"
        );

        let refusal = state.record_ping(remote_addr, remote_identity, 0, Some(12), 8);
        assert!(refusal.is_err(), "duplicate counter should be refused");
        let refusal = refusal
            .err()
            .unwrap_or_else(|| unreachable!("assert above ensures failure"));
        assert_eq!(
            refusal.reason,
            ClusterJoinRefusalReason::ReplayDetected {
                highest_seen: 12,
                attempted: 12,
            }
        );
    }
}
