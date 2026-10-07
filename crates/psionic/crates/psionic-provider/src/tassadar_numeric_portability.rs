use serde::{Deserialize, Serialize};

use psionic_runtime::TassadarNumericPortabilityReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNumericPortabilityReceipt {
    pub report_id: String,
    pub current_host_machine_class_id: String,
    pub backend_family_ids: Vec<String>,
    pub toolchain_family_ids: Vec<String>,
    pub profile_ids: Vec<String>,
    pub publication_allowed_profile_ids: Vec<String>,
    pub suppressed_profile_ids: Vec<String>,
    pub detail: String,
}

impl TassadarNumericPortabilityReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarNumericPortabilityReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            current_host_machine_class_id: report.current_host_machine_class_id.clone(),
            backend_family_ids: report.backend_family_ids.clone(),
            toolchain_family_ids: report.toolchain_family_ids.clone(),
            profile_ids: report.profile_ids.clone(),
            publication_allowed_profile_ids: report.publication_allowed_profile_ids.clone(),
            suppressed_profile_ids: report.suppressed_profile_ids.clone(),
            detail: format!(
                "numeric portability `{}` carries profiles={}, backends={}, toolchains={}, published_profiles={}, suppressed_profiles={}, current_host=`{}`",
                report.report_id,
                report.profile_ids.len(),
                report.backend_family_ids.len(),
                report.toolchain_family_ids.len(),
                report.publication_allowed_profile_ids.len(),
                report.suppressed_profile_ids.len(),
                report.current_host_machine_class_id,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarNumericPortabilityReceipt;
    use psionic_eval::build_tassadar_numeric_portability_report;

    #[test]
    fn numeric_portability_receipt_projects_report() {
        let report = build_tassadar_numeric_portability_report().expect("report");
        let receipt = TassadarNumericPortabilityReceipt::from_report(&report);

        assert!(receipt
            .backend_family_ids
            .contains(&String::from("cpu_reference")));
        assert!(receipt
            .toolchain_family_ids
            .contains(&String::from("rustc:wasm32-unknown-unknown")));
        assert!(receipt
            .publication_allowed_profile_ids
            .contains(&String::from("tassadar.numeric_profile.f32_only.v1")));
        assert!(receipt
            .suppressed_profile_ids
            .contains(&String::from(
                "tassadar.numeric_profile.bounded_f64_conversion.v1"
            )));
    }
}
