use serde::{Deserialize, Serialize};

use psionic_eval::TassadarSessionProcessProfileReport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSessionProcessProfileReceipt {
    pub report_id: String,
    pub profile_id: String,
    pub public_profile_allowed_profile_ids: Vec<String>,
    pub default_served_profile_allowed_profile_ids: Vec<String>,
    pub routeable_interaction_surface_ids: Vec<String>,
    pub refused_interaction_surface_ids: Vec<String>,
    pub overall_green: bool,
    pub detail: String,
}

impl TassadarSessionProcessProfileReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarSessionProcessProfileReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            profile_id: report.profile_id.clone(),
            public_profile_allowed_profile_ids: report.public_profile_allowed_profile_ids.clone(),
            default_served_profile_allowed_profile_ids: report
                .default_served_profile_allowed_profile_ids
                .clone(),
            routeable_interaction_surface_ids: report.routeable_interaction_surface_ids.clone(),
            refused_interaction_surface_ids: report.refused_interaction_surface_ids.clone(),
            overall_green: report.overall_green,
            detail: format!(
                "session-process profile report `{}` keeps public_profiles={}, default_served_profiles={}, routeable_surfaces={}, refused_surfaces={}, overall_green={}",
                report.report_id,
                report.public_profile_allowed_profile_ids.len(),
                report.default_served_profile_allowed_profile_ids.len(),
                report.routeable_interaction_surface_ids.len(),
                report.refused_interaction_surface_ids.len(),
                report.overall_green,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarSessionProcessProfileReceipt;
    use psionic_eval::build_tassadar_session_process_profile_report;

    #[test]
    fn session_process_profile_receipt_projects_report() {
        let report = build_tassadar_session_process_profile_report().expect("report");
        let receipt = TassadarSessionProcessProfileReceipt::from_report(&report);

        assert!(receipt.overall_green);
        assert_eq!(
            receipt.public_profile_allowed_profile_ids,
            vec![String::from("tassadar.internal_compute.session_process.v1")]
        );
        assert!(receipt
            .routeable_interaction_surface_ids
            .contains(&String::from("deterministic_echo_turn_loop")));
        assert!(receipt
            .routeable_interaction_surface_ids
            .contains(&String::from("stateful_counter_turn_loop")));
        assert!(receipt
            .refused_interaction_surface_ids
            .contains(&String::from("open_ended_external_event_stream")));
    }
}
