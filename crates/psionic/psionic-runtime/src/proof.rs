use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ActivationFingerprintProofArtifact, BackendToolchainIdentity, ClusterExecutionContext,
    ClusterPolicyDigest, ClusterTransportClass, CompilePathEvidence, DeviceInventoryQualifiers,
    ExecutionDeliveryProof, ExecutionTopologyKind, ExecutionTopologyPlan, LocalRuntimeDiagnostic,
    SandboxExecutionEvidence, SandboxExecutionExit, SandboxExecutionResourceSummary,
    ServedArtifactIdentity, SettlementLinkageInput, ValidationMatrixReference,
};

/// Current canonical execution-proof bundle schema version.
pub const EXECUTION_PROOF_BUNDLE_SCHEMA_VERSION: u16 = 1;

/// High-level family of execution covered by one canonical proof bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionProofBundleKind {
    /// Work remained local to one runtime path.
    Local,
    /// Work traversed clustered placement or routing.
    Clustered,
    /// Work ran inside a bounded sandbox execution lane.
    Sandbox,
}

/// Terminal status represented by one canonical proof bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionProofBundleStatus {
    /// Execution completed successfully.
    Succeeded,
    /// Execution was cancelled by the caller.
    Cancelled,
    /// Execution was interrupted by a disconnect.
    Disconnected,
    /// Execution failed before successful completion.
    Failed,
}

/// Product posture for one optional proof augmentation layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionProofAugmentationPosture {
    /// The product family does not surface this augmentation layer.
    Unavailable,
    /// The product family can surface this augmentation layer when applicable.
    Supported,
    /// The product family requires this augmentation layer for truthful delivery.
    Required,
}

/// Runtime identity facts that remain stable across receipt, settlement, and challenge layers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionProofRuntimeIdentity {
    /// Runtime backend that realized the request.
    pub runtime_backend: String,
    /// Explicit compile-vs-probe toolchain truth for that path.
    pub backend_toolchain: BackendToolchainIdentity,
    /// First selected device surfaced for the path, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_device_inventory: Option<DeviceInventoryQualifiers>,
    /// All selected devices participating in the path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_devices: Vec<DeviceInventoryQualifiers>,
}

impl ExecutionProofRuntimeIdentity {
    /// Creates runtime identity from explicit backend/toolchain truth.
    #[must_use]
    pub fn new(
        runtime_backend: impl Into<String>,
        backend_toolchain: BackendToolchainIdentity,
    ) -> Self {
        Self {
            runtime_backend: runtime_backend.into(),
            backend_toolchain,
            selected_device_inventory: None,
            selected_devices: Vec::new(),
        }
    }

    /// Attaches the first selected device surfaced for the path.
    #[must_use]
    pub fn with_selected_device_inventory(
        mut self,
        selected_device_inventory: Option<DeviceInventoryQualifiers>,
    ) -> Self {
        self.selected_device_inventory = selected_device_inventory;
        self
    }

    /// Attaches all selected devices participating in the path.
    #[must_use]
    pub fn with_selected_devices(
        mut self,
        selected_devices: Vec<DeviceInventoryQualifiers>,
    ) -> Self {
        self.selected_devices = selected_devices;
        self
    }
}

/// Topology and routing facts carried by one canonical execution-proof bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionProofTopology {
    /// High-level topology kind for the realized path, when one is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology_kind: Option<ExecutionTopologyKind>,
    /// Stable digest of the topology assignment facts, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology_digest: Option<String>,
    /// Cluster transport class used by the realized path, when clustered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<ClusterTransportClass>,
    /// Stable cluster identifier when clustered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_id: Option<String>,
    /// Stable cluster-state digest when clustered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_state_digest: Option<String>,
    /// Stable scheduler node when clustered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler_node_id: Option<String>,
    /// Stable selected-node identifiers surfaced for the path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_node_ids: Vec<String>,
    /// Stable replica-node identifiers surfaced for replicated routing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replica_node_ids: Vec<String>,
    /// Stable cluster-policy digests constraining the path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_digests: Vec<ClusterPolicyDigest>,
}

