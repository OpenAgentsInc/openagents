use std::{fs, net::SocketAddr, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ClusterAdmissionConfig, ClusterError, ClusterIntroductionPolicy, ClusterTrustPolicy,
    ClusterTunnelPolicy, LocalClusterConfig, NodeAttestationEvidence, NodeRole,
};

/// Schema version for persisted operator cluster manifests.
pub const CLUSTER_OPERATOR_MANIFEST_SCHEMA_VERSION: u32 = 1;

/// Reusable operator-managed manifest for one local cluster node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterOperatorManifest {
    /// Explicit schema version for stable upgrade handling.
    pub schema_version: u32,
    /// Cluster namespace and admission configuration.
    pub admission: ClusterAdmissionConfig,
    /// Local socket address to bind.
    pub bind_addr: SocketAddr,
    /// Explicit seed peers for first contact.
    pub seed_peers: Vec<SocketAddr>,
    /// Declared role for this node.
    pub role: NodeRole,
    /// Optional attestation facts attached to the local node identity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_attestation: Option<NodeAttestationEvidence>,
    /// Optional policy for accepted wider-network introduction sources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub introduction_policy: Option<ClusterIntroductionPolicy>,
    /// Policy for bounded service tunnels owned by this node.
    #[serde(default)]
    pub tunnel_policy: ClusterTunnelPolicy,
    /// Machine-checkable trust policy for the node.
    pub trust_policy: ClusterTrustPolicy,
}

impl ClusterOperatorManifest {
    /// Creates an operator manifest from one local cluster config.
    #[must_use]
    pub fn from_local_config(config: &LocalClusterConfig) -> Self {
        let mut seed_peers = config.seed_peers.clone();
        seed_peers.sort_unstable();
        seed_peers.dedup();
        Self {
            schema_version: CLUSTER_OPERATOR_MANIFEST_SCHEMA_VERSION,
            admission: config.admission.clone(),
            bind_addr: config.bind_addr,
            seed_peers,
            role: config.role,
            node_attestation: config.node_attestation.clone(),
            introduction_policy: config.introduction_policy.clone(),
            tunnel_policy: config.tunnel_policy.clone(),
            trust_policy: config.trust_policy.clone(),
        }
    }

