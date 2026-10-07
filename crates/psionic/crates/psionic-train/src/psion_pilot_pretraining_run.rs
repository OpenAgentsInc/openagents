use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionAcceptanceMatrix, PsionAcceptanceMatrixError, PsionBenchmarkFamily, PsionMetricKind,
    PsionPhaseGate, PsionPhasePromotionLedger, PsionPretrainRunObservabilityReceipt,
    PsionPretrainStageRunReceipt, PsionPromotionDecisionReceipt, PsionRouteKind,
};

/// Stable schema version for the first Psion pilot held-out-loss receipt.
pub const PSION_PILOT_HELD_OUT_LOSS_RECEIPT_SCHEMA_VERSION: &str =
    "psion.pilot_held_out_loss_receipt.v1";
/// Stable schema version for the first Psion pilot route/refusal probe receipt.
pub const PSION_PILOT_ROUTE_PROBE_RECEIPT_SCHEMA_VERSION: &str =
    "psion.pilot_route_probe_receipt.v1";
/// Stable schema version for the first Psion pilot pretraining run bundle.
pub const PSION_PILOT_PRETRAINING_RUN_BUNDLE_SCHEMA_VERSION: &str =
    "psion.pilot_pretraining_run_bundle.v1";

/// Minimum spec-vs-implementation pass rate frozen for the first pilot bundle.
pub const PSION_PILOT_MINIMUM_SPECIFICATION_BOUNDARY_PASS_RATE_BPS: u32 = 8600;

/// Held-out corpus family tracked by the first Psion pilot loss receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPilotHeldOutLossFamily {
    Textbooks,
    NormativeSpecs,
    TechnicalDocs,
}

impl PsionPilotHeldOutLossFamily {
    #[must_use]
    pub const fn required_families() -> [Self; 3] {
        [Self::Textbooks, Self::NormativeSpecs, Self::TechnicalDocs]
    }
}

/// One held-out loss delta row for the first Psion pilot run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPilotHeldOutLossRow {
    /// Held-out family represented by the row.
    pub family: PsionPilotHeldOutLossFamily,
    /// Seed baseline loss in milli-units.
    pub seed_baseline_loss_milli: u32,
    /// Pilot loss in milli-units.
    pub pilot_loss_milli: u32,
    /// Improvement over the seed baseline in basis points.
    pub improvement_over_seed_baseline_bps: u32,
    /// Short explanation of the row.
    pub detail: String,
}

/// Held-out loss receipt for the first Psion pilot run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPilotHeldOutLossReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stable run id.
    pub run_id: String,
    /// Stable stage id.
    pub stage_id: String,
    /// Stable model id.
    pub model_id: String,
    /// Stable digest of the bound pretrain-stage receipt.
    pub pretrain_stage_receipt_digest: String,
    /// Held-out family rows.
    pub rows: Vec<PsionPilotHeldOutLossRow>,
    /// Short summary of the held-out result.
    pub summary: String,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

impl PsionPilotHeldOutLossReceipt {
    /// Validates the held-out loss receipt against the pretrain stage.
    pub fn validate_against_stage(
        &self,
        stage_receipt: &PsionPretrainStageRunReceipt,
    ) -> Result<(), PsionPilotPretrainingRunError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "pilot_held_out_loss_receipt.schema_version",
        )?;
        if self.schema_version != PSION_PILOT_HELD_OUT_LOSS_RECEIPT_SCHEMA_VERSION {
            return Err(PsionPilotPretrainingRunError::SchemaVersionMismatch {
                expected: String::from(PSION_PILOT_HELD_OUT_LOSS_RECEIPT_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.receipt_id.as_str(),
            "pilot_held_out_loss_receipt.receipt_id",
        )?;
        check_string_match(
            self.run_id.as_str(),
            stage_receipt.run_id.as_str(),
            "run_id",
        )?;
        check_string_match(
            self.stage_id.as_str(),
            stage_receipt.stage_id.as_str(),
            "stage_id",
        )?;
        check_string_match(
            self.model_id.as_str(),
            stage_receipt.model_id.as_str(),
            "model_id",
        )?;
        check_string_match(
            self.pretrain_stage_receipt_digest.as_str(),
            stage_receipt.receipt_digest.as_str(),
            "pretrain_stage_receipt_digest",
        )?;
        ensure_nonempty(self.summary.as_str(), "pilot_held_out_loss_receipt.summary")?;
        if self.rows.is_empty() {
            return Err(PsionPilotPretrainingRunError::MissingField {
                field: String::from("pilot_held_out_loss_receipt.rows"),
            });
        }
        let mut families = BTreeSet::new();
        for row in &self.rows {
            if !families.insert(row.family) {
                return Err(PsionPilotPretrainingRunError::DuplicateHeldOutLossFamily {
                    family: held_out_loss_family_label(row.family).to_owned(),
                });
            }
            ensure_nonempty(
                row.detail.as_str(),
                "pilot_held_out_loss_receipt.rows[].detail",
            )?;
            if row.seed_baseline_loss_milli == 0
                || row.pilot_loss_milli == 0
                || row.pilot_loss_milli >= row.seed_baseline_loss_milli
                || row.improvement_over_seed_baseline_bps == 0
            {
                return Err(PsionPilotPretrainingRunError::InvalidHeldOutLossRow {
                    family: held_out_loss_family_label(row.family).to_owned(),
                });
            }
        }
        for family in PsionPilotHeldOutLossFamily::required_families() {
            if !families.contains(&family) {
                return Err(PsionPilotPretrainingRunError::MissingHeldOutLossFamily {
                    family: held_out_loss_family_label(family).to_owned(),
                });
            }
        }
        if self.receipt_digest != stable_pilot_held_out_loss_digest(self) {
            return Err(PsionPilotPretrainingRunError::ReceiptDigestMismatch {
                receipt_kind: String::from("pilot_held_out_loss_receipt"),
            });
        }
        Ok(())
    }

    #[must_use]
    pub fn minimum_improvement_bps(&self) -> u32 {
        self.rows
            .iter()
            .map(|row| row.improvement_over_seed_baseline_bps)
            .min()
            .unwrap_or(0)
    }
}

/// Probe class tracked by the first Psion pilot route/refusal receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPilotRouteProbeKind {
    SupportedArchitectureExplanation,
    ExactExecutionRequest,
    UnderspecifiedDesignTask,
}

