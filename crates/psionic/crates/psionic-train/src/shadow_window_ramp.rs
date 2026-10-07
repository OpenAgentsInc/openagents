//! Shadow-window join ramp contract (psionic#1125, Pluralis roadmap P1.1).
//!
//! A joining device does not enter a paid merged window on day one. The ramp
//! adapts the Pluralis agora sync phases (read-only reference lane
//! `projects/pluralis/repos/agora`, weight=0 semantics) into the Psion
//! statistical-lane window vocabulary:
//!
//! - Phase 1 (`shadow_replay`): the joiner downloads the durable checkpoint,
//!   verifies its digests, and replays a sealed window locally. Receipts are
//!   checked but never merged.
//! - Phase 2 (`live_shadow`): the joiner runs live windows whose outputs are
//!   verified and receipted but excluded from the merge (our weight=0
//!   analogue), warming scheduler trust.
//! - Then `active`: merged windows at the full class rate.
//!
//! Ramp lengths are config, not constants. Pluralis ships 400+100 steps; that
//! is their tuning, not ours, and this module deliberately has no default
//! ramp lengths. The R1 deliverable is measuring the ramp length that reduces
//! post-join divergence; the divergence-comparison contract below is the
//! measurement hook, and no live measurement is claimed here.

use std::{collections::BTreeSet, fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PSIONIC_TRAIN_CONTRIBUTION_RECEIPT_SCHEMA_VERSION,
    PSIONIC_TRAIN_WINDOW_EXECUTION_SCHEMA_VERSION,
};

pub const SHADOW_WINDOW_RAMP_SCHEMA_VERSION: &str = "psionic.train.shadow_window_ramp.v1";
pub const SHADOW_WINDOW_RAMP_CONTRACT_ID: &str = "psionic.train.shadow_window_ramp.v1";
pub const SHADOW_WINDOW_RAMP_FIXTURE_PATH: &str = "fixtures/training/shadow_window_ramp_v1.json";
pub const SHADOW_WINDOW_RAMP_CHECK_SCRIPT_PATH: &str = "scripts/check-shadow-window-ramp.sh";
pub const SHADOW_WINDOW_RAMP_DOC_PATH: &str = "docs/PSION_SHADOW_WINDOW_RAMP.md";
pub const SHADOW_WINDOW_RAMP_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";

#[derive(Debug, Error)]
pub enum ShadowWindowRampError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error("shadow window ramp config is invalid: {detail}")]
    InvalidConfig { detail: String },
    #[error("shadow join checkpoint binding is invalid: {detail}")]
    InvalidCheckpointBinding { detail: String },
    #[error("shadow window receipt `{receipt_id}` is invalid: {detail}")]
    InvalidReceipt { receipt_id: String, detail: String },
    #[error(
        "shadow window receipt `{receipt_id}` carries phase `{found:?}` but the ramp is in phase `{expected:?}`"
    )]
    PhaseMismatch {
        receipt_id: String,
        expected: ShadowWindowPhase,
        found: ShadowWindowPhase,
    },
    #[error(
        "cannot advance out of phase `{phase:?}`: {completed} of {required} verified windows completed"
    )]
    PhaseCountNotSatisfied {
        phase: ShadowWindowPhase,
        completed: u32,
        required: u32,
    },
    #[error("cannot skip from phase `{from:?}` to phase `{to:?}`; phases advance one at a time")]
    CannotSkipPhase {
        from: ShadowWindowPhase,
        to: ShadowWindowPhase,
    },
    #[error("ramp is already in phase `active`; shadow ramp progress no longer applies")]
    AlreadyActive,
    #[error(
        "receipt `{receipt_id}` is a shadow receipt and is structurally excluded from every merge set"
    )]
    ShadowReceiptExcludedFromMerge { receipt_id: String },
    #[error("shadow window ramp contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

/// Join-ramp phase, adapted from the Pluralis agora sync phases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowWindowPhase {
    /// Phase 1: replay a sealed window against the verified durable checkpoint.
    ShadowReplay,
    /// Phase 2: run live windows verified and receipted but excluded from merge.
    LiveShadow,
    /// Ramp complete: windows are merged and paid at the full class rate.
    Active,
}

impl ShadowWindowPhase {
    #[must_use]
    pub fn next(self) -> Option<Self> {
        match self {
            Self::ShadowReplay => Some(Self::LiveShadow),
            Self::LiveShadow => Some(Self::Active),
            Self::Active => None,
        }
    }
}

/// Merge eligibility marker carried by every shadow-ramp receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowMergeEligibility {
    /// Verified and receipted but structurally excluded from every merge set.
    Shadow,
    /// Admissible into a merge set.
    MergeEligible,
}

/// Ramp lengths. These are operator config, never constants: Pluralis ships
/// 400+100 steps as their tuning, and this type deliberately has no `Default`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShadowWindowRampConfig {
    /// Verified phase-1 shadow-replay windows required before phase 2.
    pub phase1_shadow_replay_window_count: u32,
    /// Verified phase-2 live-shadow windows required before active.
    pub phase2_live_shadow_window_count: u32,
}

impl ShadowWindowRampConfig {
    pub fn validate(&self) -> Result<(), ShadowWindowRampError> {
        if self.phase1_shadow_replay_window_count == 0 {
            return Err(ShadowWindowRampError::InvalidConfig {
                detail: String::from(
                    "phase1_shadow_replay_window_count must be at least 1; a zero-length phase 1 skips checkpoint replay verification",
                ),
            });
        }
        if self.phase2_live_shadow_window_count == 0 {
            return Err(ShadowWindowRampError::InvalidConfig {
                detail: String::from(
                    "phase2_live_shadow_window_count must be at least 1; a zero-length phase 2 skips live shadow verification",
                ),
            });
        }
        Ok(())
    }