impl ExecutionProofTopology {
    /// Creates topology facts from a local execution topology plan.
    #[must_use]
    pub fn from_execution_topology(topology: &ExecutionTopologyPlan) -> Self {
        Self {
            topology_kind: Some(topology.kind),
            topology_digest: Some(topology.stable_digest()),
            transport: None,
            cluster_id: None,
            cluster_state_digest: None,
            scheduler_node_id: None,
            selected_node_ids: Vec::new(),
            replica_node_ids: Vec::new(),
            policy_digests: Vec::new(),
        }
    }

    /// Creates topology facts from a clustered execution context.
    #[must_use]
    pub fn from_cluster_execution(cluster_execution: &ClusterExecutionContext) -> Self {
        Self {
            topology_kind: cluster_execution
                .execution_topology
                .as_ref()
                .map(|value| value.kind),
            topology_digest: Some(cluster_execution.topology_digest.clone()),
            transport: Some(cluster_execution.transport),
            cluster_id: Some(cluster_execution.cluster_id.clone()),
            cluster_state_digest: Some(cluster_execution.cluster_state_digest.clone()),
            scheduler_node_id: Some(cluster_execution.scheduler_node_id.clone()),
            selected_node_ids: cluster_execution
                .selected_nodes
                .iter()
                .map(|node| node.node_id.clone())
                .collect(),
            replica_node_ids: cluster_execution
                .replica_nodes
                .iter()
                .map(|node| node.node.node_id.clone())
                .collect(),
            policy_digests: cluster_execution.policy_digests.clone(),
        }
    }
}

/// Artifact residency and byte-lineage facts carried by one canonical proof bundle.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionProofArtifactResidency {
    /// Stable served-artifact digest for a served compute path, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub served_artifact_digest: Option<String>,
    /// Stable weight-bundle digest for a served compute path, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight_bundle_digest: Option<String>,
    /// Stable cluster artifact-residency digest when clustered staging participated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_artifact_residency_digest: Option<String>,
    /// Stable sharded-model manifest digest constraining the path, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sharded_model_manifest_digest: Option<String>,
    /// Stable digests for input artifacts declared by the path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_artifact_digests: Vec<String>,
    /// Stable digests for output artifacts emitted by the path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output_artifact_digests: Vec<String>,
    /// Stable stdout digest when the path retained stdout bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout_sha256: Option<String>,
    /// Stable stderr digest when the path retained stderr bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr_sha256: Option<String>,
}

impl ExecutionProofArtifactResidency {
    /// Creates residency facts from a served-artifact identity.
    #[must_use]
    pub fn from_served_artifact(served_artifact: &ServedArtifactIdentity) -> Self {
        Self {
            served_artifact_digest: Some(served_artifact.served_artifact_digest.clone()),
            weight_bundle_digest: Some(served_artifact.weight_bundle_digest.clone()),
            cluster_artifact_residency_digest: None,
            sharded_model_manifest_digest: None,
            input_artifact_digests: Vec::new(),
            output_artifact_digests: Vec::new(),
            stdout_sha256: None,
            stderr_sha256: None,
        }
    }

    /// Merges sandbox input/output digests into this residency view.
    #[must_use]
    pub fn with_sandbox_evidence(mut self, evidence: &SandboxExecutionEvidence) -> Self {
        self.input_artifact_digests = evidence.input_artifact_digests.clone();
        self.output_artifact_digests = evidence.output_artifact_digests.clone();
        self.stdout_sha256 = evidence.stdout_sha256.clone();
        self.stderr_sha256 = evidence.stderr_sha256.clone();
        self
    }

    /// Attaches clustered artifact and sharded-manifest digests.
    #[must_use]
    pub fn with_cluster_execution(mut self, cluster_execution: &ClusterExecutionContext) -> Self {
        self.cluster_artifact_residency_digest =
            cluster_execution.artifact_residency_digest.clone();
        self.sharded_model_manifest_digest =
            cluster_execution.sharded_model_manifest_digest.clone();
        self
    }
}

/// Sandbox-specific execution context carried by one canonical proof bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionProofSandboxContext {
    /// Stable digest of the bounded sandbox profile used for the request.
    pub sandbox_profile_digest: String,
    /// Stable digest of the executed command or job spec.
    pub command_digest: String,
    /// Stable digest of the execution environment exposed to the job.
    pub environment_digest: String,
    /// Terminal exit facts for the sandbox.
    pub exit: SandboxExecutionExit,
    /// Explicit resource summary for the sandbox request.
    pub resources: SandboxExecutionResourceSummary,
}

