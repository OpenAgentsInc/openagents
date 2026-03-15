use psionic_runtime::TrainingCheckpointReference;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    AdapterClusterCoordinationError, AdapterContributionValidationBundle,
    AdapterTrainingClusterCoordinator, AdapterWindowScoreSummary, CheckpointPointer,
    CheckpointRecoveryError, CheckpointScopeBinding, CheckpointScopeKind, PolicyRevision,
    TrainingWindowStatus,
};

/// Error returned by adapter aggregation and policy-promotion flows.
#[derive(Debug, Error)]
pub enum AdapterAggregationError {
    /// The coordinator had no current window to promote.
    #[error("adapter aggregation requires a current window")]
    MissingCurrentWindow,
    /// The requested current window was not present in coordinator state.
    #[error("adapter aggregation does not know window `{window_id}`")]
    UnknownWindow {
        /// Stable window identifier.
        window_id: String,
    },
    /// One accepted contribution in the window lacked a validation bundle.
    #[error("missing aggregation bundle for accepted contribution `{contribution_id}`")]
    MissingAcceptedBundle {
        /// Stable contribution identifier.
        contribution_id: String,
    },
    /// The window-scoring summary referenced a different window than the current promotion target.
    #[error(
        "adapter aggregation summary window mismatch: expected `{expected_window_id}`, found `{actual_window_id}`"
    )]
    SummaryWindowMismatch {
        /// Expected stable window identifier.
        expected_window_id: String,
        /// Actual stable window identifier.
        actual_window_id: String,
    },
    /// The underlying checkpoint pointer contract rejected the promoted checkpoint identity.
    #[error(transparent)]
    Checkpoint(#[from] CheckpointRecoveryError),
    /// The underlying adapter-window contract rejected the requested transition.
    #[error(transparent)]
    WindowContract(#[from] crate::AdapterWindowContractError),
    /// The cluster coordinator could not synchronize the reconciled window.
    #[error(transparent)]
    Cluster(#[from] AdapterClusterCoordinationError),
}

/// First supported deterministic aggregation rule for accepted adapter contributions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterAggregationRule {
    /// Aggregate accepted contributions by deterministic weighted manifest-digest merge.
    WeightedManifestDigestMergeV1,
}

impl Default for AdapterAggregationRule {
    fn default() -> Self {
        Self::WeightedManifestDigestMergeV1
    }
}

/// Promotion disposition emitted by the adapter policy aggregator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterPolicyPromotionDisposition {
    /// Accepted contributions promoted a new policy revision and checkpoint pointer.
    Promoted,
    /// Aggregation completed but the window did not satisfy promotion criteria.
    Held,
}

/// Machine-readable reason code for a held promotion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterPromotionHoldReasonCode {
    /// The window did not carry enough accepted work to promote a revision.
    InsufficientAcceptedWork,
    /// The validator-owned window summary was not promotion-ready.
    ValidatorWindowNotPromotionReady,
}

/// Inspectable accepted-contribution lineage preserved in a promotion receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedAdapterContributionLineage {
    /// Stable contribution identifier.
    pub contribution_id: String,
    /// Stable worker identifier.
    pub worker_id: String,
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable contribution manifest digest.
    pub manifest_digest: String,
    /// Stable contribution object digest.
    pub object_digest: String,
    /// Aggregation weight applied to the contribution.
    pub aggregation_weight_bps: u16,
    /// Stable validator receipt digest.
    pub validator_receipt_digest: String,
    /// Stable security receipt digest.
    pub security_receipt_digest: String,
    /// Stable provenance bundle digest.
    pub provenance_bundle_digest: String,
}

/// Deterministic promotion receipt for one aggregated adapter window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterPolicyPromotionReceipt {
    /// Aggregation rule that produced the receipt.
    pub aggregation_rule: AdapterAggregationRule,
    /// Stable training run identifier.
    pub training_run_id: String,
    /// Stable stage identifier.
    pub stage_id: String,
    /// Stable window identifier.
    pub window_id: String,
    /// Stable contributor-set revision identifier.
    pub contributor_set_revision_id: String,
    /// Stable adapter target identifier.
    pub adapter_target_id: String,
    /// Stable input policy revision id.
    pub input_policy_revision_id: String,
    /// Stable input checkpoint pointer digest.
    pub input_checkpoint_pointer_digest: String,
    /// Stable window summary digest used for promotion gating.
    pub window_summary_digest: String,
    /// Accepted contribution lineage in deterministic order.
    pub accepted_contributions: Vec<AcceptedAdapterContributionLineage>,
    /// Deterministic aggregate digest over the accepted contributions when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregated_delta_digest: Option<String>,
    /// Promotion disposition.
    pub promotion_disposition: AdapterPolicyPromotionDisposition,
    /// Hold reason codes when promotion was refused.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hold_reason_codes: Vec<AdapterPromotionHoldReasonCode>,
    /// Output policy revision when one was promoted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_policy_revision: Option<PolicyRevision>,
    /// Output checkpoint pointer when one was promoted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_checkpoint_pointer: Option<CheckpointPointer>,
    /// Aggregation time.
    pub aggregated_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Stateful deterministic policy aggregator for decentralized adapter windows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterPolicyAggregator {
    /// Active aggregation rule.
    pub rule: AdapterAggregationRule,
    /// Historical promotion receipts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub promotion_receipts: Vec<AdapterPolicyPromotionReceipt>,
}