    /// Returns a stable digest for rollout and drift checks.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"cluster_operator_manifest|");
        hasher.update(self.schema_version.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(self.admission.namespace.as_str().as_bytes());
        hasher.update(b"|");
        hasher.update(self.admission.admission_token.as_str().as_bytes());
        hasher.update(b"|");
        hasher.update(self.bind_addr.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(match self.role {
            NodeRole::CoordinatorOnly => b"coordinator_only".as_slice(),
            NodeRole::ExecutorOnly => b"executor_only".as_slice(),
            NodeRole::Mixed => b"mixed".as_slice(),
        });
        if let Some(node_attestation) = &self.node_attestation {
            hasher.update(b"|node_attestation_issuer|");
            hasher.update(node_attestation.issuer.as_bytes());
            hasher.update(b"|node_attestation_digest|");
            hasher.update(node_attestation.attestation_digest.as_bytes());
            if let Some(device_identity_digest) = &node_attestation.device_identity_digest {
                hasher.update(b"|node_device_identity_digest|");
                hasher.update(device_identity_digest.as_bytes());
            }
        }
        if let Some(introduction_policy) = &self.introduction_policy {
            hasher.update(b"|introduction_policy|");
            hasher.update(introduction_policy.stable_digest().as_bytes());
        }
        hasher.update(b"|tunnel_policy|");
        hasher.update(self.tunnel_policy.stable_digest().as_bytes());
        for seed_peer in &self.seed_peers {
            hasher.update(b"|seed|");
            hasher.update(seed_peer.to_string().as_bytes());
        }
        hasher.update(b"|trust|");
        hasher.update(self.trust_policy.stable_digest().as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Loads one operator manifest from a JSON file.
    pub fn load_json(path: impl AsRef<Path>) -> Result<Self, ClusterError> {
        let bytes = fs::read(path).map_err(ClusterError::ManifestIo)?;
        let manifest: Self =
            serde_json::from_slice(&bytes).map_err(ClusterError::ManifestFormat)?;
        manifest.validate_schema()?;
        Ok(manifest)
    }

    /// Stores one operator manifest as stable pretty JSON.
    pub fn store_json(&self, path: impl AsRef<Path>) -> Result<(), ClusterError> {
        self.validate_schema()?;
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(ClusterError::ManifestIo)?;
        }
        let encoded = serde_json::to_vec_pretty(self).map_err(ClusterError::ManifestFormat)?;
        fs::write(path, encoded).map_err(ClusterError::ManifestIo)
    }

    fn validate_schema(&self) -> Result<(), ClusterError> {
        if self.schema_version != CLUSTER_OPERATOR_MANIFEST_SCHEMA_VERSION {
            return Err(ClusterError::ManifestSchemaVersion {
                expected: CLUSTER_OPERATOR_MANIFEST_SCHEMA_VERSION,
                actual: self.schema_version,
            });
        }
        Ok(())
    }
}

impl From<ClusterOperatorManifest> for LocalClusterConfig {
    fn from(manifest: ClusterOperatorManifest) -> Self {
        Self {
            admission: manifest.admission,
            bind_addr: manifest.bind_addr,
            seed_peers: manifest.seed_peers,
            role: manifest.role,
            identity_persistence: crate::NodeIdentityPersistence::Ephemeral,
            network_state_persistence: crate::ClusterNetworkStatePersistence::Ephemeral,
            node_attestation: manifest.node_attestation,
            introduction_policy: manifest.introduction_policy,
            tunnel_policy: manifest.tunnel_policy,
            trust_policy: manifest.trust_policy,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::tempdir;

    use super::*;
    use crate::{
        ClusterIntroductionPolicy, ClusterIntroductionSource, ClusterTrustPosture,
        ClusterTunnelPolicy, ClusterTunnelServiceKind, ClusterTunnelServicePolicy,
        ConfiguredClusterPeer, NodeAttestationRequirement, NodeId,
    };

    fn loopback_addr(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }

    fn sample_manifest() -> ClusterOperatorManifest {
        ClusterOperatorManifest {
            schema_version: CLUSTER_OPERATOR_MANIFEST_SCHEMA_VERSION,
            admission: ClusterAdmissionConfig::new("multi-subnet-alpha", "shared-secret"),
            bind_addr: loopback_addr(31011),
            seed_peers: vec![loopback_addr(31012)],
            role: NodeRole::Mixed,
            node_attestation: Some(
                NodeAttestationEvidence::new("issuer-a", "attestation-a")
                    .with_device_identity_digest("device-a"),
            ),
            introduction_policy: Some(ClusterIntroductionPolicy::new(
                vec![ClusterIntroductionSource::new(
                    "introducer-a",
                    "introducer-key-a",
                )],
                30_000,
            )),
            tunnel_policy: ClusterTunnelPolicy::new(vec![
                ClusterTunnelServicePolicy::new_http(
                    "desktop-control",
                    ClusterTunnelServiceKind::DesktopControlHttp,
                    loopback_addr(41000),
                )
                .with_max_request_body_bytes(2048)
                .with_max_response_body_bytes(2048),
            ]),
            trust_policy: ClusterTrustPolicy::attested_configured_peers(vec![
                ConfiguredClusterPeer::new(NodeId::new("peer-b"), loopback_addr(31012), "peer-key")
                    .with_attestation_requirement(
                        NodeAttestationRequirement::new("issuer-b", "attestation-b")
                            .with_device_identity_digest("device-b"),
                    ),
            ]),
        }
    }

    #[test]
    fn operator_manifest_round_trips_through_json() {
        let temp = tempdir().unwrap_or_else(|_| unreachable!("tempdir should succeed"));
        let path: PathBuf = temp.path().join("cluster-manifest.json");
        let manifest = sample_manifest();

        let stored = manifest.store_json(&path);
        assert!(stored.is_ok(), "manifest should store");

        let loaded = ClusterOperatorManifest::load_json(&path);
        assert!(loaded.is_ok(), "manifest should load");
        assert_eq!(
            loaded.unwrap_or_else(|_| unreachable!("assert above ensures success")),
            manifest
        );
    }

    #[test]
    fn operator_manifest_digest_changes_when_rollout_inputs_change() {
        let manifest = sample_manifest();
        let mut changed_manifest = manifest.clone();
        changed_manifest.trust_policy =
            ClusterTrustPolicy::authenticated_configured_peers(vec![ConfiguredClusterPeer::new(
                NodeId::new("peer-b"),
                loopback_addr(31012),
                "different-peer-key",
            )]);

        assert_ne!(manifest.stable_digest(), changed_manifest.stable_digest());
    }

    #[test]
    fn local_cluster_config_can_be_built_from_manifest() {
        let manifest = sample_manifest();
        let config = LocalClusterConfig::from_operator_manifest(manifest.clone());

        assert_eq!(config.admission, manifest.admission);
        assert_eq!(config.bind_addr, manifest.bind_addr);
        assert_eq!(config.seed_peers, manifest.seed_peers);
        assert_eq!(config.role, manifest.role);
        assert_eq!(config.node_attestation, manifest.node_attestation);
        assert_eq!(config.introduction_policy, manifest.introduction_policy);
        assert_eq!(config.tunnel_policy, manifest.tunnel_policy);
        assert_eq!(config.trust_policy, manifest.trust_policy);
        assert_eq!(
            config.trust_policy.posture,
            ClusterTrustPosture::AttestedConfiguredPeers
        );
    }

    #[test]
    fn manifest_load_refuses_unsupported_schema_version() {
        let temp = tempdir().unwrap_or_else(|_| unreachable!("tempdir should succeed"));
        let path: PathBuf = temp.path().join("cluster-manifest.json");
        let manifest = ClusterOperatorManifest {
            schema_version: CLUSTER_OPERATOR_MANIFEST_SCHEMA_VERSION + 1,
            ..sample_manifest()
        };
        let encoded =
            serde_json::to_vec_pretty(&manifest).unwrap_or_else(|_| unreachable!("serialize"));
        fs::write(&path, encoded).unwrap_or_else(|_| unreachable!("manifest write should work"));

        let stored = ClusterOperatorManifest::load_json(&path);
        assert!(
            matches!(
                stored,
                Err(ClusterError::ManifestSchemaVersion {
                    expected: CLUSTER_OPERATOR_MANIFEST_SCHEMA_VERSION,
                    actual
                }) if actual == CLUSTER_OPERATOR_MANIFEST_SCHEMA_VERSION + 1
            ),
            "unsupported schema should be refused before store"
        );
    }
}