impl ExecutionProofSandboxContext {
    /// Creates sandbox proof context directly from runtime evidence.
    #[must_use]
    pub fn from_evidence(evidence: &SandboxExecutionEvidence) -> Self {
        Self {
            sandbox_profile_digest: evidence.sandbox_profile_digest.clone(),
            command_digest: evidence.command_digest.clone(),
            environment_digest: evidence.environment_digest.clone(),
            exit: evidence.exit.clone(),
            resources: evidence.resources.clone(),
        }
    }
}

/// Canonical execution-proof bundle shared by receipt, validator, and settlement layers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionProofBundle {
    /// Stable schema version for the canonical bundle encoding.
    pub schema_version: u16,
    /// High-level execution family covered by this bundle.
    pub bundle_kind: ExecutionProofBundleKind,
    /// Terminal execution status covered by the bundle.
    pub status: ExecutionProofBundleStatus,
    /// Stable request identifier.
    pub request_id: String,
    /// Stable request digest.
    pub request_digest: String,
    /// Stable compute product identifier.
    pub product_id: String,
    /// Stable model identifier when the path belongs to a served model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    /// Runtime identity facts for the realized path.
    pub runtime_identity: ExecutionProofRuntimeIdentity,
    /// Validation claim backing the path when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<ValidationMatrixReference>,
    /// Stable execution-plan digest used by the path when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_plan_digest: Option<String>,
    /// Warm/cold compile-path evidence when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_path: Option<CompilePathEvidence>,
    /// Topology and routing facts for the path when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology: Option<ExecutionProofTopology>,
    /// Artifact and residency facts for the path when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_residency: Option<ExecutionProofArtifactResidency>,
    /// Delivery-proof facts surfaced by the path when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_proof: Option<ExecutionDeliveryProof>,
    /// Settlement-linkage inputs derived from the path when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settlement_linkage: Option<SettlementLinkageInput>,
    /// Sandbox-specific context when the path is a sandbox execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<ExecutionProofSandboxContext>,
    /// Product posture for the activation-fingerprint proof layer.
    pub activation_fingerprint_posture: ExecutionProofAugmentationPosture,
    /// Activation-fingerprint proof artifact when this bundle carries one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activation_fingerprint: Option<ActivationFingerprintProofArtifact>,
    /// Stable activation-fingerprint proof reference when an augmentation exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activation_fingerprint_ref: Option<String>,
    /// Stable challenge-result references when validators have already acted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub challenge_result_refs: Vec<String>,
    /// Plain-language failure reason when the path did not succeed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    /// Structured runtime diagnostic when the path surfaced one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<LocalRuntimeDiagnostic>,
}

impl ExecutionProofBundle {
    /// Creates a canonical execution-proof bundle from explicit request/runtime truth.
    #[must_use]
    pub fn new(
        bundle_kind: ExecutionProofBundleKind,
        status: ExecutionProofBundleStatus,
        request_id: impl Into<String>,
        request_digest: impl Into<String>,
        product_id: impl Into<String>,
        runtime_identity: ExecutionProofRuntimeIdentity,
    ) -> Self {
        Self {
            schema_version: EXECUTION_PROOF_BUNDLE_SCHEMA_VERSION,
            bundle_kind,
            status,
            request_id: request_id.into(),
            request_digest: request_digest.into(),
            product_id: product_id.into(),
            model_id: None,
            runtime_identity,
            validation: None,
            execution_plan_digest: None,
            compile_path: None,
            topology: None,
            artifact_residency: None,
            delivery_proof: None,
            settlement_linkage: None,
            sandbox: None,
            activation_fingerprint_posture: ExecutionProofAugmentationPosture::Unavailable,
            activation_fingerprint: None,
            activation_fingerprint_ref: None,
            challenge_result_refs: Vec::new(),
            failure_reason: None,
            diagnostic: None,
        }
    }

    /// Attaches a stable model identifier.
    #[must_use]
    pub fn with_model_id(mut self, model_id: impl Into<String>) -> Self {
        self.model_id = Some(model_id.into());
        self
    }