    #[must_use]
    pub fn required_window_count(&self, phase: ShadowWindowPhase) -> u32 {
        match phase {
            ShadowWindowPhase::ShadowReplay => self.phase1_shadow_replay_window_count,
            ShadowWindowPhase::LiveShadow => self.phase2_live_shadow_window_count,
            ShadowWindowPhase::Active => 0,
        }
    }
}

/// Durable-checkpoint binding the joiner verified before phase 1.
///
/// The digests bind to the `PsionicTrainCheckpointManifest` fields of the same
/// names (`psionic.train.checkpoint_manifest.v1` lane surface).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShadowJoinCheckpointBinding {
    /// SHA256 hex digest of the checkpoint object the joiner downloaded.
    pub checkpoint_object_digest: String,
    /// SHA256 hex digest of the checkpoint manifest the joiner verified.
    pub manifest_digest: String,
}

impl ShadowJoinCheckpointBinding {
    pub fn validate(&self) -> Result<(), ShadowWindowRampError> {
        for (field, value) in [
            ("checkpoint_object_digest", &self.checkpoint_object_digest),
            ("manifest_digest", &self.manifest_digest),
        ] {
            if value.len() != 64 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(ShadowWindowRampError::InvalidCheckpointBinding {
                    detail: format!("{field} must be a 64-character SHA256 hex digest"),
                });
            }
        }
        Ok(())
    }
}

/// Shadow-ramp execution receipt.
///
/// This wraps the contribution-receipt concept from `train_window.rs`: it
/// references a real `PsionicTrainWindowExecution` id and the contribution
/// receipt schema, and adds the explicit phase and merge-eligibility marker.
/// These are statistical-lane contracts: `verified` records that the lane's
/// verification policy accepted the output, not bit-exact replay.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShadowWindowExecutionReceipt {
    pub receipt_id: String,
    pub window_id: String,
    /// `PsionicTrainWindowExecution.window_execution_id` this receipt covers.
    pub window_execution_id: String,
    /// Must stay `psionic.train.window_execution.v1`.
    pub window_execution_schema_version: String,
    /// Must stay `psionic.train.contribution_receipt.v1`.
    pub contribution_receipt_schema_version: String,
    pub node_pubkey: String,
    pub phase: ShadowWindowPhase,
    pub merge_eligibility: ShadowMergeEligibility,
    /// Whether the lane's verification policy accepted the window output.
    pub verified: bool,
    pub detail: String,
}

impl ShadowWindowExecutionReceipt {
    pub fn validate(&self) -> Result<(), ShadowWindowRampError> {
        let invalid = |detail: String| ShadowWindowRampError::InvalidReceipt {
            receipt_id: self.receipt_id.clone(),
            detail,
        };
        if self.receipt_id.trim().is_empty()
            || self.window_id.trim().is_empty()
            || self.window_execution_id.trim().is_empty()
            || self.node_pubkey.trim().is_empty()
        {
            return Err(invalid(String::from(
                "receipt_id, window_id, window_execution_id, and node_pubkey must be nonempty",
            )));
        }
        if self.window_execution_schema_version != PSIONIC_TRAIN_WINDOW_EXECUTION_SCHEMA_VERSION {
            return Err(invalid(format!(
                "window_execution_schema_version must stay `{PSIONIC_TRAIN_WINDOW_EXECUTION_SCHEMA_VERSION}`"
            )));
        }
        if self.contribution_receipt_schema_version
            != PSIONIC_TRAIN_CONTRIBUTION_RECEIPT_SCHEMA_VERSION
        {
            return Err(invalid(format!(
                "contribution_receipt_schema_version must stay `{PSIONIC_TRAIN_CONTRIBUTION_RECEIPT_SCHEMA_VERSION}`"
            )));
        }
        match (self.phase, self.merge_eligibility) {
            (
                ShadowWindowPhase::ShadowReplay | ShadowWindowPhase::LiveShadow,
                ShadowMergeEligibility::MergeEligible,
            ) => Err(invalid(String::from(
                "shadow-phase receipts must carry merge_eligibility `shadow`",
            ))),
            (ShadowWindowPhase::Active, ShadowMergeEligibility::Shadow) => {
                Err(invalid(String::from(
                    "active-phase receipts must carry merge_eligibility `merge_eligible`",
                )))
            }
            _ => Ok(()),
        }
    }
}

/// Proof token that a receipt passed the merge-eligibility gate.
///
/// The inner receipt is private and there is no other constructor, so a
/// `merge_eligibility: shadow` receipt cannot become a merge-set member at
/// the type level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeEligibleReceipt(ShadowWindowExecutionReceipt);

impl MergeEligibleReceipt {
    /// Admits a receipt into merge eligibility. Shadow receipts are refused
    /// with a typed error; unverified receipts are refused.
    pub fn admit(receipt: ShadowWindowExecutionReceipt) -> Result<Self, ShadowWindowRampError> {
        receipt.validate()?;
        if receipt.merge_eligibility == ShadowMergeEligibility::Shadow {
            return Err(ShadowWindowRampError::ShadowReceiptExcludedFromMerge {
                receipt_id: receipt.receipt_id,
            });
        }
        if !receipt.verified {
            return Err(ShadowWindowRampError::InvalidReceipt {
                receipt_id: receipt.receipt_id.clone(),
                detail: String::from("unverified receipts cannot enter a merge set"),
            });
        }
        Ok(Self(receipt))
    }

    #[must_use]
    pub fn receipt(&self) -> &ShadowWindowExecutionReceipt {
        &self.0
    }
}

