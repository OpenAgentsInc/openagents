//! Transport and session substrate for Psionic cluster networking.

mod operator_manifest;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use psionic_runtime::{ClusterEvidenceBundleVerificationError, SignedClusterEvidenceBundle};
use rand::random;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    net::UdpSocket,
    sync::{Mutex, mpsc, oneshot},
    task::JoinHandle,
    time::{MissedTickBehavior, interval},
};

pub use operator_manifest::*;
pub use psionic_runtime::{
    ClusterComputeMarketTrustAssessment, ClusterComputeMarketTrustDisposition,
    ClusterComputeMarketTrustRefusalReason, ClusterDiscoveryPosture, ClusterTrustPosture,
};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "transport and session substrate for psionic clusters";

const HELLO_INTERVAL: Duration = Duration::from_millis(100);
const PING_INTERVAL: Duration = Duration::from_millis(75);
const MAX_DATAGRAM_BYTES: usize = 8 * 1024;
const DEFAULT_REPLAY_WINDOW_SIZE: u64 = 64;
const SIGNING_KEY_BYTES: usize = 32;
const VERIFYING_KEY_BYTES: usize = 32;
const SIGNATURE_BYTES: usize = 64;
const DEFAULT_TUNNEL_HTTP_BODY_BYTES: usize = 4 * 1024;

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
    /// The configured network-state file could not be read or written.
    #[error("failed to read or write cluster network state: {0}")]
    NetworkStateIo(#[source] std::io::Error),
    /// The configured network-state file contained invalid data.
    #[error("failed to parse cluster network state: {0}")]
    NetworkStateFormat(#[source] serde_json::Error),
    /// The configured network-state schema version is unsupported.
    #[error(
        "unsupported cluster network-state schema version: expected {expected}, found {actual}"
    )]
    NetworkStateSchemaVersion { expected: u32, actual: u32 },
    /// A durable candidate operation required an introduction policy that is not configured.
    #[error("cluster introduction policy is not configured")]
    IntroductionPolicyMissing,
    /// A durable candidate operation referenced a candidate that is not currently tracked.
    #[error("cluster candidate `{0}` is not tracked")]
    UnknownCandidate(String),
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

/// Expected attestation facts for one configured market-facing peer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeAttestationRequirement {
    /// Stable attestation issuer or verifier authority.
    pub issuer: String,
    /// Stable digest for the attestation statement or certificate chain.
    pub attestation_digest: String,
    /// Stable device or host identity digest when one is required.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_identity_digest: Option<String>,
}

impl NodeAttestationRequirement {
    /// Creates a new attestation requirement.
    #[must_use]
    pub fn new(issuer: impl Into<String>, attestation_digest: impl Into<String>) -> Self {
        Self {
            issuer: issuer.into(),
            attestation_digest: attestation_digest.into(),
            device_identity_digest: None,
        }
    }

    /// Attaches a stable device or host identity digest.
    #[must_use]
    pub fn with_device_identity_digest(
        mut self,
        device_identity_digest: impl Into<String>,
    ) -> Self {
        self.device_identity_digest = Some(device_identity_digest.into());
        self
    }

    fn matches(&self, evidence: &NodeAttestationEvidence) -> bool {
        self.issuer == evidence.issuer
            && self.attestation_digest == evidence.attestation_digest
            && self.device_identity_digest == evidence.device_identity_digest
    }
}

/// Attestation facts carried by one cluster node identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeAttestationEvidence {
    /// Stable attestation issuer or verifier authority.
    pub issuer: String,
    /// Stable digest for the attestation statement or certificate chain.
    pub attestation_digest: String,
    /// Stable device or host identity digest when one is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_identity_digest: Option<String>,
}

impl NodeAttestationEvidence {
    /// Creates attestation evidence for one node identity.
    #[must_use]
    pub fn new(issuer: impl Into<String>, attestation_digest: impl Into<String>) -> Self {
        Self {
            issuer: issuer.into(),
            attestation_digest: attestation_digest.into(),
            device_identity_digest: None,
        }
    }

    /// Attaches a stable device or host identity digest.
    #[must_use]
    pub fn with_device_identity_digest(
        mut self,
        device_identity_digest: impl Into<String>,
    ) -> Self {
        self.device_identity_digest = Some(device_identity_digest.into());
        self
    }
}

/// Candidate node surfaced by a wider-network discovery artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterDiscoveryCandidate {
    /// Candidate cluster identity.
    pub cluster_id: ClusterId,
    /// Candidate cluster namespace.
    pub namespace: ClusterNamespace,
    /// Candidate node identity.
    pub node_id: NodeId,
    /// Candidate execution role.
    pub role: NodeRole,
    /// Candidate message-signing public key.
    pub auth_public_key: String,
    /// Candidate attestation facts, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attestation: Option<NodeAttestationEvidence>,
    /// Candidate transport addresses advertised for later contact.
    pub advertised_addrs: Vec<SocketAddr>,
}

impl ClusterDiscoveryCandidate {
    /// Creates a new discovered cluster candidate.
    #[must_use]
    pub fn new(
        cluster_id: ClusterId,
        namespace: ClusterNamespace,
        node_id: NodeId,
        role: NodeRole,
        auth_public_key: impl Into<String>,
        mut advertised_addrs: Vec<SocketAddr>,
    ) -> Self {
        advertised_addrs.sort_unstable();
        advertised_addrs.dedup();
        Self {
            cluster_id,
            namespace,
            node_id,
            role,
            auth_public_key: auth_public_key.into(),
            attestation: None,
            advertised_addrs,
        }
    }

    /// Attaches candidate attestation facts.
    #[must_use]
    pub fn with_attestation(mut self, attestation: NodeAttestationEvidence) -> Self {
        self.attestation = Some(attestation);
        self
    }

    /// Returns a stable digest for the candidate discovery record.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"cluster_discovery_candidate|");
        hasher.update(self.cluster_id.as_str().as_bytes());
        hasher.update(b"|");
        hasher.update(self.namespace.as_str().as_bytes());
        hasher.update(b"|");
        hasher.update(self.node_id.as_str().as_bytes());
        hasher.update(b"|");
        hasher.update(match self.role {
            NodeRole::CoordinatorOnly => b"coordinator_only".as_slice(),
            NodeRole::ExecutorOnly => b"executor_only".as_slice(),
            NodeRole::Mixed => b"mixed".as_slice(),
        });
        hasher.update(b"|");
        hasher.update(self.auth_public_key.as_bytes());
        if let Some(attestation) = &self.attestation {
            hasher.update(b"|attestation_issuer|");
            hasher.update(attestation.issuer.as_bytes());
            hasher.update(b"|attestation_digest|");
            hasher.update(attestation.attestation_digest.as_bytes());
            if let Some(device_identity_digest) = &attestation.device_identity_digest {
                hasher.update(b"|device_identity_digest|");
                hasher.update(device_identity_digest.as_bytes());
            }
        }
        for advertised_addr in &self.advertised_addrs {
            hasher.update(b"|addr|");
            hasher.update(advertised_addr.to_string().as_bytes());
        }
        hex::encode(hasher.finalize())
    }
}

/// One operator-approved introduction source for wider-network discovery.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClusterIntroductionSource {
    /// Stable source identifier for policy and audit.
    pub source_id: String,
    /// Public key expected for the introduction source.
    pub public_key: String,
}

impl ClusterIntroductionSource {
    /// Creates one accepted introduction source.
    #[must_use]
    pub fn new(source_id: impl Into<String>, public_key: impl Into<String>) -> Self {
        Self {
            source_id: source_id.into(),
            public_key: public_key.into(),
        }
    }
}

/// Operator-managed policy for accepted cluster introduction sources.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterIntroductionPolicy {
    /// Accepted signed introduction sources.
    pub accepted_sources: Vec<ClusterIntroductionSource>,
    /// Maximum validity window accepted for one candidate introduction.
    pub maximum_candidate_ttl_ms: u64,
}

impl ClusterIntroductionPolicy {
    /// Creates a new introduction policy.
    #[must_use]
    pub fn new(
        mut accepted_sources: Vec<ClusterIntroductionSource>,
        maximum_candidate_ttl_ms: u64,
    ) -> Self {
        accepted_sources.sort_unstable();
        accepted_sources.dedup();
        Self {
            accepted_sources,
            maximum_candidate_ttl_ms,
        }
    }

    /// Returns a stable digest for this introduction policy.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"cluster_introduction_policy|");
        hasher.update(self.maximum_candidate_ttl_ms.to_string().as_bytes());
        for source in &self.accepted_sources {
            hasher.update(b"|source|");
            hasher.update(source.source_id.as_bytes());
            hasher.update(b"|");
            hasher.update(source.public_key.as_bytes());
        }
        hex::encode(hasher.finalize())
    }

    fn accepts_source(&self, source_id: &str, public_key: &str) -> bool {
        self.accepted_sources
            .iter()
            .any(|source| source.source_id == source_id && source.public_key == public_key)
    }
}

/// Unsigned discovery payload that introduces a future wider-network cluster candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterIntroductionPayload {
    /// Candidate node introduced by this payload.
    pub candidate: ClusterDiscoveryCandidate,
    /// Inclusive issuance timestamp in milliseconds since epoch.
    pub issued_at_ms: u64,
    /// Inclusive expiry timestamp in milliseconds since epoch.
    pub expires_at_ms: u64,
}

impl ClusterIntroductionPayload {
    /// Creates a new discovery introduction payload.
    #[must_use]
    pub const fn new(
        candidate: ClusterDiscoveryCandidate,
        issued_at_ms: u64,
        expires_at_ms: u64,
    ) -> Self {
        Self {
            candidate,
            issued_at_ms,
            expires_at_ms,
        }
    }

    /// Returns a stable digest for the introduction payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"cluster_introduction_payload|");
        hasher.update(self.candidate.stable_digest().as_bytes());
        hasher.update(b"|issued_at_ms|");
        hasher.update(self.issued_at_ms.to_string().as_bytes());
        hasher.update(b"|expires_at_ms|");
        hasher.update(self.expires_at_ms.to_string().as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Signature metadata for one signed cluster introduction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterIntroductionSignature {
    /// Stable identifier for the introduction source.
    pub source_id: String,
    /// Public key that signed the introduction payload.
    pub signer_public_key: String,
    /// Stable digest for the signed payload.
    pub payload_digest: String,
    /// Hex-encoded Ed25519 signature over `payload_digest`.
    pub signature_hex: String,
}

/// Signed discovery artifact for future wider-network candidate intake.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedClusterIntroductionEnvelope {
    /// Unsigned introduction payload.
    pub payload: ClusterIntroductionPayload,
    /// Signature metadata for the payload.
    pub signature: ClusterIntroductionSignature,
}

impl SignedClusterIntroductionEnvelope {
    /// Signs an introduction payload with one accepted introduction source key.
    #[must_use]
    pub fn sign(
        payload: ClusterIntroductionPayload,
        source_id: impl Into<String>,
        signing_key: &SigningKey,
    ) -> Self {
        let payload_digest = payload.stable_digest();
        let signature = signing_key.sign(payload_digest.as_bytes());
        Self {
            payload,
            signature: ClusterIntroductionSignature {
                source_id: source_id.into(),
                signer_public_key: hex::encode(signing_key.verifying_key().to_bytes()),
                payload_digest,
                signature_hex: hex::encode(signature.to_bytes()),
            },
        }
    }

    /// Verifies one signed introduction envelope under the current introduction policy.
    pub fn verify(
        &self,
        policy: &ClusterIntroductionPolicy,
    ) -> Result<(), ClusterIntroductionVerificationError> {
        if !policy.accepts_source(&self.signature.source_id, &self.signature.signer_public_key) {
            return Err(ClusterIntroductionVerificationError::UntrustedSource {
                source_id: self.signature.source_id.clone(),
                public_key: self.signature.signer_public_key.clone(),
            });
        }
        if self.payload.expires_at_ms < self.payload.issued_at_ms {
            return Err(
                ClusterIntroductionVerificationError::InvalidValidityWindow {
                    issued_at_ms: self.payload.issued_at_ms,
                    expires_at_ms: self.payload.expires_at_ms,
                },
            );
        }
        let ttl_ms = self.payload.expires_at_ms - self.payload.issued_at_ms;
        if ttl_ms > policy.maximum_candidate_ttl_ms {
            return Err(
                ClusterIntroductionVerificationError::CandidateTtlExceedsPolicy {
                    ttl_ms,
                    maximum_ttl_ms: policy.maximum_candidate_ttl_ms,
                },
            );
        }
        let expected_payload_digest = self.payload.stable_digest();
        if self.signature.payload_digest != expected_payload_digest {
            return Err(
                ClusterIntroductionVerificationError::PayloadDigestMismatch {
                    expected: expected_payload_digest,
                    actual: self.signature.payload_digest.clone(),
                },
            );
        }
        let verifying_key = decode_introduction_verifying_key(&self.signature.signer_public_key)?;
        let signature = decode_introduction_signature(&self.signature.signature_hex)?;
        verifying_key
            .verify(self.signature.payload_digest.as_bytes(), &signature)
            .map_err(|_| ClusterIntroductionVerificationError::SignatureVerificationFailed)
    }
}

/// Verification failure while validating one signed cluster introduction envelope.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ClusterIntroductionVerificationError {
    /// The envelope was signed by a source not accepted by policy.
    #[error("cluster introduction source `{source_id}` with key `{public_key}` is not accepted")]
    UntrustedSource {
        /// Stable source identifier carried by the envelope.
        source_id: String,
        /// Public key carried by the envelope.
        public_key: String,
    },
    /// The payload validity window is malformed.
    #[error(
        "cluster introduction validity window is invalid: issued_at_ms={issued_at_ms}, expires_at_ms={expires_at_ms}"
    )]
    InvalidValidityWindow {
        /// Inclusive issuance timestamp.
        issued_at_ms: u64,
        /// Inclusive expiry timestamp.
        expires_at_ms: u64,
    },
    /// The payload validity window exceeds current operator policy.
    #[error("cluster introduction TTL {ttl_ms} exceeds maximum allowed {maximum_ttl_ms} ms")]
    CandidateTtlExceedsPolicy {
        /// Observed payload time-to-live.
        ttl_ms: u64,
        /// Maximum time-to-live allowed by policy.
        maximum_ttl_ms: u64,
    },
    /// The carried payload digest did not match the actual payload.
    #[error("cluster introduction payload digest mismatch: expected {expected}, found {actual}")]
    PayloadDigestMismatch {
        /// Digest derived from the payload.
        expected: String,
        /// Digest carried by the signature metadata.
        actual: String,
    },
    /// The source public key could not be decoded.
    #[error("invalid cluster introduction source public key: {0}")]
    InvalidSourcePublicKey(String),
    /// The signature could not be decoded.
    #[error("invalid cluster introduction signature: {0}")]
    InvalidSignatureEncoding(String),
    /// The signature did not verify against the payload digest.
    #[error("cluster introduction signature verification failed")]
    SignatureVerificationFailed,
}

/// One explicitly configured peer for the authenticated cluster posture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterRelayEndpoint {
    /// Stable operator-facing relay identifier.
    pub relay_id: String,
    /// Socket address for the relay service.
    pub relay_addr: SocketAddr,
    /// Stable session tag shared by the peers that should rendezvous on this relay.
    pub session_tag: String,
}

impl ClusterRelayEndpoint {
    /// Creates one relay endpoint reference.
    #[must_use]
    pub fn new(
        relay_id: impl Into<String>,
        relay_addr: SocketAddr,
        session_tag: impl Into<String>,
    ) -> Self {
        Self {
            relay_id: relay_id.into(),
            relay_addr,
            session_tag: session_tag.into(),
        }
    }
}

/// High-level transport path used for one peer session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterTransportPathKind {
    /// The peer was reached directly over one datagram path.
    DirectDatagram,
    /// The peer was reached directly after relay-assisted rendezvous / NAT traversal.
    NatTraversalDatagram,
    /// The peer was reached through one relay-forwarded datagram path.
    RelayedDatagram,
}

/// Machine-checkable transport path selected for one peer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterTransportPath {
    /// High-level selected path kind.
    pub kind: ClusterTransportPathKind,
    /// Remote peer address surfaced by the path.
    pub peer_addr: SocketAddr,
    /// Relay used by the path, when the path depended on relay infrastructure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relay: Option<ClusterRelayEndpoint>,
}

impl ClusterTransportPath {
    fn direct(peer_addr: SocketAddr) -> Self {
        Self {
            kind: ClusterTransportPathKind::DirectDatagram,
            peer_addr,
            relay: None,
        }
    }

    fn nat_traversal(peer_addr: SocketAddr, relay: ClusterRelayEndpoint) -> Self {
        Self {
            kind: ClusterTransportPathKind::NatTraversalDatagram,
            peer_addr,
            relay: Some(relay),
        }
    }

    fn relayed(peer_addr: SocketAddr, relay: ClusterRelayEndpoint) -> Self {
        Self {
            kind: ClusterTransportPathKind::RelayedDatagram,
            peer_addr,
            relay: Some(relay),
        }
    }
}

const fn default_max_concurrent_transport_streams() -> u16 {
    4
}

/// Honest logical stream capacity surfaced for one peer session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterSessionMultiplexProfile {
    /// Maximum concurrently reserved logical streams allowed for the session.
    pub max_concurrent_streams: u16,
}

impl ClusterSessionMultiplexProfile {
    fn new(max_concurrent_streams: u16) -> Self {
        Self {
            max_concurrent_streams,
        }
    }
}

impl Default for ClusterSessionMultiplexProfile {
    fn default() -> Self {
        Self::new(default_max_concurrent_transport_streams())
    }
}

/// Stable failure reason for session-establishment fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterSessionFailureReason {
    /// Direct connection attempts timed out.
    DirectConnectTimedOut,
    /// Relay-assisted rendezvous did not yield a usable direct path in time.
    NatTraversalTimedOut,
    /// Relay forwarding could not reach the target peer.
    RelayTargetUnavailable,
    /// The peer was refused by transport or trust policy.
    PeerRefused,
}

/// Machine-checkable failure surfaced while establishing a session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterSessionFailure {
    /// Path that failed or forced fallback.
    pub path_kind: ClusterTransportPathKind,
    /// Stable failure taxonomy.
    pub reason: ClusterSessionFailureReason,
    /// Optional operator-facing detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ClusterSessionFailure {
    fn new(path_kind: ClusterTransportPathKind, reason: ClusterSessionFailureReason) -> Self {
        Self {
            path_kind,
            reason,
            detail: None,
        }
    }

    fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Transport metrics and selected path for one discovered peer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterTransportObservation {
    /// Active transport path for the peer.
    pub path: ClusterTransportPath,
    /// Maximum concurrent logical streams supported by the session.
    pub multiplex_profile: ClusterSessionMultiplexProfile,
    /// Currently reserved logical stream count.
    pub active_streams: u16,
    /// Approximate hello round-trip latency when one could be measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_round_trip_latency_ms: Option<u64>,
    /// Count of datagrams sent on the observed path.
    pub messages_sent: u64,
    /// Count of datagrams received on the observed path.
    pub messages_received: u64,
    /// Bytes sent on the observed path.
    pub bytes_sent: u64,
    /// Bytes received on the observed path.
    pub bytes_received: u64,
}

impl ClusterTransportObservation {
    fn new(path: ClusterTransportPath, multiplex_profile: ClusterSessionMultiplexProfile) -> Self {
        Self {
            path,
            multiplex_profile,
            active_streams: 0,
            last_round_trip_latency_ms: None,
            messages_sent: 0,
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
        }
    }
}

/// Bounded service categories that Psionic Net may expose through one tunnel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterTunnelServiceKind {
    /// App- or operator-facing control HTTP endpoints.
    DesktopControlHttp,
    /// Inference or model-serving HTTP endpoints.
    InferenceHttp,
    /// Validator or proof-service HTTP endpoints.
    ValidatorHttp,
    /// Artifact or bundle-serving HTTP endpoints.
    ArtifactHttp,
}

/// Wire protocol supported by one bounded tunnel service.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterTunnelProtocol {
    /// Request/response HTTP forwarded to one local TCP listener.
    HttpRequestResponse,
}

/// Explicit transport class for one tunnel-backed service path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterTunnelTransportClass {
    /// HTTP request/response tunnel carried over one authenticated peer session.
    ServiceTunnelHttp,
}

/// One service that may be exposed through the bounded tunnel surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterTunnelServicePolicy {
    /// Stable operator-facing service identifier.
    pub service_id: String,
    /// Approved service category.
    pub kind: ClusterTunnelServiceKind,
    /// Supported protocol for the service.
    pub protocol: ClusterTunnelProtocol,
    /// Local TCP listener that actually serves the endpoint.
    pub local_addr: SocketAddr,
    /// Explicit peers allowed to open tunnels to this service. Empty means any connected peer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_peer_node_ids: Vec<NodeId>,
    /// Maximum allowed request body bytes forwarded through the tunnel.
    #[serde(default = "default_tunnel_http_body_bytes")]
    pub max_request_body_bytes: usize,
    /// Maximum allowed response body bytes forwarded through the tunnel.
    #[serde(default = "default_tunnel_http_body_bytes")]
    pub max_response_body_bytes: usize,
}

impl ClusterTunnelServicePolicy {
    /// Creates one HTTP request/response tunnel service policy.
    #[must_use]
    pub fn new_http(
        service_id: impl Into<String>,
        kind: ClusterTunnelServiceKind,
        local_addr: SocketAddr,
    ) -> Self {
        Self {
            service_id: service_id.into(),
            kind,
            protocol: ClusterTunnelProtocol::HttpRequestResponse,
            local_addr,
            allowed_peer_node_ids: Vec::new(),
            max_request_body_bytes: default_tunnel_http_body_bytes(),
            max_response_body_bytes: default_tunnel_http_body_bytes(),
        }
    }

    /// Restricts this service to one explicit set of peers.
    #[must_use]
    pub fn with_allowed_peer_node_ids(mut self, mut allowed_peer_node_ids: Vec<NodeId>) -> Self {
        allowed_peer_node_ids.sort_unstable();
        allowed_peer_node_ids.dedup();
        self.allowed_peer_node_ids = allowed_peer_node_ids;
        self
    }

    /// Overrides the request body size limit for this service.
    #[must_use]
    pub fn with_max_request_body_bytes(mut self, max_request_body_bytes: usize) -> Self {
        self.max_request_body_bytes = max_request_body_bytes.max(1);
        self
    }

    /// Overrides the response body size limit for this service.
    #[must_use]
    pub fn with_max_response_body_bytes(mut self, max_response_body_bytes: usize) -> Self {
        self.max_response_body_bytes = max_response_body_bytes.max(1);
        self
    }

    fn allows_peer(&self, peer_node_id: &NodeId) -> bool {
        self.allowed_peer_node_ids.is_empty() || self.allowed_peer_node_ids.contains(peer_node_id)
    }

    fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"cluster_tunnel_service_policy|");
        hasher.update(self.service_id.as_bytes());
        hasher.update(b"|");
        hasher.update(match self.kind {
            ClusterTunnelServiceKind::DesktopControlHttp => b"desktop_control_http".as_slice(),
            ClusterTunnelServiceKind::InferenceHttp => b"inference_http".as_slice(),
            ClusterTunnelServiceKind::ValidatorHttp => b"validator_http".as_slice(),
            ClusterTunnelServiceKind::ArtifactHttp => b"artifact_http".as_slice(),
        });
        hasher.update(b"|");
        hasher.update(match self.protocol {
            ClusterTunnelProtocol::HttpRequestResponse => b"http_request_response".as_slice(),
        });
        hasher.update(b"|");
        hasher.update(self.local_addr.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(self.max_request_body_bytes.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(self.max_response_body_bytes.to_string().as_bytes());
        for peer_node_id in &self.allowed_peer_node_ids {
            hasher.update(b"|peer|");
            hasher.update(peer_node_id.as_str().as_bytes());
        }
        hex::encode(hasher.finalize())
    }
}

const fn default_tunnel_http_body_bytes() -> usize {
    DEFAULT_TUNNEL_HTTP_BODY_BYTES
}

/// Operator-managed policy for service tunnels owned by one node.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterTunnelPolicy {
    /// Services that may be explicitly exposed through this node.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approved_services: Vec<ClusterTunnelServicePolicy>,
}

impl ClusterTunnelPolicy {
    /// Creates one bounded tunnel policy.
    #[must_use]
    pub fn new(mut approved_services: Vec<ClusterTunnelServicePolicy>) -> Self {
        approved_services.sort_by(|left, right| left.service_id.cmp(&right.service_id));
        approved_services.dedup_by(|left, right| left.service_id == right.service_id);
        Self { approved_services }
    }

    /// Returns a stable digest for the current tunnel policy.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"cluster_tunnel_policy|");
        for service in &self.approved_services {
            hasher.update(service.stable_digest().as_bytes());
        }
        hex::encode(hasher.finalize())
    }
}

/// Direction of one active or historical tunnel record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterTunnelDirection {
    /// This node opened the tunnel to a remote peer.
    Outbound,
    /// This node accepted the tunnel from a remote peer.
    Inbound,
}

/// Lifecycle state for one tunnel record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterTunnelState {
    /// Tunnel open was sent but not yet acknowledged.
    Pending,
    /// Tunnel is currently open and usable.
    Open,
    /// Tunnel was refused during open.
    Refused,
    /// Tunnel failed while operating.
    Failed,
    /// Tunnel closed cleanly.
    Closed,
}

/// Stable refusal reason when a peer rejects one tunnel open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterTunnelOpenRefusalReason {
    /// The requested service ID is not approved by policy.
    ServiceUnknown,
    /// The service exists in policy but is not currently active.
    ServiceInactive,
    /// The service exists but does not allow the requesting peer.
    PeerNotAllowed,
    /// The underlying peer session has no remaining tunnel capacity.
    StreamCapacityExceeded,
    /// The requested tunnel protocol is not supported.
    ProtocolUnsupported,
}

/// Stable close reason for one tunnel lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterTunnelCloseReason {
    /// Closed explicitly by the local operator or caller.
    OperatorClosed,
    /// Closed explicitly by the remote peer.
    PeerClosed,
    /// Closed because the service was deactivated locally.
    ServiceDeactivated,
    /// Closed because the underlying peer session was unavailable.
    TransportUnavailable,
    /// Closed because request handling failed.
    RequestFailed,
}

/// Stable tunnel identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClusterTunnelId(u64);

impl ClusterTunnelId {
    fn new(value: u64) -> Self {
        Self(value)
    }
}

/// Stable request identifier inside one tunnel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClusterTunnelRequestId(u64);

impl ClusterTunnelRequestId {
    fn new(value: u64) -> Self {
        Self(value)
    }
}

/// One HTTP header forwarded through a bounded service tunnel.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClusterTunnelHttpHeader {
    /// Header name.
    pub name: String,
    /// Header value.
    pub value: String,
}

impl ClusterTunnelHttpHeader {
    /// Creates one forwarded HTTP header.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// Structured HTTP request that may be forwarded through one tunnel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterTunnelHttpRequest {
    /// HTTP method.
    pub method: String,
    /// Absolute path and query.
    pub path: String,
    /// Headers forwarded to the local service.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<ClusterTunnelHttpHeader>,
    /// Base64-encoded request body.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub body_base64: String,
}

impl ClusterTunnelHttpRequest {
    /// Creates one empty HTTP request.
    #[must_use]
    pub fn new(method: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            path: path.into(),
            headers: Vec::new(),
            body_base64: String::new(),
        }
    }

    /// Appends one HTTP header.
    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push(ClusterTunnelHttpHeader::new(name, value));
        self
    }

    /// Attaches one UTF-8 body.
    #[must_use]
    pub fn with_utf8_body(mut self, body: impl AsRef<str>) -> Self {
        self.body_base64 = BASE64_STANDARD.encode(body.as_ref().as_bytes());
        self
    }

    /// Attaches one raw body.
    #[must_use]
    pub fn with_body_bytes(mut self, body: impl AsRef<[u8]>) -> Self {
        self.body_base64 = BASE64_STANDARD.encode(body.as_ref());
        self
    }

    /// Decodes the request body bytes.
    pub fn body_bytes(&self) -> Result<Vec<u8>, ClusterTunnelError> {
        if self.body_base64.is_empty() {
            return Ok(Vec::new());
        }
        BASE64_STANDARD
            .decode(self.body_base64.as_bytes())
            .map_err(|error| ClusterTunnelError::BodyEncoding(error.to_string()))
    }
}

/// Structured HTTP response returned through one tunnel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterTunnelHttpResponse {
    /// HTTP status code.
    pub status_code: u16,
    /// HTTP reason phrase.
    pub reason_phrase: String,
    /// Headers returned by the local service.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<ClusterTunnelHttpHeader>,
    /// Base64-encoded response body.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub body_base64: String,
}

impl ClusterTunnelHttpResponse {
    /// Creates one HTTP response.
    #[must_use]
    pub fn new(status_code: u16, reason_phrase: impl Into<String>) -> Self {
        Self {
            status_code,
            reason_phrase: reason_phrase.into(),
            headers: Vec::new(),
            body_base64: String::new(),
        }
    }

    /// Appends one response header.
    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push(ClusterTunnelHttpHeader::new(name, value));
        self
    }

    /// Attaches one UTF-8 response body.
    #[must_use]
    pub fn with_utf8_body(mut self, body: impl AsRef<str>) -> Self {
        self.body_base64 = BASE64_STANDARD.encode(body.as_ref().as_bytes());
        self
    }

    /// Attaches one raw response body.
    #[must_use]
    pub fn with_body_bytes(mut self, body: impl AsRef<[u8]>) -> Self {
        self.body_base64 = BASE64_STANDARD.encode(body.as_ref());
        self
    }

    /// Decodes the response body bytes.
    pub fn body_bytes(&self) -> Result<Vec<u8>, ClusterTunnelError> {
        if self.body_base64.is_empty() {
            return Ok(Vec::new());
        }
        BASE64_STANDARD
            .decode(self.body_base64.as_bytes())
            .map_err(|error| ClusterTunnelError::BodyEncoding(error.to_string()))
    }

    fn bad_gateway(detail: impl Into<String>) -> Self {
        Self::new(502, "Bad Gateway")
            .with_header("content-type", "text/plain; charset=utf-8")
            .with_header("x-openagents-tunnel-error", "local_service_failed")
            .with_utf8_body(detail.into())
    }
}

/// Audit-friendly status for one approved service on the local node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterTunnelServiceSnapshot {
    /// Stable service identifier.
    pub service_id: String,
    /// Approved service category.
    pub kind: ClusterTunnelServiceKind,
    /// Protocol forwarded for this service.
    pub protocol: ClusterTunnelProtocol,
    /// Local TCP address serving the endpoint.
    pub local_addr: SocketAddr,
    /// Whether the operator currently exposes the service.
    pub active: bool,
    /// Peers explicitly allowed to open the service.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_peer_node_ids: Vec<NodeId>,
    /// Maximum request body bytes.
    pub max_request_body_bytes: usize,
    /// Maximum response body bytes.
    pub max_response_body_bytes: usize,
    /// Request count observed for the service.
    pub request_count: u64,
    /// Response count observed for the service.
    pub response_count: u64,
    /// Bytes accepted into the service.
    pub bytes_in: u64,
    /// Bytes returned from the service.
    pub bytes_out: u64,
    /// Latest activity timestamp when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_activity_ms: Option<u64>,
    /// Last operator-facing error when one was observed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// Transport observations for one tunnel-backed service path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterTunnelTransportObservation {
    /// Tunnel transport class.
    pub transport_class: ClusterTunnelTransportClass,
    /// Underlying authenticated peer-session path.
    pub session_path: ClusterTransportPath,
    /// Requests sent over the tunnel.
    pub requests_sent: u64,
    /// Requests received over the tunnel.
    pub requests_received: u64,
    /// Responses sent over the tunnel.
    pub responses_sent: u64,
    /// Responses received over the tunnel.
    pub responses_received: u64,
    /// Tunnel bytes sent.
    pub bytes_sent: u64,
    /// Tunnel bytes received.
    pub bytes_received: u64,
}

impl ClusterTunnelTransportObservation {
    fn new(session_path: ClusterTransportPath) -> Self {
        Self {
            transport_class: ClusterTunnelTransportClass::ServiceTunnelHttp,
            session_path,
            requests_sent: 0,
            requests_received: 0,
            responses_sent: 0,
            responses_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
        }
    }
}

/// One active or historical tunnel lease known to the local node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterTunnelSnapshot {
    /// Stable tunnel identifier.
    pub tunnel_id: ClusterTunnelId,
    /// Whether the tunnel was opened locally or accepted from a peer.
    pub direction: ClusterTunnelDirection,
    /// Remote peer bound to the tunnel.
    pub peer_node_id: NodeId,
    /// Service identifier exposed through the tunnel.
    pub service_id: String,
    /// Service category.
    pub service_kind: ClusterTunnelServiceKind,
    /// Forwarded protocol.
    pub protocol: ClusterTunnelProtocol,
    /// Current lifecycle state.
    pub state: ClusterTunnelState,
    /// Underlying transport observation for the tunnel.
    pub transport: ClusterTunnelTransportObservation,
    /// Logical stream reserved for the tunnel, when one is held.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logical_stream_id: Option<ClusterLogicalStreamId>,
    /// Timestamp when the tunnel was created locally.
    pub opened_at_ms: u64,
    /// Most recent activity timestamp, when one is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_activity_ms: Option<u64>,
    /// Close reason when the tunnel is no longer open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub close_reason: Option<ClusterTunnelCloseReason>,
    /// Most recent operator-facing error when one was observed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// Public lease returned when one outbound tunnel is open.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterTunnelLease {
    /// Stable tunnel identifier.
    pub tunnel_id: ClusterTunnelId,
    /// Remote peer serving the exposed endpoint.
    pub peer_node_id: NodeId,
    /// Service identifier served by the remote peer.
    pub service_id: String,
    /// Remote service category.
    pub service_kind: ClusterTunnelServiceKind,
    /// Forwarded protocol.
    pub protocol: ClusterTunnelProtocol,
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
    /// Required attestation facts for this peer when attested admission is active.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attestation_requirement: Option<NodeAttestationRequirement>,
    /// Relay endpoints that may assist in NAT-aware rendezvous for this peer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nat_rendezvous_relays: Vec<ClusterRelayEndpoint>,
    /// Relay endpoints that may carry the session when direct paths fail.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relay_fallback_relays: Vec<ClusterRelayEndpoint>,
    /// Honest logical stream capacity surfaced for this peer.
    #[serde(default = "default_max_concurrent_transport_streams")]
    pub max_concurrent_streams: u16,
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
            attestation_requirement: None,
            nat_rendezvous_relays: Vec::new(),
            relay_fallback_relays: Vec::new(),
            max_concurrent_streams: default_max_concurrent_transport_streams(),
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

    /// Requires explicit attestation facts for this peer under attested admission.
    #[must_use]
    pub fn with_attestation_requirement(
        mut self,
        attestation_requirement: NodeAttestationRequirement,
    ) -> Self {
        self.attestation_requirement = Some(attestation_requirement);
        self
    }

    /// Attaches relay-assisted rendezvous endpoints for NAT-aware establishment.
    #[must_use]
    pub fn with_nat_rendezvous_relays(
        mut self,
        nat_rendezvous_relays: Vec<ClusterRelayEndpoint>,
    ) -> Self {
        self.nat_rendezvous_relays = nat_rendezvous_relays;
        self
    }

    /// Attaches relay-forwarding fallback endpoints for this peer.
    #[must_use]
    pub fn with_relay_fallback_relays(
        mut self,
        relay_fallback_relays: Vec<ClusterRelayEndpoint>,
    ) -> Self {
        self.relay_fallback_relays = relay_fallback_relays;
        self
    }

    /// Overrides the logical stream capacity surfaced for this peer.
    #[must_use]
    pub fn with_max_concurrent_streams(mut self, max_concurrent_streams: u16) -> Self {
        self.max_concurrent_streams = max_concurrent_streams.max(1);
        self
    }

    fn multiplex_profile(&self) -> ClusterSessionMultiplexProfile {
        ClusterSessionMultiplexProfile::new(self.max_concurrent_streams.max(1))
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
    /// Transport path currently carrying the session, when one is established.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_transport: Option<ClusterTransportPath>,
    /// Honest logical stream capacity exposed for the peer.
    pub multiplex_profile: ClusterSessionMultiplexProfile,
    /// Currently reserved logical stream count.
    pub active_streams: u16,
    /// Approximate hello round-trip latency when one was measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_round_trip_latency_ms: Option<u64>,
    /// Count of datagrams sent while trying or maintaining the session.
    pub messages_sent: u64,
    /// Count of datagrams received while trying or maintaining the session.
    pub messages_received: u64,
    /// Bytes sent while trying or maintaining the session.
    pub bytes_sent: u64,
    /// Bytes received while trying or maintaining the session.
    pub bytes_received: u64,
    /// Most recent machine-checkable establishment failure, when one was observed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_establishment_failure: Option<ClusterSessionFailure>,
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
            active_transport: None,
            multiplex_profile: peer.multiplex_profile(),
            active_streams: 0,
            last_round_trip_latency_ms: None,
            messages_sent: 0,
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            last_establishment_failure: None,
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

/// Current non-LAN discovery disposition derived from cluster policy truth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterNonLanDiscoveryDisposition {
    /// The current discovery posture is still bounded and not ready for wider-network claims.
    Refused,
    /// The current discovery posture is explicit enough for wider-network claims.
    Eligible,
}

/// Explicit refusal reasons for non-LAN discovery claims.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterNonLanDiscoveryRefusalReason {
    /// Discovery remains tied to LAN-scoped seed peers.
    TrustedLanSeedPeersOnly,
    /// Discovery remains tied to operator-managed configured peers.
    OperatorManagedConfiguredPeersOnly,
    /// A wider-network discovery posture was requested, but the transport/runtime seam is not implemented yet.
    WiderNetworkDiscoveryUnimplemented,
}

/// Machine-checkable assessment for whether a cluster policy supports non-LAN discovery claims.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterNonLanDiscoveryAssessment {
    /// Discovery posture active for the underlying cluster policy.
    pub discovery_posture: ClusterDiscoveryPosture,
    /// Trust posture active for the underlying cluster policy.
    pub trust_posture: ClusterTrustPosture,
    /// Stable digest of the underlying trust policy.
    pub trust_policy_digest: String,
    /// Effective non-LAN discovery disposition for the current policy.
    pub disposition: ClusterNonLanDiscoveryDisposition,
    /// Explicit reasons why wider-network discovery claims remain refused.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refusal_reasons: Vec<ClusterNonLanDiscoveryRefusalReason>,
}

impl ClusterNonLanDiscoveryAssessment {
    /// Returns a stable digest for the current non-LAN discovery assessment.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"cluster_non_lan_discovery_assessment|");
        hasher.update(match self.discovery_posture {
            ClusterDiscoveryPosture::TrustedLanSeedPeers => b"trusted_lan_seed_peers".as_slice(),
            ClusterDiscoveryPosture::OperatorManagedConfiguredPeers => {
                b"operator_managed_configured_peers".as_slice()
            }
            ClusterDiscoveryPosture::ExplicitWiderNetworkRequested => {
                b"explicit_wider_network_requested".as_slice()
            }
        });
        hasher.update(b"|");
        hasher.update(match self.trust_posture {
            ClusterTrustPosture::TrustedLanSharedAdmission => {
                b"trusted_lan_shared_admission".as_slice()
            }
            ClusterTrustPosture::AuthenticatedConfiguredPeers => {
                b"authenticated_configured_peers".as_slice()
            }
            ClusterTrustPosture::AttestedConfiguredPeers => b"attested_configured_peers".as_slice(),
        });
        hasher.update(b"|");
        hasher.update(self.trust_policy_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(match self.disposition {
            ClusterNonLanDiscoveryDisposition::Refused => b"refused".as_slice(),
            ClusterNonLanDiscoveryDisposition::Eligible => b"eligible".as_slice(),
        });
        for refusal_reason in &self.refusal_reasons {
            hasher.update(b"|refusal|");
            hasher.update(match refusal_reason {
                ClusterNonLanDiscoveryRefusalReason::TrustedLanSeedPeersOnly => {
                    b"trusted_lan_seed_peers_only".as_slice()
                }
                ClusterNonLanDiscoveryRefusalReason::OperatorManagedConfiguredPeersOnly => {
                    b"operator_managed_configured_peers_only".as_slice()
                }
                ClusterNonLanDiscoveryRefusalReason::WiderNetworkDiscoveryUnimplemented => {
                    b"wider_network_discovery_unimplemented".as_slice()
                }
            });
        }
        hex::encode(hasher.finalize())
    }
}