    /// Attaches a validation claim reference.
    #[must_use]
    pub fn with_validation(mut self, validation: ValidationMatrixReference) -> Self {
        self.validation = Some(validation);
        self
    }

    /// Attaches an execution-plan digest.
    #[must_use]
    pub fn with_execution_plan_digest(mut self, execution_plan_digest: impl Into<String>) -> Self {
        self.execution_plan_digest = Some(execution_plan_digest.into());
        self
    }

    /// Attaches compile-path evidence.
    #[must_use]
    pub fn with_compile_path(mut self, compile_path: CompilePathEvidence) -> Self {
        self.compile_path = Some(compile_path);
        self
    }

    /// Attaches local execution-topology facts.
    #[must_use]
    pub fn with_execution_topology(mut self, execution_topology: &ExecutionTopologyPlan) -> Self {
        self.topology = Some(ExecutionProofTopology::from_execution_topology(
            execution_topology,
        ));
        self
    }

    /// Attaches clustered execution facts, merging them into topology and residency views.
    #[must_use]
    pub fn with_cluster_execution(mut self, cluster_execution: &ClusterExecutionContext) -> Self {
        self.topology = Some(ExecutionProofTopology::from_cluster_execution(
            cluster_execution,
        ));
        self.artifact_residency = Some(
            self.artifact_residency
                .take()
                .unwrap_or_default()
                .with_cluster_execution(cluster_execution),
        );
        self
    }

    /// Attaches served-artifact residency facts.
    #[must_use]
    pub fn with_served_artifact(mut self, served_artifact: &ServedArtifactIdentity) -> Self {
        self.artifact_residency = Some(self.artifact_residency.take().unwrap_or_default().merge(
            ExecutionProofArtifactResidency::from_served_artifact(served_artifact),
        ));
        self
    }

    /// Attaches delivery-proof facts.
    #[must_use]
    pub fn with_delivery_proof(mut self, delivery_proof: ExecutionDeliveryProof) -> Self {
        self.execution_plan_digest = Some(delivery_proof.execution_plan_digest.clone());
        self.delivery_proof = Some(delivery_proof);
        self
    }

    /// Attaches settlement-linkage facts.
    #[must_use]
    pub fn with_settlement_linkage(mut self, settlement_linkage: SettlementLinkageInput) -> Self {
        self.settlement_linkage = Some(settlement_linkage);
        self
    }

    /// Attaches sandbox execution facts and merges sandbox artifact digests.
    #[must_use]
    pub fn with_sandbox_evidence(mut self, evidence: &SandboxExecutionEvidence) -> Self {
        self.sandbox = Some(ExecutionProofSandboxContext::from_evidence(evidence));
        self.artifact_residency = Some(
            self.artifact_residency
                .take()
                .unwrap_or_default()
                .with_sandbox_evidence(evidence),
        );
        if let Some(delivery_proof) = evidence.delivery_proof.clone() {
            self = self.with_delivery_proof(delivery_proof);
        }
        self
    }

    /// Declares the product posture for the activation-fingerprint proof layer.
    #[must_use]
    pub const fn with_activation_fingerprint_posture(
        mut self,
        activation_fingerprint_posture: ExecutionProofAugmentationPosture,
    ) -> Self {
        self.activation_fingerprint_posture = activation_fingerprint_posture;
        self
    }

    /// Attaches a stable activation-fingerprint proof reference.
    #[must_use]
    pub fn with_activation_fingerprint_ref(
        mut self,
        activation_fingerprint_ref: impl Into<String>,
    ) -> Self {
        self.activation_fingerprint_ref = Some(activation_fingerprint_ref.into());
        self
    }

    /// Attaches a concrete activation-fingerprint proof artifact.
    #[must_use]
    pub fn with_activation_fingerprint(
        mut self,
        activation_fingerprint: ActivationFingerprintProofArtifact,
    ) -> Self {
        self.activation_fingerprint_ref = Some(activation_fingerprint.artifact_digest.clone());
        self.activation_fingerprint = Some(activation_fingerprint);
        self
    }

    /// Appends one stable challenge-result reference.
    #[must_use]
    pub fn with_challenge_result_ref(mut self, challenge_result_ref: impl Into<String>) -> Self {
        self.challenge_result_refs.push(challenge_result_ref.into());
        self
    }

