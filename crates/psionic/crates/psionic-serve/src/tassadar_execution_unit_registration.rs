use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{LocalTassadarExecutorService, TassadarExecutorCapabilityPublication};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_EXECUTION_UNIT_REGISTRATION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_execution_unit_registration_report.json";

/// Pricing posture attached to one registered executor family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutionUnitPricingPosture {
    BenchmarkCalibratedIndicative,
    NotMarketEligible,
}

/// Pricing descriptor for one execution-unit family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutionUnitPricingDescriptor {
    pub pricing_posture: TassadarExecutionUnitPricingPosture,
    pub indicative_cost_per_correct_job_milliunits: u32,
    pub settlement_eligible: bool,
    pub note: String,
}

/// Machine-legible execution-unit registration descriptor for one executor family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutionUnitRegistrationDescriptor {
    pub registration_id: String,
    pub unit_id: String,
    pub family_id: String,
    pub product_id: String,
    pub model_id: String,
    pub runtime_backend: String,
    pub topology_compatibility: Vec<String>,
    pub capability_profile: Vec<String>,
    pub supported_workload_classes: Vec<String>,
    pub refusal_taxonomy: Vec<String>,
    pub benchmark_lineage_refs: Vec<String>,
    pub pricing: TassadarExecutionUnitPricingDescriptor,
    pub claim_boundary: String,
    pub descriptor_digest: String,
}

impl TassadarExecutionUnitRegistrationDescriptor {
    /// Validates descriptor completeness before publication.
    pub fn validate(&self) -> Result<(), TassadarExecutionUnitRegistrationDescriptorError> {
        if self.topology_compatibility.is_empty() {
            return Err(
                TassadarExecutionUnitRegistrationDescriptorError::MissingTopologyCompatibility,
            );
        }
        if self.capability_profile.is_empty() {
            return Err(TassadarExecutionUnitRegistrationDescriptorError::MissingCapabilityProfile);
        }
        if self.supported_workload_classes.is_empty() {
            return Err(
                TassadarExecutionUnitRegistrationDescriptorError::MissingSupportedWorkloadClasses,
            );
        }
        if self.refusal_taxonomy.is_empty() {
            return Err(TassadarExecutionUnitRegistrationDescriptorError::MissingRefusalTaxonomy);
        }
        if self.benchmark_lineage_refs.is_empty() {
            return Err(TassadarExecutionUnitRegistrationDescriptorError::MissingBenchmarkLineage);
        }
        Ok(())
    }
}

/// Completeness failure for one execution-unit descriptor.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarExecutionUnitRegistrationDescriptorError {
    #[error("execution-unit descriptor omitted topology compatibility")]
    MissingTopologyCompatibility,
    #[error("execution-unit descriptor omitted capability profile")]
    MissingCapabilityProfile,
    #[error("execution-unit descriptor omitted supported workload classes")]
    MissingSupportedWorkloadClasses,
    #[error("execution-unit descriptor omitted refusal taxonomy")]
    MissingRefusalTaxonomy,
    #[error("execution-unit descriptor omitted benchmark lineage refs")]
    MissingBenchmarkLineage,
}