/// Merge-set builder that only accepts merge-eligible receipts.
///
/// `push` takes the proof token, so the only path for a receipt into the
/// merge set runs through `MergeEligibleReceipt::admit`.
#[derive(Debug, Default)]
pub struct ShadowAwareMergeSetBuilder {
    members: Vec<MergeEligibleReceipt>,
}

impl ShadowAwareMergeSetBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, receipt: MergeEligibleReceipt) {
        self.members.push(receipt);
    }

    /// Convenience path: admit-or-refuse one raw receipt.
    pub fn try_admit(
        &mut self,
        receipt: ShadowWindowExecutionReceipt,
    ) -> Result<(), ShadowWindowRampError> {
        self.push(MergeEligibleReceipt::admit(receipt)?);
        Ok(())
    }

    #[must_use]
    pub fn members(&self) -> &[MergeEligibleReceipt] {
        &self.members
    }

    #[must_use]
    pub fn member_receipt_ids(&self) -> Vec<String> {
        self.members
            .iter()
            .map(|member| member.receipt().receipt_id.clone())
            .collect()
    }
}

/// Per-joiner ramp state machine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShadowJoinRampState {
    pub node_pubkey: String,
    pub config: ShadowWindowRampConfig,
    pub checkpoint_binding: ShadowJoinCheckpointBinding,
    pub phase: ShadowWindowPhase,
    pub phase1_completed_window_count: u32,
    pub phase2_completed_window_count: u32,
}

impl ShadowJoinRampState {
    /// Starts the ramp in phase 1. The checkpoint digest binding is required
    /// before phase 1: a joiner that has not verified the durable checkpoint
    /// cannot begin shadow replay.
    pub fn begin(
        node_pubkey: impl Into<String>,
        config: ShadowWindowRampConfig,
        checkpoint_binding: ShadowJoinCheckpointBinding,
    ) -> Result<Self, ShadowWindowRampError> {
        config.validate()?;
        checkpoint_binding.validate()?;
        let node_pubkey = node_pubkey.into();
        if node_pubkey.trim().is_empty() {
            return Err(ShadowWindowRampError::InvalidContract {
                detail: String::from("ramp state requires a nonempty node_pubkey"),
            });
        }
        Ok(Self {
            node_pubkey,
            config,
            checkpoint_binding,
            phase: ShadowWindowPhase::ShadowReplay,
            phase1_completed_window_count: 0,
            phase2_completed_window_count: 0,
        })
    }

    /// Records one verified shadow window toward the current phase.
    pub fn record_verified_window(
        &mut self,
        receipt: &ShadowWindowExecutionReceipt,
    ) -> Result<(), ShadowWindowRampError> {
        receipt.validate()?;
        if self.phase == ShadowWindowPhase::Active {
            return Err(ShadowWindowRampError::AlreadyActive);
        }
        if receipt.phase != self.phase {
            return Err(ShadowWindowRampError::PhaseMismatch {
                receipt_id: receipt.receipt_id.clone(),
                expected: self.phase,
                found: receipt.phase,
            });
        }
        if receipt.node_pubkey != self.node_pubkey {
            return Err(ShadowWindowRampError::InvalidReceipt {
                receipt_id: receipt.receipt_id.clone(),
                detail: format!(
                    "receipt node `{}` does not match ramp joiner `{}`",
                    receipt.node_pubkey, self.node_pubkey
                ),
            });
        }
        if !receipt.verified {
            return Err(ShadowWindowRampError::InvalidReceipt {
                receipt_id: receipt.receipt_id.clone(),
                detail: String::from("unverified shadow windows do not advance the ramp"),
            });
        }
        match self.phase {
            ShadowWindowPhase::ShadowReplay => self.phase1_completed_window_count += 1,
            ShadowWindowPhase::LiveShadow => self.phase2_completed_window_count += 1,
            ShadowWindowPhase::Active => unreachable!("active handled above"),
        }
        Ok(())
    }

    /// Advances to the next phase when the configured window count for the
    /// current phase is satisfied.
    pub fn advance_phase(&mut self) -> Result<ShadowWindowPhase, ShadowWindowRampError> {
        let next = self
            .phase
            .next()
            .ok_or(ShadowWindowRampError::AlreadyActive)?;
        let completed = match self.phase {
            ShadowWindowPhase::ShadowReplay => self.phase1_completed_window_count,
            ShadowWindowPhase::LiveShadow => self.phase2_completed_window_count,
            ShadowWindowPhase::Active => unreachable!("next() returned None for active"),
        };
        let required = self.config.required_window_count(self.phase);
        if completed < required {
            return Err(ShadowWindowRampError::PhaseCountNotSatisfied {
                phase: self.phase,
                completed,
                required,
            });
        }
        self.phase = next;
        Ok(self.phase)
    }

    /// Advances to an explicit target phase. Skipping a phase is a typed
    /// error: phases advance one at a time.
    pub fn advance_to(
        &mut self,
        target: ShadowWindowPhase,
    ) -> Result<ShadowWindowPhase, ShadowWindowRampError> {
        match self.phase.next() {
            Some(next) if next == target => self.advance_phase(),
            Some(_) | None => Err(ShadowWindowRampError::CannotSkipPhase {
                from: self.phase,
                to: target,
            }),
        }
    }
}

/// Join-cohort label for the divergence comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowRampJoinCohortLabel {
    /// Joined through the phase-1 + phase-2 shadow ramp.
    RampedJoin,
    /// Joined by checkpoint bootstrap straight into merged windows.
    BootstrapAndMerge,
}

/// Whether the comparison carries synthetic fixture values or a real run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowRampMeasurementStatus {
    /// Values demonstrate the comparison shape only; they are synthetic.
    SyntheticFixture,
    /// Values come from a real post-join run (R1 hardware-gated; none exist).
    Measured,
}

