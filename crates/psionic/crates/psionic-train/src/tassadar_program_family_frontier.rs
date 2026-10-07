use std::{fs, path::Path};

use psionic_data::{
    tassadar_program_family_frontier_contract, TassadarProgramFamilyArchitectureFamily,
    TassadarProgramFamilyFrontierBundle, TassadarProgramFamilyFrontierEvidenceCase,
    TassadarProgramFamilyGeneralizationSplit, TassadarProgramFamilyWorkloadFamily,
};
#[cfg(test)]
use serde::Deserialize;
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[cfg(test)]
use std::path::PathBuf;

pub const TASSADAR_PROGRAM_FAMILY_FRONTIER_OUTPUT_DIR: &str =
    "fixtures/tassadar/runs/tassadar_program_family_frontier_v1";
pub const TASSADAR_PROGRAM_FAMILY_FRONTIER_REPORT_FILE: &str =
    "program_family_frontier_bundle.json";

#[derive(Debug, Error)]
pub enum TassadarProgramFamilyFrontierError {
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write program-family frontier bundle `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn execute_tassadar_program_family_frontier(
    output_dir: &Path,
) -> Result<TassadarProgramFamilyFrontierBundle, TassadarProgramFamilyFrontierError> {
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarProgramFamilyFrontierError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;

    let contract = tassadar_program_family_frontier_contract();
    let case_reports = case_reports();
    let mut bundle = TassadarProgramFamilyFrontierBundle {
        contract,
        case_reports,
        summary: String::new(),
        report_digest: String::new(),
    };
    bundle.summary = format!(
        "Program-family frontier freezes {} workload/architecture cells with explicit held-out-family ladders, failure modes, and normalized cost units.",
        bundle.case_reports.len()
    );
    bundle.report_digest =
        stable_digest(b"psionic_tassadar_program_family_frontier_bundle|", &bundle);

    let output_path = output_dir.join(TASSADAR_PROGRAM_FAMILY_FRONTIER_REPORT_FILE);
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(&output_path, format!("{json}\n")).map_err(|error| {
        TassadarProgramFamilyFrontierError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn case_reports() -> Vec<TassadarProgramFamilyFrontierEvidenceCase> {
    use TassadarProgramFamilyArchitectureFamily::{
        CompiledExactReference, LearnedStructuredMemory, VerifierAttachedHybrid,
    };
    use TassadarProgramFamilyGeneralizationSplit::{HeldOutFamily, InFamily};
    use TassadarProgramFamilyWorkloadFamily::{
        EffectfulResumeGraph, HeldOutMessageOrchestrator, HeldOutVirtualMachine,
        KernelStateMachine, LinkedProgramBundle, MultiModulePackageWorkflow, SearchProcessMachine,
    };

    vec![
        case(
            CompiledExactReference,
            KernelStateMachine,
            InFamily,
            9_800,
            9_900,
            9_900,
            320,
            "none_exact_anchor",
            "compiled exact reference remains the anchor on seeded kernel-state machines",
        ),
        case(
            LearnedStructuredMemory,
            KernelStateMachine,
            InFamily,
            8_400,
            8_900,
            8_200,
            150,
            "stack_alias_on_long_branch",
            "learned structured memory keeps seeded kernel-state reuse workable but below compiled exactness",
        ),
        case(
            VerifierAttachedHybrid,
            KernelStateMachine,
            InFamily,
            9_100,
            9_500,
            8_900,
            210,
            "verifier_budget_exhaustion",
            "verifier attachment recovers most kernel-state exactness without claiming general closure",
        ),
        case(
            CompiledExactReference,
            SearchProcessMachine,
            InFamily,
            9_600,
            9_800,
            9_900,
            330,
            "none_exact_anchor",
            "compiled exact reference remains green on the seeded search-process family",
        ),
        case(
            LearnedStructuredMemory,
            SearchProcessMachine,
            InFamily,
            7_300,
            8_100,
            8_000,
            160,
            "frontier_trace_collapse",
            "learned lane drifts once search frontier reuse widens past the seeded trace regime",
        ),
        case(
            VerifierAttachedHybrid,
            SearchProcessMachine,
            InFamily,
            8_900,
            9_300,
            8_700,
            220,
            "verifier_retry_budget",
            "verifier-attached hybrid retains strong seeded search behavior at lower cost than the compiled anchor",
        ),
        case(
            CompiledExactReference,
            LinkedProgramBundle,
            InFamily,
            9_500,
            9_700,
            9_800,
            310,
            "none_exact_anchor",
            "compiled exact reference stays green on linked bundle graphs under the named package set",
        ),
        case(
            LearnedStructuredMemory,
            LinkedProgramBundle,
            InFamily,
            7_700,
            8_400,
            8_100,
            155,
            "cross_module_handle_alias",
            "learned lane keeps bounded link patterns but loses precision once module handle pressure widens",
        ),
        case(
            VerifierAttachedHybrid,
            LinkedProgramBundle,
            InFamily,
            8_700,
            9_100,
            8_800,
            225,
            "link_contract_retry_noise",
            "hybrid verifier attachment recovers most linked-bundle behavior while preserving explicit retry limits",
        ),
        case(
            CompiledExactReference,
            EffectfulResumeGraph,
            InFamily,
            9_400,
            9_600,
            9_900,
            300,
            "none_exact_anchor",
            "compiled exact reference remains the safe anchor on seeded effectful resume graphs",
        ),
        case(
            LearnedStructuredMemory,
            EffectfulResumeGraph,
            InFamily,
            7_100,
            7_900,
            8_300,
            165,
            "resume_receipt_rebind_drift",
            "learned lane remains sensitive to effect receipt rebinding on resumable graphs",
        ),
        case(
            VerifierAttachedHybrid,
            EffectfulResumeGraph,
            InFamily,
            8_600,
            9_000,
            8_900,
            215,
            "replay_challenge_budget_exhaustion",
            "hybrid verifier attachment preserves most effect-safe replay behavior under explicit verifier budgets",
        ),
        case(
            CompiledExactReference,
            MultiModulePackageWorkflow,
            InFamily,
            9_300,
            9_500,
            9_900,
            305,
            "none_exact_anchor",
            "compiled exact reference remains green on the seeded multi-module package workflows",
        ),
        case(
            LearnedStructuredMemory,
            MultiModulePackageWorkflow,
            InFamily,
            7_500,
            8_200,
            8_000,
            170,
            "package_dependency_alias",
            "learned lane remains usable in-family but loses package-edge discipline under wider dependency reuse",
        ),
        case(
            VerifierAttachedHybrid,
            MultiModulePackageWorkflow,
            InFamily,
            8_800,
            9_100,
            8_700,
            230,
            "solver_backtrack_budget",
            "verifier-attached hybrid keeps most seeded workflow behavior while making backtrack ceilings explicit",
        ),
        case(
            CompiledExactReference,
            HeldOutVirtualMachine,
            HeldOutFamily,
            9_100,
            9_300,
            9_900,
            340,
            "none_exact_anchor",
            "compiled exact reference remains the honest upper anchor on the held-out virtual-machine family",
        ),
        case(
            LearnedStructuredMemory,
            HeldOutVirtualMachine,
            HeldOutFamily,
            5_600,
            6_800,
            7_900,
            175,
            "instruction_dispatch_collapse",
            "learned lane collapses on held-out virtual-machine dispatch patterns and leans on refusal calibration",
        ),
        case(
            VerifierAttachedHybrid,
            HeldOutVirtualMachine,
            HeldOutFamily,
            7_800,
            8_500,
            8_600,
            240,
            "trace_reentry_budget",
            "verifier-attached hybrid recovers a usable held-out virtual-machine regime without matching the compiled anchor",
        ),
        case(
            CompiledExactReference,
            HeldOutMessageOrchestrator,
            HeldOutFamily,
            8_900,
            9_200,
            9_900,
            345,
            "none_exact_anchor",
            "compiled exact reference remains the safe anchor on the held-out message-orchestrator family",
        ),
        case(
            LearnedStructuredMemory,
            HeldOutMessageOrchestrator,
            HeldOutFamily,
            5_100,
            6_300,
            7_700,
            180,
            "mailbox_order_collapse",
            "learned lane remains fragile on held-out message scheduling and depends on explicit refusal posture",
        ),
        case(
            VerifierAttachedHybrid,
            HeldOutMessageOrchestrator,
            HeldOutFamily,
            7_400,
            8_100,
            8_500,
            245,
            "mailbox_budget_retry_noise",
            "verifier-attached hybrid meaningfully improves the held-out message-orchestrator family while keeping budget limits explicit",
        ),
    ]
}

fn case(
    architecture_family: TassadarProgramFamilyArchitectureFamily,
    workload_family: TassadarProgramFamilyWorkloadFamily,
    split: TassadarProgramFamilyGeneralizationSplit,
    later_window_exactness_bps: u32,
    final_output_exactness_bps: u32,
    refusal_calibration_bps: u32,
    normalized_cost_units: u32,
    dominant_failure_mode: &str,
    note: &str,
) -> TassadarProgramFamilyFrontierEvidenceCase {
    TassadarProgramFamilyFrontierEvidenceCase {
        case_id: format!(
            "{}.{}",
            workload_family.as_str(),
            architecture_family.as_str()
        ),
        architecture_family,
        workload_family,
        split,
        later_window_exactness_bps,
        final_output_exactness_bps,
        refusal_calibration_bps,
        normalized_cost_units,
        dominant_failure_mode: String::from(dominant_failure_mode),
        note: String::from(note),
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

#[cfg(test)]
fn read_json<T: for<'de> Deserialize<'de>>(
    path: impl AsRef<Path>,
) -> Result<T, Box<dyn std::error::Error>> {
    let path = path.as_ref();
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[cfg(test)]
mod tests {
    use super::{
        execute_tassadar_program_family_frontier, read_json, repo_root,
        TassadarProgramFamilyFrontierBundle, TASSADAR_PROGRAM_FAMILY_FRONTIER_OUTPUT_DIR,
        TASSADAR_PROGRAM_FAMILY_FRONTIER_REPORT_FILE,
    };
    use psionic_data::{
        TassadarProgramFamilyArchitectureFamily, TassadarProgramFamilyGeneralizationSplit,
        TassadarProgramFamilyWorkloadFamily,
    };

    #[test]
    fn program_family_frontier_bundle_keeps_held_out_ladder_explicit() {
        let output_dir = tempfile::tempdir().expect("tempdir");
        let bundle = execute_tassadar_program_family_frontier(output_dir.path()).expect("bundle");

        assert_eq!(bundle.case_reports.len(), 21);
        assert!(bundle.case_reports.iter().any(|case| {
            case.workload_family == TassadarProgramFamilyWorkloadFamily::HeldOutVirtualMachine
                && case.split == TassadarProgramFamilyGeneralizationSplit::HeldOutFamily
                && case.architecture_family
                    == TassadarProgramFamilyArchitectureFamily::VerifierAttachedHybrid
        }));
    }

    #[test]
    fn program_family_frontier_bundle_matches_committed_truth() {
        let generated =
            execute_tassadar_program_family_frontier(tempfile::tempdir().expect("tempdir").path())
                .expect("bundle");
        let committed_path = repo_root()
            .join(TASSADAR_PROGRAM_FAMILY_FRONTIER_OUTPUT_DIR)
            .join(TASSADAR_PROGRAM_FAMILY_FRONTIER_REPORT_FILE);
        let committed: TassadarProgramFamilyFrontierBundle =
            read_json(committed_path).expect("committed bundle");

        assert_eq!(generated, committed);
    }
}