/// Machine-checkable trust policy for one local cluster transport.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterTrustPolicy {
    /// Trust posture active for this configuration.
    pub posture: ClusterTrustPosture,
    /// Discovery posture active for this configuration.
    pub discovery_posture: ClusterDiscoveryPosture,
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
            discovery_posture: ClusterDiscoveryPosture::TrustedLanSeedPeers,
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
            discovery_posture: ClusterDiscoveryPosture::OperatorManagedConfiguredPeers,
            require_message_authentication: true,
            replay_window_size: DEFAULT_REPLAY_WINDOW_SIZE,
            trust_bundle_version: 1,
            accepted_trust_bundle_versions: Vec::new(),
            configured_peers,
            configured_peer_dial_policy: ConfiguredPeerDialPolicy::operator_managed_default(),
        }
    }

    /// Trust policy for attestation-aware configured peers across wider networks.
    #[must_use]
    pub fn attested_configured_peers(configured_peers: Vec<ConfiguredClusterPeer>) -> Self {
        Self {
            posture: ClusterTrustPosture::AttestedConfiguredPeers,
            discovery_posture: ClusterDiscoveryPosture::OperatorManagedConfiguredPeers,
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

    /// Overrides the discovery posture for this cluster config.
    #[must_use]
    pub fn with_discovery_posture(mut self, discovery_posture: ClusterDiscoveryPosture) -> Self {
        self.discovery_posture = discovery_posture;
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
            ClusterTrustPosture::AttestedConfiguredPeers => b"attested_configured_peers".as_slice(),
        });
        hasher.update(b"|");
        hasher.update(match self.discovery_posture {
            ClusterDiscoveryPosture::TrustedLanSeedPeers => b"trusted_lan_seed_peers".as_slice(),
            ClusterDiscoveryPosture::OperatorManagedConfiguredPeers => {
                b"operator_managed_configured_peers".as_slice()
            }
            ClusterDiscoveryPosture::ExplicitWiderNetworkRequested => {
                b"explicit_wider_network_requested".as_slice()
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
            hasher.update(b"|max_streams|");
            hasher.update(peer.max_concurrent_streams.to_string().as_bytes());
            for previous_key in &peer.previous_auth_public_keys {
                hasher.update(b"|previous_key|");
                hasher.update(previous_key.as_bytes());
            }
            if let Some(attestation_requirement) = &peer.attestation_requirement {
                hasher.update(b"|attestation_issuer|");
                hasher.update(attestation_requirement.issuer.as_bytes());
                hasher.update(b"|attestation_digest|");
                hasher.update(attestation_requirement.attestation_digest.as_bytes());
                if let Some(device_identity_digest) =
                    &attestation_requirement.device_identity_digest
                {
                    hasher.update(b"|device_identity_digest|");
                    hasher.update(device_identity_digest.as_bytes());
                }
            }
            for relay in &peer.nat_rendezvous_relays {
                hasher.update(b"|nat_relay|");
                hasher.update(relay.relay_id.as_bytes());
                hasher.update(b"|");
                hasher.update(relay.relay_addr.to_string().as_bytes());
                hasher.update(b"|");
                hasher.update(relay.session_tag.as_bytes());
            }
            for relay in &peer.relay_fallback_relays {
                hasher.update(b"|relay_fallback|");
                hasher.update(relay.relay_id.as_bytes());
                hasher.update(b"|");
                hasher.update(relay.relay_addr.to_string().as_bytes());
                hasher.update(b"|");
                hasher.update(relay.session_tag.as_bytes());
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

    /// Derives the current wider-network discovery posture from the shipped cluster policy.
    #[must_use]
    pub fn non_lan_discovery_assessment(&self) -> ClusterNonLanDiscoveryAssessment {
        let refusal_reasons = match self.discovery_posture {
            ClusterDiscoveryPosture::TrustedLanSeedPeers => {
                vec![ClusterNonLanDiscoveryRefusalReason::TrustedLanSeedPeersOnly]
            }
            ClusterDiscoveryPosture::OperatorManagedConfiguredPeers => {
                vec![ClusterNonLanDiscoveryRefusalReason::OperatorManagedConfiguredPeersOnly]
            }
            ClusterDiscoveryPosture::ExplicitWiderNetworkRequested => {
                vec![ClusterNonLanDiscoveryRefusalReason::WiderNetworkDiscoveryUnimplemented]
            }
        };
        ClusterNonLanDiscoveryAssessment {
            discovery_posture: self.discovery_posture,
            trust_posture: self.posture,
            trust_policy_digest: self.stable_digest(),
            disposition: if refusal_reasons.is_empty() {
                ClusterNonLanDiscoveryDisposition::Eligible
            } else {
                ClusterNonLanDiscoveryDisposition::Refused
            },
            refusal_reasons,
        }
    }

    /// Derives the current compute-market trust posture from the shipped cluster policy.
    #[must_use]
    pub fn compute_market_trust_assessment(&self) -> ClusterComputeMarketTrustAssessment {
        let discovery_assessment = self.non_lan_discovery_assessment();
        let mut refusal_reasons = Vec::new();
        match self.posture {
            ClusterTrustPosture::TrustedLanSharedAdmission => {
                refusal_reasons
                    .push(ClusterComputeMarketTrustRefusalReason::TrustedLanSharedAdmissionOnly);
                if !self.require_message_authentication {
                    refusal_reasons.push(
                        ClusterComputeMarketTrustRefusalReason::MissingAuthenticatedTransport,
                    );
                }
                refusal_reasons.push(
                    ClusterComputeMarketTrustRefusalReason::MissingAttestedNodeIdentityAdmission,
                );
            }
            ClusterTrustPosture::AuthenticatedConfiguredPeers => {
                refusal_reasons.push(
                    ClusterComputeMarketTrustRefusalReason::OperatorManagedConfiguredPeersOnly,
                );
                refusal_reasons.push(
                    ClusterComputeMarketTrustRefusalReason::MissingAttestedNodeIdentityAdmission,
                );
            }
            ClusterTrustPosture::AttestedConfiguredPeers => {
                if self
                    .configured_peers
                    .iter()
                    .any(|peer| peer.attestation_requirement.is_none())
                {
                    refusal_reasons.push(
                        ClusterComputeMarketTrustRefusalReason::MissingAttestedNodeIdentityAdmission,
                    );
                }
            }
        }
        if !self.require_message_authentication
            && !refusal_reasons
                .contains(&ClusterComputeMarketTrustRefusalReason::MissingAuthenticatedTransport)
        {
            refusal_reasons
                .push(ClusterComputeMarketTrustRefusalReason::MissingAuthenticatedTransport);
        }
        if matches!(
            discovery_assessment.disposition,
            ClusterNonLanDiscoveryDisposition::Refused
        ) {
            refusal_reasons
                .push(ClusterComputeMarketTrustRefusalReason::MissingNonLanDiscoveryPosture);
        }
        ClusterComputeMarketTrustAssessment {
            posture: self.posture,
            discovery_posture: self.discovery_posture,
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

/// Persistence policy for durable wider-network identity and trust state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClusterNetworkStatePersistence {
    /// Keep candidate and trust state in memory only.
    Ephemeral,
    /// Persist candidate and trust state in one local JSON file.
    FileBacked { path: PathBuf },
}

impl Default for ClusterNetworkStatePersistence {
    fn default() -> Self {
        Self::Ephemeral
    }
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
    /// Attestation facts carried by this node when attested admission is active.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attestation: Option<NodeAttestationEvidence>,
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
    /// Durable wider-network trust/candidate-state persistence for this node.
    pub network_state_persistence: ClusterNetworkStatePersistence,
    /// Optional local attestation facts to attach to node identity.
    pub node_attestation: Option<NodeAttestationEvidence>,
    /// Optional operator-managed policy for future wider-network introductions.
    pub introduction_policy: Option<ClusterIntroductionPolicy>,
    /// Operator-managed policy for bounded local service tunnels.
    pub tunnel_policy: ClusterTunnelPolicy,
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
            network_state_persistence: ClusterNetworkStatePersistence::Ephemeral,
            node_attestation: None,
            introduction_policy: None,
            tunnel_policy: ClusterTunnelPolicy::default(),
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

    /// Attaches a file-backed durable network-state store for trust and candidate history.
    #[must_use]
    pub fn with_file_backed_network_state(mut self, path: PathBuf) -> Self {
        self.network_state_persistence = ClusterNetworkStatePersistence::FileBacked { path };
        self
    }

    /// Attaches explicit node-attestation facts for this node identity.
    #[must_use]
    pub fn with_node_attestation(mut self, node_attestation: NodeAttestationEvidence) -> Self {
        self.node_attestation = Some(node_attestation);
        self
    }

    /// Attaches an operator-managed introduction policy for future wider-network discovery.
    #[must_use]
    pub fn with_introduction_policy(
        mut self,
        introduction_policy: ClusterIntroductionPolicy,
    ) -> Self {
        self.introduction_policy = Some(introduction_policy);
        self
    }

    /// Attaches an operator-managed policy for bounded service tunnels.
    #[must_use]
    pub fn with_tunnel_policy(mut self, tunnel_policy: ClusterTunnelPolicy) -> Self {
        self.tunnel_policy = tunnel_policy;
        self
    }

    /// Returns the current introduction policy, when one is configured.
    #[must_use]
    pub fn introduction_policy(&self) -> Option<&ClusterIntroductionPolicy> {
        self.introduction_policy.as_ref()
    }

    /// Returns the current discovery posture for this node config.
    #[must_use]
    pub const fn discovery_posture(&self) -> ClusterDiscoveryPosture {
        self.trust_policy.discovery_posture
    }

    /// Returns the current non-LAN discovery assessment for this node config.
    #[must_use]
    pub fn non_lan_discovery_assessment(&self) -> ClusterNonLanDiscoveryAssessment {
        self.trust_policy.non_lan_discovery_assessment()
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

    /// Attaches attested configured peers and seeds discovery from them.
    #[must_use]
    pub fn with_attested_configured_peers(
        mut self,
        configured_peers: Vec<ConfiguredClusterPeer>,
    ) -> Self {
        self.seed_peers = configured_peers
            .iter()
            .map(|peer| peer.remote_addr)
            .collect();
        self.trust_policy = ClusterTrustPolicy::attested_configured_peers(configured_peers);
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

    /// Overrides the cluster discovery posture without changing the current transport seam.
    #[must_use]
    pub fn with_discovery_posture(mut self, discovery_posture: ClusterDiscoveryPosture) -> Self {
        self.trust_policy = self
            .trust_policy
            .clone()
            .with_discovery_posture(discovery_posture);
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
pub struct ClusterPeerAttestationMismatch {
    /// Expected attestation issuer.
    pub expected_issuer: String,
    /// Observed attestation issuer, when one was supplied.
    pub actual_issuer: Option<String>,
    /// Expected attestation digest.
    pub expected_attestation_digest: String,
    /// Observed attestation digest, when one was supplied.
    pub actual_attestation_digest: Option<String>,
    /// Expected device or host identity digest, when one was required.
    pub expected_device_identity_digest: Option<String>,
    /// Observed device or host identity digest, when one was supplied.
    pub actual_device_identity_digest: Option<String>,
}

impl ClusterPeerAttestationMismatch {
    fn between(expected: &NodeAttestationRequirement, actual: &NodeAttestationEvidence) -> Self {
        Self {
            expected_issuer: expected.issuer.clone(),
            actual_issuer: Some(actual.issuer.clone()),
            expected_attestation_digest: expected.attestation_digest.clone(),
            actual_attestation_digest: Some(actual.attestation_digest.clone()),
            expected_device_identity_digest: expected.device_identity_digest.clone(),
            actual_device_identity_digest: actual.device_identity_digest.clone(),
        }
    }
}

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
    /// The configured peer lacked an attestation requirement under attested admission.
    ConfiguredPeerAttestationRequirementMissing,
    /// The remote peer did not advertise attestation facts when attested admission required them.
    NodeAttestationMissing,
    /// The remote peer attestation did not match the configured requirement.
    ConfiguredPeerAttestationMismatch(Box<ClusterPeerAttestationMismatch>),
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
    /// Selected path and observed transport metrics for the peer.
    pub transport: ClusterTransportObservation,
}

/// Stable logical-stream purpose carried over one peer session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterLogicalStreamKind {
    /// Control-plane RPC or metadata stream.
    Control,
    /// Serving or request/response payload stream.
    Serving,
    /// Policy-gated service tunnel stream.
    Tunnel,
    /// Collective or synchronization stream.
    Collective,
    /// Artifact or checkpoint transfer stream.
    Artifact,
}

/// Stable logical-stream identifier reserved on one peer session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClusterLogicalStreamId(u64);

impl ClusterLogicalStreamId {
    fn new(value: u64) -> Self {
        Self(value)
    }
}

/// Reserved logical stream on one established peer session.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClusterLogicalStreamLease {
    /// Peer that owns the session.
    pub peer_node_id: NodeId,
    /// Stable logical stream identifier.
    pub stream_id: ClusterLogicalStreamId,
    /// Declared purpose for the stream.
    pub kind: ClusterLogicalStreamKind,
}

/// Failure returned when reserving or releasing one logical stream.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ClusterStreamError {
    /// The requested peer is not currently connected.
    #[error("peer {peer_node_id:?} is not connected")]
    PeerNotConnected {
        /// Peer that was requested.
        peer_node_id: NodeId,
    },
    /// The session has no remaining logical-stream capacity.
    #[error("peer {peer_node_id:?} reached its logical-stream capacity ({max_concurrent_streams})")]
    CapacityExceeded {
        /// Peer that hit the limit.
        peer_node_id: NodeId,
        /// Maximum allowed logical streams.
        max_concurrent_streams: u16,
    },
    /// The supplied logical stream was not active.
    #[error("logical stream {stream_id:?} is not active for peer {peer_node_id:?}")]
    StreamNotActive {
        /// Peer that was addressed.
        peer_node_id: NodeId,
        /// Stream that could not be found.
        stream_id: ClusterLogicalStreamId,
    },
}

/// Failure surfaced by the bounded service-tunnel substrate.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ClusterTunnelError {
    /// The requested peer is not currently connected.
    #[error("peer {peer_node_id:?} is not connected")]
    PeerNotConnected {
        /// Peer that was addressed.
        peer_node_id: NodeId,
    },
    /// The requested service is not approved by policy.
    #[error("tunnel service `{service_id}` is not approved")]
    ServiceUnknown {
        /// Service identifier that was requested.
        service_id: String,
    },
    /// The requested service is approved but not currently active.
    #[error("tunnel service `{service_id}` is not active")]
    ServiceNotActive {
        /// Service identifier that was requested.
        service_id: String,
    },
    /// The requested service refused the supplied peer.
    #[error("tunnel service `{service_id}` does not allow peer {peer_node_id:?}")]
    PeerNotAllowed {
        /// Service identifier that refused the peer.
        service_id: String,
        /// Peer that was refused.
        peer_node_id: NodeId,
    },
    /// The tunnel could not reserve a logical stream.
    #[error(transparent)]
    Stream(#[from] ClusterStreamError),
    /// One referenced tunnel record is not tracked locally.
    #[error("tunnel {tunnel_id:?} is not tracked")]
    TunnelUnknown {
        /// Tunnel that was requested.
        tunnel_id: ClusterTunnelId,
    },
    /// One referenced tunnel is not open.
    #[error("tunnel {tunnel_id:?} is not open")]
    TunnelNotOpen {
        /// Tunnel that was requested.
        tunnel_id: ClusterTunnelId,
    },
    /// The tunnel open was refused by the remote peer.
    #[error("tunnel open for service `{service_id}` was refused: {reason:?}")]
    OpenRefused {
        /// Requested service identifier.
        service_id: String,
        /// Stable refusal reason.
        reason: ClusterTunnelOpenRefusalReason,
    },
    /// The HTTP payload exceeded the configured limit.
    #[error("HTTP payload size {actual_bytes} exceeds allowed maximum {maximum_bytes}")]
    PayloadTooLarge {
        /// Allowed size.
        maximum_bytes: usize,
        /// Actual observed size.
        actual_bytes: usize,
    },
    /// The HTTP body could not be decoded from base64.
    #[error("failed to decode tunnel HTTP body: {0}")]
    BodyEncoding(String),
    /// The local service I/O failed.
    #[error("local tunnel service I/O failed: {0}")]
    LocalServiceIo(String),
    /// The local service returned a malformed HTTP payload.
    #[error("local tunnel service protocol error: {0}")]
    LocalServiceProtocol(String),
    /// The transport background task is no longer available.
    #[error("cluster transport background task is offline")]
    TransportOffline,
}

/// Durable candidate disposition tracked by Psionic Net.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterCandidateDisposition {
    /// Candidate was verified from one signed introduction.
    Introduced,
    /// Candidate was refused by local admission or policy logic.
    Refused,
    /// Candidate was promoted into a stronger admitted posture.
    Promoted,
    /// Candidate was explicitly revoked by operator or policy.
    Revoked,
    /// Candidate introduction expired before promotion.
    Expired,
}

/// Stable reason code for one durable candidate-history event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterCandidateHistoryReasonCode {
    /// One signed introduction verified under the current introduction policy.
    VerifiedIntroduction,
    /// Local policy or admission logic refused the candidate.
    AdmissionRefused,
    /// The candidate was promoted for later admission or membership work.
    Promoted,
    /// The candidate was explicitly revoked.
    Revoked,
    /// The candidate introduction expired.
    Expired,
}

/// Durable event in one candidate's history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterCandidateHistoryEvent {
    /// Inclusive event timestamp.
    pub occurred_at_ms: u64,
    /// Resulting candidate disposition.
    pub disposition: ClusterCandidateDisposition,
    /// Stable reason code.
    pub reason_code: ClusterCandidateHistoryReasonCode,
    /// Stable digest of the trust policy active when the event was recorded.
    pub trust_policy_digest: String,
    /// Stable digest of the introduction policy, when one verified introduction backed the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub introduction_policy_digest: Option<String>,
    /// Stable digest of the introduction payload, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub introduction_payload_digest: Option<String>,
    /// Optional machine-readable detail string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Persisted verified introduction that currently backs one candidate record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedClusterIntroductionRecord {
    /// Signed introduction envelope.
    pub envelope: SignedClusterIntroductionEnvelope,
    /// Stable digest of the introduction policy used to verify the envelope.
    pub introduction_policy_digest: String,
    /// Timestamp when local verification accepted the introduction.
    pub verified_at_ms: u64,
}

/// Durable record for one wider-network candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterCandidateRecord {
    /// Stable candidate node identity.
    pub node_id: NodeId,
    /// Stable digest of the latest candidate descriptor.
    pub candidate_digest: String,
    /// Latest candidate descriptor.
    pub candidate: ClusterDiscoveryCandidate,
    /// Current durable disposition.
    pub disposition: ClusterCandidateDisposition,
    /// Inclusive timestamp of the most recent event.
    pub last_updated_ms: u64,
    /// Verified introduction that currently backs the candidate, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_introduction: Option<PersistedClusterIntroductionRecord>,
    /// Durable event history for the candidate.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<ClusterCandidateHistoryEvent>,
}

/// Durable snapshot of one trust-bundle revision observed by Psionic Net.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterTrustBundleRecord {
    /// Stable digest for the recorded trust policy.
    pub trust_policy_digest: String,
    /// Full trust policy snapshot.
    pub trust_policy: ClusterTrustPolicy,
    /// Inclusive timestamp when this revision became active locally.
    pub recorded_at_ms: u64,
    /// Timestamp when a later revision superseded this one, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superseded_at_ms: Option<u64>,
}

/// Durable wider-network identity and trust state owned by Psionic Net.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedClusterNetworkState {
    /// Explicit schema version for upgrades.
    pub schema_version: u32,
    /// Trust bundle history observed for the local node configuration.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trust_bundles: Vec<ClusterTrustBundleRecord>,
    /// Candidate records keyed by node identity.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub candidates: BTreeMap<NodeId, ClusterCandidateRecord>,
}

impl Default for PersistedClusterNetworkState {
    fn default() -> Self {
        Self::empty()
    }
}

impl PersistedClusterNetworkState {
    /// Current schema version for durable network state.
    pub const SCHEMA_VERSION: u32 = 1;

    fn empty() -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            trust_bundles: Vec::new(),
            candidates: BTreeMap::new(),
        }
    }

    fn load_json(path: impl AsRef<std::path::Path>) -> Result<Self, ClusterError> {
        let bytes = fs::read(path).map_err(ClusterError::NetworkStateIo)?;
        let state: Self =
            serde_json::from_slice(&bytes).map_err(ClusterError::NetworkStateFormat)?;
        if state.schema_version != Self::SCHEMA_VERSION {
            return Err(ClusterError::NetworkStateSchemaVersion {
                expected: Self::SCHEMA_VERSION,
                actual: state.schema_version,
            });
        }
        Ok(state)
    }

    fn store_json(&self, path: impl AsRef<std::path::Path>) -> Result<(), ClusterError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(ClusterError::NetworkStateIo)?;
        }
        let encoded = serde_json::to_vec_pretty(self).map_err(ClusterError::NetworkStateFormat)?;
        fs::write(path, encoded).map_err(ClusterError::NetworkStateIo)
    }

    fn record_trust_bundle(&mut self, trust_policy: ClusterTrustPolicy, recorded_at_ms: u64) {
        let trust_policy_digest = trust_policy.stable_digest();
        if self
            .trust_bundles
            .last()
            .is_some_and(|record| record.trust_policy_digest == trust_policy_digest)
        {
            return;
        }
        if let Some(previous) = self.trust_bundles.last_mut() {
            previous.superseded_at_ms = Some(recorded_at_ms);
        }
        self.trust_bundles.push(ClusterTrustBundleRecord {
            trust_policy_digest,
            trust_policy,
            recorded_at_ms,
            superseded_at_ms: None,
        });
    }
}

/// Running local-cluster node for the first hello/ping seam.
pub struct LocalClusterNode {
    local_addr: SocketAddr,
    local_identity: ClusterNodeIdentity,
    trust_policy: ClusterTrustPolicy,
    introduction_policy: Option<ClusterIntroductionPolicy>,
    tunnel_policy: ClusterTunnelPolicy,
    state: Arc<Mutex<SharedState>>,
    transport_command_tx: mpsc::UnboundedSender<TransportCommand>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), String>>>,
}