impl Default for AdapterPolicyAggregator {
    fn default() -> Self {
        Self::new(AdapterAggregationRule::default())
    }
}

impl AdapterPolicyAggregator {
    /// Creates an adapter policy aggregator for the given rule.
    #[must_use]
    pub fn new(rule: AdapterAggregationRule) -> Self {
        Self {
            rule,
            promotion_receipts: Vec::new(),
        }
    }

    /// Aggregates the current coordinator window and updates the coordinator's next input revision.
    pub fn promote_current_window(
        &mut self,
        coordinator: &mut AdapterTrainingClusterCoordinator,
        summary: &AdapterWindowScoreSummary,
        bundles: Vec<AdapterContributionValidationBundle>,
        aggregated_at_ms: u64,
        reconciled_at_ms: u64,
    ) -> Result<AdapterPolicyPromotionReceipt, AdapterAggregationError> {
        let current_window_id = coordinator
            .current_window_id
            .clone()
            .ok_or(AdapterAggregationError::MissingCurrentWindow)?;
        if summary.window_id != current_window_id {
            return Err(AdapterAggregationError::SummaryWindowMismatch {
                expected_window_id: current_window_id,
                actual_window_id: summary.window_id.clone(),
            });
        }

        let record = coordinator
            .windows
            .iter_mut()
            .find(|record| record.plan.window_id == summary.window_id)
            .ok_or_else(|| AdapterAggregationError::UnknownWindow {
                window_id: summary.window_id.clone(),
            })?;
        let receipt = self.promote_window_record(record, summary, bundles, aggregated_at_ms)?;
        if let (Some(policy_revision), Some(checkpoint_pointer)) = (
            receipt.output_policy_revision.clone(),
            receipt.output_checkpoint_pointer.clone(),
        ) {
            coordinator.current_policy_revision = policy_revision;
            coordinator.current_checkpoint_pointer = checkpoint_pointer;
        }
        record.window.reconcile()?;
        coordinator.synchronize_current_window_status(reconciled_at_ms)?;
        self.promotion_receipts.push(receipt.clone());
        Ok(receipt)
    }

