use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use psionic_ir::{
    tassadar_symbolic_program_examples, TassadarSymbolicExpr, TassadarSymbolicOperand,
    TassadarSymbolicProgram, TassadarSymbolicProgramError, TassadarSymbolicStatement,
};
use psionic_models::{
    tassadar_decompilable_executor_publication, TassadarDecompilableExecutorPublication,
    TassadarDecompilationDiscretizationKind, TassadarDecompilationFamily,
    TassadarDecompilationRetrainArtifact, TassadarDecompilationStabilityClass,
    TASSADAR_DECOMPILATION_FIDELITY_REPORT_REF,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

/// One seeded retrain comparison row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDecompilationRetrainReport {
    /// Stable retrain run identifier.
    pub retrain_run_id: String,
    /// Stable retrain seed.
    pub retrain_seed: u16,
    /// Stable checkpoint ref.
    pub checkpoint_ref: String,
    /// Stable discretization family used by the retrain.
    pub discretization_kind: TassadarDecompilationDiscretizationKind,
    /// Decompiled symbolic program identifier.
    pub decompiled_program_id: String,
    /// Digest of the readable program body including variable names.
    pub readable_program_digest: String,
    /// Digest of the normalized readable structure.
    pub normalized_structure_digest: String,
    /// Whether the retrain preserved the reference behavior exactly.
    pub semantic_equivalence: bool,
    /// Whether the retrain preserved the readable structure after canonicalization.
    pub readable_equivalence: bool,
    /// Number of symbolic statements in the decompiled program.
    pub statement_count: usize,
}

/// One seeded case-level decompilation report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDecompilationCaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Seeded symbolic-reference case identifier.
    pub source_case_id: String,
    /// Constrained learned family.
    pub family: TassadarDecompilationFamily,
    /// Stable reference program identifier.
    pub reference_program_id: String,
    /// Stable reference program digest.
    pub reference_program_digest: String,
    /// Digest of the reference readable structure.
    pub reference_normalized_structure_digest: String,
    /// Number of retrain artifacts compared for the case.
    pub retrain_count: u32,
    /// Number of distinct readable forms observed across retrains.
    pub distinct_readable_program_count: u32,
    /// Fraction of retrains that preserved reference semantics.
    pub semantic_equivalence_bps: u32,
    /// Fraction of retrains that preserved readable structure.
    pub readable_equivalence_bps: u32,
    /// Fraction of retrains that matched the modal readable structure.
    pub retrain_structure_consensus_bps: u32,
    /// Stable case-level classification.
    pub stability_class: TassadarDecompilationStabilityClass,
    /// Ordered retrain comparisons.
    pub retrains: Vec<TassadarDecompilationRetrainReport>,
}

/// One family-level decompilation summary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDecompilationFamilySummary {
    /// Stable family label.
    pub family: TassadarDecompilationFamily,
    /// Number of seeded cases in the family.
    pub case_count: u32,
    /// Mean semantic-equivalence rate across cases.
    pub mean_semantic_equivalence_bps: u32,
    /// Mean readable-equivalence rate across cases.
    pub mean_readable_equivalence_bps: u32,
    /// Mean retrain-structure consensus across cases.
    pub mean_retrain_structure_consensus_bps: u32,
    /// Whether every seeded case in the family stayed semantically equivalent.
    pub all_cases_semantically_equivalent: bool,
}

/// Committed eval report for the decompilable learned executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDecompilationFidelityReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Repo-facing publication for the lane.
    pub publication: TassadarDecompilableExecutorPublication,
    /// Ordered seeded case reports.
    pub case_reports: Vec<TassadarDecompilationCaseReport>,
    /// Family-level summaries.
    pub family_summaries: Vec<TassadarDecompilationFamilySummary>,
    /// Explicit claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Errors while building the decompilation fidelity report.
