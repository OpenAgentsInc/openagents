use serde::{Deserialize, Serialize};
use thiserror::Error;

use psionic_eval::{
    build_tassadar_full_core_wasm_public_acceptance_gate_report,
    TassadarFullCoreWasmPublicAcceptanceStatus,
    TASSADAR_FULL_CORE_WASM_PUBLIC_ACCEPTANCE_GATE_REPORT_REF,
};
use psionic_research::TASSADAR_FULL_CORE_WASM_OPERATOR_RUNBOOK_V2_SUMMARY_REF;

pub const FULL_CORE_WASM_PUBLICATION_PRODUCT_ID: &str = "psionic.full_core_wasm_publication";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarFullCoreWasmPublicationDecisionStatus {
    Published,
    Suppressed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFullCoreWasmPublicationDecision {
    pub product_id: String,
    pub acceptance_gate_report_ref: String,
    pub operator_runbook_summary_ref: String,
    pub status: TassadarFullCoreWasmPublicationDecisionStatus,
    pub blocked_requirement_ids: Vec<String>,
    pub operator_drill_commands: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarFullCoreWasmPublicationDecisionError {
    #[error("full core-Wasm public acceptance gate is suppressed: {detail}")]
    Suppressed { detail: String },
    #[error("full core-Wasm public acceptance gate failed: {detail}")]
    Failed { detail: String },
}

pub fn tassadar_full_core_wasm_publication_decision() -> TassadarFullCoreWasmPublicationDecision {
    let report = build_tassadar_full_core_wasm_public_acceptance_gate_report()
        .expect("full core-Wasm public acceptance gate should build");
    let status = match report.acceptance_status {
        TassadarFullCoreWasmPublicAcceptanceStatus::Green if report.served_publication_allowed => {
            TassadarFullCoreWasmPublicationDecisionStatus::Published
        }
        TassadarFullCoreWasmPublicAcceptanceStatus::Green
        | TassadarFullCoreWasmPublicAcceptanceStatus::Suppressed => {
            TassadarFullCoreWasmPublicationDecisionStatus::Suppressed
        }
        TassadarFullCoreWasmPublicAcceptanceStatus::Failed => {
            TassadarFullCoreWasmPublicationDecisionStatus::Failed
        }
    };
    TassadarFullCoreWasmPublicationDecision {
        product_id: String::from(FULL_CORE_WASM_PUBLICATION_PRODUCT_ID),
        acceptance_gate_report_ref: String::from(
            TASSADAR_FULL_CORE_WASM_PUBLIC_ACCEPTANCE_GATE_REPORT_REF,
        ),
        operator_runbook_summary_ref: String::from(
            TASSADAR_FULL_CORE_WASM_OPERATOR_RUNBOOK_V2_SUMMARY_REF,
        ),
        status,
        blocked_requirement_ids: report.suppressed_requirement_ids,
        operator_drill_commands: report.operator_drill_commands,
        detail: report.summary,
    }
}

pub fn require_tassadar_full_core_wasm_publication(
) -> Result<TassadarFullCoreWasmPublicationDecision, TassadarFullCoreWasmPublicationDecisionError> {
    let decision = tassadar_full_core_wasm_publication_decision();
    match decision.status {
        TassadarFullCoreWasmPublicationDecisionStatus::Published => Ok(decision),
        TassadarFullCoreWasmPublicationDecisionStatus::Suppressed => {
            Err(TassadarFullCoreWasmPublicationDecisionError::Suppressed {
                detail: decision.detail.clone(),
            })
        }
        TassadarFullCoreWasmPublicationDecisionStatus::Failed => {
            Err(TassadarFullCoreWasmPublicationDecisionError::Failed {
                detail: decision.detail.clone(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        require_tassadar_full_core_wasm_publication, tassadar_full_core_wasm_publication_decision,
        TassadarFullCoreWasmPublicationDecisionError,
        TassadarFullCoreWasmPublicationDecisionStatus,
    };

    #[test]
    fn full_core_wasm_publication_decision_stays_suppressed_until_gate_is_green() {
        let decision = tassadar_full_core_wasm_publication_decision();

        assert_eq!(
            decision.status,
            TassadarFullCoreWasmPublicationDecisionStatus::Suppressed
        );
        assert!(decision
            .blocked_requirement_ids
            .contains(&String::from("target_feature_family_coverage")));
        assert!(decision
            .blocked_requirement_ids
            .contains(&String::from("cross_machine_harness_replay")));
    }

    #[test]
    fn full_core_wasm_publication_requirement_refuses_suppressed_gate() {
        let error =
            require_tassadar_full_core_wasm_publication().expect_err("gate stays suppressed");
        assert!(matches!(
            error,
            TassadarFullCoreWasmPublicationDecisionError::Suppressed { .. }
        ));
    }
}