    fn promote_window_record(
        &self,
        record: &mut crate::AdapterClusterWindowRecord,
        summary: &AdapterWindowScoreSummary,
        bundles: Vec<AdapterContributionValidationBundle>,
        aggregated_at_ms: u64,
    ) -> Result<AdapterPolicyPromotionReceipt, AdapterAggregationError> {
        if record.window.status != TrainingWindowStatus::Sealed {
            return Err(AdapterAggregationError::WindowContract(
                crate::AdapterWindowContractError::InvalidWindowStatus {
                    window_id: record.window.window_id.clone(),
                    action: "aggregate policy promotion",
                    status: format!("{:?}", record.window.status).to_lowercase(),
                },
            ));
        }

        let mut accepted_contributions = record
            .window
            .contributions
            .iter()
            .filter(|contribution| {
                contribution.validator.as_ref().is_some_and(|receipt| {
                    receipt.disposition == crate::AdapterContributionValidatorDisposition::Accepted
                }) && contribution.aggregation.as_ref().is_some_and(|receipt| {
                    receipt.eligibility
                        == crate::AdapterContributionAggregationEligibility::Eligible
                })
            })
            .map(|contribution| {
                let contribution_id = contribution.assignment.binding.contribution_id.clone();
                let bundle = bundles
                    .iter()
                    .find(|bundle| bundle.artifact.contribution_id == contribution_id)
                    .ok_or_else(|| AdapterAggregationError::MissingAcceptedBundle {
                        contribution_id: contribution_id.clone(),
                    })?;
                Ok(AcceptedAdapterContributionLineage {
                    contribution_id,
                    worker_id: bundle.artifact.worker_id.clone(),
                    artifact_id: bundle.artifact.artifact_id.clone(),
                    manifest_digest: bundle.artifact.manifest.manifest_digest.clone(),
                    object_digest: bundle.artifact.manifest.object_digest.clone(),
                    aggregation_weight_bps: contribution
                        .aggregation
                        .as_ref()
                        .and_then(|receipt| receipt.aggregation_weight_bps)
                        .unwrap_or(10_000),
                    validator_receipt_digest: contribution
                        .validator
                        .as_ref()
                        .map(|receipt| receipt.receipt_digest.clone())
                        .unwrap_or_default(),
                    security_receipt_digest: bundle.security.receipt_digest.clone(),
                    provenance_bundle_digest: bundle.provenance.bundle_digest.clone(),
                })
            })
            .collect::<Result<Vec<_>, AdapterAggregationError>>()?;
        accepted_contributions.sort_by(|left, right| {
            left.contribution_id
                .cmp(&right.contribution_id)
                .then(left.worker_id.cmp(&right.worker_id))
        });

        let aggregated_delta_digest = (!accepted_contributions.is_empty()).then(|| {
            stable_aggregate_digest(
                self.rule,
                record.window.adapter_target.adapter_target_id.as_str(),
                record.window.input_policy_revision.revision_id.as_str(),
                summary.summary_digest.as_str(),
                accepted_contributions.as_slice(),
            )
        });

        let mut hold_reason_codes = Vec::new();
        if accepted_contributions.is_empty() {
            hold_reason_codes.push(AdapterPromotionHoldReasonCode::InsufficientAcceptedWork);
        }
        if !summary.promotion_ready {
            hold_reason_codes
                .push(AdapterPromotionHoldReasonCode::ValidatorWindowNotPromotionReady);
        }

        let (promotion_disposition, output_policy_revision, output_checkpoint_pointer) =
            if hold_reason_codes.is_empty() {
                let output_policy_revision = promoted_policy_revision(
                    &record.window,
                    aggregated_delta_digest
                        .as_deref()
                        .expect("accepted contributions imply aggregate digest"),
                    aggregated_at_ms,
                );
                let output_checkpoint_pointer = promoted_checkpoint_pointer(
                    &record.window,
                    &output_policy_revision,
                    aggregated_delta_digest
                        .as_deref()
                        .expect("accepted contributions imply aggregate digest"),
                    aggregated_at_ms,
                )?;
                (
                    AdapterPolicyPromotionDisposition::Promoted,
                    Some(output_policy_revision),
                    Some(output_checkpoint_pointer),
                )
            } else {
                (AdapterPolicyPromotionDisposition::Held, None, None)
            };

        record.window.aggregate(
            output_policy_revision.clone(),
            output_checkpoint_pointer.clone(),
            aggregated_at_ms,
        )?;

        let receipt = AdapterPolicyPromotionReceipt {
            aggregation_rule: self.rule,
            training_run_id: record.window.training_run_id.clone(),
            stage_id: record.window.stage_id.clone(),
            window_id: record.window.window_id.clone(),
            contributor_set_revision_id: record.window.contributor_set_revision_id.clone(),
            adapter_target_id: record.window.adapter_target.adapter_target_id.clone(),
            input_policy_revision_id: record.window.input_policy_revision.revision_id.clone(),
            input_checkpoint_pointer_digest: record
                .window
                .input_checkpoint_pointer
                .pointer_digest
                .clone(),
            window_summary_digest: summary.summary_digest.clone(),
            accepted_contributions,
            aggregated_delta_digest: aggregated_delta_digest.clone(),
            promotion_disposition,
            hold_reason_codes: hold_reason_codes.clone(),
            output_policy_revision: output_policy_revision.clone(),
            output_checkpoint_pointer: output_checkpoint_pointer.clone(),
            aggregated_at_ms,
            receipt_digest: stable_promotion_receipt_digest(
                self.rule,
                record.window.training_run_id.as_str(),
                record.window.stage_id.as_str(),
                record.window.window_id.as_str(),
                record.window.contributor_set_revision_id.as_str(),
                record.window.adapter_target.adapter_target_id.as_str(),
                record.window.input_policy_revision.revision_id.as_str(),
                record
                    .window
                    .input_checkpoint_pointer
                    .pointer_digest
                    .as_str(),
                summary.summary_digest.as_str(),
                aggregated_delta_digest.as_deref(),
                promotion_disposition,
                hold_reason_codes.as_slice(),
                output_policy_revision
                    .as_ref()
                    .map(|revision| revision.revision_id.as_str()),
                output_checkpoint_pointer
                    .as_ref()
                    .map(|pointer| pointer.pointer_digest.as_str()),
                aggregated_at_ms,
            ),
        };
        Ok(receipt)
    }
}