/// One per-window divergence sample after a join.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShadowRampWindowDivergenceSample {
    /// Window index counted from the join (0 = first post-join window).
    pub window_index_since_join: u32,
    pub window_id: String,
    /// Divergence metric value for this window (kind named on the comparison).
    pub divergence_metric_value: f64,
}

/// One join cohort in the divergence comparison.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShadowRampDivergenceCohort {
    pub cohort_label: ShadowRampJoinCohortLabel,
    pub joining_node_pubkey: String,
    /// Ramp config used by the ramped cohort; absent for bootstrap-and-merge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ramp_config: Option<ShadowWindowRampConfig>,
    pub per_window_divergence: Vec<ShadowRampWindowDivergenceSample>,
    pub detail: String,
}

/// Divergence-comparison contract: ramped join vs bootstrap-and-merge.
///
/// This is the R1 measurement hook. The fixture demonstrates the shape with
/// synthetic values; the live measurement is hardware-gated and not claimed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShadowRampDivergenceComparison {
    pub comparison_id: String,
    /// Named divergence metric, e.g. parameter-space L2 distance against the
    /// stayed-cohort reference at each window seal.
    pub divergence_metric_kind: String,
    pub measurement_status: ShadowRampMeasurementStatus,
    /// Placeholder acceptance threshold; a real threshold is an R1 output.
    pub decision_threshold_placeholder: f64,
    pub cohorts: Vec<ShadowRampDivergenceCohort>,
    pub detail: String,
}