impl LocalClusterNode {
    /// Starts the first local-cluster hello/ping transport.
    pub async fn spawn(config: LocalClusterConfig) -> Result<Self, ClusterError> {
        let loaded_identity = load_or_create_local_identity(&config)?;
        let local_identity = loaded_identity.identity.clone();
        let transport_config = TransportConfig::from_config(config, loaded_identity);
        let durable_network_state = load_or_create_network_state(
            &transport_config.network_state_persistence,
            &transport_config.trust_policy,
        )?;
        let trust_policy = transport_config.trust_policy.clone();
        let introduction_policy = transport_config.introduction_policy.clone();
        let socket = Arc::new(
            UdpSocket::bind(transport_config.bind_addr)
                .await
                .map_err(ClusterError::Bind)?,
        );
        let local_addr = socket.local_addr().map_err(ClusterError::LocalAddr)?;
        let tunnel_policy = transport_config.tunnel_policy.clone();
        let state = Arc::new(Mutex::new(SharedState::new(
            transport_config.seed_peers.clone(),
            &transport_config.trust_policy,
            &transport_config.tunnel_policy,
            durable_network_state,
            transport_config.network_state_persistence.clone(),
        )));
        let (transport_command_tx, transport_command_rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(run_transport(
            socket,
            state.clone(),
            transport_config,
            transport_command_rx,
            shutdown_rx,
        ));
        Ok(Self {
            local_addr,
            local_identity,
            trust_policy,
            introduction_policy,
            tunnel_policy,
            state,
            transport_command_tx,
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

    /// Returns the current introduction policy for this node, when one is configured.
    #[must_use]
    pub fn introduction_policy(&self) -> Option<&ClusterIntroductionPolicy> {
        self.introduction_policy.as_ref()
    }

    /// Returns the current bounded tunnel policy for this node.
    #[must_use]
    pub fn tunnel_policy(&self) -> &ClusterTunnelPolicy {
        &self.tunnel_policy
    }

    /// Returns the current discovery posture for this node.
    #[must_use]
    pub const fn discovery_posture(&self) -> ClusterDiscoveryPosture {
        self.trust_policy.discovery_posture
    }

    /// Returns the current non-LAN discovery assessment for this node.
    #[must_use]
    pub fn non_lan_discovery_assessment(&self) -> ClusterNonLanDiscoveryAssessment {
        self.trust_policy.non_lan_discovery_assessment()
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

    /// Returns the full durable wider-network trust and candidate state snapshot.
    pub async fn durable_network_state(&self) -> PersistedClusterNetworkState {
        self.state.lock().await.durable_network_state()
    }

    /// Returns the durable trust-bundle history for this node configuration.
    pub async fn trust_bundle_history(&self) -> Vec<ClusterTrustBundleRecord> {
        self.state.lock().await.trust_bundle_history()
    }

    /// Returns the durable candidate records known to this node.
    pub async fn candidate_records(&self) -> Vec<ClusterCandidateRecord> {
        self.state.lock().await.candidate_records()
    }

    /// Verifies and records one signed candidate introduction in durable state.
    pub async fn record_verified_candidate_introduction(
        &self,
        envelope: SignedClusterIntroductionEnvelope,
        verified_at_ms: u64,
    ) -> Result<ClusterCandidateRecord, ClusterError> {
        let Some(policy) = self.introduction_policy.as_ref() else {
            return Err(ClusterError::IntroductionPolicyMissing);
        };
        envelope
            .verify(policy)
            .map_err(|error| ClusterError::Runtime(error.to_string()))?;
        self.state.lock().await.apply_verified_introduction(
            envelope,
            policy,
            verified_at_ms,
            &self.trust_policy,
        )
    }

    /// Records an explicit refusal outcome for one known candidate.
    pub async fn record_candidate_refusal(
        &self,
        node_id: &NodeId,
        occurred_at_ms: u64,
        detail: impl Into<String>,
    ) -> Result<ClusterCandidateRecord, ClusterError> {
        self.state.lock().await.record_candidate_disposition(
            node_id,
            ClusterCandidateDisposition::Refused,
            ClusterCandidateHistoryReasonCode::AdmissionRefused,
            occurred_at_ms,
            &self.trust_policy,
            Some(detail.into()),
        )
    }

    /// Records an explicit promotion outcome for one known candidate.
    pub async fn promote_candidate(
        &self,
        node_id: &NodeId,
        occurred_at_ms: u64,
        detail: impl Into<String>,
    ) -> Result<ClusterCandidateRecord, ClusterError> {
        self.state.lock().await.record_candidate_disposition(
            node_id,
            ClusterCandidateDisposition::Promoted,
            ClusterCandidateHistoryReasonCode::Promoted,
            occurred_at_ms,
            &self.trust_policy,
            Some(detail.into()),
        )
    }

    /// Records an explicit revocation outcome for one known candidate.
    pub async fn revoke_candidate(
        &self,
        node_id: &NodeId,
        occurred_at_ms: u64,
        detail: impl Into<String>,
    ) -> Result<ClusterCandidateRecord, ClusterError> {
        self.state.lock().await.record_candidate_disposition(
            node_id,
            ClusterCandidateDisposition::Revoked,
            ClusterCandidateHistoryReasonCode::Revoked,
            occurred_at_ms,
            &self.trust_policy,
            Some(detail.into()),
        )
    }

    /// Expires any tracked candidates whose latest verified introduction is no longer valid.
    pub async fn expire_candidates(
        &self,
        now_ms: u64,
    ) -> Result<Vec<ClusterCandidateRecord>, ClusterError> {
        self.state
            .lock()
            .await
            .expire_candidates(now_ms, &self.trust_policy)
    }

    /// Returns currently reserved logical streams across all connected peers.
    pub async fn active_logical_streams(&self) -> Vec<ClusterLogicalStreamLease> {
        self.state.lock().await.active_logical_streams()
    }

    /// Returns operator-facing snapshots for approved tunnel services.
    pub async fn tunnel_service_snapshots(&self) -> Vec<ClusterTunnelServiceSnapshot> {
        self.state.lock().await.tunnel_service_snapshots()
    }

    /// Returns operator-facing snapshots for active or historical tunnels.
    pub async fn tunnel_snapshots(&self) -> Vec<ClusterTunnelSnapshot> {
        self.state.lock().await.tunnel_snapshots()
    }

    /// Activates one approved service so peers may open a bounded tunnel to it.
    pub async fn activate_tunnel_service(
        &self,
        service_id: &str,
    ) -> Result<ClusterTunnelServiceSnapshot, ClusterTunnelError> {
        self.state.lock().await.activate_tunnel_service(service_id)
    }

    /// Deactivates one approved service and closes any related active tunnels.
    pub async fn deactivate_tunnel_service(
        &self,
        service_id: &str,
    ) -> Result<ClusterTunnelServiceSnapshot, ClusterTunnelError> {
        let closures = self
            .state
            .lock()
            .await
            .deactivate_tunnel_service(service_id)?;
        for closure in closures {
            let _ = self
                .transport_command_tx
                .send(TransportCommand::SendTunnelClose {
                    peer_node_id: closure.peer_node_id,
                    tunnel_id: closure.tunnel_id,
                    reason: ClusterTunnelCloseReason::ServiceDeactivated,
                    detail: Some(String::from("service_deactivated")),
                });
        }
        self.state
            .lock()
            .await
            .tunnel_service_snapshot(service_id)
            .ok_or_else(|| ClusterTunnelError::ServiceUnknown {
                service_id: service_id.to_owned(),
            })
    }

    /// Opens one outbound HTTP tunnel to a selected service on one connected peer.
    pub async fn open_http_tunnel(
        &self,
        peer_node_id: &NodeId,
        service_id: impl Into<String>,
    ) -> Result<ClusterTunnelLease, ClusterTunnelError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.transport_command_tx
            .send(TransportCommand::OpenTunnel {
                peer_node_id: peer_node_id.clone(),
                service_id: service_id.into(),
                response_tx,
            })
            .map_err(|_| ClusterTunnelError::TransportOffline)?;
        response_rx
            .await
            .map_err(|_| ClusterTunnelError::TransportOffline)?
    }

    /// Forwards one HTTP request over an open tunnel and waits for the response.
    pub async fn send_tunneled_http_request(
        &self,
        lease: &ClusterTunnelLease,
        request: ClusterTunnelHttpRequest,
    ) -> Result<ClusterTunnelHttpResponse, ClusterTunnelError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.transport_command_tx
            .send(TransportCommand::SendTunnelHttpRequest {
                tunnel_id: lease.tunnel_id,
                request,
                response_tx,
            })
            .map_err(|_| ClusterTunnelError::TransportOffline)?;
        response_rx
            .await
            .map_err(|_| ClusterTunnelError::TransportOffline)?
    }

    /// Closes one open tunnel and releases its logical stream.
    pub async fn close_tunnel(&self, lease: &ClusterTunnelLease) -> Result<(), ClusterTunnelError> {
        self.state.lock().await.close_tunnel_and_prepare_dispatch(
            lease.tunnel_id,
            ClusterTunnelCloseReason::OperatorClosed,
            Some(String::from("local_close")),
        )?;
        self.transport_command_tx
            .send(TransportCommand::SendTunnelClose {
                peer_node_id: lease.peer_node_id.clone(),
                tunnel_id: lease.tunnel_id,
                reason: ClusterTunnelCloseReason::OperatorClosed,
                detail: Some(String::from("local_close")),
            })
            .map_err(|_| ClusterTunnelError::TransportOffline)
    }

    /// Reserves one logical stream on an established peer session.
    pub async fn open_logical_stream(
        &self,
        peer_node_id: &NodeId,
        kind: ClusterLogicalStreamKind,
    ) -> Result<ClusterLogicalStreamLease, ClusterStreamError> {
        self.state
            .lock()
            .await
            .open_logical_stream(peer_node_id, kind)
    }

    /// Releases one previously reserved logical stream.
    pub async fn close_logical_stream(
        &self,
        lease: &ClusterLogicalStreamLease,
    ) -> Result<(), ClusterStreamError> {
        self.state.lock().await.close_logical_stream(lease)
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

/// Minimal UDP relay server used for rendezvous and relay-forward fallback.
pub struct ClusterRelayServer {
    local_addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), String>>>,
}

impl ClusterRelayServer {
    /// Starts one relay server on the supplied bind address.
    pub async fn spawn(bind_addr: SocketAddr) -> Result<Self, ClusterError> {
        let socket = Arc::new(
            UdpSocket::bind(bind_addr)
                .await
                .map_err(ClusterError::Bind)?,
        );
        let local_addr = socket.local_addr().map_err(ClusterError::LocalAddr)?;
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(run_relay_server(socket, shutdown_rx));
        Ok(Self {
            local_addr,
            shutdown_tx: Some(shutdown_tx),
            task: Some(task),
        })
    }

    /// Returns the bound relay address.
    #[must_use]
    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Shuts the relay server down and waits for its background task.
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

impl Drop for ClusterRelayServer {
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
    network_state_persistence: ClusterNetworkStatePersistence,
    introduction_policy: Option<ClusterIntroductionPolicy>,
    tunnel_policy: ClusterTunnelPolicy,
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
            network_state_persistence: config.network_state_persistence,
            introduction_policy: config.introduction_policy,
            tunnel_policy: config.tunnel_policy,
            trust_policy: config.trust_policy,
        }
    }
}

#[derive(Clone, Copy)]
enum RelayRegistrationMode {
    NatTraversal,
    RelayForward,
}

enum TransportCommand {
    OpenTunnel {
        peer_node_id: NodeId,
        service_id: String,
        response_tx: oneshot::Sender<Result<ClusterTunnelLease, ClusterTunnelError>>,
    },
    SendTunnelHttpRequest {
        tunnel_id: ClusterTunnelId,
        request: ClusterTunnelHttpRequest,
        response_tx: oneshot::Sender<Result<ClusterTunnelHttpResponse, ClusterTunnelError>>,
    },
    SendTunnelClose {
        peer_node_id: NodeId,
        tunnel_id: ClusterTunnelId,
        reason: ClusterTunnelCloseReason,
        detail: Option<String>,
    },
}

#[derive(Clone)]
enum ConfiguredPeerDialAction {
    DirectHello {
        peer_node_id: NodeId,
        remote_addr: SocketAddr,
        path: ClusterTransportPath,
    },
    RelayRegister {
        relay: ClusterRelayEndpoint,
        peer_node_id: NodeId,
        mode: RelayRegistrationMode,
    },
    RelayHello {
        peer_node_id: NodeId,
        relay: ClusterRelayEndpoint,
        path: ClusterTransportPath,
    },
}

struct PendingHelloProbe {
    started_at: tokio::time::Instant,
}

#[derive(Clone)]
struct NatIntroductionRecord {
    peer_addr: SocketAddr,
    relay: ClusterRelayEndpoint,
}

#[derive(Clone)]
struct TunnelServiceRuntime {
    policy: ClusterTunnelServicePolicy,
    active: bool,
    request_count: u64,
    response_count: u64,
    bytes_in: u64,
    bytes_out: u64,
    last_activity_ms: Option<u64>,
    last_error: Option<String>,
}

impl TunnelServiceRuntime {
    fn new(policy: ClusterTunnelServicePolicy) -> Self {
        Self {
            policy,
            active: false,
            request_count: 0,
            response_count: 0,
            bytes_in: 0,
            bytes_out: 0,
            last_activity_ms: None,
            last_error: None,
        }
    }

    fn snapshot(&self) -> ClusterTunnelServiceSnapshot {
        ClusterTunnelServiceSnapshot {
            service_id: self.policy.service_id.clone(),
            kind: self.policy.kind,
            protocol: self.policy.protocol,
            local_addr: self.policy.local_addr,
            active: self.active,
            allowed_peer_node_ids: self.policy.allowed_peer_node_ids.clone(),
            max_request_body_bytes: self.policy.max_request_body_bytes,
            max_response_body_bytes: self.policy.max_response_body_bytes,
            request_count: self.request_count,
            response_count: self.response_count,
            bytes_in: self.bytes_in,
            bytes_out: self.bytes_out,
            last_activity_ms: self.last_activity_ms,
            last_error: self.last_error.clone(),
        }
    }
}

#[derive(Clone)]
struct TunnelRuntimeRecord {
    snapshot: ClusterTunnelSnapshot,
    logical_stream: Option<ClusterLogicalStreamLease>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ClusterTunnelRequestKey {
    tunnel_id: ClusterTunnelId,
    request_id: ClusterTunnelRequestId,
}

#[derive(Clone)]
struct PendingTunnelClosureDispatch {
    peer_node_id: NodeId,
    tunnel_id: ClusterTunnelId,
}

struct SharedState {
    peers: BTreeMap<NodeId, PeerSnapshot>,
    configured_peers: BTreeMap<NodeId, ConfiguredClusterPeer>,
    configured_peer_health: BTreeMap<NodeId, ConfiguredPeerHealthSnapshot>,
    trust_rollout_diagnostics: BTreeMap<NodeId, ClusterTrustRolloutDiagnostic>,
    peer_replay_windows: BTreeMap<NodeId, PeerReplayWindow>,
    join_refusals: Vec<ClusterJoinRefusal>,
    seed_peers: BTreeSet<SocketAddr>,
    durable_network_state: PersistedClusterNetworkState,
    network_state_persistence: ClusterNetworkStatePersistence,
    pending_hello_probes: BTreeMap<NodeId, PendingHelloProbe>,
    nat_introductions: BTreeMap<NodeId, NatIntroductionRecord>,
    active_logical_streams:
        BTreeMap<NodeId, BTreeMap<ClusterLogicalStreamId, ClusterLogicalStreamKind>>,
    tunnel_services: BTreeMap<String, TunnelServiceRuntime>,
    tunnels: BTreeMap<ClusterTunnelId, TunnelRuntimeRecord>,
    pending_tunnel_opens:
        BTreeMap<ClusterTunnelId, oneshot::Sender<Result<ClusterTunnelLease, ClusterTunnelError>>>,
    pending_tunnel_requests: BTreeMap<
        ClusterTunnelRequestKey,
        oneshot::Sender<Result<ClusterTunnelHttpResponse, ClusterTunnelError>>,
    >,
    next_ping_sequence: u64,
    next_authenticated_message_counter: u64,
    next_logical_stream_id: u64,
    next_tunnel_id: u64,
    next_tunnel_request_id: u64,
}

impl SharedState {
    fn new(
        seed_peers: BTreeSet<SocketAddr>,
        trust_policy: &ClusterTrustPolicy,
        tunnel_policy: &ClusterTunnelPolicy,
        durable_network_state: PersistedClusterNetworkState,
        network_state_persistence: ClusterNetworkStatePersistence,
    ) -> Self {
        let configured_peers = trust_policy
            .configured_peers
            .iter()
            .map(|peer| (peer.node_id.clone(), peer.clone()))
            .collect::<BTreeMap<_, _>>();
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
        let tunnel_services = tunnel_policy
            .approved_services
            .iter()
            .cloned()
            .map(|policy| (policy.service_id.clone(), TunnelServiceRuntime::new(policy)))
            .collect();
        Self {
            peers: BTreeMap::new(),
            configured_peers,
            configured_peer_health,
            trust_rollout_diagnostics: BTreeMap::new(),
            peer_replay_windows: BTreeMap::new(),
            join_refusals: Vec::new(),
            seed_peers,
            durable_network_state,
            network_state_persistence,
            pending_hello_probes: BTreeMap::new(),
            nat_introductions: BTreeMap::new(),
            active_logical_streams: BTreeMap::new(),
            tunnel_services,
            tunnels: BTreeMap::new(),
            pending_tunnel_opens: BTreeMap::new(),
            pending_tunnel_requests: BTreeMap::new(),
            next_ping_sequence: 0,
            next_authenticated_message_counter: 1,
            next_logical_stream_id: 1,
            next_tunnel_id: 1,
            next_tunnel_request_id: 1,
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

    fn durable_network_state(&self) -> PersistedClusterNetworkState {
        self.durable_network_state.clone()
    }

    fn trust_bundle_history(&self) -> Vec<ClusterTrustBundleRecord> {
        self.durable_network_state.trust_bundles.clone()
    }

    fn candidate_records(&self) -> Vec<ClusterCandidateRecord> {
        self.durable_network_state
            .candidates
            .values()
            .cloned()
            .collect()
    }

    fn active_logical_streams(&self) -> Vec<ClusterLogicalStreamLease> {
        self.active_logical_streams
            .iter()
            .flat_map(|(peer_node_id, streams)| {
                streams
                    .iter()
                    .map(|(stream_id, kind)| ClusterLogicalStreamLease {
                        peer_node_id: peer_node_id.clone(),
                        stream_id: *stream_id,
                        kind: *kind,
                    })
            })
            .collect()
    }

    fn tunnel_service_snapshot(&self, service_id: &str) -> Option<ClusterTunnelServiceSnapshot> {
        self.tunnel_services
            .get(service_id)
            .map(TunnelServiceRuntime::snapshot)
    }

    fn tunnel_service_snapshots(&self) -> Vec<ClusterTunnelServiceSnapshot> {
        self.tunnel_services
            .values()
            .map(TunnelServiceRuntime::snapshot)
            .collect()
    }

    fn tunnel_snapshots(&self) -> Vec<ClusterTunnelSnapshot> {
        self.tunnels
            .values()
            .map(|record| record.snapshot.clone())
            .collect()
    }

    fn activate_tunnel_service(
        &mut self,
        service_id: &str,
    ) -> Result<ClusterTunnelServiceSnapshot, ClusterTunnelError> {
        let Some(service) = self.tunnel_services.get_mut(service_id) else {
            return Err(ClusterTunnelError::ServiceUnknown {
                service_id: service_id.to_owned(),
            });
        };
        service.active = true;
        service.last_error = None;
        Ok(service.snapshot())
    }

    fn deactivate_tunnel_service(
        &mut self,
        service_id: &str,
    ) -> Result<Vec<PendingTunnelClosureDispatch>, ClusterTunnelError> {
        let Some(service) = self.tunnel_services.get_mut(service_id) else {
            return Err(ClusterTunnelError::ServiceUnknown {
                service_id: service_id.to_owned(),
            });
        };
        service.active = false;
        let affected = self
            .tunnels
            .values()
            .filter(|record| {
                record.snapshot.service_id == service_id
                    && matches!(
                        record.snapshot.state,
                        ClusterTunnelState::Pending | ClusterTunnelState::Open
                    )
            })
            .map(|record| PendingTunnelClosureDispatch {
                peer_node_id: record.snapshot.peer_node_id.clone(),
                tunnel_id: record.snapshot.tunnel_id,
            })
            .collect::<Vec<_>>();
        for closure in &affected {
            self.close_tunnel_record(
                closure.tunnel_id,
                ClusterTunnelState::Closed,
                Some(ClusterTunnelCloseReason::ServiceDeactivated),
                Some(String::from("service_deactivated")),
            );
        }
        Ok(affected)
    }

    fn peer_transport_path(&self, peer_node_id: &NodeId) -> Option<ClusterTransportPath> {
        self.peers
            .get(peer_node_id)
            .map(|peer| peer.transport.path.clone())
    }

    fn next_tunnel_id(&mut self) -> ClusterTunnelId {
        let tunnel_id = ClusterTunnelId::new(self.next_tunnel_id);
        self.next_tunnel_id = self.next_tunnel_id.saturating_add(1);
        tunnel_id
    }

    fn next_tunnel_request_id(&mut self) -> ClusterTunnelRequestId {
        let request_id = ClusterTunnelRequestId::new(self.next_tunnel_request_id);
        self.next_tunnel_request_id = self.next_tunnel_request_id.saturating_add(1);
        request_id
    }

    fn prepare_outbound_tunnel_open(
        &mut self,
        peer_node_id: &NodeId,
        service_id: &str,
        response_tx: oneshot::Sender<Result<ClusterTunnelLease, ClusterTunnelError>>,
    ) -> Result<(ClusterTunnelId, ClusterTransportPath), ClusterTunnelError> {
        let Some(session_path) = self.peer_transport_path(peer_node_id) else {
            return Err(ClusterTunnelError::PeerNotConnected {
                peer_node_id: peer_node_id.clone(),
            });
        };
        let logical_stream =
            self.open_logical_stream(peer_node_id, ClusterLogicalStreamKind::Tunnel)?;
        let tunnel_id = self.next_tunnel_id();
        self.pending_tunnel_opens.insert(tunnel_id, response_tx);
        self.tunnels.insert(
            tunnel_id,
            TunnelRuntimeRecord {
                snapshot: ClusterTunnelSnapshot {
                    tunnel_id,
                    direction: ClusterTunnelDirection::Outbound,
                    peer_node_id: peer_node_id.clone(),
                    service_id: service_id.to_owned(),
                    service_kind: ClusterTunnelServiceKind::DesktopControlHttp,
                    protocol: ClusterTunnelProtocol::HttpRequestResponse,
                    state: ClusterTunnelState::Pending,
                    transport: ClusterTunnelTransportObservation::new(session_path.clone()),
                    logical_stream_id: Some(logical_stream.stream_id),
                    opened_at_ms: current_time_ms(),
                    last_activity_ms: None,
                    close_reason: None,
                    last_error: None,
                },
                logical_stream: Some(logical_stream),
            },
        );
        Ok((tunnel_id, session_path))
    }

    fn accept_inbound_tunnel_open(
        &mut self,
        peer_node_id: &NodeId,
        tunnel_id: ClusterTunnelId,
        service_id: &str,
        session_path: &ClusterTransportPath,
    ) -> Result<(ClusterTunnelServiceKind, ClusterTunnelProtocol), ClusterTunnelOpenRefusalReason>
    {
        let Some(service) = self.tunnel_services.get(service_id).cloned() else {
            return Err(ClusterTunnelOpenRefusalReason::ServiceUnknown);
        };
        if !service.active {
            return Err(ClusterTunnelOpenRefusalReason::ServiceInactive);
        }
        if !service.policy.allows_peer(peer_node_id) {
            return Err(ClusterTunnelOpenRefusalReason::PeerNotAllowed);
        }
        let logical_stream = self
            .open_logical_stream(peer_node_id, ClusterLogicalStreamKind::Tunnel)
            .map_err(|_| ClusterTunnelOpenRefusalReason::StreamCapacityExceeded)?;
        self.tunnels.insert(
            tunnel_id,
            TunnelRuntimeRecord {
                snapshot: ClusterTunnelSnapshot {
                    tunnel_id,
                    direction: ClusterTunnelDirection::Inbound,
                    peer_node_id: peer_node_id.clone(),
                    service_id: service.policy.service_id.clone(),
                    service_kind: service.policy.kind,
                    protocol: service.policy.protocol,
                    state: ClusterTunnelState::Open,
                    transport: ClusterTunnelTransportObservation::new(session_path.clone()),
                    logical_stream_id: Some(logical_stream.stream_id),
                    opened_at_ms: current_time_ms(),
                    last_activity_ms: Some(current_time_ms()),
                    close_reason: None,
                    last_error: None,
                },
                logical_stream: Some(logical_stream),
            },
        );
        Ok((service.policy.kind, service.policy.protocol))
    }

    fn mark_outbound_tunnel_open(
        &mut self,
        tunnel_id: ClusterTunnelId,
        service_kind: ClusterTunnelServiceKind,
        protocol: ClusterTunnelProtocol,
    ) {
        let lease = self.tunnels.get_mut(&tunnel_id).map(|record| {
            record.snapshot.state = ClusterTunnelState::Open;
            record.snapshot.service_kind = service_kind;
            record.snapshot.protocol = protocol;
            record.snapshot.last_activity_ms = Some(current_time_ms());
            ClusterTunnelLease {
                tunnel_id,
                peer_node_id: record.snapshot.peer_node_id.clone(),
                service_id: record.snapshot.service_id.clone(),
                service_kind,
                protocol,
            }
        });
        if let Some(sender) = self.pending_tunnel_opens.remove(&tunnel_id) {
            let _ = sender.send(lease.ok_or(ClusterTunnelError::TunnelUnknown { tunnel_id }));
        }
    }

    fn refuse_outbound_tunnel_open(
        &mut self,
        tunnel_id: ClusterTunnelId,
        reason: ClusterTunnelOpenRefusalReason,
        detail: Option<String>,
    ) {
        let service_id = self
            .tunnels
            .get(&tunnel_id)
            .map(|record| record.snapshot.service_id.clone())
            .unwrap_or_default();
        self.close_tunnel_record(tunnel_id, ClusterTunnelState::Refused, None, detail);
        if let Some(sender) = self.pending_tunnel_opens.remove(&tunnel_id) {
            let _ = sender.send(Err(ClusterTunnelError::OpenRefused { service_id, reason }));
        }
    }

    fn prepare_outbound_tunnel_request(
        &mut self,
        tunnel_id: ClusterTunnelId,
        response_tx: oneshot::Sender<Result<ClusterTunnelHttpResponse, ClusterTunnelError>>,
        request_body_bytes: usize,
    ) -> Result<(ClusterTunnelRequestId, NodeId, ClusterTransportPath), ClusterTunnelError> {
        let request_id = self.next_tunnel_request_id();
        let Some(record) = self.tunnels.get_mut(&tunnel_id) else {
            return Err(ClusterTunnelError::TunnelUnknown { tunnel_id });
        };
        if record.snapshot.state != ClusterTunnelState::Open {
            return Err(ClusterTunnelError::TunnelNotOpen { tunnel_id });
        }
        record.snapshot.last_activity_ms = Some(current_time_ms());
        record.snapshot.transport.requests_sent =
            record.snapshot.transport.requests_sent.saturating_add(1);
        record.snapshot.transport.bytes_sent = record
            .snapshot
            .transport
            .bytes_sent
            .saturating_add(request_body_bytes as u64);
        let peer_node_id = record.snapshot.peer_node_id.clone();
        let session_path = record.snapshot.transport.session_path.clone();
        self.pending_tunnel_requests.insert(
            ClusterTunnelRequestKey {
                tunnel_id,
                request_id,
            },
            response_tx,
        );
        Ok((request_id, peer_node_id, session_path))
    }

    fn record_inbound_tunnel_request(
        &mut self,
        tunnel_id: ClusterTunnelId,
        request_body_bytes: usize,
    ) -> Result<(), ClusterTunnelError> {
        let service_id = {
            let Some(record) = self.tunnels.get_mut(&tunnel_id) else {
                return Err(ClusterTunnelError::TunnelUnknown { tunnel_id });
            };
            if record.snapshot.state != ClusterTunnelState::Open {
                return Err(ClusterTunnelError::TunnelNotOpen { tunnel_id });
            }
            record.snapshot.last_activity_ms = Some(current_time_ms());
            record.snapshot.transport.requests_received = record
                .snapshot
                .transport
                .requests_received
                .saturating_add(1);
            record.snapshot.transport.bytes_received = record
                .snapshot
                .transport
                .bytes_received
                .saturating_add(request_body_bytes as u64);
            record.snapshot.service_id.clone()
        };
        if let Some(service) = self.tunnel_services.get_mut(&service_id) {
            service.request_count = service.request_count.saturating_add(1);
            service.bytes_in = service.bytes_in.saturating_add(request_body_bytes as u64);
            service.last_activity_ms = Some(current_time_ms());
        }
        Ok(())
    }

    fn complete_inbound_tunnel_response(
        &mut self,
        tunnel_id: ClusterTunnelId,
        response_body_bytes: usize,
    ) {
        let service_id = self.tunnels.get_mut(&tunnel_id).map(|record| {
            record.snapshot.last_activity_ms = Some(current_time_ms());
            record.snapshot.transport.responses_sent =
                record.snapshot.transport.responses_sent.saturating_add(1);
            record.snapshot.transport.bytes_sent = record
                .snapshot
                .transport
                .bytes_sent
                .saturating_add(response_body_bytes as u64);
            record.snapshot.service_id.clone()
        });
        if let Some(service_id) = service_id {
            if let Some(service) = self.tunnel_services.get_mut(&service_id) {
                service.response_count = service.response_count.saturating_add(1);
                service.bytes_out = service.bytes_out.saturating_add(response_body_bytes as u64);
                service.last_activity_ms = Some(current_time_ms());
            }
        }
    }

    fn complete_outbound_tunnel_response(
        &mut self,
        tunnel_id: ClusterTunnelId,
        request_id: ClusterTunnelRequestId,
        response: Result<ClusterTunnelHttpResponse, ClusterTunnelError>,
    ) {
        if let Some(record) = self.tunnels.get_mut(&tunnel_id) {
            record.snapshot.last_activity_ms = Some(current_time_ms());
            if let Ok(response_ok) = &response {
                let response_body_bytes = response_ok
                    .body_bytes()
                    .map_or(0_u64, |body| body.len().min(usize::MAX) as u64);
                record.snapshot.transport.responses_received = record
                    .snapshot
                    .transport
                    .responses_received
                    .saturating_add(1);
                record.snapshot.transport.bytes_received = record
                    .snapshot
                    .transport
                    .bytes_received
                    .saturating_add(response_body_bytes);
            } else if let Err(error) = &response {
                record.snapshot.last_error = Some(error.to_string());
            }
        }
        if let Some(sender) = self
            .pending_tunnel_requests
            .remove(&ClusterTunnelRequestKey {
                tunnel_id,
                request_id,
            })
        {
            let _ = sender.send(response);
        }
    }

    fn close_tunnel_and_prepare_dispatch(
        &mut self,
        tunnel_id: ClusterTunnelId,
        reason: ClusterTunnelCloseReason,
        detail: Option<String>,
    ) -> Result<(NodeId, ClusterTransportPath), ClusterTunnelError> {
        let Some(record) = self.tunnels.get(&tunnel_id).cloned() else {
            return Err(ClusterTunnelError::TunnelUnknown { tunnel_id });
        };
        if !matches!(
            record.snapshot.state,
            ClusterTunnelState::Pending | ClusterTunnelState::Open
        ) {
            return Err(ClusterTunnelError::TunnelNotOpen { tunnel_id });
        }
        let peer_node_id = record.snapshot.peer_node_id.clone();
        let session_path = record.snapshot.transport.session_path.clone();
        self.close_tunnel_record(tunnel_id, ClusterTunnelState::Closed, Some(reason), detail);
        Ok((peer_node_id, session_path))
    }

    fn tunnel_dispatch_path(
        &self,
        tunnel_id: ClusterTunnelId,
    ) -> Option<(NodeId, ClusterTransportPath)> {
        self.tunnels.get(&tunnel_id).map(|record| {
            (
                record.snapshot.peer_node_id.clone(),
                record.snapshot.transport.session_path.clone(),
            )
        })
    }

    fn close_tunnel_record(
        &mut self,
        tunnel_id: ClusterTunnelId,
        state: ClusterTunnelState,
        close_reason: Option<ClusterTunnelCloseReason>,
        detail: Option<String>,
    ) {
        let logical_stream = self.tunnels.get_mut(&tunnel_id).and_then(|record| {
            record.snapshot.state = state;
            record.snapshot.close_reason = close_reason;
            record.snapshot.last_activity_ms = Some(current_time_ms());
            if let Some(detail) = detail.clone() {
                record.snapshot.last_error = Some(detail);
            }
            record.snapshot.logical_stream_id = None;
            record.logical_stream.take()
        });
        if let Some(logical_stream) = logical_stream {
            let _ = self.close_logical_stream(&logical_stream);
        }
        if state != ClusterTunnelState::Refused {
            if let Some(sender) = self.pending_tunnel_opens.remove(&tunnel_id) {
                let _ = sender.send(Err(ClusterTunnelError::TunnelNotOpen { tunnel_id }));
            }
        }
        let pending_keys = self
            .pending_tunnel_requests
            .keys()
            .copied()
            .filter(|key| key.tunnel_id == tunnel_id)
            .collect::<Vec<_>>();
        for key in pending_keys {
            if let Some(sender) = self.pending_tunnel_requests.remove(&key) {
                let _ = sender.send(Err(ClusterTunnelError::TunnelNotOpen { tunnel_id }));
            }
        }
    }

    fn record_tunnel_service_error(&mut self, service_id: &str, error: impl Into<String>) {
        if let Some(service) = self.tunnel_services.get_mut(service_id) {
            service.last_error = Some(error.into());
            service.last_activity_ms = Some(current_time_ms());
        }
    }

    fn apply_verified_introduction(
        &mut self,
        envelope: SignedClusterIntroductionEnvelope,
        policy: &ClusterIntroductionPolicy,
        verified_at_ms: u64,
        trust_policy: &ClusterTrustPolicy,
    ) -> Result<ClusterCandidateRecord, ClusterError> {
        let candidate = envelope.payload.candidate.clone();
        let candidate_digest = candidate.stable_digest();
        let introduction_policy_digest = policy.stable_digest();
        let introduction_payload_digest = envelope.payload.stable_digest();
        let record = self
            .durable_network_state
            .candidates
            .entry(candidate.node_id.clone())
            .or_insert_with(|| ClusterCandidateRecord {
                node_id: candidate.node_id.clone(),
                candidate_digest: candidate_digest.clone(),
                candidate: candidate.clone(),
                disposition: ClusterCandidateDisposition::Introduced,
                last_updated_ms: verified_at_ms,
                latest_introduction: None,
                history: Vec::new(),
            });
        record.node_id = candidate.node_id.clone();
        record.candidate_digest = candidate_digest;
        record.candidate = candidate.clone();
        record.disposition = ClusterCandidateDisposition::Introduced;
        record.last_updated_ms = verified_at_ms;
        record.latest_introduction = Some(PersistedClusterIntroductionRecord {
            envelope,
            introduction_policy_digest: introduction_policy_digest.clone(),
            verified_at_ms,
        });
        record.history.push(ClusterCandidateHistoryEvent {
            occurred_at_ms: verified_at_ms,
            disposition: ClusterCandidateDisposition::Introduced,
            reason_code: ClusterCandidateHistoryReasonCode::VerifiedIntroduction,
            trust_policy_digest: trust_policy.stable_digest(),
            introduction_policy_digest: Some(introduction_policy_digest),
            introduction_payload_digest: Some(introduction_payload_digest),
            detail: None,
        });
        let record = record.clone();
        self.persist_network_state()?;
        Ok(record)
    }

    fn record_candidate_disposition(
        &mut self,
        node_id: &NodeId,
        disposition: ClusterCandidateDisposition,
        reason_code: ClusterCandidateHistoryReasonCode,
        occurred_at_ms: u64,
        trust_policy: &ClusterTrustPolicy,
        detail: Option<String>,
    ) -> Result<ClusterCandidateRecord, ClusterError> {
        let Some(record) = self.durable_network_state.candidates.get_mut(node_id) else {
            return Err(ClusterError::UnknownCandidate(node_id.as_str().to_owned()));
        };
        let (introduction_policy_digest, introduction_payload_digest) = record
            .latest_introduction
            .as_ref()
            .map(|introduction| {
                (
                    Some(introduction.introduction_policy_digest.clone()),
                    Some(introduction.envelope.payload.stable_digest()),
                )
            })
            .unwrap_or((None, None));
        record.disposition = disposition;
        record.last_updated_ms = occurred_at_ms;
        record.history.push(ClusterCandidateHistoryEvent {
            occurred_at_ms,
            disposition,
            reason_code,
            trust_policy_digest: trust_policy.stable_digest(),
            introduction_policy_digest,
            introduction_payload_digest,
            detail,
        });
        let record = record.clone();
        self.persist_network_state()?;
        Ok(record)
    }

    fn expire_candidates(
        &mut self,
        now_ms: u64,
        trust_policy: &ClusterTrustPolicy,
    ) -> Result<Vec<ClusterCandidateRecord>, ClusterError> {
        let expiring_node_ids = self
            .durable_network_state
            .candidates
            .iter()
            .filter_map(|(node_id, record)| {
                let expires_at_ms = record
                    .latest_introduction
                    .as_ref()
                    .map(|introduction| introduction.envelope.payload.expires_at_ms)?;
                if expires_at_ms <= now_ms
                    && matches!(
                        record.disposition,
                        ClusterCandidateDisposition::Introduced
                            | ClusterCandidateDisposition::Refused
                    )
                {
                    Some(node_id.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let mut expired = Vec::new();
        for node_id in expiring_node_ids {
            expired.push(self.record_candidate_disposition(
                &node_id,
                ClusterCandidateDisposition::Expired,
                ClusterCandidateHistoryReasonCode::Expired,
                now_ms,
                trust_policy,
                Some(String::from("introduction_expired")),
            )?);
        }
        Ok(expired)
    }

    fn open_logical_stream(
        &mut self,
        peer_node_id: &NodeId,
        kind: ClusterLogicalStreamKind,
    ) -> Result<ClusterLogicalStreamLease, ClusterStreamError> {
        if !self.peers.contains_key(peer_node_id) {
            return Err(ClusterStreamError::PeerNotConnected {
                peer_node_id: peer_node_id.clone(),
            });
        }
        let capacity = self
            .peers
            .get(peer_node_id)
            .map(|peer| peer.transport.multiplex_profile.max_concurrent_streams)
            .unwrap_or_else(default_max_concurrent_transport_streams);
        let streams = self
            .active_logical_streams
            .entry(peer_node_id.clone())
            .or_default();
        if streams.len() >= usize::from(capacity) {
            return Err(ClusterStreamError::CapacityExceeded {
                peer_node_id: peer_node_id.clone(),
                max_concurrent_streams: capacity,
            });
        }
        let stream_id = ClusterLogicalStreamId::new(self.next_logical_stream_id);
        self.next_logical_stream_id = self.next_logical_stream_id.saturating_add(1);
        streams.insert(stream_id, kind);
        self.refresh_active_stream_count(peer_node_id);
        Ok(ClusterLogicalStreamLease {
            peer_node_id: peer_node_id.clone(),
            stream_id,
            kind,
        })
    }

    fn close_logical_stream(
        &mut self,
        lease: &ClusterLogicalStreamLease,
    ) -> Result<(), ClusterStreamError> {
        let Some(streams) = self.active_logical_streams.get_mut(&lease.peer_node_id) else {
            return Err(ClusterStreamError::StreamNotActive {
                peer_node_id: lease.peer_node_id.clone(),
                stream_id: lease.stream_id,
            });
        };
        if streams.remove(&lease.stream_id).is_none() {
            return Err(ClusterStreamError::StreamNotActive {
                peer_node_id: lease.peer_node_id.clone(),
                stream_id: lease.stream_id,
            });
        }
        if streams.is_empty() {
            self.active_logical_streams.remove(&lease.peer_node_id);
        }
        self.refresh_active_stream_count(&lease.peer_node_id);
        Ok(())
    }

    fn persist_network_state(&self) -> Result<(), ClusterError> {
        if let ClusterNetworkStatePersistence::FileBacked { path } = &self.network_state_persistence
        {
            self.durable_network_state.store_json(path)?;
        }
        Ok(())
    }

    fn push_join_refusal(&mut self, refusal: ClusterJoinRefusal) {
        if let Some(node_id) = refusal.remote_node_id.as_ref() {
            if let Some(health) = self.configured_peer_health.get_mut(node_id) {
                health.last_establishment_failure = Some(
                    ClusterSessionFailure::new(
                        health
                            .active_transport
                            .as_ref()
                            .map(|path| path.kind)
                            .unwrap_or(ClusterTransportPathKind::DirectDatagram),
                        ClusterSessionFailureReason::PeerRefused,
                    )
                    .with_detail(format!("{:?}", refusal.reason)),
                );
            }
        }
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
    ) -> Vec<ConfiguredPeerDialAction> {
        let mut actions = Vec::new();
        for peer in &trust_policy.configured_peers {
            if self.peers.contains_key(&peer.node_id) {
                self.mark_configured_peer_reachable(&peer.node_id, None, None);
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

            actions.push(ConfiguredPeerDialAction::DirectHello {
                peer_node_id: peer.node_id.clone(),
                remote_addr: peer.remote_addr,
                path: ClusterTransportPath::direct(peer.remote_addr),
            });

            if health.unanswered_hello_attempts
                == trust_policy
                    .configured_peer_dial_policy
                    .degraded_after_unanswered_hellos
            {
                health.last_establishment_failure = Some(
                    ClusterSessionFailure::new(
                        ClusterTransportPathKind::DirectDatagram,
                        ClusterSessionFailureReason::DirectConnectTimedOut,
                    )
                    .with_detail(format!(
                        "no hello reply from {} after {} attempts",
                        peer.remote_addr, health.unanswered_hello_attempts
                    )),
                );
            }

            if health.unanswered_hello_attempts
                >= trust_policy
                    .configured_peer_dial_policy
                    .degraded_after_unanswered_hellos
            {
                for relay in &peer.nat_rendezvous_relays {
                    actions.push(ConfiguredPeerDialAction::RelayRegister {
                        relay: relay.clone(),
                        peer_node_id: peer.node_id.clone(),
                        mode: RelayRegistrationMode::NatTraversal,
                    });
                }
                if let Some(introduction) = self.nat_introductions.get(&peer.node_id) {
                    actions.push(ConfiguredPeerDialAction::DirectHello {
                        peer_node_id: peer.node_id.clone(),
                        remote_addr: introduction.peer_addr,
                        path: ClusterTransportPath::nat_traversal(
                            introduction.peer_addr,
                            introduction.relay.clone(),
                        ),
                    });
                }
            }

            if health.unanswered_hello_attempts
                >= trust_policy
                    .configured_peer_dial_policy
                    .unreachable_after_unanswered_hellos
            {
                if !peer.nat_rendezvous_relays.is_empty()
                    && !self.nat_introductions.contains_key(&peer.node_id)
                {
                    health.last_establishment_failure = Some(
                        ClusterSessionFailure::new(
                            ClusterTransportPathKind::NatTraversalDatagram,
                            ClusterSessionFailureReason::NatTraversalTimedOut,
                        )
                        .with_detail(String::from(
                            "relay-assisted rendezvous did not surface a direct candidate",
                        )),
                    );
                }
                for relay in &peer.relay_fallback_relays {
                    actions.push(ConfiguredPeerDialAction::RelayRegister {
                        relay: relay.clone(),
                        peer_node_id: peer.node_id.clone(),
                        mode: RelayRegistrationMode::RelayForward,
                    });
                    actions.push(ConfiguredPeerDialAction::RelayHello {
                        peer_node_id: peer.node_id.clone(),
                        relay: relay.clone(),
                        path: ClusterTransportPath::relayed(peer.remote_addr, relay.clone()),
                    });
                }
            }
        }
        actions
    }

    fn discovered_peer_paths(&self) -> Vec<(NodeId, SocketAddr, ClusterTransportPath)> {
        self.peers
            .values()
            .map(|peer| {
                (
                    peer.identity.node_id.clone(),
                    peer.remote_addr,
                    peer.transport.path.clone(),
                )
            })
            .collect()
    }

    fn configured_peer_relay_endpoint(
        &self,
        node_id: &NodeId,
        relay_id: &str,
        relay_addr: SocketAddr,
        session_tag: &str,
        allow_nat: bool,
    ) -> Option<ClusterRelayEndpoint> {
        let peer = self.configured_peers.get(node_id)?;
        peer.relay_fallback_relays
            .iter()
            .chain(
                allow_nat
                    .then_some(peer.nat_rendezvous_relays.iter())
                    .into_iter()
                    .flatten(),
            )
            .find(|relay| {
                relay.relay_id == relay_id
                    && relay.relay_addr == relay_addr
                    && relay.session_tag == session_tag
            })
            .cloned()
    }

    fn record_nat_introduction(
        &mut self,
        peer_node_id: NodeId,
        peer_addr: SocketAddr,
        relay: ClusterRelayEndpoint,
    ) {
        self.nat_introductions.insert(
            peer_node_id.clone(),
            NatIntroductionRecord { peer_addr, relay },
        );
        if let Some(health) = self.configured_peer_health.get_mut(&peer_node_id) {
            health.last_establishment_failure = None;
        }
    }

    fn record_outbound_message(
        &mut self,
        peer_node_id: &NodeId,
        path: &ClusterTransportPath,
        bytes: usize,
        is_hello: bool,
    ) {
        if let Some(health) = self.configured_peer_health.get_mut(peer_node_id) {
            health.active_transport = Some(path.clone());
            health.messages_sent = health.messages_sent.saturating_add(1);
            health.bytes_sent = health.bytes_sent.saturating_add(bytes as u64);
        }
        if let Some(peer) = self.peers.get_mut(peer_node_id) {
            peer.transport.path = path.clone();
            peer.transport.messages_sent = peer.transport.messages_sent.saturating_add(1);
            peer.transport.bytes_sent = peer.transport.bytes_sent.saturating_add(bytes as u64);
        }
        if is_hello {
            self.pending_hello_probes.insert(
                peer_node_id.clone(),
                PendingHelloProbe {
                    started_at: tokio::time::Instant::now(),
                },
            );
        }
    }

    fn record_hello(
        &mut self,
        remote_addr: SocketAddr,
        identity: ClusterNodeIdentity,
        path: ClusterTransportPath,
        message_bytes: usize,
        authenticated_counter: Option<u64>,
        replay_window_size: u64,
    ) -> Result<bool, Box<ClusterJoinRefusal>> {
        let outcome = self.validate_peer_epoch(remote_addr, &identity)?;
        if let Some(counter) = authenticated_counter {
            self.record_authenticated_counter(remote_addr, &identity, counter, replay_window_size)?;
        }
        let last_round_trip_latency_ms = self.take_hello_latency_ms(&identity.node_id);
        self.mark_configured_peer_reachable(
            &identity.node_id,
            Some(path.clone()),
            last_round_trip_latency_ms,
        );
        if let Some(health) = self.configured_peer_health.get_mut(&identity.node_id) {
            health.messages_received = health.messages_received.saturating_add(1);
            health.bytes_received = health.bytes_received.saturating_add(message_bytes as u64);
            if let Some(latency_ms) = last_round_trip_latency_ms {
                health.last_round_trip_latency_ms = Some(latency_ms);
            }
        }
        let snapshot = self.ensure_peer_snapshot(remote_addr, identity, path);
        snapshot.handshake.saw_hello = true;
        snapshot.transport.messages_received =
            snapshot.transport.messages_received.saturating_add(1);
        snapshot.transport.bytes_received = snapshot
            .transport
            .bytes_received
            .saturating_add(message_bytes as u64);
        if let Some(latency_ms) = last_round_trip_latency_ms {
            snapshot.transport.last_round_trip_latency_ms = Some(latency_ms);
        }
        Ok(outcome.should_reply_hello)
    }

    fn record_ping(
        &mut self,
        remote_addr: SocketAddr,
        identity: ClusterNodeIdentity,
        path: ClusterTransportPath,
        message_bytes: usize,
        sequence: u64,
        authenticated_counter: Option<u64>,
        replay_window_size: u64,
    ) -> Result<(), Box<ClusterJoinRefusal>> {
        let _ = self.validate_peer_epoch(remote_addr, &identity)?;
        if let Some(counter) = authenticated_counter {
            self.record_authenticated_counter(remote_addr, &identity, counter, replay_window_size)?;
        }
        self.mark_configured_peer_reachable(&identity.node_id, Some(path.clone()), None);
        if let Some(health) = self.configured_peer_health.get_mut(&identity.node_id) {
            health.messages_received = health.messages_received.saturating_add(1);
            health.bytes_received = health.bytes_received.saturating_add(message_bytes as u64);
        }
        let snapshot = self.ensure_peer_snapshot(remote_addr, identity, path);
        snapshot.handshake.last_ping_sequence = Some(sequence);
        snapshot.transport.messages_received =
            snapshot.transport.messages_received.saturating_add(1);
        snapshot.transport.bytes_received = snapshot
            .transport
            .bytes_received
            .saturating_add(message_bytes as u64);
        Ok(())
    }

    fn ensure_peer_snapshot(
        &mut self,
        remote_addr: SocketAddr,
        identity: ClusterNodeIdentity,
        path: ClusterTransportPath,
    ) -> &mut PeerSnapshot {
        let multiplex_profile = self.configured_peer_multiplex_profile(&identity.node_id);
        let active_streams = self.active_stream_count(&identity.node_id);
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
                transport: ClusterTransportObservation::new(path.clone(), multiplex_profile),
            });
        if identity.node_epoch > entry.identity.node_epoch {
            entry.handshake = PeerHandshakeState {
                saw_hello: false,
                last_ping_sequence: None,
            };
            entry.transport = ClusterTransportObservation::new(path.clone(), multiplex_profile);
        }
        entry.remote_addr = remote_addr;
        entry.identity = identity.clone();
        entry.transport.path = path;
        entry.transport.multiplex_profile = multiplex_profile;
        entry.transport.active_streams = active_streams;
        entry
    }

    fn mark_configured_peer_reachable(
        &mut self,
        node_id: &NodeId,
        active_transport: Option<ClusterTransportPath>,
        last_round_trip_latency_ms: Option<u64>,
    ) {
        let active_streams = self.active_stream_count(node_id);
        if let Some(health) = self.configured_peer_health.get_mut(node_id) {
            let was_reachable =
                matches!(health.reachability, ConfiguredPeerReachability::Reachable);
            health.reachability = ConfiguredPeerReachability::Reachable;
            health.unanswered_hello_attempts = 0;
            health.remaining_backoff_ticks = 0;
            if let Some(active_transport) = active_transport {
                health.active_transport = Some(active_transport);
            }
            if let Some(last_round_trip_latency_ms) = last_round_trip_latency_ms {
                health.last_round_trip_latency_ms = Some(last_round_trip_latency_ms);
            }
            health.last_establishment_failure = None;
            health.active_streams = active_streams;
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

    fn take_hello_latency_ms(&mut self, node_id: &NodeId) -> Option<u64> {
        self.pending_hello_probes.remove(node_id).map(|probe| {
            probe
                .started_at
                .elapsed()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX)
        })
    }

    fn configured_peer_multiplex_profile(
        &self,
        node_id: &NodeId,
    ) -> ClusterSessionMultiplexProfile {
        self.configured_peers
            .get(node_id)
            .map(ConfiguredClusterPeer::multiplex_profile)
            .unwrap_or_default()
    }

    fn active_stream_count(&self, node_id: &NodeId) -> u16 {
        self.active_logical_streams
            .get(node_id)
            .map(|streams| streams.len().min(usize::from(u16::MAX)) as u16)
            .unwrap_or(0)
    }

    fn refresh_active_stream_count(&mut self, node_id: &NodeId) {
        let active_streams = self.active_stream_count(node_id);
        if let Some(peer) = self.peers.get_mut(node_id) {
            peer.transport.active_streams = active_streams;
        }
        if let Some(health) = self.configured_peer_health.get_mut(node_id) {
            health.active_streams = active_streams;
        }
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
struct TunnelOpenMessage {
    sender: ClusterNodeIdentity,
    tunnel_id: ClusterTunnelId,
    service_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TunnelOpenAckMessage {
    sender: ClusterNodeIdentity,
    tunnel_id: ClusterTunnelId,
    service_kind: ClusterTunnelServiceKind,
    protocol: ClusterTunnelProtocol,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TunnelOpenRefusedMessage {
    sender: ClusterNodeIdentity,
    tunnel_id: ClusterTunnelId,
    service_id: String,
    reason: ClusterTunnelOpenRefusalReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TunnelHttpRequestMessage {
    sender: ClusterNodeIdentity,
    tunnel_id: ClusterTunnelId,
    request_id: ClusterTunnelRequestId,
    request: ClusterTunnelHttpRequest,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TunnelHttpResponseMessage {
    sender: ClusterNodeIdentity,
    tunnel_id: ClusterTunnelId,
    request_id: ClusterTunnelRequestId,
    response: ClusterTunnelHttpResponse,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TunnelCloseMessage {
    sender: ClusterNodeIdentity,
    tunnel_id: ClusterTunnelId,
    reason: ClusterTunnelCloseReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WireMessage {
    Hello(HelloMessage),
    Ping(PingMessage),
    TunnelOpen(TunnelOpenMessage),
    TunnelOpenAck(TunnelOpenAckMessage),
    TunnelOpenRefused(TunnelOpenRefusedMessage),
    TunnelHttpRequest(TunnelHttpRequestMessage),
    TunnelHttpResponse(TunnelHttpResponseMessage),
    TunnelClose(TunnelCloseMessage),
}

impl WireMessage {
    fn sender(&self) -> &ClusterNodeIdentity {
        match self {
            Self::Hello(message) => &message.sender,
            Self::Ping(message) => &message.sender,
            Self::TunnelOpen(message) => &message.sender,
            Self::TunnelOpenAck(message) => &message.sender,
            Self::TunnelOpenRefused(message) => &message.sender,
            Self::TunnelHttpRequest(message) => &message.sender,
            Self::TunnelHttpResponse(message) => &message.sender,
            Self::TunnelClose(message) => &message.sender,
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

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RelayDatagramMode {
    NatTraversal,
    RelayForward,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum TransportDatagram {
    Direct {
        envelope: WireEnvelope,
    },
    RelayRegister {
        relay_id: String,
        session_tag: String,
        sender_node_id: NodeId,
        target_node_id: NodeId,
        mode: RelayDatagramMode,
    },
    RelayForward {
        relay_id: String,
        session_tag: String,
        source_node_id: NodeId,
        target_node_id: NodeId,
        envelope: WireEnvelope,
    },
    RelayDelivery {
        relay_id: String,
        session_tag: String,
        source_addr: SocketAddr,
        source_node_id: NodeId,
        envelope: WireEnvelope,
    },
    NatIntroduction {
        relay_id: String,
        session_tag: String,
        peer_node_id: NodeId,
        peer_addr: SocketAddr,
    },
    RelayTargetUnavailable {
        relay_id: String,
        session_tag: String,
        target_node_id: NodeId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RelaySessionKey {
    relay_id: String,
    session_tag: String,
}

#[derive(Clone)]
struct RelayRegistration {
    remote_addr: SocketAddr,
}

#[derive(Default)]
struct RelayServerState {
    registrations: BTreeMap<RelaySessionKey, BTreeMap<NodeId, RelayRegistration>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedNodeIdentityRecord {
    cluster_id: ClusterId,
    node_id: NodeId,
    last_epoch: NodeEpoch,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    auth_secret_key_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    node_attestation: Option<NodeAttestationEvidence>,
}

struct LoadedLocalIdentity {
    identity: ClusterNodeIdentity,
    signing_key: SigningKey,
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_millis(0))
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn load_or_create_network_state(
    persistence: &ClusterNetworkStatePersistence,
    trust_policy: &ClusterTrustPolicy,
) -> Result<PersistedClusterNetworkState, ClusterError> {
    let mut state = match persistence {
        ClusterNetworkStatePersistence::Ephemeral => PersistedClusterNetworkState::empty(),
        ClusterNetworkStatePersistence::FileBacked { path } => {
            if path.exists() {
                PersistedClusterNetworkState::load_json(path)?
            } else {
                PersistedClusterNetworkState::empty()
            }
        }
    };
    state.record_trust_bundle(trust_policy.clone(), current_time_ms());
    if let ClusterNetworkStatePersistence::FileBacked { path } = persistence {
        state.store_json(path)?;
    }
    Ok(state)
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
                    attestation: config.node_attestation.clone(),
                },
                signing_key,
            })
        }
        NodeIdentityPersistence::FileBacked { path } => {
            let mut node_id = NodeId::random();
            let mut node_epoch = NodeEpoch::initial();
            let mut signing_key = SigningKey::from_bytes(&random::<[u8; SIGNING_KEY_BYTES]>());
            let mut node_attestation = config.node_attestation.clone();
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
                    if node_attestation.is_none() {
                        node_attestation = record.node_attestation;
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
                node_attestation: node_attestation.clone(),
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
                    attestation: node_attestation,
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

fn decode_introduction_verifying_key(
    auth_public_key: &str,
) -> Result<VerifyingKey, ClusterIntroductionVerificationError> {
    let bytes = hex::decode(auth_public_key).map_err(|error| {
        ClusterIntroductionVerificationError::InvalidSourcePublicKey(error.to_string())
    })?;
    let verifying_key_bytes: [u8; VERIFYING_KEY_BYTES] = bytes.try_into().map_err(|_| {
        ClusterIntroductionVerificationError::InvalidSourcePublicKey(String::from(
            "invalid public key length",
        ))
    })?;
    VerifyingKey::from_bytes(&verifying_key_bytes).map_err(|error| {
        ClusterIntroductionVerificationError::InvalidSourcePublicKey(error.to_string())
    })
}

fn decode_introduction_signature(
    signature_hex: &str,
) -> Result<Signature, ClusterIntroductionVerificationError> {
    let bytes = hex::decode(signature_hex).map_err(|error| {
        ClusterIntroductionVerificationError::InvalidSignatureEncoding(error.to_string())
    })?;
    let signature_bytes: [u8; SIGNATURE_BYTES] = bytes.try_into().map_err(|_| {
        ClusterIntroductionVerificationError::InvalidSignatureEncoding(String::from(
            "invalid signature length",
        ))
    })?;
    Ok(Signature::from_bytes(&signature_bytes))
}

async fn run_transport(
    socket: Arc<UdpSocket>,
    state: Arc<Mutex<SharedState>>,
    config: TransportConfig,
    mut command_rx: mpsc::UnboundedReceiver<TransportCommand>,
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
            Some(command) = command_rx.recv() => {
                handle_transport_command(&socket, &state, &config, command).await?;
            }
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

#[derive(Clone)]
struct InboundTransportContext {
    socket_remote_addr: SocketAddr,
    path: ClusterTransportPath,
}

async fn send_hello_to_seed_peers(
    socket: &Arc<UdpSocket>,
    config: &TransportConfig,
    state: &Arc<Mutex<SharedState>>,
) -> Result<(), String> {
    let configured_actions = {
        let mut guard = state.lock().await;
        if matches!(
            config.trust_policy.posture,
            ClusterTrustPosture::AuthenticatedConfiguredPeers
                | ClusterTrustPosture::AttestedConfiguredPeers
        ) {
            Some(guard.configured_peers_due_for_dial(&config.trust_policy))
        } else {
            None
        }
    };
    if let Some(actions) = configured_actions {
        for action in actions {
            match action {
                ConfiguredPeerDialAction::DirectHello {
                    peer_node_id,
                    remote_addr,
                    path,
                } => {
                    send_wire_message_to_path(
                        socket,
                        state,
                        config,
                        Some(&peer_node_id),
                        &path,
                        remote_addr,
                        WireMessage::Hello(HelloMessage {
                            sender: config.local_identity.clone(),
                        }),
                        true,
                    )
                    .await?;
                }
                ConfiguredPeerDialAction::RelayRegister {
                    relay,
                    peer_node_id,
                    mode,
                } => {
                    let datagram = TransportDatagram::RelayRegister {
                        relay_id: relay.relay_id.clone(),
                        session_tag: relay.session_tag.clone(),
                        sender_node_id: config.local_identity.node_id.clone(),
                        target_node_id: peer_node_id,
                        mode: match mode {
                            RelayRegistrationMode::NatTraversal => RelayDatagramMode::NatTraversal,
                            RelayRegistrationMode::RelayForward => RelayDatagramMode::RelayForward,
                        },
                    };
                    send_transport_datagram(socket, relay.relay_addr, &datagram).await?;
                }
                ConfiguredPeerDialAction::RelayHello {
                    peer_node_id,
                    relay,
                    path,
                } => {
                    send_wire_message_to_path(
                        socket,
                        state,
                        config,
                        Some(&peer_node_id),
                        &path,
                        relay.relay_addr,
                        WireMessage::Hello(HelloMessage {
                            sender: config.local_identity.clone(),
                        }),
                        true,
                    )
                    .await?;
                }
            }
        }
        return Ok(());
    }

    let remote_addrs = { state.lock().await.undiscovered_seed_peers() };
    for remote_addr in remote_addrs {
        send_wire_message_to_path(
            socket,
            state,
            config,
            None,
            &ClusterTransportPath::direct(remote_addr),
            remote_addr,
            WireMessage::Hello(HelloMessage {
                sender: config.local_identity.clone(),
            }),
            true,
        )
        .await?;
    }
    Ok(())
}

async fn send_ping_to_discovered_peers(
    socket: &Arc<UdpSocket>,
    config: &TransportConfig,
    state: &Arc<Mutex<SharedState>>,
) -> Result<(), String> {
    let (peer_paths, sequence) = {
        let mut guard = state.lock().await;
        (guard.discovered_peer_paths(), guard.next_ping_sequence())
    };
    for (peer_node_id, remote_addr, path) in peer_paths {
        send_wire_message_to_path(
            socket,
            state,
            config,
            Some(&peer_node_id),
            &path,
            remote_addr,
            WireMessage::Ping(PingMessage {
                sender: config.local_identity.clone(),
                sequence,
            }),
            false,
        )
        .await?;
    }
    Ok(())
}

async fn handle_transport_command(
    socket: &Arc<UdpSocket>,
    state: &Arc<Mutex<SharedState>>,
    config: &TransportConfig,
    command: TransportCommand,
) -> Result<(), String> {
    match command {
        TransportCommand::OpenTunnel {
            peer_node_id,
            service_id,
            response_tx,
        } => {
            let prepared = state.lock().await.prepare_outbound_tunnel_open(
                &peer_node_id,
                service_id.as_str(),
                response_tx,
            );
            let Ok((tunnel_id, session_path)) = prepared else {
                return Ok(());
            };
            let send_outcome = send_wire_message_to_path(
                socket,
                state,
                config,
                Some(&peer_node_id),
                &session_path,
                session_path.peer_addr,
                WireMessage::TunnelOpen(TunnelOpenMessage {
                    sender: config.local_identity.clone(),
                    tunnel_id,
                    service_id,
                }),
                false,
            )
            .await;
            if let Err(error) = send_outcome {
                let mut guard = state.lock().await;
                guard.refuse_outbound_tunnel_open(
                    tunnel_id,
                    ClusterTunnelOpenRefusalReason::ProtocolUnsupported,
                    Some(error),
                );
            }
        }
        TransportCommand::SendTunnelHttpRequest {
            tunnel_id,
            request,
            response_tx,
        } => {
            let request_body_bytes = match request.body_bytes() {
                Ok(body) => body.len(),
                Err(error) => {
                    let _ = response_tx.send(Err(error));
                    return Ok(());
                }
            };
            let prepared = state.lock().await.prepare_outbound_tunnel_request(
                tunnel_id,
                response_tx,
                request_body_bytes,
            );
            let Ok((request_id, peer_node_id, session_path)) = prepared else {
                return Ok(());
            };
            let send_outcome = send_wire_message_to_path(
                socket,
                state,
                config,
                Some(&peer_node_id),
                &session_path,
                session_path.peer_addr,
                WireMessage::TunnelHttpRequest(TunnelHttpRequestMessage {
                    sender: config.local_identity.clone(),
                    tunnel_id,
                    request_id,
                    request,
                }),
                false,
            )
            .await;
            if send_outcome.is_err() {
                state.lock().await.complete_outbound_tunnel_response(
                    tunnel_id,
                    request_id,
                    Err(ClusterTunnelError::TransportOffline),
                );
            }
        }
        TransportCommand::SendTunnelClose {
            peer_node_id,
            tunnel_id,
            reason,
            detail,
        } => {
            let session_path = {
                let guard = state.lock().await;
                guard
                    .tunnel_dispatch_path(tunnel_id)
                    .map(|(_, path)| path)
                    .or_else(|| guard.peer_transport_path(&peer_node_id))
            };
            let Some(session_path) = session_path else {
                return Ok(());
            };
            send_wire_message_to_path(
                socket,
                state,
                config,
                Some(&peer_node_id),
                &session_path,
                session_path.peer_addr,
                WireMessage::TunnelClose(TunnelCloseMessage {
                    sender: config.local_identity.clone(),
                    tunnel_id,
                    reason,
                    detail,
                }),
                false,
            )
            .await?;
        }
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
    let datagram = match serde_json::from_slice::<TransportDatagram>(payload) {
        Ok(datagram) => datagram,
        Err(_) => match serde_json::from_slice::<WireEnvelope>(payload) {
            Ok(envelope) => TransportDatagram::Direct { envelope },
            Err(_) => return Ok(()),
        },
    };

    match datagram {
        TransportDatagram::Direct { envelope } => {
            let path = {
                let guard = state.lock().await;
                guard
                    .nat_introductions
                    .get(&envelope.message.sender().node_id)
                    .filter(|introduction| introduction.peer_addr == remote_addr)
                    .map(|introduction| {
                        ClusterTransportPath::nat_traversal(remote_addr, introduction.relay.clone())
                    })
                    .unwrap_or_else(|| ClusterTransportPath::direct(remote_addr))
            };
            handle_wire_envelope(
                socket,
                state,
                config,
                InboundTransportContext {
                    socket_remote_addr: remote_addr,
                    path,
                },
                envelope,
                payload.len(),
            )
            .await
        }
        TransportDatagram::RelayDelivery {
            relay_id,
            session_tag,
            source_addr,
            source_node_id,
            envelope,
        } => {
            let relay = ClusterRelayEndpoint::new(relay_id, remote_addr, session_tag);
            let _ = source_node_id;
            handle_wire_envelope(
                socket,
                state,
                config,
                InboundTransportContext {
                    socket_remote_addr: remote_addr,
                    path: ClusterTransportPath::relayed(source_addr, relay),
                },
                envelope,
                payload.len(),
            )
            .await
        }
        TransportDatagram::NatIntroduction {
            relay_id,
            session_tag,
            peer_node_id,
            peer_addr,
        } => {
            let relay = {
                let guard = state.lock().await;
                guard.configured_peer_relay_endpoint(
                    &peer_node_id,
                    &relay_id,
                    remote_addr,
                    &session_tag,
                    true,
                )
            };
            let Some(relay) = relay else {
                return Ok(());
            };
            state.lock().await.record_nat_introduction(
                peer_node_id.clone(),
                peer_addr,
                relay.clone(),
            );
            send_wire_message_to_path(
                socket,
                state,
                config,
                Some(&peer_node_id),
                &ClusterTransportPath::nat_traversal(peer_addr, relay),
                peer_addr,
                WireMessage::Hello(HelloMessage {
                    sender: config.local_identity.clone(),
                }),
                true,
            )
            .await
        }
        TransportDatagram::RelayTargetUnavailable {
            relay_id,
            session_tag,
            target_node_id,
        } => {
            let relay = {
                let guard = state.lock().await;
                guard.configured_peer_relay_endpoint(
                    &target_node_id,
                    &relay_id,
                    remote_addr,
                    &session_tag,
                    false,
                )
            };
            if let Some(relay) = relay {
                let mut guard = state.lock().await;
                if let Some(health) = guard.configured_peer_health.get_mut(&target_node_id) {
                    health.active_transport =
                        Some(ClusterTransportPath::relayed(health.remote_addr, relay));
                    health.last_establishment_failure = Some(
                        ClusterSessionFailure::new(
                            ClusterTransportPathKind::RelayedDatagram,
                            ClusterSessionFailureReason::RelayTargetUnavailable,
                        )
                        .with_detail(String::from(
                            "relay has not observed the target peer registration yet",
                        )),
                    );
                }
            }
            Ok(())
        }
        TransportDatagram::RelayRegister { .. } | TransportDatagram::RelayForward { .. } => Ok(()),
    }
}

async fn handle_wire_envelope(
    socket: &Arc<UdpSocket>,
    state: &Arc<Mutex<SharedState>>,
    config: &TransportConfig,
    transport: InboundTransportContext,
    envelope: WireEnvelope,
    message_bytes: usize,
) -> Result<(), String> {
    if envelope.namespace != config.namespace {
        state.lock().await.push_join_refusal(ClusterJoinRefusal {
            remote_addr: transport.socket_remote_addr,
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
            remote_addr: transport.socket_remote_addr,
            remote_node_id: Some(envelope.message.sender().node_id.clone()),
            remote_cluster_id: Some(envelope.message.sender().cluster_id.clone()),
            remote_node_epoch: Some(envelope.message.sender().node_epoch),
            reason: ClusterJoinRefusalReason::AdmissionMismatch,
        });
        return Ok(());
    }

    let trust_rollout_diagnostic = {
        let guard = state.lock().await;
        match authenticate_incoming_envelope(&envelope, &transport, config, &guard) {
            Ok(trust_rollout_diagnostic) => trust_rollout_diagnostic,
            Err(reason) => {
                let rollout_diagnostic = trust_rollout_diagnostic_from_refusal(
                    &envelope,
                    transport.socket_remote_addr,
                    &reason,
                    config,
                );
                drop(guard);
                let mut guard = state.lock().await;
                if let Some(rollout_diagnostic) = rollout_diagnostic {
                    guard.push_trust_rollout_diagnostic(rollout_diagnostic);
                }
                guard.push_join_refusal(ClusterJoinRefusal {
                    remote_addr: transport.socket_remote_addr,
                    remote_node_id: Some(envelope.message.sender().node_id.clone()),
                    remote_cluster_id: Some(envelope.message.sender().cluster_id.clone()),
                    remote_node_epoch: Some(envelope.message.sender().node_epoch),
                    reason,
                });
                return Ok(());
            }
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
                    remote_addr: transport.socket_remote_addr,
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
                    transport.socket_remote_addr,
                    hello.sender.clone(),
                    transport.path.clone(),
                    message_bytes,
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
                send_wire_message_to_path(
                    socket,
                    state,
                    config,
                    Some(&hello.sender.node_id),
                    &transport.path,
                    transport.path.peer_addr,
                    WireMessage::Hello(HelloMessage {
                        sender: config.local_identity.clone(),
                    }),
                    true,
                )
                .await?;
            }

            let sequence = state.lock().await.next_ping_sequence();
            send_wire_message_to_path(
                socket,
                state,
                config,
                Some(&hello.sender.node_id),
                &transport.path,
                transport.path.peer_addr,
                WireMessage::Ping(PingMessage {
                    sender: config.local_identity.clone(),
                    sequence,
                }),
                false,
            )
            .await?;
        }
        WireMessage::Ping(ping) => {
            if ping.sender.node_id == config.local_identity.node_id {
                return Ok(());
            }
            if ping.sender.cluster_id != config.local_identity.cluster_id {
                state.lock().await.push_join_refusal(ClusterJoinRefusal {
                    remote_addr: transport.socket_remote_addr,
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
                transport.socket_remote_addr,
                ping.sender,
                transport.path,
                message_bytes,
                ping.sequence,
                envelope.authenticated_counter,
                config.trust_policy.replay_window_size,
            ) {
                guard.push_join_refusal(*refusal);
            }
        }
        WireMessage::TunnelOpen(open) => {
            if open.sender.node_id == config.local_identity.node_id {
                return Ok(());
            }
            let accepted = state.lock().await.accept_inbound_tunnel_open(
                &open.sender.node_id,
                open.tunnel_id,
                open.service_id.as_str(),
                &transport.path,
            );
            match accepted {
                Ok((service_kind, protocol)) => {
                    send_wire_message_to_path(
                        socket,
                        state,
                        config,
                        Some(&open.sender.node_id),
                        &transport.path,
                        transport.path.peer_addr,
                        WireMessage::TunnelOpenAck(TunnelOpenAckMessage {
                            sender: config.local_identity.clone(),
                            tunnel_id: open.tunnel_id,
                            service_kind,
                            protocol,
                        }),
                        false,
                    )
                    .await?;
                }
                Err(reason) => {
                    send_wire_message_to_path(
                        socket,
                        state,
                        config,
                        Some(&open.sender.node_id),
                        &transport.path,
                        transport.path.peer_addr,
                        WireMessage::TunnelOpenRefused(TunnelOpenRefusedMessage {
                            sender: config.local_identity.clone(),
                            tunnel_id: open.tunnel_id,
                            service_id: open.service_id,
                            reason,
                            detail: None,
                        }),
                        false,
                    )
                    .await?;
                }
            }
        }
        WireMessage::TunnelOpenAck(ack) => {
            state.lock().await.mark_outbound_tunnel_open(
                ack.tunnel_id,
                ack.service_kind,
                ack.protocol,
            );
        }
        WireMessage::TunnelOpenRefused(refused) => {
            state.lock().await.refuse_outbound_tunnel_open(
                refused.tunnel_id,
                refused.reason,
                refused.detail,
            );
        }
        WireMessage::TunnelHttpRequest(request_message) => {
            let request_body_bytes = request_message.request.body_bytes();
            let request_body_len = match request_body_bytes {
                Ok(body) => body.len(),
                Err(error) => {
                    state
                        .lock()
                        .await
                        .complete_inbound_tunnel_response(request_message.tunnel_id, 0);
                    send_wire_message_to_path(
                        socket,
                        state,
                        config,
                        Some(&request_message.sender.node_id),
                        &transport.path,
                        transport.path.peer_addr,
                        WireMessage::TunnelHttpResponse(TunnelHttpResponseMessage {
                            sender: config.local_identity.clone(),
                            tunnel_id: request_message.tunnel_id,
                            request_id: request_message.request_id,
                            response: ClusterTunnelHttpResponse::bad_gateway(error.to_string()),
                        }),
                        false,
                    )
                    .await?;
                    return Ok(());
                }
            };
            let service_policy = {
                let mut guard = state.lock().await;
                let record_request = guard
                    .record_inbound_tunnel_request(request_message.tunnel_id, request_body_len);
                if record_request.is_err() {
                    None
                } else {
                    guard
                        .tunnels
                        .get(&request_message.tunnel_id)
                        .and_then(|record| {
                            guard
                                .tunnel_services
                                .get(record.snapshot.service_id.as_str())
                                .map(|service| service.policy.clone())
                        })
                }
            };
            let response = if let Some(service_policy) = service_policy {
                if request_body_len > service_policy.max_request_body_bytes {
                    ClusterTunnelHttpResponse::new(413, "Payload Too Large")
                        .with_header("content-type", "text/plain; charset=utf-8")
                        .with_utf8_body("request body exceeds tunnel policy")
                } else {
                    match forward_http_request_to_local_service(
                        &service_policy,
                        &request_message.request,
                    )
                    .await
                    {
                        Ok(response) => response,
                        Err(error) => {
                            state.lock().await.record_tunnel_service_error(
                                service_policy.service_id.as_str(),
                                error.to_string(),
                            );
                            ClusterTunnelHttpResponse::bad_gateway(error.to_string())
                        }
                    }
                }
            } else {
                ClusterTunnelHttpResponse::new(410, "Gone")
                    .with_header("content-type", "text/plain; charset=utf-8")
                    .with_utf8_body("tunnel is not open")
            };
            let response_body_len = response.body_bytes().map_or(0_usize, |body| body.len());
            state
                .lock()
                .await
                .complete_inbound_tunnel_response(request_message.tunnel_id, response_body_len);
            send_wire_message_to_path(
                socket,
                state,
                config,
                Some(&request_message.sender.node_id),
                &transport.path,
                transport.path.peer_addr,
                WireMessage::TunnelHttpResponse(TunnelHttpResponseMessage {
                    sender: config.local_identity.clone(),
                    tunnel_id: request_message.tunnel_id,
                    request_id: request_message.request_id,
                    response,
                }),
                false,
            )
            .await?;
        }
        WireMessage::TunnelHttpResponse(response_message) => {
            state.lock().await.complete_outbound_tunnel_response(
                response_message.tunnel_id,
                response_message.request_id,
                Ok(response_message.response),
            );
        }
        WireMessage::TunnelClose(close) => {
            state.lock().await.close_tunnel_record(
                close.tunnel_id,
                ClusterTunnelState::Closed,
                Some(close.reason),
                close.detail,
            );
        }
    }
    Ok(())
}

async fn forward_http_request_to_local_service(
    service_policy: &ClusterTunnelServicePolicy,
    request: &ClusterTunnelHttpRequest,
) -> Result<ClusterTunnelHttpResponse, ClusterTunnelError> {
    let body = request.body_bytes()?;
    if body.len() > service_policy.max_request_body_bytes {
        return Err(ClusterTunnelError::PayloadTooLarge {
            maximum_bytes: service_policy.max_request_body_bytes,
            actual_bytes: body.len(),
        });
    }
    let raw_request = encode_raw_http_request(service_policy, request, &body);
    let mut stream = TcpStream::connect(service_policy.local_addr)
        .await
        .map_err(|error| ClusterTunnelError::LocalServiceIo(error.to_string()))?;
    stream
        .write_all(&raw_request)
        .await
        .map_err(|error| ClusterTunnelError::LocalServiceIo(error.to_string()))?;
    stream
        .shutdown()
        .await
        .map_err(|error| ClusterTunnelError::LocalServiceIo(error.to_string()))?;
    let mut raw_response = Vec::new();
    stream
        .read_to_end(&mut raw_response)
        .await
        .map_err(|error| ClusterTunnelError::LocalServiceIo(error.to_string()))?;
    let response = decode_raw_http_response(&raw_response)?;
    let response_body_len = response.body_bytes()?.len();
    if response_body_len > service_policy.max_response_body_bytes {
        return Err(ClusterTunnelError::PayloadTooLarge {
            maximum_bytes: service_policy.max_response_body_bytes,
            actual_bytes: response_body_len,
        });
    }
    Ok(response)
}

fn encode_raw_http_request(
    service_policy: &ClusterTunnelServicePolicy,
    request: &ClusterTunnelHttpRequest,
    body: &[u8],
) -> Vec<u8> {
    let method = request.method.trim();
    let path = if request.path.starts_with('/') {
        request.path.clone()
    } else {
        format!("/{}", request.path.trim())
    };
    let mut raw = format!("{method} {path} HTTP/1.1\r\n");
    let mut saw_host = false;
    let mut saw_connection = false;
    let mut saw_content_length = false;
    for header in &request.headers {
        if header.name.eq_ignore_ascii_case("host") {
            saw_host = true;
        }
        if header.name.eq_ignore_ascii_case("connection") {
            saw_connection = true;
        }
        if header.name.eq_ignore_ascii_case("content-length") {
            saw_content_length = true;
        }
        raw.push_str(&header.name);
        raw.push_str(": ");
        raw.push_str(&header.value);
        raw.push_str("\r\n");
    }
    if !saw_host {
        raw.push_str(format!("Host: {}\r\n", service_policy.local_addr).as_str());
    }
    if !saw_connection {
        raw.push_str("Connection: close\r\n");
    }
    if !saw_content_length {
        raw.push_str(format!("Content-Length: {}\r\n", body.len()).as_str());
    }
    raw.push_str("\r\n");
    let mut encoded = raw.into_bytes();
    encoded.extend_from_slice(body);
    encoded
}

fn decode_raw_http_response(
    raw_response: &[u8],
) -> Result<ClusterTunnelHttpResponse, ClusterTunnelError> {
    let Some(header_end) = raw_response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
    else {
        return Err(ClusterTunnelError::LocalServiceProtocol(String::from(
            "response missing header terminator",
        )));
    };
    let header_bytes = &raw_response[..header_end];
    let body_bytes = &raw_response[header_end + 4..];
    let header_text = std::str::from_utf8(header_bytes)
        .map_err(|error| ClusterTunnelError::LocalServiceProtocol(error.to_string()))?;
    let mut lines = header_text.split("\r\n");
    let Some(status_line) = lines.next() else {
        return Err(ClusterTunnelError::LocalServiceProtocol(String::from(
            "response missing status line",
        )));
    };
    let mut status_parts = status_line.splitn(3, ' ');
    let Some(_http_version) = status_parts.next() else {
        return Err(ClusterTunnelError::LocalServiceProtocol(String::from(
            "response status line missing HTTP version",
        )));
    };
    let Some(status_code) = status_parts.next() else {
        return Err(ClusterTunnelError::LocalServiceProtocol(String::from(
            "response status line missing status code",
        )));
    };
    let status_code = status_code
        .parse::<u16>()
        .map_err(|error| ClusterTunnelError::LocalServiceProtocol(error.to_string()))?;
    let reason_phrase = status_parts.next().unwrap_or("").trim().to_owned();
    let mut response = ClusterTunnelHttpResponse::new(status_code, reason_phrase);
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(ClusterTunnelError::LocalServiceProtocol(String::from(
                "malformed response header",
            )));
        };
        response = response.with_header(name.trim(), value.trim());
    }
    Ok(response.with_body_bytes(body_bytes))
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

async fn send_wire_message_to_path(
    socket: &Arc<UdpSocket>,
    state: &Arc<Mutex<SharedState>>,
    config: &TransportConfig,
    peer_node_id: Option<&NodeId>,
    path: &ClusterTransportPath,
    _remote_addr: SocketAddr,
    message: WireMessage,
    is_hello: bool,
) -> Result<(), String> {
    let envelope = outbound_envelope(state, config, message).await?;
    let datagram = match path.kind {
        ClusterTransportPathKind::DirectDatagram
        | ClusterTransportPathKind::NatTraversalDatagram => TransportDatagram::Direct { envelope },
        ClusterTransportPathKind::RelayedDatagram => {
            let relay = path
                .relay
                .as_ref()
                .ok_or_else(|| String::from("relayed transport path requires relay metadata"))?;
            let target_node_id = peer_node_id
                .cloned()
                .ok_or_else(|| String::from("relayed transport path requires a peer node id"))?;
            TransportDatagram::RelayForward {
                relay_id: relay.relay_id.clone(),
                session_tag: relay.session_tag.clone(),
                source_node_id: config.local_identity.node_id.clone(),
                target_node_id,
                envelope,
            }
        }
    };
    let encoded = encode_transport_datagram(&datagram)?;
    if let Some(peer_node_id) = peer_node_id {
        state
            .lock()
            .await
            .record_outbound_message(peer_node_id, path, encoded.len(), is_hello);
    }
    let outbound_addr = match path.kind {
        ClusterTransportPathKind::DirectDatagram
        | ClusterTransportPathKind::NatTraversalDatagram => path.peer_addr,
        ClusterTransportPathKind::RelayedDatagram => {
            path.relay
                .as_ref()
                .ok_or_else(|| String::from("relayed transport path requires relay metadata"))?
                .relay_addr
        }
    };
    send_encoded_datagram(socket, outbound_addr, &encoded).await
}

fn authenticate_incoming_envelope(
    envelope: &WireEnvelope,
    transport: &InboundTransportContext,
    config: &TransportConfig,
    state: &SharedState,
) -> Result<Option<ClusterTrustRolloutDiagnostic>, ClusterJoinRefusalReason> {
    let mut trust_rollout_diagnostic = None;
    if matches!(
        config.trust_policy.posture,
        ClusterTrustPosture::AuthenticatedConfiguredPeers
            | ClusterTrustPosture::AttestedConfiguredPeers
    ) {
        let Some(configured_peer) = config
            .trust_policy
            .configured_peer(&envelope.message.sender().node_id)
        else {
            return Err(ClusterJoinRefusalReason::ConfiguredPeerUnknown);
        };
        match transport.path.kind {
            ClusterTransportPathKind::DirectDatagram => {
                if configured_peer.remote_addr != transport.path.peer_addr {
                    return Err(ClusterJoinRefusalReason::ConfiguredPeerAddressMismatch {
                        expected: configured_peer.remote_addr,
                        actual: transport.path.peer_addr,
                    });
                }
            }
            ClusterTransportPathKind::NatTraversalDatagram => {
                let Some(introduction) = state
                    .nat_introductions
                    .get(&envelope.message.sender().node_id)
                else {
                    return Err(ClusterJoinRefusalReason::ConfiguredPeerAddressMismatch {
                        expected: configured_peer.remote_addr,
                        actual: transport.path.peer_addr,
                    });
                };
                if introduction.peer_addr != transport.path.peer_addr {
                    return Err(ClusterJoinRefusalReason::ConfiguredPeerAddressMismatch {
                        expected: introduction.peer_addr,
                        actual: transport.path.peer_addr,
                    });
                }
            }
            ClusterTransportPathKind::RelayedDatagram => {
                let relay = transport.path.relay.as_ref().ok_or(
                    ClusterJoinRefusalReason::ConfiguredPeerAddressMismatch {
                        expected: configured_peer.remote_addr,
                        actual: transport.socket_remote_addr,
                    },
                )?;
                if state
                    .configured_peer_relay_endpoint(
                        &envelope.message.sender().node_id,
                        &relay.relay_id,
                        relay.relay_addr,
                        &relay.session_tag,
                        false,
                    )
                    .is_none()
                {
                    return Err(ClusterJoinRefusalReason::ConfiguredPeerAddressMismatch {
                        expected: configured_peer.remote_addr,
                        actual: transport.socket_remote_addr,
                    });
                }
            }
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
                remote_addr: transport.socket_remote_addr,
                expected_trust_bundle_version: config.trust_policy.trust_bundle_version,
                actual_trust_bundle_version: Some(actual_trust_bundle_version),
                key_match: Some(key_match),
                disposition: ClusterTrustRolloutDisposition::AcceptedOverlap,
            });
        }
        if matches!(
            config.trust_policy.posture,
            ClusterTrustPosture::AttestedConfiguredPeers
        ) {
            let Some(attestation_requirement) = configured_peer.attestation_requirement.as_ref()
            else {
                return Err(ClusterJoinRefusalReason::ConfiguredPeerAttestationRequirementMissing);
            };
            let Some(attestation) = envelope.message.sender().attestation.as_ref() else {
                return Err(ClusterJoinRefusalReason::NodeAttestationMissing);
            };
            if !attestation_requirement.matches(attestation) {
                return Err(ClusterJoinRefusalReason::ConfiguredPeerAttestationMismatch(
                    Box::new(ClusterPeerAttestationMismatch::between(
                        attestation_requirement,
                        attestation,
                    )),
                ));
            }
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

async fn run_relay_server(
    socket: Arc<UdpSocket>,
    mut shutdown_rx: oneshot::Receiver<()>,
) -> Result<(), String> {
    let mut state = RelayServerState::default();
    let mut recv_buf = vec![0_u8; MAX_DATAGRAM_BYTES];
    loop {
        tokio::select! {
            _ = &mut shutdown_rx => return Ok(()),
            received = socket.recv_from(&mut recv_buf) => {
                let (len, remote_addr) = received.map_err(|error| error.to_string())?;
                let datagram = match serde_json::from_slice::<TransportDatagram>(&recv_buf[..len]) {
                    Ok(datagram) => datagram,
                    Err(_) => continue,
                };
                match datagram {
                    TransportDatagram::RelayRegister {
                        relay_id,
                        session_tag,
                        sender_node_id,
                        target_node_id,
                        mode,
                    } => {
                        let session_key = RelaySessionKey { relay_id: relay_id.clone(), session_tag: session_tag.clone() };
                        state
                            .registrations
                            .entry(session_key.clone())
                            .or_default()
                            .insert(sender_node_id.clone(), RelayRegistration { remote_addr });
                        if matches!(mode, RelayDatagramMode::NatTraversal) {
                            if let Some(target) = state
                                .registrations
                                .get(&session_key)
                                .and_then(|registrations| registrations.get(&target_node_id))
                            {
                                send_transport_datagram(
                                    &socket,
                                    remote_addr,
                                    &TransportDatagram::NatIntroduction {
                                        relay_id: relay_id.clone(),
                                        session_tag: session_tag.clone(),
                                        peer_node_id: target_node_id.clone(),
                                        peer_addr: target.remote_addr,
                                    },
                                )
                                .await?;
                                send_transport_datagram(
                                    &socket,
                                    target.remote_addr,
                                    &TransportDatagram::NatIntroduction {
                                        relay_id,
                                        session_tag,
                                        peer_node_id: sender_node_id,
                                        peer_addr: remote_addr,
                                    },
                                )
                                .await?;
                            }
                        }
                    }
                    TransportDatagram::RelayForward {
                        relay_id,
                        session_tag,
                        source_node_id,
                        target_node_id,
                        envelope,
                    } => {
                        let session_key = RelaySessionKey { relay_id: relay_id.clone(), session_tag: session_tag.clone() };
                        state
                            .registrations
                            .entry(session_key.clone())
                            .or_default()
                            .insert(source_node_id.clone(), RelayRegistration { remote_addr });
                        if let Some(target) = state
                            .registrations
                            .get(&session_key)
                            .and_then(|registrations| registrations.get(&target_node_id))
                        {
                            send_transport_datagram(
                                &socket,
                                target.remote_addr,
                                &TransportDatagram::RelayDelivery {
                                    relay_id,
                                    session_tag,
                                    source_addr: remote_addr,
                                    source_node_id,
                                    envelope,
                                },
                            )
                            .await?;
                        } else {
                            send_transport_datagram(
                                &socket,
                                remote_addr,
                                &TransportDatagram::RelayTargetUnavailable {
                                    relay_id,
                                    session_tag,
                                    target_node_id,
                                },
                            )
                            .await?;
                        }
                    }
                    TransportDatagram::Direct { .. }
                    | TransportDatagram::RelayDelivery { .. }
                    | TransportDatagram::NatIntroduction { .. }
                    | TransportDatagram::RelayTargetUnavailable { .. } => {}
                }
            }
        }
    }
}

fn encode_transport_datagram(datagram: &TransportDatagram) -> Result<Vec<u8>, String> {
    serde_json::to_vec(datagram).map_err(|error| error.to_string())
}

async fn send_transport_datagram(
    socket: &Arc<UdpSocket>,
    remote_addr: SocketAddr,
    datagram: &TransportDatagram,
) -> Result<(), String> {
    let encoded = encode_transport_datagram(datagram)?;
    send_encoded_datagram(socket, remote_addr, &encoded).await
}

async fn send_encoded_datagram(
    socket: &Arc<UdpSocket>,
    remote_addr: SocketAddr,
    payload: &[u8],
) -> Result<(), String> {
    socket
        .send_to(payload, remote_addr)
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
    #![allow(clippy::expect_used, clippy::panic_in_result_fn)]

    use super::*;
    use tempfile::tempdir;

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
            attestation: None,
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
            network_state_persistence: ClusterNetworkStatePersistence::Ephemeral,
            introduction_policy: None,
            tunnel_policy: ClusterTunnelPolicy::default(),
            trust_policy,
        }
    }

    fn direct_transport(remote_addr: SocketAddr) -> InboundTransportContext {
        InboundTransportContext {
            socket_remote_addr: remote_addr,
            path: ClusterTransportPath::direct(remote_addr),
        }
    }

    fn sample_discovery_candidate(
        admission: &ClusterAdmissionConfig,
        node_id: &str,
        role: NodeRole,
        signing_key: &SigningKey,
        advertised_addrs: Vec<SocketAddr>,
    ) -> ClusterDiscoveryCandidate {
        ClusterDiscoveryCandidate::new(
            ClusterId::new(&admission.namespace, &admission.admission_token),
            admission.namespace.clone(),
            NodeId::new(node_id),
            role,
            encode_auth_public_key(&signing_key.verifying_key()),
            advertised_addrs,
        )
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
        let attested = ClusterTrustPolicy::attested_configured_peers(vec![
            ConfiguredClusterPeer::new(NodeId::new("node-b"), loopback_addr(31001), "peer-key-a")
                .with_attestation_requirement(
                    NodeAttestationRequirement::new("issuer-b", "attestation-b")
                        .with_device_identity_digest("device-b"),
                ),
        ]);
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
        let configured_other_discovery = configured
            .clone()
            .with_discovery_posture(ClusterDiscoveryPosture::ExplicitWiderNetworkRequested);

        assert_ne!(trusted_lan.stable_digest(), configured.stable_digest());
        assert_ne!(configured.stable_digest(), attested.stable_digest());
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
        assert_ne!(
            configured.stable_digest(),
            configured_other_discovery.stable_digest()
        );
    }

    #[test]
    fn non_lan_discovery_assessment_refuses_trusted_lan_seed_peers() {
        let assessment = ClusterTrustPolicy::trusted_lan().non_lan_discovery_assessment();

        assert_eq!(
            assessment.discovery_posture,
            ClusterDiscoveryPosture::TrustedLanSeedPeers
        );
        assert_eq!(
            assessment.trust_posture,
            ClusterTrustPosture::TrustedLanSharedAdmission
        );
        assert_eq!(
            assessment.disposition,
            ClusterNonLanDiscoveryDisposition::Refused
        );
        assert_eq!(
            assessment.refusal_reasons,
            vec![ClusterNonLanDiscoveryRefusalReason::TrustedLanSeedPeersOnly]
        );
    }

    #[test]
    fn non_lan_discovery_assessment_refuses_operator_managed_configured_peers() {
        let assessment =
            ClusterTrustPolicy::authenticated_configured_peers(vec![ConfiguredClusterPeer::new(
                NodeId::new("node-b"),
                loopback_addr(31001),
                "peer-key-a",
            )])
            .non_lan_discovery_assessment();

        assert_eq!(
            assessment.discovery_posture,
            ClusterDiscoveryPosture::OperatorManagedConfiguredPeers
        );
        assert_eq!(
            assessment.disposition,
            ClusterNonLanDiscoveryDisposition::Refused
        );
        assert_eq!(
            assessment.refusal_reasons,
            vec![ClusterNonLanDiscoveryRefusalReason::OperatorManagedConfiguredPeersOnly]
        );
    }

    #[test]
    fn explicit_wider_network_discovery_request_is_bounded_until_implemented() {
        let assessment = ClusterTrustPolicy::attested_configured_peers(vec![
            ConfiguredClusterPeer::new(NodeId::new("node-b"), loopback_addr(31003), "peer-key-a")
                .with_attestation_requirement(
                    NodeAttestationRequirement::new("issuer-b", "attestation-b")
                        .with_device_identity_digest("device-b"),
                ),
        ])
        .with_discovery_posture(ClusterDiscoveryPosture::ExplicitWiderNetworkRequested)
        .non_lan_discovery_assessment();

        assert_eq!(
            assessment.discovery_posture,
            ClusterDiscoveryPosture::ExplicitWiderNetworkRequested
        );
        assert_eq!(
            assessment.disposition,
            ClusterNonLanDiscoveryDisposition::Refused
        );
        assert_eq!(
            assessment.refusal_reasons,
            vec![ClusterNonLanDiscoveryRefusalReason::WiderNetworkDiscoveryUnimplemented]
        );
    }

    #[test]
    fn introduction_policy_digest_changes_with_sources_and_ttl() {
        let policy = ClusterIntroductionPolicy::new(
            vec![ClusterIntroductionSource::new("introducer-a", "key-a")],
            30_000,
        );
        let other_source = ClusterIntroductionPolicy::new(
            vec![ClusterIntroductionSource::new("introducer-b", "key-a")],
            30_000,
        );
        let other_ttl = ClusterIntroductionPolicy::new(
            vec![ClusterIntroductionSource::new("introducer-a", "key-a")],
            60_000,
        );

        assert_ne!(policy.stable_digest(), other_source.stable_digest());
        assert_ne!(policy.stable_digest(), other_ttl.stable_digest());
    }

    #[test]
    fn signed_cluster_introduction_verifies_under_matching_policy() {
        let admission = sample_admission();
        let candidate_signing_key = sample_signing_key(30);
        let candidate = sample_discovery_candidate(
            &admission,
            "candidate-a",
            NodeRole::ExecutorOnly,
            &candidate_signing_key,
            vec![loopback_addr(32001)],
        );
        let introducer_signing_key = sample_signing_key(31);
        let policy = ClusterIntroductionPolicy::new(
            vec![ClusterIntroductionSource::new(
                "introducer-a",
                encode_auth_public_key(&introducer_signing_key.verifying_key()),
            )],
            30_000,
        );
        let envelope = SignedClusterIntroductionEnvelope::sign(
            ClusterIntroductionPayload::new(candidate, 10_000, 20_000),
            "introducer-a",
            &introducer_signing_key,
        );

        assert_eq!(envelope.verify(&policy), Ok(()));
    }

    #[test]
    fn signed_cluster_introduction_refuses_untrusted_source() {
        let admission = sample_admission();
        let candidate_signing_key = sample_signing_key(32);
        let candidate = sample_discovery_candidate(
            &admission,
            "candidate-a",
            NodeRole::Mixed,
            &candidate_signing_key,
            vec![loopback_addr(32002)],
        );
        let introducer_signing_key = sample_signing_key(33);
        let policy = ClusterIntroductionPolicy::new(
            vec![ClusterIntroductionSource::new(
                "introducer-a",
                "different-key",
            )],
            30_000,
        );
        let envelope = SignedClusterIntroductionEnvelope::sign(
            ClusterIntroductionPayload::new(candidate, 10_000, 20_000),
            "introducer-a",
            &introducer_signing_key,
        );

        assert_eq!(
            envelope.verify(&policy),
            Err(ClusterIntroductionVerificationError::UntrustedSource {
                source_id: String::from("introducer-a"),
                public_key: encode_auth_public_key(&introducer_signing_key.verifying_key()),
            })
        );
    }

    #[test]
    fn signed_cluster_introduction_refuses_tampered_payload_digest() {
        let admission = sample_admission();
        let candidate_signing_key = sample_signing_key(34);
        let candidate = sample_discovery_candidate(
            &admission,
            "candidate-a",
            NodeRole::Mixed,
            &candidate_signing_key,
            vec![loopback_addr(32003)],
        );
        let introducer_signing_key = sample_signing_key(35);
        let policy = ClusterIntroductionPolicy::new(
            vec![ClusterIntroductionSource::new(
                "introducer-a",
                encode_auth_public_key(&introducer_signing_key.verifying_key()),
            )],
            30_000,
        );
        let mut envelope = SignedClusterIntroductionEnvelope::sign(
            ClusterIntroductionPayload::new(candidate, 10_000, 20_000),
            "introducer-a",
            &introducer_signing_key,
        );
        envelope.signature.payload_digest = String::from("tampered");

        assert!(matches!(
            envelope.verify(&policy),
            Err(ClusterIntroductionVerificationError::PayloadDigestMismatch { .. })
        ));
    }

    #[test]
    fn signed_cluster_introduction_refuses_ttl_that_exceeds_policy() {
        let admission = sample_admission();
        let candidate_signing_key = sample_signing_key(36);
        let candidate = sample_discovery_candidate(
            &admission,
            "candidate-a",
            NodeRole::Mixed,
            &candidate_signing_key,
            vec![loopback_addr(32004)],
        );
        let introducer_signing_key = sample_signing_key(37);
        let policy = ClusterIntroductionPolicy::new(
            vec![ClusterIntroductionSource::new(
                "introducer-a",
                encode_auth_public_key(&introducer_signing_key.verifying_key()),
            )],
            5_000,
        );
        let envelope = SignedClusterIntroductionEnvelope::sign(
            ClusterIntroductionPayload::new(candidate, 10_000, 20_000),
            "introducer-a",
            &introducer_signing_key,
        );

        assert_eq!(
            envelope.verify(&policy),
            Err(
                ClusterIntroductionVerificationError::CandidateTtlExceedsPolicy {
                    ttl_ms: 10_000,
                    maximum_ttl_ms: 5_000,
                }
            )
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
        assert_eq!(
            assessment.discovery_posture,
            ClusterDiscoveryPosture::TrustedLanSeedPeers
        );
        assert_eq!(assessment.trust_policy_digest, trusted_lan.stable_digest());
        assert_eq!(
            assessment.refusal_reasons,
            vec![
                ClusterComputeMarketTrustRefusalReason::TrustedLanSharedAdmissionOnly,
                ClusterComputeMarketTrustRefusalReason::MissingAuthenticatedTransport,
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
        assert_eq!(
            assessment.discovery_posture,
            ClusterDiscoveryPosture::OperatorManagedConfiguredPeers
        );
        assert_eq!(assessment.trust_policy_digest, configured.stable_digest());
        assert_eq!(
            assessment.refusal_reasons,
            vec![
                ClusterComputeMarketTrustRefusalReason::OperatorManagedConfiguredPeersOnly,
                ClusterComputeMarketTrustRefusalReason::MissingAttestedNodeIdentityAdmission,
                ClusterComputeMarketTrustRefusalReason::MissingNonLanDiscoveryPosture,
            ]
        );
    }

    #[test]
    fn compute_market_assessment_for_attested_peers_only_waits_on_non_lan_discovery() {
        let attested = ClusterTrustPolicy::attested_configured_peers(vec![
            ConfiguredClusterPeer::new(NodeId::new("node-b"), loopback_addr(31003), "peer-key-a")
                .with_attestation_requirement(
                    NodeAttestationRequirement::new("issuer-b", "attestation-b")
                        .with_device_identity_digest("device-b"),
                ),
        ]);

        let assessment = attested.compute_market_trust_assessment();

        assert_eq!(
            assessment.disposition,
            ClusterComputeMarketTrustDisposition::Refused
        );
        assert_eq!(
            assessment.posture,
            ClusterTrustPosture::AttestedConfiguredPeers
        );
        assert_eq!(
            assessment.discovery_posture,
            ClusterDiscoveryPosture::OperatorManagedConfiguredPeers
        );
        assert_eq!(
            assessment.refusal_reasons,
            vec![ClusterComputeMarketTrustRefusalReason::MissingNonLanDiscoveryPosture]
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
    fn attested_configured_peers_refuse_missing_node_attestation() {
        let admission = sample_admission();
        let local_signing_key = sample_signing_key(16);
        let local_identity = sample_identity(
            &admission,
            "local",
            NodeRole::CoordinatorOnly,
            &local_signing_key,
        );
        let remote_signing_key = sample_signing_key(17);
        let remote_identity = sample_identity(
            &admission,
            "remote",
            NodeRole::ExecutorOnly,
            &remote_signing_key,
        );
        let remote_addr = loopback_addr(31004);
        let config = sample_transport_config(
            local_identity,
            local_signing_key,
            ClusterTrustPolicy::attested_configured_peers(vec![
                ConfiguredClusterPeer::new(
                    remote_identity.node_id.clone(),
                    remote_addr,
                    remote_identity.auth_public_key.clone(),
                )
                .with_attestation_requirement(
                    NodeAttestationRequirement::new("issuer-remote", "attestation-remote")
                        .with_device_identity_digest("device-remote"),
                ),
            ]),
        );
        let envelope = signed_ping_envelope(
            &config.namespace,
            &config.admission_digest,
            Some(config.trust_policy.trust_bundle_version),
            remote_identity,
            &remote_signing_key,
            1,
            7,
        );

        let state = SharedState::new(
            BTreeSet::new(),
            &config.trust_policy,
            &ClusterTunnelPolicy::default(),
            PersistedClusterNetworkState::empty(),
            ClusterNetworkStatePersistence::Ephemeral,
        );
        let refusal = authenticate_incoming_envelope(
            &envelope,
            &direct_transport(remote_addr),
            &config,
            &state,
        );

        assert_eq!(
            refusal,
            Err(ClusterJoinRefusalReason::NodeAttestationMissing)
        );
    }

    #[test]
    fn attested_configured_peers_refuse_mismatched_node_attestation() {
        let admission = sample_admission();
        let local_signing_key = sample_signing_key(18);
        let local_identity = sample_identity(
            &admission,
            "local",
            NodeRole::CoordinatorOnly,
            &local_signing_key,
        );
        let remote_signing_key = sample_signing_key(19);
        let mut remote_identity = sample_identity(
            &admission,
            "remote",
            NodeRole::ExecutorOnly,
            &remote_signing_key,
        );
        remote_identity.attestation = Some(
            NodeAttestationEvidence::new("issuer-remote", "wrong-attestation")
                .with_device_identity_digest("device-remote"),
        );
        let remote_addr = loopback_addr(31005);
        let config = sample_transport_config(
            local_identity,
            local_signing_key,
            ClusterTrustPolicy::attested_configured_peers(vec![
                ConfiguredClusterPeer::new(
                    remote_identity.node_id.clone(),
                    remote_addr,
                    remote_identity.auth_public_key.clone(),
                )
                .with_attestation_requirement(
                    NodeAttestationRequirement::new("issuer-remote", "attestation-remote")
                        .with_device_identity_digest("device-remote"),
                ),
            ]),
        );
        let envelope = signed_ping_envelope(
            &config.namespace,
            &config.admission_digest,
            Some(config.trust_policy.trust_bundle_version),
            remote_identity,
            &remote_signing_key,
            1,
            7,
        );

        let state = SharedState::new(
            BTreeSet::new(),
            &config.trust_policy,
            &ClusterTunnelPolicy::default(),
            PersistedClusterNetworkState::empty(),
            ClusterNetworkStatePersistence::Ephemeral,
        );
        let refusal = authenticate_incoming_envelope(
            &envelope,
            &direct_transport(remote_addr),
            &config,
            &state,
        );

        assert_eq!(
            refusal,
            Err(ClusterJoinRefusalReason::ConfiguredPeerAttestationMismatch(
                Box::new(ClusterPeerAttestationMismatch {
                    expected_issuer: String::from("issuer-remote"),
                    actual_issuer: Some(String::from("issuer-remote")),
                    expected_attestation_digest: String::from("attestation-remote"),
                    actual_attestation_digest: Some(String::from("wrong-attestation")),
                    expected_device_identity_digest: Some(String::from("device-remote")),
                    actual_device_identity_digest: Some(String::from("device-remote")),
                }),
            ))
        );
    }

    #[test]
    fn attested_configured_peers_accept_matching_node_attestation() {
        let admission = sample_admission();
        let local_signing_key = sample_signing_key(20);
        let local_identity = sample_identity(
            &admission,
            "local",
            NodeRole::CoordinatorOnly,
            &local_signing_key,
        );
        let remote_signing_key = sample_signing_key(21);
        let mut remote_identity = sample_identity(
            &admission,
            "remote",
            NodeRole::ExecutorOnly,
            &remote_signing_key,
        );
        remote_identity.attestation = Some(
            NodeAttestationEvidence::new("issuer-remote", "attestation-remote")
                .with_device_identity_digest("device-remote"),
        );
        let remote_addr = loopback_addr(31006);
        let config = sample_transport_config(
            local_identity,
            local_signing_key,
            ClusterTrustPolicy::attested_configured_peers(vec![
                ConfiguredClusterPeer::new(
                    remote_identity.node_id.clone(),
                    remote_addr,
                    remote_identity.auth_public_key.clone(),
                )
                .with_attestation_requirement(
                    NodeAttestationRequirement::new("issuer-remote", "attestation-remote")
                        .with_device_identity_digest("device-remote"),
                ),
            ]),
        );
        let envelope = signed_ping_envelope(
            &config.namespace,
            &config.admission_digest,
            Some(config.trust_policy.trust_bundle_version),
            remote_identity,
            &remote_signing_key,
            1,
            7,
        );

        let state = SharedState::new(
            BTreeSet::new(),
            &config.trust_policy,
            &ClusterTunnelPolicy::default(),
            PersistedClusterNetworkState::empty(),
            ClusterNetworkStatePersistence::Ephemeral,
        );
        let refusal = authenticate_incoming_envelope(
            &envelope,
            &direct_transport(remote_addr),
            &config,
            &state,
        );

        assert!(refusal.is_ok(), "matching attestation should be accepted");
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

        let state = SharedState::new(
            BTreeSet::new(),
            &config.trust_policy,
            &ClusterTunnelPolicy::default(),
            PersistedClusterNetworkState::empty(),
            ClusterNetworkStatePersistence::Ephemeral,
        );
        let refusal = authenticate_incoming_envelope(
            &envelope,
            &direct_transport(remote_addr),
            &config,
            &state,
        );
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
        let mut state = SharedState::new(
            BTreeSet::new(),
            &ClusterTrustPolicy::trusted_lan(),
            &ClusterTunnelPolicy::default(),
            PersistedClusterNetworkState::empty(),
            ClusterNetworkStatePersistence::Ephemeral,
        );

        let hello = state.record_hello(
            remote_addr,
            remote_identity.clone(),
            ClusterTransportPath::direct(remote_addr),
            64,
            Some(12),
            8,
        );
        assert!(
            hello.is_ok(),
            "first authenticated hello should be accepted"
        );

        let refusal = state.record_ping(
            remote_addr,
            remote_identity,
            ClusterTransportPath::direct(remote_addr),
            64,
            0,
            Some(12),
            8,
        );
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

    #[test]
    fn authenticated_configured_peers_accept_relay_fallback_path_when_relay_is_configured() {
        let admission = sample_admission();
        let local_signing_key = sample_signing_key(41);
        let local_identity = sample_identity(
            &admission,
            "local",
            NodeRole::CoordinatorOnly,
            &local_signing_key,
        );
        let remote_signing_key = sample_signing_key(42);
        let remote_identity = sample_identity(
            &admission,
            "remote",
            NodeRole::ExecutorOnly,
            &remote_signing_key,
        );
        let relay = ClusterRelayEndpoint::new("relay-a", loopback_addr(32021), "pair-a");
        let config = sample_transport_config(
            local_identity,
            local_signing_key,
            ClusterTrustPolicy::authenticated_configured_peers(vec![
                ConfiguredClusterPeer::new(
                    remote_identity.node_id.clone(),
                    loopback_addr(32022),
                    remote_identity.auth_public_key.clone(),
                )
                .with_relay_fallback_relays(vec![relay.clone()]),
            ]),
        );
        let state = SharedState::new(
            BTreeSet::new(),
            &config.trust_policy,
            &ClusterTunnelPolicy::default(),
            PersistedClusterNetworkState::empty(),
            ClusterNetworkStatePersistence::Ephemeral,
        );
        let envelope = signed_ping_envelope(
            &config.namespace,
            &config.admission_digest,
            Some(config.trust_policy.trust_bundle_version),
            remote_identity,
            &remote_signing_key,
            1,
            9,
        );

        let outcome = authenticate_incoming_envelope(
            &envelope,
            &InboundTransportContext {
                socket_remote_addr: relay.relay_addr,
                path: ClusterTransportPath::relayed(loopback_addr(32023), relay),
            },
            &config,
            &state,
        );

        assert!(
            outcome.is_ok(),
            "configured relay fallback should authenticate"
        );
    }

    #[test]
    fn logical_stream_reservation_respects_peer_capacity() {
        let admission = sample_admission();
        let remote_signing_key = sample_signing_key(51);
        let remote_identity = sample_identity(
            &admission,
            "remote",
            NodeRole::ExecutorOnly,
            &remote_signing_key,
        );
        let remote_addr = loopback_addr(32031);
        let trust_policy = ClusterTrustPolicy::authenticated_configured_peers(vec![
            ConfiguredClusterPeer::new(
                remote_identity.node_id.clone(),
                remote_addr,
                remote_identity.auth_public_key.clone(),
            )
            .with_max_concurrent_streams(2),
        ]);
        let mut state = SharedState::new(
            BTreeSet::new(),
            &trust_policy,
            &ClusterTunnelPolicy::default(),
            PersistedClusterNetworkState::empty(),
            ClusterNetworkStatePersistence::Ephemeral,
        );

        let hello = state.record_hello(
            remote_addr,
            remote_identity.clone(),
            ClusterTransportPath::direct(remote_addr),
            64,
            Some(1),
            8,
        );
        assert!(
            hello.is_ok(),
            "peer should be established before opening streams"
        );

        let first =
            state.open_logical_stream(&remote_identity.node_id, ClusterLogicalStreamKind::Serving);
        assert!(first.is_ok(), "first stream should open");
        let second = state.open_logical_stream(
            &remote_identity.node_id,
            ClusterLogicalStreamKind::Collective,
        );
        assert!(second.is_ok(), "second stream should open");
        let third =
            state.open_logical_stream(&remote_identity.node_id, ClusterLogicalStreamKind::Artifact);
        assert_eq!(
            third,
            Err(ClusterStreamError::CapacityExceeded {
                peer_node_id: remote_identity.node_id.clone(),
                max_concurrent_streams: 2,
            })
        );
        let second = second.unwrap_or_else(|_| unreachable!("assert above ensures success"));
        let close = state.close_logical_stream(&second);
        assert!(close.is_ok(), "closing an active stream should succeed");
        let active_streams = state.active_logical_streams();
        assert_eq!(active_streams.len(), 1);
        assert_eq!(active_streams[0].kind, ClusterLogicalStreamKind::Serving);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn file_backed_network_state_persists_trust_bundles_and_candidate_history() {
        let temp = tempdir().unwrap_or_else(|_| unreachable!("tempdir should succeed"));
        let network_state_path = temp.path().join("network-state.json");
        let introducer_signing_key = sample_signing_key(61);
        let introduction_policy = ClusterIntroductionPolicy::new(
            vec![ClusterIntroductionSource::new(
                "introducer-a",
                encode_auth_public_key(&introducer_signing_key.verifying_key()),
            )],
            30_000,
        );
        let candidate_signing_key = sample_signing_key(62);
        let candidate = sample_discovery_candidate(
            &sample_admission(),
            "candidate-a",
            NodeRole::ExecutorOnly,
            &candidate_signing_key,
            vec![loopback_addr(32041)],
        );
        let envelope = SignedClusterIntroductionEnvelope::sign(
            ClusterIntroductionPayload::new(candidate.clone(), 10_000, 20_000),
            "introducer-a",
            &introducer_signing_key,
        );

        let node = LocalClusterNode::spawn(
            LocalClusterConfig::new(
                "lan-alpha",
                "shared-secret",
                loopback_addr(0),
                NodeRole::CoordinatorOnly,
            )
            .with_introduction_policy(introduction_policy.clone())
            .with_file_backed_network_state(network_state_path.clone())
            .with_trust_policy(
                ClusterTrustPolicy::authenticated_configured_peers(vec![
                    ConfiguredClusterPeer::new(
                        NodeId::new("peer-b"),
                        loopback_addr(32051),
                        "peer-key-b",
                    ),
                ])
                .with_trust_bundle_version(1),
            ),
        )
        .await;
        assert!(node.is_ok(), "node should start");
        let node = node
            .ok()
            .unwrap_or_else(|| unreachable!("assert above ensures success"));

        let initial_bundles = node.trust_bundle_history().await;
        assert_eq!(initial_bundles.len(), 1);
        assert_eq!(initial_bundles[0].trust_policy.trust_bundle_version, 1);

        let introduced = node
            .record_verified_candidate_introduction(envelope.clone(), 15_000)
            .await;
        assert!(introduced.is_ok(), "introduction should persist");
        let refused = node
            .record_candidate_refusal(&candidate.node_id, 16_000, "policy_refused")
            .await;
        assert!(refused.is_ok(), "refusal should persist");
        let promoted = node
            .promote_candidate(&candidate.node_id, 17_000, "promoted_for_admission")
            .await;
        assert!(promoted.is_ok(), "promotion should persist");
        let revoked = node
            .revoke_candidate(&candidate.node_id, 18_000, "operator_revoked")
            .await;
        assert!(revoked.is_ok(), "revocation should persist");
        let reintroduced = node
            .record_verified_candidate_introduction(envelope.clone(), 19_000)
            .await;
        assert!(reintroduced.is_ok(), "re-introduction should persist");

        let shutdown = node.shutdown().await;
        assert!(shutdown.is_ok(), "node should shut down cleanly");

        let restarted = LocalClusterNode::spawn(
            LocalClusterConfig::new(
                "lan-alpha",
                "shared-secret",
                loopback_addr(0),
                NodeRole::CoordinatorOnly,
            )
            .with_introduction_policy(introduction_policy.clone())
            .with_file_backed_network_state(network_state_path.clone())
            .with_trust_policy(
                ClusterTrustPolicy::authenticated_configured_peers(vec![
                    ConfiguredClusterPeer::new(
                        NodeId::new("peer-b"),
                        loopback_addr(32051),
                        "peer-key-b",
                    ),
                ])
                .with_trust_bundle_version(2),
            ),
        )
        .await;
        assert!(restarted.is_ok(), "restarted node should start");
        let restarted = restarted
            .ok()
            .unwrap_or_else(|| unreachable!("assert above ensures success"));

        let trust_bundles = restarted.trust_bundle_history().await;
        assert_eq!(trust_bundles.len(), 2);
        assert_eq!(trust_bundles[0].trust_policy.trust_bundle_version, 1);
        assert!(trust_bundles[0].superseded_at_ms.is_some());
        assert_eq!(trust_bundles[1].trust_policy.trust_bundle_version, 2);

        let candidate_records = restarted.candidate_records().await;
        assert_eq!(candidate_records.len(), 1);
        let record = &candidate_records[0];
        assert_eq!(record.node_id, candidate.node_id);
        assert_eq!(record.disposition, ClusterCandidateDisposition::Introduced);
        assert_eq!(
            record
                .history
                .iter()
                .map(|event| event.disposition)
                .collect::<Vec<_>>(),
            vec![
                ClusterCandidateDisposition::Introduced,
                ClusterCandidateDisposition::Refused,
                ClusterCandidateDisposition::Promoted,
                ClusterCandidateDisposition::Revoked,
                ClusterCandidateDisposition::Introduced,
            ]
        );

        let expired = restarted.expire_candidates(25_000).await;
        assert!(expired.is_ok(), "candidate expiry should persist");
        let expired = expired
            .ok()
            .unwrap_or_else(|| unreachable!("assert above ensures success"));
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].disposition, ClusterCandidateDisposition::Expired);

        let restarted_shutdown = restarted.shutdown().await;
        assert!(
            restarted_shutdown.is_ok(),
            "restarted node should shut down cleanly"
        );

        let replayed = LocalClusterNode::spawn(
            LocalClusterConfig::new(
                "lan-alpha",
                "shared-secret",
                loopback_addr(0),
                NodeRole::CoordinatorOnly,
            )
            .with_introduction_policy(introduction_policy)
            .with_file_backed_network_state(network_state_path)
            .with_trust_policy(
                ClusterTrustPolicy::authenticated_configured_peers(vec![
                    ConfiguredClusterPeer::new(
                        NodeId::new("peer-b"),
                        loopback_addr(32051),
                        "peer-key-b",
                    ),
                ])
                .with_trust_bundle_version(2),
            ),
        )
        .await;
        assert!(replayed.is_ok(), "replayed node should start");
        let replayed = replayed
            .ok()
            .unwrap_or_else(|| unreachable!("assert above ensures success"));
        let replayed_records = replayed.candidate_records().await;
        assert_eq!(replayed_records.len(), 1);
        assert_eq!(
            replayed_records[0].disposition,
            ClusterCandidateDisposition::Expired
        );
        assert_eq!(replayed_records[0].history.len(), 6);
        let replayed_shutdown = replayed.shutdown().await;
        assert!(
            replayed_shutdown.is_ok(),
            "replayed node should shut down cleanly"
        );
    }
}