    /// Attaches a plain-language failure reason.
    #[must_use]
    pub fn with_failure_reason(mut self, failure_reason: impl Into<String>) -> Self {
        self.failure_reason = Some(failure_reason.into());
        self
    }

    /// Attaches a structured runtime diagnostic.
    #[must_use]
    pub fn with_diagnostic(mut self, diagnostic: LocalRuntimeDiagnostic) -> Self {
        if self.failure_reason.is_none() {
            self.failure_reason = Some(diagnostic.message.clone());
        }
        self.diagnostic = Some(diagnostic);
        self
    }

    /// Returns the stable digest for the canonical proof bundle.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded = serde_json::to_vec(self)
            .unwrap_or_else(|_| unreachable!("execution proof bundle should serialize"));
        let mut hasher = Sha256::new();
        hasher.update(b"execution_proof_bundle|");
        hasher.update(encoded);
        hex::encode(hasher.finalize())
    }
}

impl ExecutionProofArtifactResidency {
    fn merge(mut self, other: Self) -> Self {
        self.served_artifact_digest = other.served_artifact_digest.or(self.served_artifact_digest);
        self.weight_bundle_digest = other.weight_bundle_digest.or(self.weight_bundle_digest);
        self.cluster_artifact_residency_digest = other
            .cluster_artifact_residency_digest
            .or(self.cluster_artifact_residency_digest);
        self.sharded_model_manifest_digest = other
            .sharded_model_manifest_digest
            .or(self.sharded_model_manifest_digest);
        if !other.input_artifact_digests.is_empty() {
            self.input_artifact_digests = other.input_artifact_digests;
        }
        if !other.output_artifact_digests.is_empty() {
            self.output_artifact_digests = other.output_artifact_digests;
        }
        self.stdout_sha256 = other.stdout_sha256.or(self.stdout_sha256);
        self.stderr_sha256 = other.stderr_sha256.or(self.stderr_sha256);
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ActivationFingerprintInput, ActivationFingerprintProofAdapter,
        ActivationFingerprintVectorSample, BackendProbeState, BackendToolchainIdentity,
        CacheAction, CacheInvalidationTrigger, CacheKind, CacheObservation,
        ClusterExecutionContext, ClusterExecutionDisposition, ClusterSelectedNode,
        ClusterTransportClass, CompilePathEvidence, CompilePathTemperature,
        DeviceInventoryQualifiers, DeviceMemoryClass, DevicePerformanceClass,
        ExecutionDeliveryProof, ExecutionProofAugmentationPosture, ExecutionProofBundle,
        ExecutionProofBundleKind, ExecutionProofBundleStatus, ExecutionProofRuntimeIdentity,
        ExecutionTopologyPlan, LocalRuntimeDiagnostic, LocalRuntimeErrorCode,
        QuantizedActivationFingerprintAdapter, SandboxExecutionEvidence, SandboxExecutionExit,
        SandboxExecutionExitKind, SandboxExecutionResourceSummary, ServedArtifactIdentity,
        SettlementLinkageInput, ValidationCoverage, ValidationMatrixReference,
    };
    use psionic_core::QuantizationMode;

    fn runtime_identity() -> ExecutionProofRuntimeIdentity {
        ExecutionProofRuntimeIdentity::new(
            "cuda",
            BackendToolchainIdentity::new("cuda", "cuda@1.2.3", vec!["tensor_cores".to_string()])
                .with_probe(
                    BackendProbeState::CompiledAndProbed,
                    vec!["sm90".to_string()],
                ),
        )
        .with_selected_device_inventory(Some(DeviceInventoryQualifiers {
            stable_device_id: "gpu-0".to_string(),
            topology_key: Some("0000:01:00.0".to_string()),
            performance_class: DevicePerformanceClass::DiscreteAccelerator,
            memory_class: DeviceMemoryClass::DedicatedDevice,
            total_memory_bytes: Some(24 * 1024 * 1024 * 1024),
            free_memory_bytes: Some(20 * 1024 * 1024 * 1024),
        }))
        .with_selected_devices(vec![DeviceInventoryQualifiers {
            stable_device_id: "gpu-0".to_string(),
            topology_key: Some("0000:01:00.0".to_string()),
            performance_class: DevicePerformanceClass::DiscreteAccelerator,
            memory_class: DeviceMemoryClass::DedicatedDevice,
            total_memory_bytes: Some(24 * 1024 * 1024 * 1024),
            free_memory_bytes: Some(20 * 1024 * 1024 * 1024),
        }])
    }

