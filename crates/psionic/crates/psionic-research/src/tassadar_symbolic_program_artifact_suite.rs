use std::{
    collections::BTreeSet,
    fs,
    path::Path,
};

use psionic_compiler::{
    TassadarSymbolicArtifactBundleError, compile_tassadar_symbolic_program_to_artifact_bundle,
};
use psionic_ir::tassadar_symbolic_program_examples;
use psionic_runtime::{TassadarProgramArtifactError, TassadarProgramSourceKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_SUITE_OUTPUT_DIR: &str = "fixtures/tassadar/reports";
pub const TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_SUITE_REPORT_FILE: &str =
    "tassadar_symbolic_program_artifact_suite.json";
pub const TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_SUITE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_symbolic_program_artifact_suite.json";
pub const TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_SUITE_EXAMPLE_COMMAND: &str =
    "cargo run -p psionic-research --example tassadar_symbolic_program_artifact_suite";
pub const TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_SUITE_TEST_COMMAND: &str =
    "cargo test -p psionic-research symbolic_program_artifact_suite_matches_committed_truth -- --nocapture";

const REPORT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSymbolicProgramArtifactCaseReport {
    pub case_id: String,
    pub workload_family_id: String,
    pub summary: String,
    pub symbolic_program_id: String,
    pub symbolic_program_digest: String,
    pub bundle_id: String,
    pub bundle_digest: String,
    pub artifact_id: String,
    pub artifact_digest: String,
    pub source_kind: TassadarProgramSourceKind,
    pub compiler_family: String,
    pub compiler_version: String,
    pub target_profile_id: String,
    pub validated_instruction_count: usize,
    pub memory_slots: usize,
    pub local_count: usize,
    pub required_lowering_opcodes: Vec<String>,
    pub expected_outputs: Vec<i32>,
    pub expected_final_memory: Vec<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSymbolicProgramArtifactSuiteReport {
    pub schema_version: u16,
    pub suite_id: String,
    pub report_ref: String,
    pub regeneration_commands: Vec<String>,
    pub total_case_count: u32,
    pub covered_workload_families: Vec<String>,
    pub required_workload_families: Vec<String>,
    pub required_family_coverage_complete: bool,
    pub case_reports: Vec<TassadarSymbolicProgramArtifactCaseReport>,
    pub claim_class: String,
    pub claim_boundary: String,
    pub report_digest: String,
}

impl TassadarSymbolicProgramArtifactSuiteReport {
    fn new(case_reports: Vec<TassadarSymbolicProgramArtifactCaseReport>) -> Self {
        let covered_workload_families = case_reports
            .iter()
            .map(|case| case.workload_family_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let required_workload_families = vec![
            String::from("addition"),
            String::from("parity"),
            String::from("finite_state"),
            String::from("simple_stack_machine"),
        ];
        let required_family_coverage_complete = required_workload_families
            .iter()
            .all(|family| covered_workload_families.contains(family));
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            suite_id: String::from("tassadar.symbolic_program_artifact_suite.v0"),
            report_ref: String::from(TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_SUITE_REPORT_REF),
            regeneration_commands: vec![
                String::from(TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_SUITE_EXAMPLE_COMMAND),
                String::from(TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_SUITE_TEST_COMMAND),
            ],
            total_case_count: case_reports.len() as u32,
            covered_workload_families,
            required_workload_families,
            required_family_coverage_complete,
            case_reports,
            claim_class: String::from("compiled_bounded_exactness"),
            claim_boundary: String::from(
                "this suite freezes the first public program-to-artifact compiler lane for bounded straight-line symbolic programs only; it proves digest-bound runtime artifacts plus explicit expected execution manifests on seeded addition, parity, finite-state, memory, and simple stack-machine cases, and does not imply arbitrary Wasm or learned generalization",
            ),
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_symbolic_program_artifact_suite|", &report);
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarSymbolicProgramArtifactSuiteError {
    #[error(transparent)]
    Compiler(#[from] TassadarSymbolicArtifactBundleError),
    #[error(transparent)]
    ProgramArtifact(#[from] TassadarProgramArtifactError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
}

pub fn build_tassadar_symbolic_program_artifact_suite_report()
-> Result<TassadarSymbolicProgramArtifactSuiteReport, TassadarSymbolicProgramArtifactSuiteError> {
    let mut case_reports = Vec::new();
    for example in tassadar_symbolic_program_examples() {
        let bundle = compile_tassadar_symbolic_program_to_artifact_bundle(
            &example.program,
            &example.input_assignments,
        )?;
        bundle.program_artifact.validate_internal_consistency()?;
        case_reports.push(TassadarSymbolicProgramArtifactCaseReport {
            case_id: example.case_id.clone(),
            workload_family_id: workload_family_id(example.case_id.as_str()).to_string(),
            summary: example.summary.clone(),
            symbolic_program_id: bundle.symbolic_program_id.clone(),
            symbolic_program_digest: bundle.symbolic_program_digest.clone(),
            bundle_id: bundle.bundle_id.clone(),
            bundle_digest: bundle.bundle_digest.clone(),
            artifact_id: bundle.program_artifact.artifact_id.clone(),
            artifact_digest: bundle.program_artifact.artifact_digest.clone(),
            source_kind: bundle.program_artifact.source_identity.source_kind,
            compiler_family: bundle.program_artifact.toolchain_identity.compiler_family.clone(),
            compiler_version: bundle.program_artifact.toolchain_identity.compiler_version.clone(),
            target_profile_id: bundle.program_artifact.wasm_profile_id.clone(),
            validated_instruction_count: bundle.program_artifact.validated_program.instructions.len(),
            memory_slots: bundle.program_artifact.validated_program.memory_slots,
            local_count: bundle.program_artifact.validated_program.local_count,
            required_lowering_opcodes: bundle
                .required_lowering_opcodes
                .iter()
                .map(|opcode| opcode.mnemonic().to_string())
                .collect(),
            expected_outputs: bundle.execution_manifest.expected_outputs.clone(),
            expected_final_memory: bundle.execution_manifest.expected_final_memory.clone(),
        });
    }
    Ok(TassadarSymbolicProgramArtifactSuiteReport::new(case_reports))
}

pub fn run_tassadar_symbolic_program_artifact_suite(
    output_dir: &Path,
) -> Result<TassadarSymbolicProgramArtifactSuiteReport, TassadarSymbolicProgramArtifactSuiteError>
{
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarSymbolicProgramArtifactSuiteError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;
    let report = build_tassadar_symbolic_program_artifact_suite_report()?;
    let report_path = output_dir.join(TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_SUITE_REPORT_FILE);
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(&report_path, bytes).map_err(|error| {
        TassadarSymbolicProgramArtifactSuiteError::Write {
            path: report_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn workload_family_id(case_id: &str) -> &'static str {
    match case_id {
        "addition_pair" => "addition",
        "parity_two_bits" => "parity",
        "finite_state_counter" => "finite_state",
        "stack_machine_add_step" => "simple_stack_machine",
        "memory_accumulator" => "memory",
        _ => "bounded_symbolic",
    }
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("symbolic program artifact suite should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde::de::DeserializeOwned;
    use tempfile::tempdir;

    use super::{
        TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_SUITE_REPORT_REF,
        build_tassadar_symbolic_program_artifact_suite_report,
        run_tassadar_symbolic_program_artifact_suite,
        TassadarSymbolicProgramArtifactSuiteReport,
    };

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
    }

    fn read_repo_json<T>(
        repo_relative_path: &str,
    ) -> Result<T, Box<dyn std::error::Error>>
    where
        T: DeserializeOwned,
    {
        let path = repo_root().join(repo_relative_path);
        let bytes = std::fs::read(&path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[test]
    fn symbolic_program_artifact_suite_covers_required_bounded_families()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_symbolic_program_artifact_suite_report()?;
        assert!(report.required_family_coverage_complete);
        assert_eq!(
            report.covered_workload_families,
            vec![
                String::from("addition"),
                String::from("finite_state"),
                String::from("memory"),
                String::from("parity"),
                String::from("simple_stack_machine"),
            ]
        );
        assert!(
            report
                .case_reports
                .iter()
                .all(|case| case.source_kind == psionic_runtime::TassadarProgramSourceKind::SymbolicProgram)
        );
        Ok(())
    }

    #[test]
    fn symbolic_program_artifact_suite_matches_committed_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_symbolic_program_artifact_suite_report()?;
        let persisted: TassadarSymbolicProgramArtifactSuiteReport =
            read_repo_json(TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_SUITE_REPORT_REF)?;
        assert_eq!(persisted, report);
        assert!(report.required_family_coverage_complete);
        Ok(())
    }

    #[test]
    fn symbolic_program_artifact_suite_writes_current_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let report = run_tassadar_symbolic_program_artifact_suite(temp_dir.path())?;
        let persisted: TassadarSymbolicProgramArtifactSuiteReport = serde_json::from_slice(
            &std::fs::read(
                temp_dir
                    .path()
                    .join("tassadar_symbolic_program_artifact_suite.json"),
            )?,
        )?;
        assert_eq!(persisted, report);
        Ok(())
    }
}