#[derive(Debug, Error)]
pub enum TassadarDecompilationFidelityReportError {
    /// One seeded source case was missing.
    #[error("missing symbolic reference case `{case_id}`")]
    MissingSourceCase { case_id: String },
    /// One seeded decompiled program could not execute.
    #[error(transparent)]
    Symbolic(#[from] TassadarSymbolicProgramError),
    /// Failed to create the output directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the report.
    #[error("failed to write decompilation fidelity report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// JSON serialization or parsing failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed report for the decompilable learned executor lane.
pub fn build_tassadar_decompilation_fidelity_report(
) -> Result<TassadarDecompilationFidelityReport, TassadarDecompilationFidelityReportError> {
    let publication = tassadar_decompilable_executor_publication();
    let source_cases = tassadar_symbolic_program_examples()
        .into_iter()
        .map(|example| (example.case_id.clone(), example))
        .collect::<BTreeMap<_, _>>();
    let case_reports = publication
        .cases
        .iter()
        .map(|case| {
            let source = source_cases
                .get(case.source_case_id.as_str())
                .ok_or_else(
                    || TassadarDecompilationFidelityReportError::MissingSourceCase {
                        case_id: case.source_case_id.clone(),
                    },
                )?;
            build_case_report(case, source)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let family_summaries = build_family_summaries(case_reports.as_slice());
    let mut report = TassadarDecompilationFidelityReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.decompilation_fidelity.report.v1"),
        publication,
        case_reports,
        family_summaries,
        claim_boundary: String::from(
            "this report compares seeded decompiled learned-executor artifacts against bounded symbolic compiled references only; it proves readable-structure preservation and retrain stability on the published symbolic and state-delta families, and does not imply broad learned exactness, arbitrary Wasm closure, or served readiness",
        ),
        report_digest: String::new(),
    };
    report.report_digest =
        stable_digest(b"psionic_tassadar_decompilation_fidelity_report|", &report);
    Ok(report)
}

/// Returns the canonical absolute path for the committed report.
pub fn tassadar_decompilation_fidelity_report_path() -> PathBuf {
    repo_root().join(TASSADAR_DECOMPILATION_FIDELITY_REPORT_REF)
}

/// Writes the committed report for the decompilable learned executor lane.
pub fn write_tassadar_decompilation_fidelity_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarDecompilationFidelityReport, TassadarDecompilationFidelityReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarDecompilationFidelityReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_decompilation_fidelity_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarDecompilationFidelityReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_case_report(
    case: &psionic_models::TassadarDecompilableExecutorCase,
    source: &psionic_ir::TassadarSymbolicProgramExample,
) -> Result<TassadarDecompilationCaseReport, TassadarDecompilationFidelityReportError> {
    let reference_normalized_structure_digest = normalized_structure_digest(&source.program);
    let retrains = case
        .retrains
        .iter()
        .map(|retrain| {
            build_retrain_report(retrain, source, &reference_normalized_structure_digest)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let semantic_equivalence_count = retrains
        .iter()
        .filter(|retrain| retrain.semantic_equivalence)
        .count();
    let readable_equivalence_count = retrains
        .iter()
        .filter(|retrain| retrain.readable_equivalence)
        .count();
    let mut structure_counts = BTreeMap::<String, usize>::new();
    let mut readable_forms = BTreeSet::new();
    for retrain in &retrains {
        *structure_counts
            .entry(retrain.normalized_structure_digest.clone())
            .or_default() += 1;
        readable_forms.insert(retrain.readable_program_digest.clone());
    }
    let modal_structure_count = structure_counts.values().copied().max().unwrap_or(0);
    let retrain_count = retrains.len();
    let distinct_readable_program_count = readable_forms.len() as u32;
    let stability_class = if distinct_readable_program_count <= 1 {
        TassadarDecompilationStabilityClass::StableExactForm
    } else {
        TassadarDecompilationStabilityClass::StableEquivalentForms
    };
    Ok(TassadarDecompilationCaseReport {
        case_id: case.case_id.clone(),
        source_case_id: case.source_case_id.clone(),
        family: case.family,
        reference_program_id: case.reference_program_id.clone(),
        reference_program_digest: case.reference_program_digest.clone(),
        reference_normalized_structure_digest,
        retrain_count: retrain_count as u32,
        distinct_readable_program_count,
        semantic_equivalence_bps: ratio_bps(semantic_equivalence_count, retrain_count),
        readable_equivalence_bps: ratio_bps(readable_equivalence_count, retrain_count),
        retrain_structure_consensus_bps: ratio_bps(modal_structure_count, retrain_count),
        stability_class,
        retrains,
    })
}

fn build_retrain_report(
    retrain: &TassadarDecompilationRetrainArtifact,
    source: &psionic_ir::TassadarSymbolicProgramExample,
    reference_normalized_structure_digest: &str,
) -> Result<TassadarDecompilationRetrainReport, TassadarDecompilationFidelityReportError> {
    let input_assignments = project_input_assignments(source, &retrain.decompiled_program)?;
    let execution = retrain.decompiled_program.evaluate(&input_assignments)?;
    let normalized_structure_digest = normalized_structure_digest(&retrain.decompiled_program);
    Ok(TassadarDecompilationRetrainReport {
        retrain_run_id: retrain.retrain_run_id.clone(),
        retrain_seed: retrain.retrain_seed,
        checkpoint_ref: retrain.checkpoint_ref.clone(),
        discretization_kind: retrain.discretization_kind,
        decompiled_program_id: retrain.decompiled_program.program_id.clone(),
        readable_program_digest: readable_program_digest(&retrain.decompiled_program),
        normalized_structure_digest: normalized_structure_digest.clone(),
        semantic_equivalence: execution.outputs == source.expected_outputs
            && execution.final_memory == source.expected_final_memory,
        readable_equivalence: normalized_structure_digest == reference_normalized_structure_digest,
        statement_count: retrain.decompiled_program.statements.len(),
    })
}

fn build_family_summaries(
    case_reports: &[TassadarDecompilationCaseReport],
) -> Vec<TassadarDecompilationFamilySummary> {
    let mut grouped =
        BTreeMap::<TassadarDecompilationFamily, Vec<&TassadarDecompilationCaseReport>>::new();
    for report in case_reports {
        grouped.entry(report.family).or_default().push(report);
    }
    let mut summaries = grouped
        .into_iter()
        .map(|(family, reports)| TassadarDecompilationFamilySummary {
            family,
            case_count: reports.len() as u32,
            mean_semantic_equivalence_bps: mean_u32(
                reports
                    .iter()
                    .map(|report| report.semantic_equivalence_bps)
                    .collect::<Vec<_>>()
                    .as_slice(),
            ),
            mean_readable_equivalence_bps: mean_u32(
                reports
                    .iter()
                    .map(|report| report.readable_equivalence_bps)
                    .collect::<Vec<_>>()
                    .as_slice(),
            ),
            mean_retrain_structure_consensus_bps: mean_u32(
                reports
                    .iter()
                    .map(|report| report.retrain_structure_consensus_bps)
                    .collect::<Vec<_>>()
                    .as_slice(),
            ),
            all_cases_semantically_equivalent: reports
                .iter()
                .all(|report| report.semantic_equivalence_bps == 10_000),
        })
        .collect::<Vec<_>>();
    summaries.sort_by_key(|summary| summary.family);
    summaries
}

fn project_input_assignments(
    source: &psionic_ir::TassadarSymbolicProgramExample,
    decompiled_program: &TassadarSymbolicProgram,
) -> Result<BTreeMap<String, i32>, TassadarDecompilationFidelityReportError> {
    let slot_values = source
        .program
        .inputs
        .iter()
        .map(|input| {
            let value = source
                .input_assignments
                .get(input.name.as_str())
                .copied()
                .ok_or_else(|| TassadarSymbolicProgramError::MissingInputAssignment {
                    input: input.name.clone(),
                })?;
            Ok((input.memory_slot, value))
        })
        .collect::<Result<BTreeMap<_, _>, TassadarSymbolicProgramError>>()?;
    decompiled_program
        .inputs
        .iter()
        .map(|input| {
            let value = slot_values
                .get(&input.memory_slot)
                .copied()
                .ok_or_else(|| TassadarSymbolicProgramError::MissingInputAssignment {
                    input: input.name.clone(),
                })?;
            Ok((input.name.clone(), value))
        })
        .collect::<Result<BTreeMap<_, _>, TassadarSymbolicProgramError>>()
        .map_err(Into::into)
}

fn readable_program_digest(program: &TassadarSymbolicProgram) -> String {
    #[derive(Serialize)]
    struct ReadableProgramBody<'a> {
        memory_slots: usize,
        inputs: &'a [psionic_ir::TassadarSymbolicInput],
        initial_memory: &'a [psionic_ir::TassadarSymbolicMemoryCell],
        statements: &'a [TassadarSymbolicStatement],
    }

    stable_digest(
        b"psionic_tassadar_decompiled_readable_program|",
        &ReadableProgramBody {
            memory_slots: program.memory_slots,
            inputs: program.inputs.as_slice(),
            initial_memory: program.initial_memory.as_slice(),
            statements: program.statements.as_slice(),
        },
    )
}

fn normalized_structure_digest(program: &TassadarSymbolicProgram) -> String {
    let mut names = BTreeMap::new();
    for (index, input) in program.inputs.iter().enumerate() {
        names.insert(input.name.as_str(), format!("input{index}"));
    }
    let mut next_binding_index = 0usize;
    let initial_memory = program
        .initial_memory
        .iter()
        .map(|cell| format!("init:{}={}", cell.slot, cell.value))
        .collect::<Vec<_>>();
    let inputs = program
        .inputs
        .iter()
        .enumerate()
        .map(|(index, input)| format!("input{index}=slot({})", input.memory_slot))
        .collect::<Vec<_>>();
    let statements = program
        .statements
        .iter()
        .map(|statement| match statement {
            TassadarSymbolicStatement::Let { name, expr } => {
                let canonical_name = format!("tmp{next_binding_index}");
                next_binding_index += 1;
                let signature = match expr {
                    TassadarSymbolicExpr::Operand { operand } => {
                        format!(
                            "let:{canonical_name}=operand({})",
                            operand_signature(operand, &names)
                        )
                    }
                    TassadarSymbolicExpr::Binary { op, left, right } => format!(
                        "let:{canonical_name}={}( {}, {} )",
                        op.as_str(),
                        operand_signature(left, &names),
                        operand_signature(right, &names),
                    ),
                };
                names.insert(name.as_str(), canonical_name);
                signature
            }
            TassadarSymbolicStatement::Store { slot, value } => {
                format!("store:{slot}={}", operand_signature(value, &names))
            }
            TassadarSymbolicStatement::Output { value } => {
                format!("output={}", operand_signature(value, &names))
            }
        })
        .collect::<Vec<_>>();
    stable_digest(
        b"psionic_tassadar_decompiled_normalized_structure|",
        &serde_json::json!({
            "memory_slots": program.memory_slots,
            "inputs": inputs,
            "initial_memory": initial_memory,
            "statements": statements,
        }),
    )
}

fn operand_signature(operand: &TassadarSymbolicOperand, names: &BTreeMap<&str, String>) -> String {
    match operand {
        TassadarSymbolicOperand::Name { name } => names
            .get(name.as_str())
            .cloned()
            .unwrap_or_else(|| format!("unknown:{name}")),
        TassadarSymbolicOperand::Const { value } => format!("const({value})"),
        TassadarSymbolicOperand::MemorySlot { slot } => format!("slot({slot})"),
    }
}

fn ratio_bps(numerator: usize, denominator: usize) -> u32 {
    if denominator == 0 {
        0
    } else {
        ((numerator as u64) * 10_000 / denominator as u64) as u32
    }
}

fn mean_u32(values: &[u32]) -> u32 {
    if values.is_empty() {
        0
    } else {
        (values.iter().map(|value| u64::from(*value)).sum::<u64>() / values.len() as u64) as u32
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde::de::DeserializeOwned;
    use tempfile::tempdir;

    use super::{
        build_tassadar_decompilation_fidelity_report, tassadar_decompilation_fidelity_report_path,
        write_tassadar_decompilation_fidelity_report, TassadarDecompilationFamily,
        TassadarDecompilationFidelityReport, TassadarDecompilationStabilityClass,
        TASSADAR_DECOMPILATION_FIDELITY_REPORT_REF,
    };

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
    }

    fn read_repo_json<T>(repo_relative_path: &str) -> Result<T, Box<dyn std::error::Error>>
    where
        T: DeserializeOwned,
    {
        let path = repo_root().join(repo_relative_path);
        let bytes = std::fs::read(&path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[test]
    fn decompilation_fidelity_report_keeps_reference_equivalence(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_decompilation_fidelity_report()?;
        assert_eq!(report.case_reports.len(), 5);
        assert!(report
            .case_reports
            .iter()
            .all(|case| case.semantic_equivalence_bps == 10_000
                && case.readable_equivalence_bps == 10_000));
        assert!(report.case_reports.iter().any(|case| case.stability_class
            == TassadarDecompilationStabilityClass::StableEquivalentForms));
        let state_delta = report
            .family_summaries
            .iter()
            .find(|family| family.family == TassadarDecompilationFamily::StateDeltaSketch)
            .expect("state-delta family should exist");
        assert!(state_delta.all_cases_semantically_equivalent);
        Ok(())
    }

    #[test]
    fn decompilation_fidelity_report_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated = build_tassadar_decompilation_fidelity_report()?;
        let committed: TassadarDecompilationFidelityReport =
            read_repo_json(TASSADAR_DECOMPILATION_FIDELITY_REPORT_REF)?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_decompilation_fidelity_report_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let output_path = temp_dir
            .path()
            .join("tassadar_decompilation_fidelity_report.json");
        let written = write_tassadar_decompilation_fidelity_report(&output_path)?;
        let bytes = std::fs::read(&output_path)?;
        let roundtrip: TassadarDecompilationFidelityReport = serde_json::from_slice(&bytes)?;
        assert_eq!(written, roundtrip);
        assert_eq!(
            tassadar_decompilation_fidelity_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_decompilation_fidelity_report.json")
        );
        Ok(())
    }
}