fn promoted_policy_revision(
    window: &crate::AdapterTrainingWindowStateMachine,
    aggregated_delta_digest: &str,
    aggregated_at_ms: u64,
) -> PolicyRevision {
    let next_revision_number = window
        .input_policy_revision
        .revision_number
        .unwrap_or_default()
        .saturating_add(1);
    let checkpoint =
        promoted_checkpoint_reference(window, aggregated_delta_digest, aggregated_at_ms);
    PolicyRevision::new(
        window.input_policy_revision.policy_family.clone(),
        format!(
            "{}:aggregate:{}:r{}",
            window.input_policy_revision.revision_id, window.window_id, next_revision_number
        ),
        aggregated_delta_digest.to_string(),
        aggregated_at_ms,
    )
    .with_parent_revision_id(window.input_policy_revision.revision_id.clone())
    .with_revision_number(next_revision_number)
    .with_checkpoint(checkpoint)
}

fn promoted_checkpoint_reference(
    window: &crate::AdapterTrainingWindowStateMachine,
    aggregated_delta_digest: &str,
    aggregated_at_ms: u64,
) -> TrainingCheckpointReference {
    let input_checkpoint = &window.input_checkpoint_pointer.checkpoint;
    let next_step = window
        .input_policy_revision
        .revision_number
        .unwrap_or_default()
        .saturating_add(1);
    TrainingCheckpointReference::new(
        window.input_policy_revision.policy_family.clone(),
        format!(
            "stream://adapter-aggregate/{}/{}",
            window.window_id, aggregated_delta_digest
        ),
        aggregated_delta_digest.to_string(),
        aggregated_delta_digest.to_string(),
        String::from("adapter-aggregator"),
        input_checkpoint.membership_epoch,
        input_checkpoint.cluster_state_digest.clone(),
        input_checkpoint.topology_digest.clone(),
        aggregated_at_ms,
    )
    .with_checkpoint_ref(format!(
        "adapter-aggregate/{}/{}",
        window.window_id, aggregated_delta_digest
    ))
    .with_step(next_step)
    .with_durable_at_ms(aggregated_at_ms)
}

fn promoted_checkpoint_pointer(
    window: &crate::AdapterTrainingWindowStateMachine,
    output_policy_revision: &PolicyRevision,
    aggregated_delta_digest: &str,
    aggregated_at_ms: u64,
) -> Result<CheckpointPointer, CheckpointRecoveryError> {
    CheckpointPointer::new(
        CheckpointScopeBinding::new(CheckpointScopeKind::Window, window.window_id.clone()),
        output_policy_revision.policy_family.clone(),
        output_policy_revision
            .checkpoint
            .clone()
            .expect("promoted policy revision carries checkpoint lineage"),
        aggregated_delta_digest.to_string(),
        aggregated_at_ms,
    )
}

fn stable_aggregate_digest(
    rule: AdapterAggregationRule,
    adapter_target_id: &str,
    input_policy_revision_id: &str,
    summary_digest: &str,
    accepted_contributions: &[AcceptedAdapterContributionLineage],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_adapter_aggregate|");
    hasher.update(adapter_aggregation_rule_label(rule));
    hasher.update(b"|");
    hasher.update(adapter_target_id.as_bytes());
    hasher.update(b"|");
    hasher.update(input_policy_revision_id.as_bytes());
    hasher.update(b"|");
    hasher.update(summary_digest.as_bytes());
    for contribution in accepted_contributions {
        hasher.update(b"|accepted|");
        for part in [
            contribution.contribution_id.as_str(),
            contribution.worker_id.as_str(),
            contribution.artifact_id.as_str(),
            contribution.manifest_digest.as_str(),
            contribution.object_digest.as_str(),
            contribution.aggregation_weight_bps.to_string().as_str(),
            contribution.validator_receipt_digest.as_str(),
            contribution.security_receipt_digest.as_str(),
            contribution.provenance_bundle_digest.as_str(),
        ] {
            hasher.update(part.as_bytes());
            hasher.update(b"|");
        }
    }
    hex::encode(hasher.finalize())
}

