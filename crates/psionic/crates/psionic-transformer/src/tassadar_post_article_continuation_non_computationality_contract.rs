use serde::{Deserialize, Serialize};

use crate::{
    TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CARRIER_CLASS_ID,
    TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID,
};

pub const TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_ID: &str =
    "tassadar.post_article.continuation_non_computationality.contract.v1";
pub const TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_BOUNDARY_ID: &str =
    "tassadar.post_article.continuation_non_computationality.boundary.v1";
pub const TASSADAR_POST_ARTICLE_CONTINUATION_CHECKPOINT_SURFACE_ID: &str =
    "tassadar.post_article.continuation.checkpoint_surface.v1";
pub const TASSADAR_POST_ARTICLE_CONTINUATION_SESSION_PROCESS_SURFACE_ID: &str =
    "tassadar.post_article.continuation.session_process_surface.v1";
pub const TASSADAR_POST_ARTICLE_CONTINUATION_SPILL_TAPE_SURFACE_ID: &str =
    "tassadar.post_article.continuation.spill_tape_surface.v1";
pub const TASSADAR_POST_ARTICLE_CONTINUATION_PROCESS_OBJECT_SURFACE_ID: &str =
    "tassadar.post_article.continuation.process_object_surface.v1";
pub const TASSADAR_POST_ARTICLE_CONTINUATION_INSTALLED_PROCESS_SURFACE_ID: &str =
    "tassadar.post_article.continuation.installed_process_surface.v1";
pub const TASSADAR_POST_ARTICLE_CONTINUATION_WEIGHTED_CONTROLLER_SURFACE_ID: &str =
    "tassadar.post_article.continuation.weighted_controller_surface.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleContinuationNonComputationalityLawRow {
    pub rule_id: String,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleContinuationNonComputationalityContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub machine_identity_id: String,
    pub tuple_id: String,
    pub carrier_class_id: String,
    pub boundary_id: String,
    pub admitted_continuation_surface_ids: Vec<String>,
    pub admitted_state_class_ids: Vec<String>,
    pub blocked_hidden_compute_ids: Vec<String>,
    pub contract_rule_rows: Vec<TassadarPostArticleContinuationNonComputationalityLawRow>,
    pub invalidation_rule_rows: Vec<TassadarPostArticleContinuationNonComputationalityLawRow>,
    pub next_stability_issue_id: String,
    pub claim_boundary: String,
    pub summary: String,
}

#[must_use]
pub fn build_tassadar_post_article_continuation_non_computationality_contract(
) -> TassadarPostArticleContinuationNonComputationalityContract {
    let admitted_continuation_surface_ids = vec![
        String::from(TASSADAR_POST_ARTICLE_CONTINUATION_CHECKPOINT_SURFACE_ID),
        String::from(TASSADAR_POST_ARTICLE_CONTINUATION_SESSION_PROCESS_SURFACE_ID),
        String::from(TASSADAR_POST_ARTICLE_CONTINUATION_SPILL_TAPE_SURFACE_ID),
        String::from(TASSADAR_POST_ARTICLE_CONTINUATION_PROCESS_OBJECT_SURFACE_ID),
        String::from(TASSADAR_POST_ARTICLE_CONTINUATION_INSTALLED_PROCESS_SURFACE_ID),
        String::from(TASSADAR_POST_ARTICLE_CONTINUATION_WEIGHTED_CONTROLLER_SURFACE_ID),
    ];
    let admitted_state_class_ids = vec![
        String::from("weights_owned_state"),
        String::from("ephemeral_execution_state"),
        String::from("resumed_continuation_state"),
        String::from("durable_receipt_backed_state"),
    ];
    let blocked_hidden_compute_ids = vec![
        String::from("checkpoint_embeds_hidden_workflow_logic"),
        String::from("spill_or_tape_segments_choose_next_action"),
        String::from("process_objects_smuggle_planner_state"),
        String::from("installed_process_lifecycle_becomes_second_machine"),
        String::from("session_resume_widens_into_open_ended_external_control"),
        String::from("plugin_controller_uses_resume_as_hidden_compute"),
        String::from("continuation_recomposition_creates_second_machine"),
    ];
    let contract_rule_rows = vec![
        law_row(
            "continuation_artifacts_transport_declared_state_only",
            "checkpoint, spill, tape, process-object, and installed-process surfaces may transport declared continuation state, but they may not encode hidden planner logic or a second workflow machine.",
        ),
        law_row(
            "checkpoint_and_spill_resume_must_be_exact_or_refusal_bounded",
            "checkpoint and spill/tape continuation only inherit the canonical machine while exact parity or typed refusal posture stays explicit.",
        ),
        law_row(
            "session_process_resume_must_stay_finite_and_deterministic",
            "session-process continuation may extend deterministic finite transcripts only; open-ended external-event control remains outside the contract.",
        ),
        law_row(
            "process_snapshot_and_lifecycle_receipts_are_transport_not_compute",
            "durable process snapshots, tapes, work queues, migrations, and rollbacks preserve declared execution state and lineage, but they do not become the locus of fresh computation.",
        ),
        law_row(
            "plugin_controller_may_consume_continuation_only_while_host_stays_non_planner",
            "plugin and controller layers may consume the declared continuation carrier only while selection, retry, stop, and refusal handling remain model-owned and the host stays execution-only.",
        ),
    ];
    let invalidation_rule_rows = vec![
        law_row(
            "checkpoint_workflow_logic_invalidates_contract",
            "the contract fails if checkpoint objects start carrying hidden workflow instructions, planner branches, or undeclared policy state.",
        ),
        law_row(
            "spill_or_tape_directive_logic_invalidates_contract",
            "the contract fails if spill or tape artifacts start choosing control flow instead of transporting bounded state.",
        ),
        law_row(
            "process_object_policy_smuggling_invalidates_contract",
            "the contract fails if process snapshots, tapes, or work queues start carrying undeclared planner or heuristic authority.",
        ),
        law_row(
            "installed_process_recomposition_invalidates_contract",
            "the contract fails if installed-process migration or rollback changes the canonical machine identity, continuation contract, or route binding.",
        ),
        law_row(
            "session_surface_widening_invalidates_contract",
            "the contract fails if finite deterministic session continuation widens into open-ended external-event control.",
        ),
        law_row(
            "resume_hidden_compute_invalidates_contract",
            "the contract fails if retry, resume, or stop semantics relocate real computation into host-managed continuation.",
        ),
        law_row(
            "second_machine_overclaim_invalidates_contract",
            "the contract fails if continuation is presented as a second proof-bearing machine rather than an extension of the same canonical machine.",
        ),
    ];

    TassadarPostArticleContinuationNonComputationalityContract {
        schema_version: 1,
        contract_id: String::from(
            TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_ID,
        ),
        machine_identity_id: String::from(
            crate::TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_ID,
        ),
        tuple_id: String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID),
        carrier_class_id: String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CARRIER_CLASS_ID),
        boundary_id: String::from(
            TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_BOUNDARY_ID,
        ),
        admitted_continuation_surface_ids,
        admitted_state_class_ids,
        blocked_hidden_compute_ids,
        contract_rule_rows,
        invalidation_rule_rows,
        next_stability_issue_id: String::from("TAS-215"),
        claim_boundary: String::from(
            "this transformer-owned contract freezes the continuation non-computationality boundary only. It says declared continuation surfaces extend one canonical post-article machine without becoming a second machine, but it still leaves fast-route legitimacy, anti-drift closure, served/public universality, and the final closure bundle to later issues.",
        ),
        summary: String::from(
            "Transformer continuation non-computationality contract freezes 5 contract rules and 7 invalidation rules across checkpoint, session, spill, process, installed-process, and weighted-controller continuation surfaces.",
        ),
    }
}