    fn served_artifact() -> ServedArtifactIdentity {
        ServedArtifactIdentity::new(
            "gpt-oss-20b",
            "rev-1",
            "weights-123",
            Some("model-blob-123".to_string()),
            Some("tokenizer-123".to_string()),
            Some("chat-template-123".to_string()),
            "defaults-123",
            "gguf",
            QuantizationMode::None,
            BackendToolchainIdentity::new("cuda", "cuda@1.2.3", vec![]),
        )
    }

    #[test]
    fn proof_bundle_digest_is_stable_for_identical_inputs() {
        let topology = ExecutionTopologyPlan::replicated(
            "cuda",
            vec![DeviceInventoryQualifiers {
                stable_device_id: "gpu-0".to_string(),
                topology_key: Some("0000:01:00.0".to_string()),
                performance_class: DevicePerformanceClass::DiscreteAccelerator,
                memory_class: DeviceMemoryClass::DedicatedDevice,
                total_memory_bytes: Some(24 * 1024 * 1024 * 1024),
                free_memory_bytes: Some(20 * 1024 * 1024 * 1024),
            }],
        );
        let delivery_proof = ExecutionDeliveryProof {
            execution_plan_digest: "plan-123".to_string(),
            kernel_count: 42,
            bytes_moved: 65_536,
            plan_cache_hits: 2,
            plan_cache_misses: 1,
            kv_growth: None,
            prefill_decode_handoff: None,
        };
        let compile_path = CompilePathEvidence {
            temperature: CompilePathTemperature::WarmReuse,
            execution_plan_cache: CacheObservation::new(
                CacheKind::ExecutionPlan,
                CacheAction::Reuse,
                "warm",
            ),
            kernel_cache: CacheObservation::new(CacheKind::KernelCache, CacheAction::Reuse, "warm")
                .with_trigger(CacheInvalidationTrigger::ExplicitReset),
        };
        let bundle = ExecutionProofBundle::new(
            ExecutionProofBundleKind::Local,
            ExecutionProofBundleStatus::Succeeded,
            "req-1",
            "digest-1",
            "psionic.text_generation",
            runtime_identity(),
        )
        .with_model_id("gpt-oss-20b")
        .with_validation(ValidationMatrixReference::minimum(
            "cuda_text_generation_positive",
            ValidationCoverage::PositiveExecution,
        ))
        .with_served_artifact(&served_artifact())
        .with_execution_topology(&topology)
        .with_compile_path(compile_path)
        .with_delivery_proof(delivery_proof)
        .with_settlement_linkage(SettlementLinkageInput {
            request_digest: "digest-1".to_string(),
            product_id: "psionic.text_generation".to_string(),
            model_id: "gpt-oss-20b".to_string(),
            served_artifact_digest: served_artifact().served_artifact_digest.clone(),
            execution_plan_digest: "plan-123".to_string(),
            runtime_backend: "cuda".to_string(),
            kernel_count: 42,
            bytes_moved: 65_536,
            plan_cache_hits: 2,
            plan_cache_misses: 1,
            kv_growth: None,
            output_tokens: Some(128),
            cluster_provenance: None,
        });
        let encoded = serde_json::to_vec(&bundle).expect("bundle should serialize");
        let decoded: ExecutionProofBundle =
            serde_json::from_slice(encoded.as_slice()).expect("bundle should deserialize");
        assert_eq!(bundle.stable_digest(), decoded.stable_digest());
    }