fn stable_promotion_receipt_digest(
    rule: AdapterAggregationRule,
    training_run_id: &str,
    stage_id: &str,
    window_id: &str,
    contributor_set_revision_id: &str,
    adapter_target_id: &str,
    input_policy_revision_id: &str,
    input_checkpoint_pointer_digest: &str,
    window_summary_digest: &str,
    aggregated_delta_digest: Option<&str>,
    promotion_disposition: AdapterPolicyPromotionDisposition,
    hold_reason_codes: &[AdapterPromotionHoldReasonCode],
    output_policy_revision_id: Option<&str>,
    output_checkpoint_pointer_digest: Option<&str>,
    aggregated_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    for part in [
        "adapter_policy_promotion_receipt",
        std::str::from_utf8(adapter_aggregation_rule_label(rule)).expect("static bytes"),
        training_run_id,
        stage_id,
        window_id,
        contributor_set_revision_id,
        adapter_target_id,
        input_policy_revision_id,
        input_checkpoint_pointer_digest,
        window_summary_digest,
        aggregated_delta_digest.unwrap_or("-"),
        adapter_promotion_disposition_label(promotion_disposition),
        output_policy_revision_id.unwrap_or("-"),
        output_checkpoint_pointer_digest.unwrap_or("-"),
        aggregated_at_ms.to_string().as_str(),
    ] {
        hasher.update(part.as_bytes());
        hasher.update(b"|");
    }
    for reason_code in hold_reason_codes {
        hasher.update(b"|hold|");
        hasher.update(adapter_hold_reason_label(*reason_code).as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn adapter_aggregation_rule_label(rule: AdapterAggregationRule) -> &'static [u8] {
    match rule {
        AdapterAggregationRule::WeightedManifestDigestMergeV1 => {
            b"weighted_manifest_digest_merge_v1"
        }
    }
}

fn adapter_promotion_disposition_label(
    disposition: AdapterPolicyPromotionDisposition,
) -> &'static str {
    match disposition {
        AdapterPolicyPromotionDisposition::Promoted => "promoted",
        AdapterPolicyPromotionDisposition::Held => "held",
    }
}

fn adapter_hold_reason_label(reason_code: AdapterPromotionHoldReasonCode) -> &'static str {
    match reason_code {
        AdapterPromotionHoldReasonCode::InsufficientAcceptedWork => "insufficient_accepted_work",
        AdapterPromotionHoldReasonCode::ValidatorWindowNotPromotionReady => {
            "validator_window_not_promotion_ready"
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        net::{IpAddr, Ipv4Addr, SocketAddr},
    };

    use ed25519_dalek::SigningKey;
    use psionic_cluster::{
        AdmissionToken, ClusterBackendReadinessStatus, ClusterId, ClusterMembershipRecord,
        ClusterMembershipStatus, ClusterNamespace, ClusterNodeIdentity, ClusterNodeTelemetry,
        ClusterSnapshot, ClusterStabilityPosture, NodeEpoch, NodeId, NodeRole,
    };
    use psionic_datastream::{DatastreamEncoding, DatastreamManifest, DatastreamSubjectKind};
    use psionic_eval::{
        BenchmarkAggregationKind, BenchmarkCase, BenchmarkExecutionMode, BenchmarkPackage,
        BenchmarkPackageKey, EvalRunContract, EvalRunMode, EvalRunState, EvalSampleRecord,
        EvalSampleStatus,
    };

    use super::{
        AdapterPolicyAggregator, AdapterPolicyPromotionDisposition, AdapterPromotionHoldReasonCode,
    };
    use crate::{
        AdapterArtifactRetentionPolicy, AdapterArtifactStorageState,
        AdapterContributionReplayReceipt, AdapterContributionSecurityController,
        AdapterContributionSecurityPolicy, AdapterContributionValidationBundle,
        AdapterContributionValidatorPolicy, AdapterContributionValidatorState,
        AdapterContributorCapabilityPolicy, AdapterDatasetSliceIdentity, AdapterTargetIdentity,
        AdapterTrainingClusterCoordinator, AdapterWindowCandidateEvaluation, AdapterWorkerIdentity,
        AdapterWorkerProtocolPolicy, AdapterWorkerProtocolState, AdapterWorkerTrustClass,
        CheckpointPointer, CheckpointScopeBinding, CheckpointScopeKind, PolicyRevision,
    };

    const GIB_BYTES: u64 = 1024 * 1024 * 1024;

    fn cluster_state() -> psionic_cluster::ClusterState {
        let cluster_id = ClusterId::new(
            &ClusterNamespace::new("adapter-aggregation"),
            &AdmissionToken::new("shared-secret"),
        );
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot.memberships = BTreeMap::from([(
            NodeId::new("worker-b"),
            ClusterMembershipRecord::new(
                ClusterNodeIdentity {
                    cluster_id: cluster_id.clone(),
                    node_id: NodeId::new("worker-b"),
                    node_epoch: NodeEpoch::initial(),
                    role: NodeRole::ExecutorOnly,
                    auth_public_key: String::from("worker-b-pk"),
                    attestation: None,
                },
                Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 33_500)),
                ClusterMembershipStatus::Ready,
            ),
        )]);
        snapshot.telemetry = BTreeMap::from([(
            NodeId::new("worker-b"),
            ClusterNodeTelemetry::new(NodeId::new("worker-b"))
                .with_memory(Some(24 * GIB_BYTES), Some(24 * GIB_BYTES))
                .with_accelerator_count(1)
                .with_backend_readiness(
                    AdapterContributorCapabilityPolicy::default().backend_label,
                    ClusterBackendReadinessStatus::Ready,
                )
                .with_stability_posture(ClusterStabilityPosture::Stable),
        )]);
        psionic_cluster::ClusterState::from_snapshot(snapshot)
    }

    fn checkpoint_reference(
        checkpoint_ref: &str,
        started_at_ms: u64,
    ) -> psionic_runtime::TrainingCheckpointReference {
        psionic_runtime::TrainingCheckpointReference::new(
            "apple.weather.policy",
            format!("stream://{checkpoint_ref}"),
            format!("manifest://{checkpoint_ref}"),
            format!("object://{checkpoint_ref}"),
            "worker-b",
            7,
            "cluster-digest-weather",
            "topology-digest-weather",
            started_at_ms,
        )
        .with_checkpoint_ref(checkpoint_ref)
        .with_step(70)
    }

    fn finalized_eval_run(
        eval_run_id: &str,
        mode: EvalRunMode,
        environment: psionic_environments::EnvironmentPackageKey,
        expected_sample_count: u64,
        score_bps: u32,
    ) -> Result<EvalRunState, Box<dyn std::error::Error>> {
        let mut run = EvalRunState::open(
            EvalRunContract::new(eval_run_id, mode, environment)
                .with_expected_sample_count(expected_sample_count),
        )?;
        run.start(10_000)?;
        run.append_sample(EvalSampleRecord {
            sample_id: String::from("sample-1"),
            ordinal: Some(1),
            environment: run.contract.environment.clone(),
            status: EvalSampleStatus::Passed,
            input_ref: Some(String::from("input://1")),
            output_ref: Some(String::from("output://1")),
            expected_output_ref: Some(String::from("expected://1")),
            score_bps: Some(score_bps),
            metrics: Vec::new(),
            artifacts: Vec::new(),
            error_reason: None,
            verification: None,
            session_digest: None,
            metadata: BTreeMap::new(),
        })?;
        run.finalize(10_010, Vec::new())?;
        Ok(run)
    }

    fn benchmark_summary(
        environment: psionic_environments::EnvironmentPackageKey,
        score_bps: u32,
    ) -> Result<psionic_eval::BenchmarkAggregateSummary, Box<dyn std::error::Error>> {
        let package = BenchmarkPackage::new(
            BenchmarkPackageKey::new("apple.adapter.benchmark", "2026.03"),
            "Apple Adapter Benchmark",
            environment.clone(),
            1,
            BenchmarkAggregationKind::MedianScore,
        )
        .with_cases(vec![BenchmarkCase::new("case-1")]);
        let mut round = EvalRunState::open(
            EvalRunContract::new("benchmark-round-1", EvalRunMode::Benchmark, environment)
                .with_expected_sample_count(1)
                .with_benchmark_package(package.key.clone()),
        )?;
        round.start(10_000)?;
        round.append_sample(EvalSampleRecord {
            sample_id: String::from("sample-1"),
            ordinal: Some(1),
            environment: round.contract.environment.clone(),
            status: EvalSampleStatus::Passed,
            input_ref: Some(String::from("input://1")),
            output_ref: Some(String::from("output://1")),
            expected_output_ref: Some(String::from("expected://1")),
            score_bps: Some(score_bps),
            metrics: Vec::new(),
            artifacts: Vec::new(),
            error_reason: None,
            verification: None,
            session_digest: None,
            metadata: BTreeMap::new(),
        })?;
        round.finalize(10_010, Vec::new())?;
        let mut execution = package.open_execution(BenchmarkExecutionMode::Validator)?;
        execution.record_round(&round)?;
        Ok(execution.finalize()?)
    }

    fn coordinator_fixture(
        signing_key: &SigningKey,
    ) -> Result<
        (
            AdapterTrainingClusterCoordinator,
            AdapterContributionValidationBundle,
        ),
        Box<dyn std::error::Error>,
    > {
        let state = cluster_state();
        let run = crate::TrainingRunState::new(
            "adapter-run-aggregation",
            "adapter-sft",
            state.cluster_id().as_str(),
            "apple.weather.policy",
            psionic_environments::EnvironmentPackageKey::new("oa.apple.adapter", "2026.03"),
        )?;
        let mut coordinator = AdapterTrainingClusterCoordinator::new(
            run,
            AdapterTargetIdentity::new(
                "apple.weather.adapter",
                "apple.foundation_models",
                "apple://foundation-model/base",
                "apple.fmadapter",
            )?,
            PolicyRevision::new(
                "apple.weather.policy",
                "policy-r7",
                "policy-digest-r7",
                1_000,
            )
            .with_revision_number(7),
            CheckpointPointer::new(
                CheckpointScopeBinding::new(CheckpointScopeKind::Window, "window-weather-7"),
                "apple.weather.policy",
                checkpoint_reference("checkpoint/weather/r7", 1_000),
                "manifest-digest-r7",
                1_001,
            )?,
            AdapterContributorCapabilityPolicy {
                minimum_free_memory_bytes: 12 * GIB_BYTES,
                ..AdapterContributorCapabilityPolicy::default()
            },
        );
        coordinator.observe_cluster_state(&state, 1_010)?;
        let record = coordinator.plan_next_window(
            vec![AdapterDatasetSliceIdentity::new(
                "dataset.weather",
                "train",
                "slice-b",
                "slice-digest-b",
            )?],
            1,
            1_020,
        )?;
        coordinator.activate_current_window(1_030)?;

        let mut protocol = AdapterWorkerProtocolState::from_window_record(
            &record,
            AdapterWorkerProtocolPolicy::default(),
        );
        protocol.activate_window()?;
        let public_key_hex = hex::encode(signing_key.verifying_key().to_bytes());
        let identity = AdapterWorkerIdentity::new(
            "worker-b",
            "session-1",
            AdapterWorkerTrustClass::SemiTrustedContributor,
            "auth://worker-b",
        )
        .with_submission_signing_public_key_hex(public_key_hex);
        protocol.record_heartbeat(identity, None, None, 1_030)?;

        let assignment = protocol.assignments[0].clone();
        let claim =
            protocol.claim_assignment("worker-b", assignment.assignment_id.as_str(), 1_031)?;
        protocol.acknowledge_assignment("worker-b", "session-1", claim.claim_id.as_str(), 1_032)?;
        let submission = protocol.submit_contribution(
            claim.claim_id.as_str(),
            "worker-b",
            "session-1",
            "policy-r7",
            record.plan.input_checkpoint_pointer.pointer_digest.as_str(),
            crate::AdapterContributionExecutionSummary::new(
                1_033,
                1_040,
                5,
                20,
                Some(205),
                "delta-digest-aggregation",
            )?,
            crate::AdapterContributionUploadLocator::new(
                format!(
                    "object://adapter-window/{}/{}",
                    record.plan.window_id, claim.contribution_id
                ),
                "upload-manifest-aggregation",
                4_096,
            )?,
            1_041,
        )?;

        let payload = b"adapter-delta-payload".repeat(4);
        let manifest = DatastreamManifest::from_bytes(
            format!(
                "adapter-contribution:{}:{}",
                assignment.window_id, assignment.contribution_id
            ),
            DatastreamSubjectKind::AdapterPackage,
            &payload,
            8,
            DatastreamEncoding::RawBinary,
        )
        .with_provenance_digest(assignment.upload_expectation.expectation_digest.clone());
        let upload = crate::AdapterContributionUploadLocator::new(
            assignment
                .upload_expectation
                .upload_reference_prefix
                .clone(),
            manifest.manifest_ref().manifest_digest.clone(),
            manifest.manifest_ref().total_bytes,
        )?;
        let mut storage =
            AdapterArtifactStorageState::new(AdapterArtifactRetentionPolicy::default())?;
        let cursor = storage.start_contribution_upload(
            &assignment,
            upload,
            &payload,
            8,
            "worker-b",
            1_042,
        )?;
        for chunk in payload.chunks(8) {
            storage.commit_next_chunk(cursor.upload_id.as_str(), chunk)?;
        }
        let artifact = storage.complete_contribution_upload(cursor.upload_id.as_str(), 1_043)?;

        let session = protocol.sessions[0].identity.clone();
        let provenance = crate::AdapterContributionProvenanceBundle::new_signed(
            &assignment,
            &claim,
            &session,
            &submission,
            &artifact,
            signing_key,
            1_044,
        );
        let mut security_controller = AdapterContributionSecurityController::new(
            AdapterContributionSecurityPolicy::default(),
        );
        let security = security_controller.assess_submission(
            &protocol,
            &artifact,
            &submission,
            provenance.clone(),
            1_045,
        )?;
        coordinator.windows[0].window = protocol.window;
        Ok((
            coordinator,
            AdapterContributionValidationBundle::new(
                submission, artifact, provenance, security, None,
            ),
        ))
    }

    #[test]
    fn aggregator_promotes_window_and_advances_coordinator_inputs()
    -> Result<(), Box<dyn std::error::Error>> {
        let signing_key = SigningKey::from_bytes(&[53_u8; 32]);
        let (mut coordinator, mut bundle) = coordinator_fixture(&signing_key)?;
        bundle.replay = Some(AdapterContributionReplayReceipt::new(
            bundle.artifact.contribution_id.clone(),
            bundle
                .submission
                .execution_summary
                .adapter_delta_digest
                .clone(),
            bundle
                .submission
                .execution_summary
                .adapter_delta_digest
                .clone(),
            1_046,
        ));
        let environment =
            psionic_environments::EnvironmentPackageKey::new("oa.apple.adapter", "2026.03");
        let candidate = AdapterWindowCandidateEvaluation {
            candidate_policy_revision: Some(PolicyRevision::new(
                "apple.weather.policy",
                "policy-r8-candidate",
                "policy-digest-r8-candidate",
                1_050,
            )),
            held_out_eval: Some(finalized_eval_run(
                "heldout-1",
                EvalRunMode::OfflineHeldOut,
                environment.clone(),
                1,
                9_200,
            )?),
            benchmark_summary: Some(benchmark_summary(environment.clone(), 9_500)?),
            runtime_smoke_eval: Some(finalized_eval_run(
                "runtime-smoke-1",
                EvalRunMode::OnlineShadow,
                environment,
                1,
                10_000,
            )?),
        };
        let mut validator =
            AdapterContributionValidatorState::new(AdapterContributionValidatorPolicy {
                replay_sample_bps: 10_000,
                ..AdapterContributionValidatorPolicy::default()
            });
        let summary = validator.validate_window(
            &mut coordinator.windows[0].window,
            vec![bundle.clone()],
            Some(&candidate),
            1_047,
        )?;
        let mut aggregator = AdapterPolicyAggregator::default();
        let receipt = aggregator.promote_current_window(
            &mut coordinator,
            &summary,
            vec![bundle],
            1_050,
            1_051,
        )?;
        assert_eq!(
            receipt.promotion_disposition,
            AdapterPolicyPromotionDisposition::Promoted
        );
        let output_policy = receipt
            .output_policy_revision
            .as_ref()
            .expect("promotion should produce a policy");
        assert_eq!(
            output_policy.parent_revision_id.as_deref(),
            Some("policy-r7")
        );
        assert_eq!(
            coordinator.current_policy_revision.revision_id,
            output_policy.revision_id
        );
        assert_eq!(
            coordinator.current_checkpoint_pointer.pointer_digest,
            receipt
                .output_checkpoint_pointer
                .as_ref()
                .expect("promotion should produce checkpoint pointer")
                .pointer_digest
        );
        assert!(coordinator.current_window_id.is_none());

        let next = coordinator.plan_next_window(
            vec![AdapterDatasetSliceIdentity::new(
                "dataset.weather",
                "train",
                "slice-c",
                "slice-digest-c",
            )?],
            1,
            1_060,
        )?;
        assert_eq!(
            next.plan.input_policy_revision.revision_id,
            output_policy.revision_id
        );
        assert_eq!(
            next.plan.input_checkpoint_pointer.pointer_digest,
            coordinator.current_checkpoint_pointer.pointer_digest
        );
        Ok(())
    }

    #[test]
    fn aggregator_holds_when_validator_summary_blocks_promotion()
    -> Result<(), Box<dyn std::error::Error>> {
        let signing_key = SigningKey::from_bytes(&[59_u8; 32]);
        let (mut coordinator, mut bundle) = coordinator_fixture(&signing_key)?;
        bundle.replay = Some(AdapterContributionReplayReceipt::new(
            bundle.artifact.contribution_id.clone(),
            bundle
                .submission
                .execution_summary
                .adapter_delta_digest
                .clone(),
            bundle
                .submission
                .execution_summary
                .adapter_delta_digest
                .clone(),
            1_046,
        ));
        let environment =
            psionic_environments::EnvironmentPackageKey::new("oa.apple.adapter", "2026.03");
        let candidate = AdapterWindowCandidateEvaluation {
            candidate_policy_revision: Some(PolicyRevision::new(
                "apple.weather.policy",
                "policy-r8-candidate",
                "policy-digest-r8-candidate",
                1_050,
            )),
            held_out_eval: Some(finalized_eval_run(
                "heldout-1",
                EvalRunMode::OfflineHeldOut,
                environment.clone(),
                1,
                9_200,
            )?),
            benchmark_summary: Some(benchmark_summary(environment, 9_500)?),
            runtime_smoke_eval: None,
        };
        let mut validator =
            AdapterContributionValidatorState::new(AdapterContributionValidatorPolicy {
                replay_sample_bps: 10_000,
                ..AdapterContributionValidatorPolicy::default()
            });
        let summary = validator.validate_window(
            &mut coordinator.windows[0].window,
            vec![bundle.clone()],
            Some(&candidate),
            1_047,
        )?;
        let prior_revision_id = coordinator.current_policy_revision.revision_id.clone();
        let mut aggregator = AdapterPolicyAggregator::default();
        let receipt = aggregator.promote_current_window(
            &mut coordinator,
            &summary,
            vec![bundle],
            1_050,
            1_051,
        )?;
        assert_eq!(
            receipt.promotion_disposition,
            AdapterPolicyPromotionDisposition::Held
        );
        assert!(
            receipt
                .hold_reason_codes
                .contains(&AdapterPromotionHoldReasonCode::ValidatorWindowNotPromotionReady)
        );
        assert!(receipt.output_policy_revision.is_none());
        assert_eq!(
            coordinator.current_policy_revision.revision_id,
            prior_revision_id
        );
        Ok(())
    }
}