impl ShadowRampDivergenceComparison {
    pub fn validate(&self) -> Result<(), ShadowWindowRampError> {
        let invalid = |detail: String| ShadowWindowRampError::InvalidContract { detail };
        if self.comparison_id.trim().is_empty() || self.divergence_metric_kind.trim().is_empty() {
            return Err(invalid(String::from(
                "comparison_id and divergence_metric_kind must be nonempty",
            )));
        }
        if !self.decision_threshold_placeholder.is_finite()
            || self.decision_threshold_placeholder <= 0.0
        {
            return Err(invalid(String::from(
                "decision_threshold_placeholder must be a finite positive value",
            )));
        }
        let mut labels = BTreeSet::new();
        for cohort in &self.cohorts {
            if !labels.insert(format!("{:?}", cohort.cohort_label)) {
                return Err(invalid(format!(
                    "duplicate cohort label `{:?}`",
                    cohort.cohort_label
                )));
            }
            match cohort.cohort_label {
                ShadowRampJoinCohortLabel::RampedJoin => {
                    let config = cohort.ramp_config.as_ref().ok_or_else(|| {
                        invalid(String::from("ramped-join cohort must carry its ramp config"))
                    })?;
                    config.validate()?;
                }
                ShadowRampJoinCohortLabel::BootstrapAndMerge => {
                    if cohort.ramp_config.is_some() {
                        return Err(invalid(String::from(
                            "bootstrap-and-merge cohort must not carry a ramp config",
                        )));
                    }
                }
            }
            if cohort.per_window_divergence.is_empty() {
                return Err(invalid(format!(
                    "cohort `{:?}` must carry at least one per-window sample",
                    cohort.cohort_label
                )));
            }
            for (index, sample) in cohort.per_window_divergence.iter().enumerate() {
                if sample.window_index_since_join as usize != index {
                    return Err(invalid(format!(
                        "cohort `{:?}` sample {index} lost contiguous window indexing",
                        cohort.cohort_label
                    )));
                }
                if !sample.divergence_metric_value.is_finite()
                    || sample.divergence_metric_value < 0.0
                {
                    return Err(invalid(format!(
                        "cohort `{:?}` sample {index} must carry a finite nonnegative metric",
                        cohort.cohort_label
                    )));
                }
            }
        }
        if labels.len() != 2 {
            return Err(invalid(String::from(
                "comparison must carry exactly one ramped-join and one bootstrap-and-merge cohort",
            )));
        }
        if self.measurement_status == ShadowRampMeasurementStatus::SyntheticFixture
            && !self.detail.to_ascii_lowercase().contains("synthetic")
        {
            return Err(invalid(String::from(
                "synthetic-fixture comparisons must say so in their detail",
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShadowWindowRampAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub train_system_doc_path: String,
}

/// Canonical shadow-window join-ramp contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShadowWindowRampContract {
    pub schema_version: String,
    pub contract_id: String,
    /// Fixture-scale ramp lengths chosen for this contract walkthrough.
    pub ramp_config: ShadowWindowRampConfig,
    pub checkpoint_binding: ShadowJoinCheckpointBinding,
    pub final_ramp_state: ShadowJoinRampState,
    pub shadow_receipts: Vec<ShadowWindowExecutionReceipt>,
    /// Receipt ids the merge-set builder admitted.
    pub merge_set_member_receipt_ids: Vec<String>,
    /// Receipt ids the merge-set builder refused as shadow.
    pub merge_excluded_receipt_ids: Vec<String>,
    pub divergence_comparison: ShadowRampDivergenceComparison,
    pub authority_paths: ShadowWindowRampAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl ShadowWindowRampContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_shadow_window_ramp_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), ShadowWindowRampError> {
        if self.schema_version != SHADOW_WINDOW_RAMP_SCHEMA_VERSION {
            return Err(ShadowWindowRampError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    SHADOW_WINDOW_RAMP_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != SHADOW_WINDOW_RAMP_CONTRACT_ID {
            return Err(ShadowWindowRampError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.authority_paths.fixture_path != SHADOW_WINDOW_RAMP_FIXTURE_PATH
            || self.authority_paths.check_script_path != SHADOW_WINDOW_RAMP_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path != SHADOW_WINDOW_RAMP_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != SHADOW_WINDOW_RAMP_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(ShadowWindowRampError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }
        self.ramp_config.validate()?;
        self.checkpoint_binding.validate()?;

        let mut receipt_ids = BTreeSet::new();
        let mut execution_ids = BTreeSet::new();
        for receipt in &self.shadow_receipts {
            receipt.validate()?;
            if !receipt_ids.insert(receipt.receipt_id.as_str()) {
                return Err(ShadowWindowRampError::InvalidContract {
                    detail: format!("duplicate receipt_id `{}`", receipt.receipt_id),
                });
            }
            if !execution_ids.insert(receipt.window_execution_id.as_str()) {
                return Err(ShadowWindowRampError::InvalidContract {
                    detail: format!(
                        "duplicate window_execution_id `{}`",
                        receipt.window_execution_id
                    ),
                });
            }
        }

        // Replay the ramp over the committed receipts to confirm the recorded
        // final state is reachable without skipping a phase.
        let mut state = ShadowJoinRampState::begin(
            self.final_ramp_state.node_pubkey.clone(),
            self.ramp_config,
            self.checkpoint_binding.clone(),
        )?;
        for receipt in &self.shadow_receipts {
            match receipt.phase {
                ShadowWindowPhase::ShadowReplay | ShadowWindowPhase::LiveShadow => {
                    if receipt.phase != state.phase {
                        state.advance_phase()?;
                    }
                    state.record_verified_window(receipt)?;
                }
                ShadowWindowPhase::Active => {
                    if state.phase != ShadowWindowPhase::Active {
                        state.advance_phase()?;
                    }
                }
            }
        }
        if state != self.final_ramp_state {
            return Err(ShadowWindowRampError::InvalidContract {
                detail: String::from(
                    "final_ramp_state is not the state reached by replaying the committed receipts",
                ),
            });
        }

        // Rebuild the merge set through the type-level gate and confirm the
        // committed membership and exclusion lists match.
        let mut builder = ShadowAwareMergeSetBuilder::new();
        let mut excluded = Vec::new();
        for receipt in &self.shadow_receipts {
            match builder.try_admit(receipt.clone()) {
                Ok(()) => {}
                Err(ShadowWindowRampError::ShadowReceiptExcludedFromMerge { receipt_id }) => {
                    excluded.push(receipt_id);
                }
                Err(error) => return Err(error),
            }
        }
        if builder.member_receipt_ids() != self.merge_set_member_receipt_ids {
            return Err(ShadowWindowRampError::InvalidContract {
                detail: String::from("merge_set_member_receipt_ids drifted from the gate output"),
            });
        }
        if excluded != self.merge_excluded_receipt_ids {
            return Err(ShadowWindowRampError::InvalidContract {
                detail: String::from("merge_excluded_receipt_ids drifted from the gate output"),
            });
        }
        for member_id in &self.merge_set_member_receipt_ids {
            if self.merge_excluded_receipt_ids.contains(member_id) {
                return Err(ShadowWindowRampError::InvalidContract {
                    detail: format!("receipt `{member_id}` is both merged and excluded"),
                });
            }
        }

        self.divergence_comparison.validate()?;
        if self.divergence_comparison.measurement_status != ShadowRampMeasurementStatus::Measured
            && !self.claim_boundary.to_ascii_lowercase().contains("synthetic")
        {
            return Err(ShadowWindowRampError::InvalidContract {
                detail: String::from(
                    "claim_boundary must state that the divergence values are synthetic until a real R1 run lands",
                ),
            });
        }

        if self.contract_digest != self.stable_digest() {
            return Err(ShadowWindowRampError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }
        Ok(())
    }
}

static SHADOW_WINDOW_RAMP_CONTRACT_CACHE: std::sync::OnceLock<ShadowWindowRampContract> =
    std::sync::OnceLock::new();

pub fn canonical_shadow_window_ramp_contract(
) -> Result<ShadowWindowRampContract, ShadowWindowRampError> {
    if let Some(contract) = SHADOW_WINDOW_RAMP_CONTRACT_CACHE.get() {
        return Ok(contract.clone());
    }

    // Fixture-scale ramp lengths. These are config inputs to the walkthrough,
    // not recommended values; Pluralis ships 400+100 steps as their tuning and
    // the right lengths for us are the R1 measurement output.
    let ramp_config = ShadowWindowRampConfig {
        phase1_shadow_replay_window_count: 3,
        phase2_live_shadow_window_count: 2,
    };
    // Deterministic synthetic digests: this fixture binds the shape of the
    // checkpoint verification step, not a real durable checkpoint.
    let checkpoint_binding = ShadowJoinCheckpointBinding {
        checkpoint_object_digest: synthetic_digest(
            "psionic.shadow_window_ramp.fixture.checkpoint_object.v1",
        ),
        manifest_digest: synthetic_digest(
            "psionic.shadow_window_ramp.fixture.checkpoint_manifest.v1",
        ),
    };
    let node_pubkey = "shadow_ramp_fixture_joiner.pubkey";

    let mut shadow_receipts = Vec::new();
    for index in 0..ramp_config.phase1_shadow_replay_window_count {
        shadow_receipts.push(fixture_receipt(
            node_pubkey,
            ShadowWindowPhase::ShadowReplay,
            ShadowMergeEligibility::Shadow,
            index,
            "Phase-1 shadow replay of a sealed window against the verified durable checkpoint; receipts are checked but never merged.",
        ));
    }
    for index in 0..ramp_config.phase2_live_shadow_window_count {
        shadow_receipts.push(fixture_receipt(
            node_pubkey,
            ShadowWindowPhase::LiveShadow,
            ShadowMergeEligibility::Shadow,
            index,
            "Phase-2 live shadow window: output verified and receipted but excluded from the merge (weight=0 analogue), warming scheduler trust.",
        ));
    }
    shadow_receipts.push(fixture_receipt(
        node_pubkey,
        ShadowWindowPhase::Active,
        ShadowMergeEligibility::MergeEligible,
        0,
        "First post-ramp active window: merged and paid at the full class rate.",
    ));

    let mut state =
        ShadowJoinRampState::begin(node_pubkey, ramp_config, checkpoint_binding.clone())?;
    for receipt in &shadow_receipts {
        match receipt.phase {
            ShadowWindowPhase::ShadowReplay | ShadowWindowPhase::LiveShadow => {
                if receipt.phase != state.phase {
                    state.advance_phase()?;
                }
                state.record_verified_window(receipt)?;
            }
            ShadowWindowPhase::Active => {
                if state.phase != ShadowWindowPhase::Active {
                    state.advance_phase()?;
                }
            }
        }
    }

    let mut builder = ShadowAwareMergeSetBuilder::new();
    let mut merge_excluded_receipt_ids = Vec::new();
    for receipt in &shadow_receipts {
        match builder.try_admit(receipt.clone()) {
            Ok(()) => {}
            Err(ShadowWindowRampError::ShadowReceiptExcludedFromMerge { receipt_id }) => {
                merge_excluded_receipt_ids.push(receipt_id);
            }
            Err(error) => return Err(error),
        }
    }

    let divergence_comparison = ShadowRampDivergenceComparison {
        comparison_id: String::from("shadow_ramp_divergence_comparison.fixture.v1"),
        divergence_metric_kind: String::from(
            "parameter_space_l2_distance_to_stayed_cohort_reference_at_window_seal",
        ),
        measurement_status: ShadowRampMeasurementStatus::SyntheticFixture,
        decision_threshold_placeholder: 0.05,
        cohorts: vec![
            ShadowRampDivergenceCohort {
                cohort_label: ShadowRampJoinCohortLabel::RampedJoin,
                joining_node_pubkey: String::from(node_pubkey),
                ramp_config: Some(ramp_config),
                per_window_divergence: synthetic_divergence_samples(
                    "ramped",
                    &[0.041, 0.028, 0.017, 0.011],
                ),
                detail: String::from(
                    "Synthetic shape-only values for a joiner that completed the shadow ramp before merging.",
                ),
            },
            ShadowRampDivergenceCohort {
                cohort_label: ShadowRampJoinCohortLabel::BootstrapAndMerge,
                joining_node_pubkey: String::from("bootstrap_fixture_joiner.pubkey"),
                ramp_config: None,
                per_window_divergence: synthetic_divergence_samples(
                    "bootstrap",
                    &[0.094, 0.071, 0.052, 0.039],
                ),
                detail: String::from(
                    "Synthetic shape-only values for a joiner that bootstrapped the checkpoint and merged immediately.",
                ),
            },
        ],
        detail: String::from(
            "All divergence values are synthetic and demonstrate the comparison shape only. The real ramped-vs-bootstrap measurement is the R1 deliverable and requires live multi-node hardware.",
        ),
    };

    let mut contract = ShadowWindowRampContract {
        schema_version: String::from(SHADOW_WINDOW_RAMP_SCHEMA_VERSION),
        contract_id: String::from(SHADOW_WINDOW_RAMP_CONTRACT_ID),
        ramp_config,
        checkpoint_binding,
        final_ramp_state: state,
        shadow_receipts,
        merge_set_member_receipt_ids: builder.member_receipt_ids(),
        merge_excluded_receipt_ids,
        divergence_comparison,
        authority_paths: ShadowWindowRampAuthorityPaths {
            fixture_path: String::from(SHADOW_WINDOW_RAMP_FIXTURE_PATH),
            check_script_path: String::from(SHADOW_WINDOW_RAMP_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(SHADOW_WINDOW_RAMP_DOC_PATH),
            train_system_doc_path: String::from(SHADOW_WINDOW_RAMP_TRAIN_SYSTEM_DOC_PATH),
        },
        claim_boundary: String::from(
            "This contract freezes the shadow-window join ramp shape: config ramp lengths, digest-bound phase 1 replay, merge-excluded phase 2 live shadow windows, type-level merge-eligibility gating, and the divergence-comparison hook. The checkpoint digests and every divergence value are synthetic fixture inputs. No live ramp has run; measuring the ramp length that reduces post-join divergence is the hardware-gated R1 deliverable.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    let _ = SHADOW_WINDOW_RAMP_CONTRACT_CACHE.set(contract.clone());
    Ok(contract)
}

pub fn write_shadow_window_ramp_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), ShadowWindowRampError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| ShadowWindowRampError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let contract = canonical_shadow_window_ramp_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| ShadowWindowRampError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn fixture_receipt(
    node_pubkey: &str,
    phase: ShadowWindowPhase,
    merge_eligibility: ShadowMergeEligibility,
    index: u32,
    detail: &str,
) -> ShadowWindowExecutionReceipt {
    let phase_slug = match phase {
        ShadowWindowPhase::ShadowReplay => "shadow_replay",
        ShadowWindowPhase::LiveShadow => "live_shadow",
        ShadowWindowPhase::Active => "active",
    };
    ShadowWindowExecutionReceipt {
        receipt_id: format!("shadow_ramp_receipt.{phase_slug}.{index:06}"),
        window_id: format!("window.shadow_ramp_fixture.{phase_slug}.{index:06}"),
        window_execution_id: format!("window_execution.shadow_ramp_fixture.{phase_slug}.{index:06}"),
        window_execution_schema_version: String::from(
            PSIONIC_TRAIN_WINDOW_EXECUTION_SCHEMA_VERSION,
        ),
        contribution_receipt_schema_version: String::from(
            PSIONIC_TRAIN_CONTRIBUTION_RECEIPT_SCHEMA_VERSION,
        ),
        node_pubkey: String::from(node_pubkey),
        phase,
        merge_eligibility,
        verified: true,
        detail: String::from(detail),
    }
}

fn synthetic_divergence_samples(
    cohort_slug: &str,
    values: &[f64],
) -> Vec<ShadowRampWindowDivergenceSample> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| ShadowRampWindowDivergenceSample {
            window_index_since_join: index as u32,
            window_id: format!("window.shadow_ramp_fixture.{cohort_slug}_post_join.{index:06}"),
            divergence_metric_value: *value,
        })
        .collect()
}

fn synthetic_digest(label: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_shadow_window_ramp_synthetic_digest|");
    hasher.update(label.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("stable digest serialization must succeed for shadow window ramp contract"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_binding() -> ShadowJoinCheckpointBinding {
        ShadowJoinCheckpointBinding {
            checkpoint_object_digest: synthetic_digest("test.checkpoint_object"),
            manifest_digest: synthetic_digest("test.checkpoint_manifest"),
        }
    }

    fn fixture_config() -> ShadowWindowRampConfig {
        ShadowWindowRampConfig {
            phase1_shadow_replay_window_count: 2,
            phase2_live_shadow_window_count: 1,
        }
    }

    fn receipt(
        phase: ShadowWindowPhase,
        merge_eligibility: ShadowMergeEligibility,
        index: u32,
    ) -> ShadowWindowExecutionReceipt {
        fixture_receipt("test_joiner.pubkey", phase, merge_eligibility, index, "test")
    }

    #[test]
    fn canonical_shadow_window_ramp_contract_is_valid() -> Result<(), ShadowWindowRampError> {
        let contract = canonical_shadow_window_ramp_contract()?;
        contract.validate()
    }

    #[test]
    fn canonical_contract_round_trips_through_json() -> Result<(), ShadowWindowRampError> {
        let contract = canonical_shadow_window_ramp_contract()?;
        let bytes = serde_json::to_vec(&contract)?;
        let decoded: ShadowWindowRampContract = serde_json::from_slice(&bytes)?;
        assert_eq!(contract, decoded);
        decoded.validate()
    }

    #[test]
    fn committed_fixture_round_trips_and_matches_canonical(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .ok_or("missing repo root")?;
        let fixture_path = repo_root.join(SHADOW_WINDOW_RAMP_FIXTURE_PATH);
        let bytes = fs::read(&fixture_path)?;
        let committed: ShadowWindowRampContract = serde_json::from_slice(&bytes)?;
        committed.validate()?;
        assert_eq!(committed, canonical_shadow_window_ramp_contract()?);
        Ok(())
    }

    #[test]
    fn legal_phase_progression_reaches_active() -> Result<(), ShadowWindowRampError> {
        let mut state =
            ShadowJoinRampState::begin("test_joiner.pubkey", fixture_config(), fixture_binding())?;
        assert_eq!(state.phase, ShadowWindowPhase::ShadowReplay);
        state.record_verified_window(&receipt(
            ShadowWindowPhase::ShadowReplay,
            ShadowMergeEligibility::Shadow,
            0,
        ))?;
        state.record_verified_window(&receipt(
            ShadowWindowPhase::ShadowReplay,
            ShadowMergeEligibility::Shadow,
            1,
        ))?;
        assert_eq!(state.advance_phase()?, ShadowWindowPhase::LiveShadow);
        state.record_verified_window(&receipt(
            ShadowWindowPhase::LiveShadow,
            ShadowMergeEligibility::Shadow,
            0,
        ))?;
        assert_eq!(state.advance_phase()?, ShadowWindowPhase::Active);
        assert!(matches!(
            state.advance_phase(),
            Err(ShadowWindowRampError::AlreadyActive)
        ));
        Ok(())
    }

    #[test]
    fn advance_refuses_before_phase_count_satisfied() -> Result<(), ShadowWindowRampError> {
        let mut state =
            ShadowJoinRampState::begin("test_joiner.pubkey", fixture_config(), fixture_binding())?;
        state.record_verified_window(&receipt(
            ShadowWindowPhase::ShadowReplay,
            ShadowMergeEligibility::Shadow,
            0,
        ))?;
        let error = state
            .advance_phase()
            .expect_err("one of two phase-1 windows cannot satisfy the ramp");
        assert!(matches!(
            error,
            ShadowWindowRampError::PhaseCountNotSatisfied {
                phase: ShadowWindowPhase::ShadowReplay,
                completed: 1,
                required: 2,
            }
        ));
        Ok(())
    }

    #[test]
    fn phases_cannot_be_skipped() -> Result<(), ShadowWindowRampError> {
        let mut state =
            ShadowJoinRampState::begin("test_joiner.pubkey", fixture_config(), fixture_binding())?;
        state.record_verified_window(&receipt(
            ShadowWindowPhase::ShadowReplay,
            ShadowMergeEligibility::Shadow,
            0,
        ))?;
        state.record_verified_window(&receipt(
            ShadowWindowPhase::ShadowReplay,
            ShadowMergeEligibility::Shadow,
            1,
        ))?;
        let error = state
            .advance_to(ShadowWindowPhase::Active)
            .expect_err("shadow replay cannot jump straight to active");
        assert!(matches!(
            error,
            ShadowWindowRampError::CannotSkipPhase {
                from: ShadowWindowPhase::ShadowReplay,
                to: ShadowWindowPhase::Active,
            }
        ));
        Ok(())
    }

    #[test]
    fn wrong_phase_receipt_is_refused() -> Result<(), ShadowWindowRampError> {
        let mut state =
            ShadowJoinRampState::begin("test_joiner.pubkey", fixture_config(), fixture_binding())?;
        let error = state
            .record_verified_window(&receipt(
                ShadowWindowPhase::LiveShadow,
                ShadowMergeEligibility::Shadow,
                0,
            ))
            .expect_err("phase-2 receipt cannot count toward phase 1");
        assert!(matches!(
            error,
            ShadowWindowRampError::PhaseMismatch {
                expected: ShadowWindowPhase::ShadowReplay,
                found: ShadowWindowPhase::LiveShadow,
                ..
            }
        ));
        Ok(())
    }

    #[test]
    fn digest_binding_is_required_before_phase_one() {
        let error = ShadowJoinRampState::begin(
            "test_joiner.pubkey",
            fixture_config(),
            ShadowJoinCheckpointBinding {
                checkpoint_object_digest: String::new(),
                manifest_digest: synthetic_digest("test.checkpoint_manifest"),
            },
        )
        .expect_err("missing checkpoint object digest cannot begin the ramp");
        assert!(matches!(
            error,
            ShadowWindowRampError::InvalidCheckpointBinding { .. }
        ));
    }

    #[test]
    fn zero_length_ramp_phases_are_refused() {
        let error = ShadowWindowRampConfig {
            phase1_shadow_replay_window_count: 0,
            phase2_live_shadow_window_count: 1,
        }
        .validate()
        .expect_err("zero-length phase 1 must be refused");
        assert!(matches!(error, ShadowWindowRampError::InvalidConfig { .. }));
    }

    #[test]
    fn ramp_lengths_are_config_driven() -> Result<(), ShadowWindowRampError> {
        let mut short = ShadowJoinRampState::begin(
            "test_joiner.pubkey",
            ShadowWindowRampConfig {
                phase1_shadow_replay_window_count: 1,
                phase2_live_shadow_window_count: 1,
            },
            fixture_binding(),
        )?;
        short.record_verified_window(&receipt(
            ShadowWindowPhase::ShadowReplay,
            ShadowMergeEligibility::Shadow,
            0,
        ))?;
        assert_eq!(short.advance_phase()?, ShadowWindowPhase::LiveShadow);

        let mut long = ShadowJoinRampState::begin(
            "test_joiner.pubkey",
            ShadowWindowRampConfig {
                phase1_shadow_replay_window_count: 4,
                phase2_live_shadow_window_count: 1,
            },
            fixture_binding(),
        )?;
        long.record_verified_window(&receipt(
            ShadowWindowPhase::ShadowReplay,
            ShadowMergeEligibility::Shadow,
            0,
        ))?;
        assert!(matches!(
            long.advance_phase(),
            Err(ShadowWindowRampError::PhaseCountNotSatisfied { required: 4, .. })
        ));
        Ok(())
    }

    #[test]
    fn merge_set_refuses_shadow_receipts() {
        let mut builder = ShadowAwareMergeSetBuilder::new();
        let error = builder
            .try_admit(receipt(
                ShadowWindowPhase::LiveShadow,
                ShadowMergeEligibility::Shadow,
                0,
            ))
            .expect_err("shadow receipt cannot enter a merge set");
        assert!(matches!(
            error,
            ShadowWindowRampError::ShadowReceiptExcludedFromMerge { .. }
        ));
        assert!(builder.members().is_empty());

        builder
            .try_admit(receipt(
                ShadowWindowPhase::Active,
                ShadowMergeEligibility::MergeEligible,
                0,
            ))
            .expect("merge-eligible active receipt must be admitted");
        assert_eq!(builder.members().len(), 1);
    }

    #[test]
    fn shadow_phase_receipt_cannot_claim_merge_eligibility() {
        let error = receipt(
            ShadowWindowPhase::LiveShadow,
            ShadowMergeEligibility::MergeEligible,
            0,
        )
        .validate()
        .expect_err("live-shadow receipt cannot claim merge eligibility");
        assert!(matches!(error, ShadowWindowRampError::InvalidReceipt { .. }));
    }

    #[test]
    fn contract_refuses_shadow_receipt_flipped_to_merge_eligible(
    ) -> Result<(), ShadowWindowRampError> {
        let mut contract = canonical_shadow_window_ramp_contract()?;
        let shadow_receipt = contract
            .shadow_receipts
            .iter_mut()
            .find(|receipt| receipt.phase == ShadowWindowPhase::LiveShadow)
            .expect("canonical contract must carry a live-shadow receipt");
        shadow_receipt.merge_eligibility = ShadowMergeEligibility::MergeEligible;
        contract.contract_digest = contract.stable_digest();
        let error = contract
            .validate()
            .expect_err("a live-shadow receipt cannot flip into the merge set");
        assert!(matches!(error, ShadowWindowRampError::InvalidReceipt { .. }));
        Ok(())
    }

    #[test]
    fn contract_refuses_synthetic_comparison_without_synthetic_label(
    ) -> Result<(), ShadowWindowRampError> {
        let mut contract = canonical_shadow_window_ramp_contract()?;
        contract.divergence_comparison.detail = String::from("looks great");
        contract.contract_digest = contract.stable_digest();
        let error = contract
            .validate()
            .expect_err("synthetic comparison must say it is synthetic");
        assert!(matches!(
            error,
            ShadowWindowRampError::InvalidContract { .. }
        ));
        Ok(())
    }
}