fn law_row(
    rule_id: &str,
    detail: &str,
) -> TassadarPostArticleContinuationNonComputationalityLawRow {
    TassadarPostArticleContinuationNonComputationalityLawRow {
        rule_id: String::from(rule_id),
        green: true,
        detail: String::from(detail),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_continuation_non_computationality_contract,
        TASSADAR_POST_ARTICLE_CONTINUATION_CHECKPOINT_SURFACE_ID,
        TASSADAR_POST_ARTICLE_CONTINUATION_INSTALLED_PROCESS_SURFACE_ID,
        TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_BOUNDARY_ID,
        TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_ID,
        TASSADAR_POST_ARTICLE_CONTINUATION_PROCESS_OBJECT_SURFACE_ID,
        TASSADAR_POST_ARTICLE_CONTINUATION_SESSION_PROCESS_SURFACE_ID,
        TASSADAR_POST_ARTICLE_CONTINUATION_SPILL_TAPE_SURFACE_ID,
        TASSADAR_POST_ARTICLE_CONTINUATION_WEIGHTED_CONTROLLER_SURFACE_ID,
    };
    use crate::{
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CARRIER_CLASS_ID,
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_ID,
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID,
    };

    #[test]
    fn continuation_non_computationality_contract_covers_declared_surfaces() {
        let contract = build_tassadar_post_article_continuation_non_computationality_contract();

        assert_eq!(
            contract.contract_id,
            TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_ID
        );
        assert_eq!(
            contract.machine_identity_id,
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_ID
        );
        assert_eq!(
            contract.tuple_id,
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID
        );
        assert_eq!(
            contract.carrier_class_id,
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CARRIER_CLASS_ID
        );
        assert_eq!(
            contract.boundary_id,
            TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_BOUNDARY_ID
        );
        assert_eq!(
            contract.admitted_continuation_surface_ids,
            vec![
                String::from(TASSADAR_POST_ARTICLE_CONTINUATION_CHECKPOINT_SURFACE_ID),
                String::from(TASSADAR_POST_ARTICLE_CONTINUATION_SESSION_PROCESS_SURFACE_ID),
                String::from(TASSADAR_POST_ARTICLE_CONTINUATION_SPILL_TAPE_SURFACE_ID),
                String::from(TASSADAR_POST_ARTICLE_CONTINUATION_PROCESS_OBJECT_SURFACE_ID),
                String::from(TASSADAR_POST_ARTICLE_CONTINUATION_INSTALLED_PROCESS_SURFACE_ID),
                String::from(TASSADAR_POST_ARTICLE_CONTINUATION_WEIGHTED_CONTROLLER_SURFACE_ID),
            ]
        );
        assert_eq!(contract.admitted_state_class_ids.len(), 4);
        assert_eq!(contract.blocked_hidden_compute_ids.len(), 7);
        assert_eq!(contract.contract_rule_rows.len(), 5);
        assert_eq!(contract.invalidation_rule_rows.len(), 7);
        assert_eq!(contract.next_stability_issue_id, "TAS-215");
        assert!(contract
            .contract_rule_rows
            .iter()
            .chain(contract.invalidation_rule_rows.iter())
            .all(|row| row.green));
    }
}
