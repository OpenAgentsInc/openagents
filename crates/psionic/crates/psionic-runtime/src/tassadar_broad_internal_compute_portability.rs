use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_BROAD_INTERNAL_COMPUTE_PORTABILITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_broad_internal_compute_portability_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBroadInternalComputePortabilityRowStatus {
    PublishedMeasuredCurrentHost,
    PublishedDeclaredClass,
    SuppressedPendingPortabilityEvidence,
    SuppressedBackendEnvelopeConstrained,
    SuppressedDriftedOutsideEnvelope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBroadInternalComputeSuppressionReason {
    PortabilityEvidenceIncomplete,
    ProfileNotImplemented,
    BackendEnvelopeConstrained,
    OutsideDeclaredEnvelope,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadInternalComputePortabilityRow {
    pub profile_id: String,
    pub backend_family: String,
    pub toolchain_family: String,
    pub machine_class_id: String,
    pub row_status: TassadarBroadInternalComputePortabilityRowStatus,
    pub publication_allowed: bool,
    pub evidence_complete: bool,
    pub refusal_suite_complete: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppression_reason: Option<TassadarBroadInternalComputeSuppressionReason>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadInternalComputePortabilityReport {
    pub schema_version: u16,
    pub report_id: String,
    pub current_host_machine_class_id: String,
    pub generated_from_refs: Vec<String>,
    pub backend_family_ids: Vec<String>,
    pub toolchain_family_ids: Vec<String>,
    pub rows: Vec<TassadarBroadInternalComputePortabilityRow>,
    pub publication_allowed_profile_ids: Vec<String>,
    pub suppressed_profile_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarBroadInternalComputePortabilityReport {
    #[must_use]
    pub fn new(
        current_host_machine_class_id: impl Into<String>,
        mut generated_from_refs: Vec<String>,
        rows: Vec<TassadarBroadInternalComputePortabilityRow>,
    ) -> Self {
        generated_from_refs.sort();
        generated_from_refs.dedup();
        let backend_family_ids = rows
            .iter()
            .map(|row| row.backend_family.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let toolchain_family_ids = rows
            .iter()
            .map(|row| row.toolchain_family.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let publication_allowed_profile_ids = rows
            .iter()
            .filter(|row| row.publication_allowed)
            .map(|row| row.profile_id.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let suppressed_profile_ids = rows
            .iter()
            .filter(|row| !row.publication_allowed)
            .map(|row| row.profile_id.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let published_row_count = rows.iter().filter(|row| row.publication_allowed).count();
        let suppressed_row_count = rows.len().saturating_sub(published_row_count);
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.broad_internal_compute_portability.report.v1"),
            current_host_machine_class_id: current_host_machine_class_id.into(),
            generated_from_refs,
            backend_family_ids,
            toolchain_family_ids,
            rows,
            publication_allowed_profile_ids,
            suppressed_profile_ids,
            claim_boundary: String::from(
                "this report freezes profile-bound portability envelopes for broader internal compute above the Rust-only article baseline. It makes current-host publication, declared-class publication, suppressed portability gaps, and drift outside the declared machine envelope explicit instead of letting one lab machine stand in for broader deployment truth",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Broad internal-compute portability now records backends={}, toolchains={}, published_rows={}, suppressed_rows={}, published_profiles={}, suppressed_profiles={}, current_host=`{}`.",
            report.backend_family_ids.len(),
            report.toolchain_family_ids.len(),
            published_row_count,
            suppressed_row_count,
            report.publication_allowed_profile_ids.len(),
            report.suppressed_profile_ids.len(),
            report.current_host_machine_class_id,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_broad_internal_compute_portability_report|",
            &report,
        );
        report
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarBroadInternalComputePortabilityReport, TassadarBroadInternalComputePortabilityRow,
        TassadarBroadInternalComputePortabilityRowStatus,
        TassadarBroadInternalComputeSuppressionReason,
    };

    #[test]
    fn broad_internal_compute_portability_report_deduplicates_profile_sets() {
        let report = TassadarBroadInternalComputePortabilityReport::new(
            "host_cpu_x86_64",
            vec![String::from("fixtures/example.json")],
            vec![
                TassadarBroadInternalComputePortabilityRow {
                    profile_id: String::from("profile.green"),
                    backend_family: String::from("cpu_reference"),
                    toolchain_family: String::from("rustc-stable"),
                    machine_class_id: String::from("host_cpu_x86_64"),
                    row_status:
                        TassadarBroadInternalComputePortabilityRowStatus::PublishedMeasuredCurrentHost,
                    publication_allowed: true,
                    evidence_complete: true,
                    refusal_suite_complete: true,
                    suppression_reason: None,
                    note: String::from("green row"),
                },
                TassadarBroadInternalComputePortabilityRow {
                    profile_id: String::from("profile.red"),
                    backend_family: String::from("cpu_reference"),
                    toolchain_family: String::from("rustc-stable"),
                    machine_class_id: String::from("other_host_cpu"),
                    row_status:
                        TassadarBroadInternalComputePortabilityRowStatus::SuppressedDriftedOutsideEnvelope,
                    publication_allowed: false,
                    evidence_complete: false,
                    refusal_suite_complete: true,
                    suppression_reason: Some(
                        TassadarBroadInternalComputeSuppressionReason::OutsideDeclaredEnvelope,
                    ),
                    note: String::from("red row"),
                },
            ],
        );

        assert_eq!(
            report.publication_allowed_profile_ids,
            vec![String::from("profile.green")]
        );
        assert_eq!(
            report.suppressed_profile_ids,
            vec![String::from("profile.red")]
        );
        assert_eq!(
            report.backend_family_ids,
            vec![String::from("cpu_reference")]
        );
        assert_eq!(
            report.toolchain_family_ids,
            vec![String::from("rustc-stable")]
        );
    }
}