impl PsionPilotRouteProbeKind {
    #[must_use]
    pub const fn required_kinds() -> [Self; 3] {
        [
            Self::SupportedArchitectureExplanation,
            Self::ExactExecutionRequest,
            Self::UnderspecifiedDesignTask,
        ]
    }
}

/// One route or refusal probe row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPilotRouteProbeRow {
    /// Probe class represented by the row.
    pub probe_kind: PsionPilotRouteProbeKind,
    /// Expected route for the probe.
    pub expected_route: PsionRouteKind,
    /// Observed route-selection accuracy for the probe.
    pub observed_route_accuracy_bps: u32,
    /// Short explanation of the row.
    pub detail: String,
}

/// Route and refusal probe receipt for the first Psion pilot run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPilotRouteProbeReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stable run id.
    pub run_id: String,
    /// Stable stage id.
    pub stage_id: String,
    /// Probe rows for the pilot.
    pub rows: Vec<PsionPilotRouteProbeRow>,
    /// Aggregate route-selection accuracy in basis points.
    pub aggregate_route_selection_accuracy_bps: u32,
    /// Short summary of the probe surface.
    pub summary: String,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

impl PsionPilotRouteProbeReceipt {
    /// Validates the route/refusal probe receipt against the pretrain stage.
    pub fn validate_against_stage(
        &self,
        stage_receipt: &PsionPretrainStageRunReceipt,
    ) -> Result<(), PsionPilotPretrainingRunError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "pilot_route_probe_receipt.schema_version",
        )?;
        if self.schema_version != PSION_PILOT_ROUTE_PROBE_RECEIPT_SCHEMA_VERSION {
            return Err(PsionPilotPretrainingRunError::SchemaVersionMismatch {
                expected: String::from(PSION_PILOT_ROUTE_PROBE_RECEIPT_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.receipt_id.as_str(),
            "pilot_route_probe_receipt.receipt_id",
        )?;
        check_string_match(
            self.run_id.as_str(),
            stage_receipt.run_id.as_str(),
            "run_id",
        )?;
        check_string_match(
            self.stage_id.as_str(),
            stage_receipt.stage_id.as_str(),
            "stage_id",
        )?;
        ensure_nonempty(self.summary.as_str(), "pilot_route_probe_receipt.summary")?;
        if self.rows.is_empty() {
            return Err(PsionPilotPretrainingRunError::MissingField {
                field: String::from("pilot_route_probe_receipt.rows"),
            });
        }
        let mut seen_kinds = BTreeSet::new();
        let mut accuracy_sum = 0_u32;
        for row in &self.rows {
            if !seen_kinds.insert(row.probe_kind) {
                return Err(PsionPilotPretrainingRunError::DuplicateRouteProbeKind {
                    probe_kind: route_probe_kind_label(row.probe_kind).to_owned(),
                });
            }
            ensure_nonempty(
                row.detail.as_str(),
                "pilot_route_probe_receipt.rows[].detail",
            )?;
            validate_bps(
                row.observed_route_accuracy_bps,
                "pilot_route_probe_receipt.rows[].observed_route_accuracy_bps",
            )?;
            let expected_route = required_route_for_probe(row.probe_kind);
            if row.expected_route != expected_route {
                return Err(
                    PsionPilotPretrainingRunError::RouteProbeExpectedRouteMismatch {
                        probe_kind: route_probe_kind_label(row.probe_kind).to_owned(),
                        expected_route: route_kind_label(expected_route).to_owned(),
                        actual_route: route_kind_label(row.expected_route).to_owned(),
                    },
                );
            }
            accuracy_sum = accuracy_sum.saturating_add(row.observed_route_accuracy_bps);
        }
        for required_kind in PsionPilotRouteProbeKind::required_kinds() {
            if !seen_kinds.contains(&required_kind) {
                return Err(PsionPilotPretrainingRunError::MissingRouteProbeKind {
                    probe_kind: route_probe_kind_label(required_kind).to_owned(),
                });
            }
        }
        if self.aggregate_route_selection_accuracy_bps
            != accuracy_sum / (self.rows.len() as u32).max(1)
        {
            return Err(PsionPilotPretrainingRunError::InvalidRouteProbeAggregate);
        }
        if self.receipt_digest != stable_pilot_route_probe_digest(self) {
            return Err(PsionPilotPretrainingRunError::ReceiptDigestMismatch {
                receipt_kind: String::from("pilot_route_probe_receipt"),
            });
        }
        Ok(())
    }
}

/// Full pilot pretraining run bundle tied to the pretrain and acceptance contracts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionPilotPretrainingRunBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Acceptance matrix identifier this bundle targets.
    pub acceptance_matrix_id: String,
    /// Acceptance matrix version this bundle targets.
    pub acceptance_matrix_version: String,
    /// Bound pretrain-stage receipt.
    pub pretrain_stage_receipt: PsionPretrainStageRunReceipt,
    /// Bound run-observability receipt.
    pub observability_receipt: PsionPretrainRunObservabilityReceipt,
    /// Held-out loss receipt across the three pilot family slices.
    pub held_out_loss_receipt: PsionPilotHeldOutLossReceipt,
    /// Explicit route and refusal probe receipt for pilot boundary tasks.
    pub route_probe_receipt: PsionPilotRouteProbeReceipt,
    /// Promotion decision recorded against the acceptance matrix.
    pub promotion_decision_receipt: PsionPromotionDecisionReceipt,
    /// Short operator-visible summary.
    pub summary: String,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

impl PsionPilotPretrainingRunBundle {
    /// Validates the bundle against the acceptance matrix and linked receipts.
    pub fn validate_against_matrix(
        &self,
        matrix: &PsionAcceptanceMatrix,
    ) -> Result<(), PsionPilotPretrainingRunError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "pilot_pretraining_run_bundle.schema_version",
        )?;
        if self.schema_version != PSION_PILOT_PRETRAINING_RUN_BUNDLE_SCHEMA_VERSION {
            return Err(PsionPilotPretrainingRunError::SchemaVersionMismatch {
                expected: String::from(PSION_PILOT_PRETRAINING_RUN_BUNDLE_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.bundle_id.as_str(),
            "pilot_pretraining_run_bundle.bundle_id",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "pilot_pretraining_run_bundle.summary",
        )?;
        check_string_match(
            self.acceptance_matrix_id.as_str(),
            matrix.matrix_id.as_str(),
            "acceptance_matrix_id",
        )?;
        check_string_match(
            self.acceptance_matrix_version.as_str(),
            matrix.matrix_version.as_str(),
            "acceptance_matrix_version",
        )?;
        self.observability_receipt
            .validate_against_stage(&self.pretrain_stage_receipt)
            .map_err(PsionPilotPretrainingRunError::ObservabilityContract)?;
        self.held_out_loss_receipt
            .validate_against_stage(&self.pretrain_stage_receipt)?;
        self.route_probe_receipt
            .validate_against_stage(&self.pretrain_stage_receipt)?;
        self.validate_promotion_decision(matrix)?;
        if self.bundle_digest != stable_pilot_pretraining_run_bundle_digest(self) {
            return Err(PsionPilotPretrainingRunError::ReceiptDigestMismatch {
                receipt_kind: String::from("pilot_pretraining_run_bundle"),
            });
        }
        Ok(())
    }

    fn validate_promotion_decision(
        &self,
        matrix: &PsionAcceptanceMatrix,
    ) -> Result<(), PsionPilotPretrainingRunError> {
        self.promotion_decision_receipt
            .validate_against_matrix(matrix)
            .map_err(PsionPilotPretrainingRunError::AcceptanceMatrix)?;
        let mut ledger = PsionPhasePromotionLedger::default();
        ledger
            .record_decision(matrix, self.promotion_decision_receipt.clone())
            .map_err(PsionPilotPretrainingRunError::AcceptanceMatrix)?;
        if self.promotion_decision_receipt.phase != PsionPhaseGate::Pilot {
            return Err(PsionPilotPretrainingRunError::PilotPhaseMismatch);
        }
        check_string_match(
            self.promotion_decision_receipt
                .candidate_artifact_id
                .as_str(),
            self.pretrain_stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint_label
                .as_str(),
            "promotion_decision.candidate_artifact_id",
        )?;
        check_string_match(
            self.promotion_decision_receipt
                .checkpoint_receipt
                .checkpoint_id
                .as_str(),
            self.pretrain_stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint_label
                .as_str(),
            "promotion_decision.checkpoint_receipt.checkpoint_id",
        )?;
        if self
            .promotion_decision_receipt
            .replay_receipt
            .successful_replays
            != self
                .pretrain_stage_receipt
                .replay_receipt
                .successful_replays
            || self
                .promotion_decision_receipt
                .replay_receipt
                .exact_replay_observed
                != self
                    .pretrain_stage_receipt
                    .replay_receipt
                    .exact_replay_observed
        {
            return Err(PsionPilotPretrainingRunError::ReplayDecisionMismatch);
        }

        let benchmark_map = self
            .promotion_decision_receipt
            .benchmark_receipts
            .iter()
            .map(|receipt| (receipt.family, receipt))
            .collect::<BTreeMap<_, _>>();
        let Some(held_out_receipt) =
            benchmark_map.get(&PsionBenchmarkFamily::HeldOutTechnicalReasoning)
        else {
            return Err(PsionPilotPretrainingRunError::MissingBenchmarkReceipt {
                family: String::from("held_out_technical_reasoning"),
            });
        };
        let Some(improvement_metric) = benchmark_metric(
            held_out_receipt,
            PsionMetricKind::ImprovementOverSeedBaselineBps,
        ) else {
            return Err(PsionPilotPretrainingRunError::MissingBenchmarkMetric {
                family: String::from("held_out_technical_reasoning"),
                metric: String::from("improvement_over_seed_baseline_bps"),
            });
        };
        if improvement_metric.observed_bps != self.held_out_loss_receipt.minimum_improvement_bps() {
            return Err(PsionPilotPretrainingRunError::HeldOutImprovementMismatch);
        }

        let Some(spec_receipt) = benchmark_map.get(&PsionBenchmarkFamily::NormativeSpecReading)
        else {
            return Err(PsionPilotPretrainingRunError::MissingBenchmarkReceipt {
                family: String::from("normative_spec_reading"),
            });
        };
        let Some(spec_pass_rate) = benchmark_metric(spec_receipt, PsionMetricKind::PassRateBps)
        else {
            return Err(PsionPilotPretrainingRunError::MissingBenchmarkMetric {
                family: String::from("normative_spec_reading"),
                metric: String::from("pass_rate_bps"),
            });
        };
        if spec_pass_rate.observed_bps < PSION_PILOT_MINIMUM_SPECIFICATION_BOUNDARY_PASS_RATE_BPS {
            return Err(PsionPilotPretrainingRunError::SpecificationBoundaryMissed {
                observed_bps: spec_pass_rate.observed_bps,
                minimum_bps: PSION_PILOT_MINIMUM_SPECIFICATION_BOUNDARY_PASS_RATE_BPS,
            });
        }
        if self
            .promotion_decision_receipt
            .route_calibration_receipt
            .route_selection_accuracy_bps
            != self
                .route_probe_receipt
                .aggregate_route_selection_accuracy_bps
        {
            return Err(PsionPilotPretrainingRunError::RouteCalibrationMismatch);
        }
        Ok(())
    }
}

/// Creates one held-out loss receipt tied to the pilot pretrain stage.
pub fn record_psion_pilot_held_out_loss(
    receipt_id: impl Into<String>,
    rows: Vec<PsionPilotHeldOutLossRow>,
    summary: impl Into<String>,
    stage_receipt: &PsionPretrainStageRunReceipt,
) -> Result<PsionPilotHeldOutLossReceipt, PsionPilotPretrainingRunError> {
    let mut receipt = PsionPilotHeldOutLossReceipt {
        schema_version: String::from(PSION_PILOT_HELD_OUT_LOSS_RECEIPT_SCHEMA_VERSION),
        receipt_id: receipt_id.into(),
        run_id: stage_receipt.run_id.clone(),
        stage_id: stage_receipt.stage_id.clone(),
        model_id: stage_receipt.model_id.clone(),
        pretrain_stage_receipt_digest: stage_receipt.receipt_digest.clone(),
        rows,
        summary: summary.into(),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_pilot_held_out_loss_digest(&receipt);
    receipt.validate_against_stage(stage_receipt)?;
    Ok(receipt)
}

/// Creates one route/refusal probe receipt tied to the pilot pretrain stage.
pub fn record_psion_pilot_route_probe(
    receipt_id: impl Into<String>,
    rows: Vec<PsionPilotRouteProbeRow>,
    summary: impl Into<String>,
    stage_receipt: &PsionPretrainStageRunReceipt,
) -> Result<PsionPilotRouteProbeReceipt, PsionPilotPretrainingRunError> {
    let aggregate_route_selection_accuracy_bps = rows
        .iter()
        .map(|row| row.observed_route_accuracy_bps)
        .sum::<u32>()
        / (rows.len() as u32).max(1);
    let mut receipt = PsionPilotRouteProbeReceipt {
        schema_version: String::from(PSION_PILOT_ROUTE_PROBE_RECEIPT_SCHEMA_VERSION),
        receipt_id: receipt_id.into(),
        run_id: stage_receipt.run_id.clone(),
        stage_id: stage_receipt.stage_id.clone(),
        rows,
        aggregate_route_selection_accuracy_bps,
        summary: summary.into(),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_pilot_route_probe_digest(&receipt);
    receipt.validate_against_stage(stage_receipt)?;
    Ok(receipt)
}

/// Creates the first pilot pretraining run bundle and validates it against the matrix.
pub fn record_psion_pilot_pretraining_run(
    bundle_id: impl Into<String>,
    held_out_loss_receipt: PsionPilotHeldOutLossReceipt,
    route_probe_receipt: PsionPilotRouteProbeReceipt,
    promotion_decision_receipt: PsionPromotionDecisionReceipt,
    summary: impl Into<String>,
    pretrain_stage_receipt: PsionPretrainStageRunReceipt,
    observability_receipt: PsionPretrainRunObservabilityReceipt,
    acceptance_matrix: &PsionAcceptanceMatrix,
) -> Result<PsionPilotPretrainingRunBundle, PsionPilotPretrainingRunError> {
    let mut bundle = PsionPilotPretrainingRunBundle {
        schema_version: String::from(PSION_PILOT_PRETRAINING_RUN_BUNDLE_SCHEMA_VERSION),
        bundle_id: bundle_id.into(),
        acceptance_matrix_id: acceptance_matrix.matrix_id.clone(),
        acceptance_matrix_version: acceptance_matrix.matrix_version.clone(),
        pretrain_stage_receipt,
        observability_receipt,
        held_out_loss_receipt,
        route_probe_receipt,
        promotion_decision_receipt,
        summary: summary.into(),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_pilot_pretraining_run_bundle_digest(&bundle);
    bundle.validate_against_matrix(acceptance_matrix)?;
    Ok(bundle)
}

/// Error returned by the Psion pilot pretraining bundle contract.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionPilotPretrainingRunError {
    /// One required field was missing or empty.
    #[error("Psion pilot pretraining field `{field}` is missing")]
    MissingField {
        /// Field path.
        field: String,
    },
    /// One schema version drifted from the expected contract.
    #[error("Psion pilot pretraining expected schema version `{expected}`, found `{actual}`")]
    SchemaVersionMismatch {
        /// Expected schema version.
        expected: String,
        /// Actual schema version.
        actual: String,
    },
    /// One string field drifted from the expected value.
    #[error("Psion pilot pretraining field `{field}` expected `{expected}`, found `{actual}`")]
    FieldMismatch {
        /// Field name.
        field: String,
        /// Expected value.
        expected: String,
        /// Actual value.
        actual: String,
    },
    /// The wrapped observability receipt drifted from the stage.
    #[error(transparent)]
    ObservabilityContract(#[from] crate::PsionPretrainRunObservabilityError),
    /// The promotion decision did not validate or record against the acceptance matrix.
    #[error(transparent)]
    AcceptanceMatrix(#[from] PsionAcceptanceMatrixError),
    /// One held-out family was duplicated.
    #[error("Psion pilot pretraining repeated held-out loss family `{family}`")]
    DuplicateHeldOutLossFamily {
        /// Family label.
        family: String,
    },
    /// One held-out family was missing.
    #[error("Psion pilot pretraining requires held-out loss family `{family}`")]
    MissingHeldOutLossFamily {
        /// Family label.
        family: String,
    },
    /// One held-out row failed the bounded pilot contract.
    #[error("Psion pilot pretraining held-out loss row for `{family}` is invalid")]
    InvalidHeldOutLossRow {
        /// Family label.
        family: String,
    },
    /// One route probe kind was duplicated.
    #[error("Psion pilot pretraining repeated route probe `{probe_kind}`")]
    DuplicateRouteProbeKind {
        /// Probe kind label.
        probe_kind: String,
    },
    /// One required route probe kind was missing.
    #[error("Psion pilot pretraining requires route probe `{probe_kind}`")]
    MissingRouteProbeKind {
        /// Probe kind label.
        probe_kind: String,
    },
    /// One probe row carried the wrong expected route.
    #[error("Psion pilot pretraining probe `{probe_kind}` expected route `{expected_route}`, found `{actual_route}`")]
    RouteProbeExpectedRouteMismatch {
        /// Probe kind label.
        probe_kind: String,
        /// Expected route label.
        expected_route: String,
        /// Actual route label.
        actual_route: String,
    },
    /// Aggregate route accuracy drifted from the row set.
    #[error("Psion pilot pretraining route-probe aggregate accuracy drifted from the row set")]
    InvalidRouteProbeAggregate,
    /// The bundle decision did not target the pilot phase.
    #[error("Psion pilot pretraining bundle must target the `pilot` phase")]
    PilotPhaseMismatch,
    /// Replay evidence in the decision drifted from the stage receipt.
    #[error("Psion pilot pretraining replay evidence drifted from the bound stage receipt")]
    ReplayDecisionMismatch,
    /// One benchmark receipt expected by the bundle was missing.
    #[error("Psion pilot pretraining bundle is missing benchmark receipt `{family}`")]
    MissingBenchmarkReceipt {
        /// Benchmark family label.
        family: String,
    },
    /// One metric expected inside a benchmark receipt was missing.
    #[error("Psion pilot pretraining benchmark receipt `{family}` is missing metric `{metric}`")]
    MissingBenchmarkMetric {
        /// Benchmark family label.
        family: String,
        /// Metric label.
        metric: String,
    },
    /// The held-out improvement metric drifted from the held-out loss receipt.
    #[error("Psion pilot pretraining held-out improvement metric must match the minimum improvement surfaced by the held-out loss receipt")]
    HeldOutImprovementMismatch,
    /// The spec-vs-implementation benchmark missed the frozen pilot minimum.
    #[error("Psion pilot pretraining specification-vs-implementation pass rate `{observed_bps}` is below the frozen minimum `{minimum_bps}`")]
    SpecificationBoundaryMissed {
        /// Observed pass rate.
        observed_bps: u32,
        /// Minimum allowed pass rate.
        minimum_bps: u32,
    },
    /// Route calibration drifted from the explicit probe receipt.
    #[error(
        "Psion pilot pretraining route calibration must match the aggregate route-probe accuracy"
    )]
    RouteCalibrationMismatch,
    /// One receipt or bundle digest drifted from the payload.
    #[error("Psion pilot pretraining digest drifted for `{receipt_kind}`")]
    ReceiptDigestMismatch {
        /// Receipt or bundle label.
        receipt_kind: String,
    },
}

fn stable_pilot_held_out_loss_digest(receipt: &PsionPilotHeldOutLossReceipt) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_pilot_held_out_loss|");
    hasher.update(receipt.receipt_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.stage_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.pretrain_stage_receipt_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.summary.as_bytes());
    for row in &receipt.rows {
        hasher.update(b"|row|");
        hasher.update(held_out_loss_family_label(row.family).as_bytes());
        hasher.update(b"|");
        hasher.update(row.seed_baseline_loss_milli.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(row.pilot_loss_milli.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(
            row.improvement_over_seed_baseline_bps
                .to_string()
                .as_bytes(),
        );
    }
    hex::encode(hasher.finalize())
}

fn stable_pilot_route_probe_digest(receipt: &PsionPilotRouteProbeReceipt) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_pilot_route_probe|");
    hasher.update(receipt.receipt_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.stage_id.as_bytes());
    hasher.update(b"|");
    hasher.update(
        receipt
            .aggregate_route_selection_accuracy_bps
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(receipt.summary.as_bytes());
    for row in &receipt.rows {
        hasher.update(b"|row|");
        hasher.update(route_probe_kind_label(row.probe_kind).as_bytes());
        hasher.update(b"|");
        hasher.update(route_kind_label(row.expected_route).as_bytes());
        hasher.update(b"|");
        hasher.update(row.observed_route_accuracy_bps.to_string().as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_pilot_pretraining_run_bundle_digest(bundle: &PsionPilotPretrainingRunBundle) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_pilot_pretraining_run_bundle|");
    hasher.update(bundle.bundle_id.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.acceptance_matrix_id.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.acceptance_matrix_version.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.pretrain_stage_receipt.receipt_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.observability_receipt.observability_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.held_out_loss_receipt.receipt_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.route_probe_receipt.receipt_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.promotion_decision_receipt.decision_id.as_bytes());
    hasher.update(b"|");
    hasher.update(
        bundle
            .promotion_decision_receipt
            .candidate_artifact_id
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(bundle.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn required_route_for_probe(probe_kind: PsionPilotRouteProbeKind) -> PsionRouteKind {
    match probe_kind {
        PsionPilotRouteProbeKind::SupportedArchitectureExplanation => {
            PsionRouteKind::DirectModelAnswer
        }
        PsionPilotRouteProbeKind::ExactExecutionRequest => PsionRouteKind::ExactExecutorHandoff,
        PsionPilotRouteProbeKind::UnderspecifiedDesignTask => PsionRouteKind::Refusal,
    }
}

fn held_out_loss_family_label(family: PsionPilotHeldOutLossFamily) -> &'static str {
    match family {
        PsionPilotHeldOutLossFamily::Textbooks => "textbooks",
        PsionPilotHeldOutLossFamily::NormativeSpecs => "normative_specs",
        PsionPilotHeldOutLossFamily::TechnicalDocs => "technical_docs",
    }
}

fn route_probe_kind_label(probe_kind: PsionPilotRouteProbeKind) -> &'static str {
    match probe_kind {
        PsionPilotRouteProbeKind::SupportedArchitectureExplanation => {
            "supported_architecture_explanation"
        }
        PsionPilotRouteProbeKind::ExactExecutionRequest => "exact_execution_request",
        PsionPilotRouteProbeKind::UnderspecifiedDesignTask => "underspecified_design_task",
    }
}

fn route_kind_label(route_kind: PsionRouteKind) -> &'static str {
    match route_kind {
        PsionRouteKind::DirectModelAnswer => "direct_model_answer",
        PsionRouteKind::ExactExecutorHandoff => "exact_executor_handoff",
        PsionRouteKind::Refusal => "refusal",
    }
}

fn benchmark_metric(
    receipt: &crate::PsionBenchmarkEvidenceReceipt,
    metric_kind: PsionMetricKind,
) -> Option<&crate::PsionObservedMetric> {
    receipt
        .metrics
        .iter()
        .find(|metric| metric.metric_kind == metric_kind)
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionPilotPretrainingRunError> {
    if value.trim().is_empty() {
        return Err(PsionPilotPretrainingRunError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn validate_bps(value: u32, field: &str) -> Result<(), PsionPilotPretrainingRunError> {
    if value > 10_000 {
        return Err(PsionPilotPretrainingRunError::FieldMismatch {
            field: String::from(field),
            expected: String::from("0..=10000"),
            actual: value.to_string(),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionPilotPretrainingRunError> {
    ensure_nonempty(actual, field)?;
    if actual != expected {
        return Err(PsionPilotPretrainingRunError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        record_psion_pilot_held_out_loss, record_psion_pilot_pretraining_run,
        record_psion_pilot_route_probe, PsionBenchmarkEvidenceReceipt,
        PsionCheckpointRecoveryReceipt, PsionContaminationReviewDisposition,
        PsionContaminationReviewReceipt, PsionObservedMetric, PsionPromotionDecisionDisposition,
        PsionRefusalCalibrationReceipt, PsionReplayEvidenceReceipt, PsionRouteCalibrationReceipt,
    };

    fn acceptance_matrix() -> PsionAcceptanceMatrix {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/acceptance/psion_acceptance_matrix_v1.json"
        ))
        .expect("acceptance matrix should parse")
    }

    fn pilot_stage_receipt() -> PsionPretrainStageRunReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_pretrain_stage_receipt_v1.json"
        ))
        .expect("pilot stage receipt should parse")
    }

    fn pilot_observability_receipt() -> PsionPretrainRunObservabilityReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/observability/psion_pilot_pretrain_run_observability_receipt_v1.json"
        ))
        .expect("pilot observability receipt should parse")
    }

    fn pilot_bundle() -> PsionPilotPretrainingRunBundle {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pilot/psion_pilot_pretraining_run_bundle_v1.json"
        ))
        .expect("pilot bundle should parse")
    }

    fn pilot_held_out_loss_receipt() -> PsionPilotHeldOutLossReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pilot/psion_pilot_held_out_loss_receipt_v1.json"
        ))
        .expect("pilot held-out loss receipt should parse")
    }

    fn pilot_route_probe_receipt() -> PsionPilotRouteProbeReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pilot/psion_pilot_route_probe_receipt_v1.json"
        ))
        .expect("pilot route probe receipt should parse")
    }

    fn pilot_refusal_calibration_receipt() -> PsionRefusalCalibrationReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/refusal/psion_refusal_calibration_receipt_v1.json"
        ))
        .expect("pilot refusal calibration receipt should parse")
    }

    #[test]
    fn pilot_bundle_validates_and_records_decision_against_acceptance_matrix(
    ) -> Result<(), Box<dyn std::error::Error>> {
        pilot_bundle().validate_against_matrix(&acceptance_matrix())?;
        Ok(())
    }

    #[test]
    fn held_out_loss_receipt_requires_all_three_families() {
        let mut receipt = pilot_held_out_loss_receipt();
        receipt.rows.pop();
        receipt.receipt_digest = stable_pilot_held_out_loss_digest(&receipt);
        let error = receipt
            .validate_against_stage(&pilot_stage_receipt())
            .expect_err("receipt should require all three held-out families");
        assert!(matches!(
            error,
            PsionPilotPretrainingRunError::MissingHeldOutLossFamily { .. }
        ));
    }

    #[test]
    fn route_probe_receipt_requires_direct_handoff_and_refusal_rows() {
        let mut receipt = pilot_route_probe_receipt();
        receipt.rows.pop();
        receipt.aggregate_route_selection_accuracy_bps = receipt
            .rows
            .iter()
            .map(|row| row.observed_route_accuracy_bps)
            .sum::<u32>()
            / (receipt.rows.len() as u32).max(1);
        receipt.receipt_digest = stable_pilot_route_probe_digest(&receipt);
        let error = receipt
            .validate_against_stage(&pilot_stage_receipt())
            .expect_err("receipt should require all route probe kinds");
        assert!(matches!(
            error,
            PsionPilotPretrainingRunError::MissingRouteProbeKind { .. }
        ));
    }

    #[test]
    fn bundle_rejects_specification_boundary_benchmark_below_minimum() {
        let mut bundle = pilot_bundle();
        let spec_receipt = bundle
            .promotion_decision_receipt
            .benchmark_receipts
            .iter_mut()
            .find(|receipt| receipt.family == PsionBenchmarkFamily::NormativeSpecReading)
            .expect("bundle should include spec boundary receipt");
        let pass_rate = spec_receipt
            .metrics
            .iter_mut()
            .find(|metric| metric.metric_kind == PsionMetricKind::PassRateBps)
            .expect("bundle should include spec pass rate");
        pass_rate.observed_bps = 8200;
        let error = bundle
            .validate_against_matrix(&acceptance_matrix())
            .expect_err("spec boundary should stay above the frozen pilot minimum");
        assert!(matches!(
            error,
            PsionPilotPretrainingRunError::SpecificationBoundaryMissed { .. }
        ));
    }

    #[test]
    fn pilot_bundle_builder_recomputes_fixture_digest() -> Result<(), Box<dyn std::error::Error>> {
        let stage_receipt = pilot_stage_receipt();
        let held_out_loss_receipt = record_psion_pilot_held_out_loss(
            "psion-pilot-held-out-loss-v1",
            vec![
                PsionPilotHeldOutLossRow {
                    family: PsionPilotHeldOutLossFamily::Textbooks,
                    seed_baseline_loss_milli: 1410,
                    pilot_loss_milli: 1120,
                    improvement_over_seed_baseline_bps: 2060,
                    detail: String::from(
                        "Pilot textbooks slice improved enough to justify keeping the prose-heavy lane in pilot scope.",
                    ),
                },
                PsionPilotHeldOutLossRow {
                    family: PsionPilotHeldOutLossFamily::NormativeSpecs,
                    seed_baseline_loss_milli: 1520,
                    pilot_loss_milli: 1260,
                    improvement_over_seed_baseline_bps: 1710,
                    detail: String::from(
                        "Pilot specification slice improved while preserving the spec-heavy held-out boundary.",
                    ),
                },
                PsionPilotHeldOutLossRow {
                    family: PsionPilotHeldOutLossFamily::TechnicalDocs,
                    seed_baseline_loss_milli: 1480,
                    pilot_loss_milli: 1185,
                    improvement_over_seed_baseline_bps: 1990,
                    detail: String::from(
                        "Pilot technical-doc slice improved enough to keep the docs lane in the acceptance story.",
                    ),
                },
            ],
            "Held-out loss improved across textbooks, specs, and technical docs versus the seed baseline.",
            &stage_receipt,
        )?;
        let route_probe_receipt = record_psion_pilot_route_probe(
            "psion-pilot-route-probe-v1",
            vec![
                PsionPilotRouteProbeRow {
                    probe_kind: PsionPilotRouteProbeKind::SupportedArchitectureExplanation,
                    expected_route: PsionRouteKind::DirectModelAnswer,
                    observed_route_accuracy_bps: 9750,
                    detail: String::from(
                        "Supported architecture explanations stayed on the direct answer route at pilot time.",
                    ),
                },
                PsionPilotRouteProbeRow {
                    probe_kind: PsionPilotRouteProbeKind::ExactExecutionRequest,
                    expected_route: PsionRouteKind::ExactExecutorHandoff,
                    observed_route_accuracy_bps: 9860,
                    detail: String::from(
                        "Exact execution asks were handed off to the exact executor rather than improvised by the learned lane.",
                    ),
                },
                PsionPilotRouteProbeRow {
                    probe_kind: PsionPilotRouteProbeKind::UnderspecifiedDesignTask,
                    expected_route: PsionRouteKind::Refusal,
                    observed_route_accuracy_bps: 9920,
                    detail: String::from(
                        "Underspecified design tasks triggered refusal rather than false precision.",
                    ),
                },
            ],
            "Pilot route probes covered direct answer, exact-executor handoff, and refusal on the bounded route surface.",
            &stage_receipt,
        )?;
        let matrix = acceptance_matrix();
        let refusal_calibration_receipt = pilot_refusal_calibration_receipt();
        let promotion_decision = PsionPromotionDecisionReceipt {
            schema_version: String::from(crate::PSION_PROMOTION_DECISION_SCHEMA_VERSION),
            decision_id: String::from("psion-pilot-promotion-decision-v1"),
            matrix_id: matrix.matrix_id.clone(),
            matrix_version: matrix.matrix_version.clone(),
            phase: PsionPhaseGate::Pilot,
            candidate_artifact_id: stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint_label
                .clone(),
            benchmark_receipts: vec![
                PsionBenchmarkEvidenceReceipt {
                    receipt_id: String::from("psion-pilot-architecture-reasoning-receipt-v1"),
                    phase: PsionPhaseGate::Pilot,
                    family: PsionBenchmarkFamily::ArchitectureReasoning,
                    benchmark_artifact_id: String::from(
                        "psion_architecture_reasoning_benchmark_v1",
                    ),
                    benchmark_artifact_digest: String::from(
                        "703142f680a8a2702700fcdd18309b24d6e7889e07c4621e287178c9ac6af674",
                    ),
                    metrics: vec![PsionObservedMetric {
                        metric_kind: PsionMetricKind::PassRateBps,
                        observed_bps: 8450,
                        regression_from_baseline_bps: 0,
                    }],
                    summary: String::from(
                        "Pilot candidate cleared the bounded architecture reasoning floor.",
                    ),
                },
                PsionBenchmarkEvidenceReceipt {
                    receipt_id: String::from("psion-pilot-held-out-reasoning-receipt-v1"),
                    phase: PsionPhaseGate::Pilot,
                    family: PsionBenchmarkFamily::HeldOutTechnicalReasoning,
                    benchmark_artifact_id: String::from(
                        "psion_held_out_reasoning_benchmark_v1",
                    ),
                    benchmark_artifact_digest: String::from(
                        "sha256:psion_held_out_reasoning_benchmark_v1",
                    ),
                    metrics: vec![
                        PsionObservedMetric {
                            metric_kind: PsionMetricKind::PassRateBps,
                            observed_bps: 7900,
                            regression_from_baseline_bps: 0,
                        },
                        PsionObservedMetric {
                            metric_kind: PsionMetricKind::ImprovementOverSeedBaselineBps,
                            observed_bps: held_out_loss_receipt.minimum_improvement_bps(),
                            regression_from_baseline_bps: 0,
                        },
                    ],
                    summary: String::from(
                        "Pilot candidate improved over the seed baseline on the held-out technical reasoning set.",
                    ),
                },
                PsionBenchmarkEvidenceReceipt {
                    receipt_id: String::from(
                        "psion-pilot-specification-boundary-receipt-v1",
                    ),
                    phase: PsionPhaseGate::Pilot,
                    family: PsionBenchmarkFamily::NormativeSpecReading,
                    benchmark_artifact_id: String::from("psion_normative_spec_benchmark_v1"),
                    benchmark_artifact_digest: String::from(
                        "dd5741c92863fe4525d67eadd513b955b01f97adb9aa0f1cfa22619e8b273b32",
                    ),
                    metrics: vec![PsionObservedMetric {
                        metric_kind: PsionMetricKind::PassRateBps,
                        observed_bps: 8820,
                        regression_from_baseline_bps: 0,
                    }],
                    summary: String::from(
                        "Pilot candidate kept normative spec reading separate from implementation commentary.",
                    ),
                },
                PsionBenchmarkEvidenceReceipt {
                    receipt_id: String::from(
                        "psion-pilot-unsupported-request-refusal-receipt-v1",
                    ),
                    phase: PsionPhaseGate::Pilot,
                    family: PsionBenchmarkFamily::UnsupportedRequestRefusal,
                    benchmark_artifact_id: refusal_calibration_receipt.package_id.clone(),
                    benchmark_artifact_digest: refusal_calibration_receipt.package_digest.clone(),
                    metrics: vec![
                        PsionObservedMetric {
                            metric_kind: PsionMetricKind::UnsupportedRequestRefusalBps,
                            observed_bps: refusal_calibration_receipt
                                .aggregate_unsupported_request_refusal_bps,
                            regression_from_baseline_bps: 0,
                        },
                        PsionObservedMetric {
                            metric_kind: PsionMetricKind::OverrefusalBps,
                            observed_bps: refusal_calibration_receipt
                                .supported_control_overrefusal_bps,
                            regression_from_baseline_bps: 0,
                        },
                    ],
                    summary: String::from(
                        "Pilot candidate refused unsupported requests while keeping over-refusal within band.",
                    ),
                },
            ],
            replay_receipt: PsionReplayEvidenceReceipt {
                receipt_id: String::from("psion-pilot-replay-evidence-v1"),
                successful_replays: stage_receipt.replay_receipt.successful_replays,
                exact_replay_observed: stage_receipt.replay_receipt.exact_replay_observed,
                summary: String::from(
                    "Pilot replay evidence matched the bound pretrain-stage replay receipt exactly.",
                ),
            },
            checkpoint_receipt: PsionCheckpointRecoveryReceipt {
                receipt_id: String::from("psion-pilot-checkpoint-recovery-v1"),
                checkpoint_id: stage_receipt
                    .checkpoint_lineage
                    .promoted_checkpoint_label
                    .clone(),
                successful_restart_roundtrips: 1,
                restart_recovery_observed: true,
                resume_regression_bps: 220,
                summary: String::from(
                    "Pilot checkpoint resumed once within the declared regression budget.",
                ),
            },
            contamination_review_receipt: PsionContaminationReviewReceipt {
                receipt_id: String::from("psion-pilot-contamination-review-v1"),
                benchmark_isolation_schema_version: String::from(
                    "psion.benchmark_isolation.v1",
                ),
                exclusion_manifest_digest: String::from("sha256:psion_exclusion_manifest_v1"),
                near_duplicate_review_completed: true,
                tokenizer_exposure_review_completed: true,
                disposition: PsionContaminationReviewDisposition::Clean,
                applied_consequences: Vec::new(),
                summary: String::from(
                    "Pilot held-out review stayed clean against the frozen exclusion manifest.",
                ),
            },
            route_calibration_receipt: PsionRouteCalibrationReceipt {
                receipt_id: String::from("psion-pilot-route-calibration-v1"),
                covered_routes: vec![
                    PsionRouteKind::DirectModelAnswer,
                    PsionRouteKind::ExactExecutorHandoff,
                    PsionRouteKind::Refusal,
                ],
                route_selection_accuracy_bps: route_probe_receipt
                    .aggregate_route_selection_accuracy_bps,
                route_regression_bps: 120,
                summary: String::from(
                    "Pilot route calibration covered direct answer, exact-executor handoff, and refusal.",
                ),
            },
            refusal_calibration_receipt,
            decision: PsionPromotionDecisionDisposition::Promoted,
            hold_reason_codes: Vec::new(),
            decision_summary: String::from(
                "Pilot candidate cleared the pilot gate on architecture reasoning, held-out improvement, checkpoint recovery, replay, route calibration, refusal calibration, and contamination review.",
            ),
        };

        let bundle = record_psion_pilot_pretraining_run(
            "psion-pilot-pretraining-run-v1",
            held_out_loss_receipt,
            route_probe_receipt,
            promotion_decision,
            "Pilot pretraining bundle binds the first bounded pretrain run, held-out loss deltas, route probes, and acceptance-matrix promotion decision into one reviewable artifact.",
            stage_receipt,
            pilot_observability_receipt(),
            &matrix,
        )?;
        assert_eq!(bundle.bundle_digest, pilot_bundle().bundle_digest);
        Ok(())
    }
}
