use psionic_ir::TassadarMixedNumericSupportPosture;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_NUMERIC_PORTABILITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_numeric_portability_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarNumericPortabilityRowStatus {
    PublishedMeasuredCurrentHost,
    PublishedDeclaredClass,
    SuppressedBackendEnvelopeConstrained,
    SuppressedApproximateProfile,
    SuppressedOutsideDeclaredEnvelope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarNumericPortabilitySuppressionReason {
    BackendEnvelopeConstrained,
    ApproximateProfileNotPublished,
    OutsideDeclaredEnvelope,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNumericPortabilityRow {
    pub profile_id: String,
    pub support_posture: TassadarMixedNumericSupportPosture,
    pub backend_family: String,
    pub toolchain_family: String,
    pub machine_class_id: String,
    pub max_allowed_ulp_drift: u32,
    pub observed_max_ulp_drift: u32,
    pub row_status: TassadarNumericPortabilityRowStatus,
    pub publication_allowed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppression_reason: Option<TassadarNumericPortabilitySuppressionReason>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNumericPortabilityReport {
    pub schema_version: u16,
    pub report_id: String,
    pub current_host_machine_class_id: String,
    pub generated_from_refs: Vec<String>,
    pub backend_family_ids: Vec<String>,
    pub toolchain_family_ids: Vec<String>,
    pub profile_ids: Vec<String>,
    pub publication_allowed_profile_ids: Vec<String>,
    pub suppressed_profile_ids: Vec<String>,
    pub rows: Vec<TassadarNumericPortabilityRow>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarNumericPortabilityReport {
    #[must_use]
    pub fn new(
        current_host_machine_class_id: impl Into<String>,
        mut generated_from_refs: Vec<String>,
        rows: Vec<TassadarNumericPortabilityRow>,
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
        let profile_ids = rows
            .iter()
            .map(|row| row.profile_id.clone())
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
            report_id: String::from("tassadar.numeric_portability.report.v1"),
            current_host_machine_class_id: current_host_machine_class_id.into(),
            generated_from_refs,
            backend_family_ids,
            toolchain_family_ids,
            profile_ids,
            publication_allowed_profile_ids,
            suppressed_profile_ids,
            rows,
            claim_boundary: String::from(
                "this report freezes machine-, backend-, and toolchain-scoped numeric portability envelopes for the bounded float and mixed-numeric lanes. It keeps exact CPU-reference publication separate from suppressed non-CPU backends and bounded-approximate numeric families, and it does not claim generic Wasm numeric closure, backend-invariant float exactness, or full f64 portability",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Numeric portability now records profiles={}, backends={}, toolchains={}, published_rows={}, suppressed_rows={}, published_profiles={}, suppressed_profiles={}, current_host=`{}`.",
            report.profile_ids.len(),
            report.backend_family_ids.len(),
            report.toolchain_family_ids.len(),
            published_row_count,
            suppressed_row_count,
            report.publication_allowed_profile_ids.len(),
            report.suppressed_profile_ids.len(),
            report.current_host_machine_class_id,
        );
        report.report_digest =
            stable_digest(b"psionic_tassadar_numeric_portability_report|", &report);
        report
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "error_kind", rename_all = "snake_case")]
pub enum TassadarNumericPortabilityError {
    DriftAbovePublishedEnvelope {
        observed_max_ulp_drift: u32,
        max_allowed_ulp_drift: u32,
    },
}

pub fn validate_tassadar_numeric_portability_drift(
    observed_max_ulp_drift: u32,
    max_allowed_ulp_drift: u32,
) -> Result<(), TassadarNumericPortabilityError> {
    if observed_max_ulp_drift > max_allowed_ulp_drift {
        return Err(TassadarNumericPortabilityError::DriftAbovePublishedEnvelope {
            observed_max_ulp_drift,
            max_allowed_ulp_drift,
        });
    }
    Ok(())
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
        validate_tassadar_numeric_portability_drift, TassadarNumericPortabilityError,
        TassadarNumericPortabilityReport, TassadarNumericPortabilityRow,
        TassadarNumericPortabilityRowStatus, TassadarNumericPortabilitySuppressionReason,
    };
    use psionic_ir::TassadarMixedNumericSupportPosture;

    #[test]
    fn numeric_portability_report_deduplicates_sets() {
        let report = TassadarNumericPortabilityReport::new(
            "host_cpu_x86_64",
            vec![String::from("fixtures/example.json")],
            vec![
                TassadarNumericPortabilityRow {
                    profile_id: String::from("profile.exact"),
                    support_posture: TassadarMixedNumericSupportPosture::Exact,
                    backend_family: String::from("cpu_reference"),
                    toolchain_family: String::from("rustc:wasm32-unknown-unknown"),
                    machine_class_id: String::from("host_cpu_x86_64"),
                    max_allowed_ulp_drift: 0,
                    observed_max_ulp_drift: 0,
                    row_status: TassadarNumericPortabilityRowStatus::PublishedMeasuredCurrentHost,
                    publication_allowed: true,
                    suppression_reason: None,
                    note: String::from("green"),
                },
                TassadarNumericPortabilityRow {
                    profile_id: String::from("profile.approx"),
                    support_posture: TassadarMixedNumericSupportPosture::BoundedApproximate,
                    backend_family: String::from("cpu_reference"),
                    toolchain_family: String::from("rustc:wasm32-unknown-unknown"),
                    machine_class_id: String::from("host_cpu_x86_64"),
                    max_allowed_ulp_drift: 4,
                    observed_max_ulp_drift: 0,
                    row_status: TassadarNumericPortabilityRowStatus::SuppressedApproximateProfile,
                    publication_allowed: false,
                    suppression_reason: Some(
                        TassadarNumericPortabilitySuppressionReason::ApproximateProfileNotPublished,
                    ),
                    note: String::from("red"),
                },
            ],
        );

        assert_eq!(report.profile_ids.len(), 2);
        assert_eq!(report.backend_family_ids, vec![String::from("cpu_reference")]);
        assert_eq!(
            report.toolchain_family_ids,
            vec![String::from("rustc:wasm32-unknown-unknown")]
        );
    }

    #[test]
    fn numeric_portability_refuses_drift_above_envelope() {
        let err = validate_tassadar_numeric_portability_drift(2, 0).expect_err("drift refusal");
        assert_eq!(
            err,
            TassadarNumericPortabilityError::DriftAbovePublishedEnvelope {
                observed_max_ulp_drift: 2,
                max_allowed_ulp_drift: 0,
            }
        );
    }
}
