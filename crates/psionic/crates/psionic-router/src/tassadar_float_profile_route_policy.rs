use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
#[cfg(not(test))]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::TassadarBroadInternalComputeRouteDecisionStatus;
use psionic_runtime::{
    TassadarNumericPortabilityReport, TASSADAR_NUMERIC_PORTABILITY_REPORT_REF,
};

const TASSADAR_FLOAT_PROFILE_ACCEPTANCE_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_float_profile_acceptance_gate_report.json";

pub const TASSADAR_FLOAT_PROFILE_ROUTE_POLICY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_float_profile_route_policy_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFloatProfileRoutePolicyRow {
    pub route_policy_id: String,
    pub target_profile_id: String,
    pub backend_family: String,
    pub toolchain_family: String,
    pub decision_status: TassadarBroadInternalComputeRouteDecisionStatus,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFloatProfileRoutePolicyReport {
    pub schema_version: u16,
    pub report_id: String,
    pub acceptance_gate_report_ref: String,
    pub numeric_portability_report_ref: String,
    pub rows: Vec<TassadarFloatProfileRoutePolicyRow>,
    pub promoted_profile_specific_route_count: u32,
    pub suppressed_route_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarFloatProfileRoutePolicyReportError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
}

pub fn build_tassadar_float_profile_route_policy_report(
) -> Result<TassadarFloatProfileRoutePolicyReport, TassadarFloatProfileRoutePolicyReportError> {
    let gate: FloatProfileGateSource = read_json(
        repo_root().join(TASSADAR_FLOAT_PROFILE_ACCEPTANCE_GATE_REPORT_REF),
    )?;
    let portability: TassadarNumericPortabilityReport =
        read_json(repo_root().join(TASSADAR_NUMERIC_PORTABILITY_REPORT_REF))?;
    let cpu_toolchain = portability
        .toolchain_family_ids
        .iter()
        .find(|toolchain: &&String| !toolchain.contains('+'))
        .cloned()
        .unwrap_or_else(|| String::from("rustc:wasm32-unknown-unknown"));
    let rows = vec![
        route_row(
            "route.numeric.f32_only.profile_specific",
            "tassadar.numeric_profile.f32_only.v1",
            &gate.public_profile_allowed_profile_ids,
            cpu_toolchain.as_str(),
        ),
        route_row(
            "route.numeric.mixed_i32_f32.profile_specific",
            "tassadar.numeric_profile.mixed_i32_f32.v1",
            &gate.public_profile_allowed_profile_ids,
            cpu_toolchain.as_str(),
        ),
        route_row(
            "route.numeric.bounded_f64.profile_specific",
            "tassadar.numeric_profile.bounded_f64_conversion.v1",
            &gate.public_profile_allowed_profile_ids,
            cpu_toolchain.as_str(),
        ),
    ];
    let promoted_profile_specific_route_count = rows
        .iter()
        .filter(|row| {
            row.decision_status
                == TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific
        })
        .count() as u32;
    let suppressed_route_count = rows
        .iter()
        .filter(|row| {
            row.decision_status == TassadarBroadInternalComputeRouteDecisionStatus::Suppressed
        })
        .count() as u32;
    let mut report = TassadarFloatProfileRoutePolicyReport {
        schema_version: 1,
        report_id: String::from("tassadar.float_profile_route_policy.report.v1"),
        acceptance_gate_report_ref: String::from(TASSADAR_FLOAT_PROFILE_ACCEPTANCE_GATE_REPORT_REF),
        numeric_portability_report_ref: String::from(TASSADAR_NUMERIC_PORTABILITY_REPORT_REF),
        rows,
        promoted_profile_specific_route_count,
        suppressed_route_count,
        claim_boundary: String::from(
            "this router report names exact float-enabled numeric profiles as profile-specific cpu-reference routes only when the dedicated float-profile gate is green. It keeps approximate f64 narrowing suppressed and does not widen any numeric profile into the default served route",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Float profile route policy now records promoted_profile_specific_routes={}, suppressed_routes={}.",
        report.promoted_profile_specific_route_count, report.suppressed_route_count,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_float_profile_route_policy_report|", &report);
    Ok(report)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct FloatProfileGateSource {
    public_profile_allowed_profile_ids: Vec<String>,
}

fn route_row(
    route_policy_id: &str,
    profile_id: &str,
    public_profile_allowed_profile_ids: &[String],
    cpu_toolchain: &str,
) -> TassadarFloatProfileRoutePolicyRow {
    let promoted = public_profile_allowed_profile_ids
        .iter()
        .any(|allowed| allowed == profile_id);
    TassadarFloatProfileRoutePolicyRow {
        route_policy_id: String::from(route_policy_id),
        target_profile_id: String::from(profile_id),
        backend_family: String::from("cpu_reference"),
        toolchain_family: String::from(cpu_toolchain),
        decision_status: if promoted {
            TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific
        } else {
            TassadarBroadInternalComputeRouteDecisionStatus::Suppressed
        },
        note: if promoted {
            format!(
                "numeric profile `{profile_id}` is routeable only as a named cpu-reference profile-specific lane"
            )
        } else {
            format!(
                "numeric profile `{profile_id}` stays suppressed because the float-profile gate does not allow public route promotion"
            )
        },
    }
}

pub fn tassadar_float_profile_route_policy_report_path() -> PathBuf {
    repo_root().join(TASSADAR_FLOAT_PROFILE_ROUTE_POLICY_REPORT_REF)
}

pub fn write_tassadar_float_profile_route_policy_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarFloatProfileRoutePolicyReport, TassadarFloatProfileRoutePolicyReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarFloatProfileRoutePolicyReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_float_profile_route_policy_report()?;
    let bytes =
        serde_json::to_vec_pretty(&report).expect("float profile route policy serializes");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarFloatProfileRoutePolicyReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_float_profile_route_policy_report(
    path: impl AsRef<Path>,
) -> Result<TassadarFloatProfileRoutePolicyReport, TassadarFloatProfileRoutePolicyReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarFloatProfileRoutePolicyReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarFloatProfileRoutePolicyReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .expect("workspace root")
}

fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarFloatProfileRoutePolicyReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarFloatProfileRoutePolicyReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarFloatProfileRoutePolicyReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
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
        build_tassadar_float_profile_route_policy_report,
        load_tassadar_float_profile_route_policy_report,
        tassadar_float_profile_route_policy_report_path,
        write_tassadar_float_profile_route_policy_report,
    };
    use crate::TassadarBroadInternalComputeRouteDecisionStatus;

    #[test]
    fn float_profile_route_policy_promotes_exact_profiles_only() {
        let report = build_tassadar_float_profile_route_policy_report().expect("report");

        assert!(report.rows.iter().any(|row| {
            row.target_profile_id == "tassadar.numeric_profile.f32_only.v1"
                && row.decision_status
                    == TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific
        }));
        assert!(report.rows.iter().any(|row| {
            row.target_profile_id == "tassadar.numeric_profile.mixed_i32_f32.v1"
                && row.decision_status
                    == TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific
        }));
        assert!(report.rows.iter().any(|row| {
            row.target_profile_id == "tassadar.numeric_profile.bounded_f64_conversion.v1"
                && row.decision_status
                    == TassadarBroadInternalComputeRouteDecisionStatus::Suppressed
        }));
    }

    #[test]
    fn float_profile_route_policy_matches_committed_truth() {
        let generated = build_tassadar_float_profile_route_policy_report().expect("report");
        let committed = load_tassadar_float_profile_route_policy_report(
            tassadar_float_profile_route_policy_report_path(),
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_float_profile_route_policy_persists_current_truth() {
        let output_path =
            std::env::temp_dir().join("tassadar_float_profile_route_policy_report.json");
        let generated =
            write_tassadar_float_profile_route_policy_report(&output_path).expect("report");
        let reloaded =
            load_tassadar_float_profile_route_policy_report(&output_path).expect("reloaded");
        assert_eq!(generated, reloaded);
        let _ = std::fs::remove_file(output_path);
    }
}