/// Serve-owned report over executor-family execution-unit registration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutionUnitRegistrationReport {
    pub schema_version: u16,
    pub report_id: String,
    pub descriptor: TassadarExecutionUnitRegistrationDescriptor,
    pub publishable_workload_class_count: u32,
    pub refusal_taxonomy_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarExecutionUnitRegistrationReportError {
    #[error("failed to build executor capability publication: {detail}")]
    CapabilityPublication { detail: String },
    #[error(transparent)]
    Descriptor(#[from] TassadarExecutionUnitRegistrationDescriptorError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the serve-owned execution-unit registration report.
pub fn build_tassadar_execution_unit_registration_report()
-> Result<TassadarExecutionUnitRegistrationReport, TassadarExecutionUnitRegistrationReportError> {
    let publication = LocalTassadarExecutorService::new()
        .capability_publication(None)
        .map_err(
            |error| TassadarExecutionUnitRegistrationReportError::CapabilityPublication {
                detail: error.to_string(),
            },
        )?;
    let descriptor = descriptor_from_publication(&publication);
    descriptor.validate()?;
    let publishable_workload_class_count = descriptor.supported_workload_classes.len() as u32;
    let refusal_taxonomy_count = descriptor.refusal_taxonomy.len() as u32;
    let mut report = TassadarExecutionUnitRegistrationReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.execution_unit_registration.report.v1"),
        descriptor,
        publishable_workload_class_count,
        refusal_taxonomy_count,
        claim_boundary: String::from(
            "this serve report registers the executor family as a first-class execution unit with identity, topology compatibility, capability profile, refusal taxonomy, benchmark lineage, and pricing posture. It does not imply settlement or market eligibility",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Execution-unit registration report now freezes {} publishable workload classes with {} refusal-taxonomy entries.",
        report.publishable_workload_class_count, report.refusal_taxonomy_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_execution_unit_registration_report|",
        &report,
    );
    Ok(report)
}

fn descriptor_from_publication(
    publication: &TassadarExecutorCapabilityPublication,
) -> TassadarExecutionUnitRegistrationDescriptor {
    let mut topology_compatibility = publication
        .runtime_capability
        .supported_wasm_profiles
        .clone();
    topology_compatibility.push(format!(
        "runtime_backend:{}",
        publication.runtime_capability.runtime_backend
    ));
    topology_compatibility.sort();
    topology_compatibility.dedup();

    let mut capability_profile = publication
        .runtime_capability
        .supported_decode_modes
        .iter()
        .map(|mode| format!("decode:{}", mode.as_str()))
        .collect::<Vec<_>>();
    capability_profile.push(String::from("capability:executor_trace"));
    capability_profile.push(String::from("capability:bounded_module_execution"));
    capability_profile.sort();
    capability_profile.dedup();

    let mut supported_workload_classes = publication
        .workload_capability_matrix
        .rows
        .iter()
        .filter(|row| {
            row.support_posture != psionic_models::TassadarWorkloadSupportPosture::Unsupported
        })
        .map(|row| row.workload_class.as_str().to_string())
        .collect::<Vec<_>>();
    supported_workload_classes.sort();
    supported_workload_classes.dedup();

    let mut refusal_taxonomy = publication
        .workload_capability_matrix
        .rows
        .iter()
        .flat_map(|row| row.refusal_reasons.iter())
        .map(|reason| format!("workload:{}", workload_refusal_reason_label(*reason)))
        .collect::<Vec<_>>();
    refusal_taxonomy.push(format!(
        "module_execution:{}",
        host_import_refusal_label(
            publication
                .module_execution_capability
                .runtime_capability
                .host_import_boundary
                .unsupported_host_call_refusal
        )
    ));
    refusal_taxonomy.sort();
    refusal_taxonomy.dedup();

    let mut benchmark_lineage_refs = publication
        .workload_capability_matrix
        .rows
        .iter()
        .filter_map(|row| row.benchmark_gate.as_ref())
        .flat_map(|gate| [gate.benchmark_gate_ref.clone(), gate.evidence_ref.clone()])
        .collect::<Vec<_>>();
    benchmark_lineage_refs.sort();
    benchmark_lineage_refs.dedup();

    let pricing = TassadarExecutionUnitPricingDescriptor {
        pricing_posture: TassadarExecutionUnitPricingPosture::BenchmarkCalibratedIndicative,
        indicative_cost_per_correct_job_milliunits: 240,
        settlement_eligible: false,
        note: String::from(
            "pricing is indicative and benchmark-calibrated only; later accepted-outcome and market bridges remain separate",
        ),
    };

    let mut descriptor = TassadarExecutionUnitRegistrationDescriptor {
        registration_id: format!(
            "tassadar.execution_unit_registration.{}.v1",
            publication.model_descriptor.model.model_id
        ),
        unit_id: format!(
            "execution-unit:tassadar:{}",
            publication.model_descriptor.model.model_id
        ),
        family_id: String::from("tassadar_executor_family"),
        product_id: publication.product_id.clone(),
        model_id: publication.model_descriptor.model.model_id.clone(),
        runtime_backend: publication.runtime_capability.runtime_backend.clone(),
        topology_compatibility,
        capability_profile,
        supported_workload_classes,
        refusal_taxonomy,
        benchmark_lineage_refs,
        pricing,
        claim_boundary: String::from(
            "registration is benchmark-gated execution-unit truth for the current executor family and does not imply accepted-outcome or market closure",
        ),
        descriptor_digest: String::new(),
    };
    descriptor.descriptor_digest = stable_digest(
        b"psionic_tassadar_execution_unit_registration_descriptor|",
        &descriptor,
    );
    descriptor
}

/// Returns the canonical absolute path for the committed report.
#[must_use]
pub fn tassadar_execution_unit_registration_report_path() -> PathBuf {
    repo_root().join(TASSADAR_EXECUTION_UNIT_REGISTRATION_REPORT_REF)
}

/// Writes the committed report.
pub fn write_tassadar_execution_unit_registration_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarExecutionUnitRegistrationReport, TassadarExecutionUnitRegistrationReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarExecutionUnitRegistrationReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_execution_unit_registration_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarExecutionUnitRegistrationReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_execution_unit_registration_report(
    path: impl AsRef<Path>,
) -> Result<TassadarExecutionUnitRegistrationReport, TassadarExecutionUnitRegistrationReportError> {
    read_json(path)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarExecutionUnitRegistrationReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| TassadarExecutionUnitRegistrationReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarExecutionUnitRegistrationReportError::Deserialize {
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

fn workload_refusal_reason_label(
    reason: psionic_models::TassadarWorkloadCapabilityRefusalReason,
) -> &'static str {
    match reason {
        psionic_models::TassadarWorkloadCapabilityRefusalReason::WorkloadClassOutOfScope => {
            "workload_class_out_of_scope"
        }
        psionic_models::TassadarWorkloadCapabilityRefusalReason::BenchmarkGateMissing => {
            "benchmark_gate_missing"
        }
        psionic_models::TassadarWorkloadCapabilityRefusalReason::ClaimBoundaryUnvalidated => {
            "claim_boundary_unvalidated"
        }
    }
}

fn host_import_refusal_label(
    reason: psionic_runtime::TassadarModuleExecutionRefusalKind,
) -> &'static str {
    match reason {
        psionic_runtime::TassadarModuleExecutionRefusalKind::UnsupportedHostImport => {
            "unsupported_host_import"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_execution_unit_registration_report,
        load_tassadar_execution_unit_registration_report,
        tassadar_execution_unit_registration_report_path,
    };

    #[test]
    fn execution_unit_registration_report_keeps_descriptor_complete() {
        let report = build_tassadar_execution_unit_registration_report().expect("report");

        assert!(!report.descriptor.topology_compatibility.is_empty());
        assert!(!report.descriptor.capability_profile.is_empty());
        assert!(!report.descriptor.supported_workload_classes.is_empty());
        assert!(!report.descriptor.refusal_taxonomy.is_empty());
        assert!(!report.descriptor.benchmark_lineage_refs.is_empty());
        assert_eq!(report.descriptor.pricing.settlement_eligible, false);
    }

    #[test]
    fn execution_unit_registration_report_matches_committed_truth() {
        let expected = build_tassadar_execution_unit_registration_report().expect("report");
        let committed = load_tassadar_execution_unit_registration_report(
            tassadar_execution_unit_registration_report_path(),
        )
        .expect("committed report");

        assert_eq!(committed, expected);
    }
}
