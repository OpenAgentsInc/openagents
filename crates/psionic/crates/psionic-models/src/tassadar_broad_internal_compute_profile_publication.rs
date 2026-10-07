use serde::{Deserialize, Serialize};

pub const TASSADAR_BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_broad_internal_compute_profile_publication_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBroadInternalComputeProfilePublicationStatus {
    Published,
    Suppressed,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBroadInternalComputeWorldMountBindingStatus {
    CompatibleWithCurrentExactComputeMount,
    ProfileSpecificMountTemplateAvailable,
    RequiresProfileSpecificMountPolicy,
    RefusedPendingProfileEvidence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBroadInternalComputeAcceptedOutcomeBindingStatus {
    ExactComputeEnvelopeAvailable,
    ProfileSpecificAcceptedOutcomeTemplateAvailable,
    RequiresProfileSpecificAcceptedOutcomeTemplate,
    RefusedPendingProfileEvidence,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadInternalComputeProfilePublicationRow {
    pub profile_id: String,
    pub publication_status: TassadarBroadInternalComputeProfilePublicationStatus,
    pub exactness_posture: super::TassadarInternalComputeExactnessPosture,
    pub import_posture: super::TassadarInternalComputeImportPosture,
    pub portability_posture: super::TassadarInternalComputePortabilityPosture,
    pub publication_allowed_row_count: u32,
    pub missing_required_evidence_refs: Vec<String>,
    pub world_mount_binding_status: TassadarBroadInternalComputeWorldMountBindingStatus,
    pub accepted_outcome_binding_status: TassadarBroadInternalComputeAcceptedOutcomeBindingStatus,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadInternalComputeProfilePublicationReport {
    pub schema_version: u16,
    pub report_id: String,
    pub portability_report_ref: String,
    pub acceptance_gate_report_ref: String,
    pub world_mount_compatibility_report_ref: String,
    pub accepted_outcome_binding_report_ref: String,
    pub exact_compute_market_report_ref: String,
    pub current_served_profile_id: String,
    pub profile_rows: Vec<TassadarBroadInternalComputeProfilePublicationRow>,
    pub published_profile_ids: Vec<String>,
    pub suppressed_profile_ids: Vec<String>,
    pub failed_profile_ids: Vec<String>,
    pub world_mount_dependency_marker: String,
    pub nexus_dependency_marker: String,
    pub kernel_policy_dependency_marker: String,
    pub compute_market_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}