    #[test]
    fn cluster_execution_changes_bundle_digest() {
        let base = ExecutionProofBundle::new(
            ExecutionProofBundleKind::Clustered,
            ExecutionProofBundleStatus::Succeeded,
            "req-2",
            "digest-2",
            "psionic.embeddings",
            runtime_identity(),
        );
        let first = base
            .clone()
            .with_cluster_execution(&ClusterExecutionContext::new(
                "cluster-a",
                "state-a",
                "topology-a",
                "scheduler-a",
                ClusterTransportClass::WiderNetworkStream,
                ClusterExecutionDisposition::RemoteWholeRequest,
            ));
        let second = base.with_cluster_execution(
            &ClusterExecutionContext::new(
                "cluster-b",
                "state-b",
                "topology-b",
                "scheduler-b",
                ClusterTransportClass::WiderNetworkStream,
                ClusterExecutionDisposition::RemoteWholeRequest,
            )
            .with_selected_nodes(vec![ClusterSelectedNode::new("node-b", "cuda")]),
        );
        assert_ne!(first.stable_digest(), second.stable_digest());
    }

    #[test]
    fn sandbox_bundle_carries_sandbox_context_and_artifact_digests() {
        let evidence = SandboxExecutionEvidence {
            request_digest: "sandbox-digest".to_string(),
            sandbox_profile_digest: "profile-digest".to_string(),
            command_digest: "command-digest".to_string(),
            environment_digest: "environment-digest".to_string(),
            input_artifact_digests: vec!["input-1".to_string()],
            output_artifact_digests: vec!["output-1".to_string()],
            exit: SandboxExecutionExit {
                kind: SandboxExecutionExitKind::Succeeded,
                exit_code: Some(0),
                detail: "ok".to_string(),
            },
            resources: SandboxExecutionResourceSummary {
                wall_time_ms: 100,
                cpu_time_ms: 90,
                peak_memory_bytes: 1024,
                filesystem_write_bytes: 128,
                stdout_bytes: 64,
                stderr_bytes: 0,
                network_egress_bytes: 0,
            },
            stdout_sha256: Some("stdout-123".to_string()),
            stderr_sha256: None,
            delivery_proof: Some(ExecutionDeliveryProof {
                execution_plan_digest: "sandbox-plan".to_string(),
                kernel_count: 1,
                bytes_moved: 512,
                plan_cache_hits: 0,
                plan_cache_misses: 1,
                kv_growth: None,
                prefill_decode_handoff: None,
            }),
        };
        let diagnostic =
            LocalRuntimeDiagnostic::new(LocalRuntimeErrorCode::Internal, 500, "sandbox diagnostic");
        let bundle = ExecutionProofBundle::new(
            ExecutionProofBundleKind::Sandbox,
            ExecutionProofBundleStatus::Failed,
            "job-1",
            "sandbox-digest",
            "psionic.sandbox_execution",
            runtime_identity(),
        )
        .with_sandbox_evidence(&evidence)
        .with_failure_reason("sandbox failed")
        .with_diagnostic(diagnostic);
        assert_eq!(
            bundle
                .sandbox
                .as_ref()
                .map(|value| value.sandbox_profile_digest.as_str()),
            Some("profile-digest")
        );
        assert_eq!(
            bundle
                .artifact_residency
                .as_ref()
                .map(|value| value.input_artifact_digests.as_slice()),
            Some(&["input-1".to_string()][..])
        );
        assert_eq!(
            bundle.execution_plan_digest.as_deref(),
            Some("sandbox-plan")
        );
    }

    #[test]
    fn activation_fingerprint_posture_and_ref_are_explicit() {
        let artifact = QuantizedActivationFingerprintAdapter::default().generate(
            &ActivationFingerprintInput::new(
                "digest-3",
                "psionic.embeddings",
                "smoke-embed",
                "cpu",
            )
            .with_sample(ActivationFingerprintVectorSample::new(
                "embedding:0",
                vec![0.1, 0.2, 0.3, 0.4],
            )),
        );
        let bundle = ExecutionProofBundle::new(
            ExecutionProofBundleKind::Local,
            ExecutionProofBundleStatus::Succeeded,
            "req-3",
            "digest-3",
            "psionic.embeddings",
            runtime_identity(),
        )
        .with_activation_fingerprint_posture(ExecutionProofAugmentationPosture::Supported)
        .with_activation_fingerprint(artifact.clone());
        assert_eq!(
            bundle.activation_fingerprint_posture,
            ExecutionProofAugmentationPosture::Supported
        );
        assert_eq!(
            bundle.activation_fingerprint_ref.as_deref(),
            Some(artifact.artifact_digest.as_str())
        );
    }
}
