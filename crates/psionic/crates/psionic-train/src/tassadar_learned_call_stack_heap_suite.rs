use std::{fs, path::Path};

use psionic_data::{
    TassadarCallStackHeapGeneralizationSplit, TassadarCallStackHeapModelVariant,
    TassadarCallStackHeapWorkloadFamily, TassadarLearnedCallStackHeapEvidenceCase,
    TassadarLearnedCallStackHeapSuiteBundle, tassadar_learned_call_stack_heap_suite_contract,
};
#[cfg(test)]
use serde::Deserialize;
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[cfg(test)]
use std::path::PathBuf;

pub const TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_OUTPUT_DIR: &str =
    "fixtures/tassadar/runs/tassadar_learned_call_stack_heap_suite_v1";
pub const TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_REPORT_FILE: &str =
    "learned_call_stack_heap_suite_bundle.json";

#[derive(Debug, Error)]
pub enum TassadarLearnedCallStackHeapSuiteError {
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write learned call-stack/heap suite bundle `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn execute_tassadar_learned_call_stack_heap_suite(
    output_dir: &Path,
) -> Result<TassadarLearnedCallStackHeapSuiteBundle, TassadarLearnedCallStackHeapSuiteError> {
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarLearnedCallStackHeapSuiteError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;

    let contract = tassadar_learned_call_stack_heap_suite_contract();
    let case_reports = case_reports();
    let mut bundle = TassadarLearnedCallStackHeapSuiteBundle {
        contract,
        case_reports,
        summary: String::new(),
        report_digest: String::new(),
    };
    bundle.summary = format!(
        "Learned call-stack/heap suite freezes {} workload/variant cells with explicit held-out families and later-window exactness gaps.",
        bundle.case_reports.len()
    );
    bundle.report_digest = stable_digest(
        b"psionic_tassadar_learned_call_stack_heap_suite_bundle|",
        &bundle,
    );

    let output_path = output_dir.join(TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_REPORT_FILE);
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(&output_path, format!("{json}\n")).map_err(|error| {
        TassadarLearnedCallStackHeapSuiteError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn case_reports() -> Vec<TassadarLearnedCallStackHeapEvidenceCase> {
    vec![
        case(
            TassadarCallStackHeapModelVariant::BaselineTransformer,
            TassadarCallStackHeapWorkloadFamily::RecursiveEvaluator,
            TassadarCallStackHeapGeneralizationSplit::InFamily,
            7_700,
            8_500,
            7_800,
            12,
            32,
            "deep_frame_alias",
            "baseline handles shallow recursion but drifts on deeper frame reuse",
        ),
        case(
            TassadarCallStackHeapModelVariant::StructuredMemory,
            TassadarCallStackHeapWorkloadFamily::RecursiveEvaluator,
            TassadarCallStackHeapGeneralizationSplit::InFamily,
            8_700,
            9_200,
            8_600,
            24,
            64,
            "residual_frame_noise",
            "structured memory preserves deeper recursive frame reuse on the seeded evaluator family",
        ),
        case(
            TassadarCallStackHeapModelVariant::BaselineTransformer,
            TassadarCallStackHeapWorkloadFamily::ParserFrameMachine,
            TassadarCallStackHeapGeneralizationSplit::InFamily,
            7_900,
            8_600,
            8_000,
            14,
            24,
            "frame_pop_order_drift",
            "baseline parser frames remain usable in-family but lose later-window stack discipline",
        ),
        case(
            TassadarCallStackHeapModelVariant::StructuredMemory,
            TassadarCallStackHeapWorkloadFamily::ParserFrameMachine,
            TassadarCallStackHeapGeneralizationSplit::InFamily,
            8_900,
            9_300,
            8_800,
            28,
            48,
            "late_reduce_noise",
            "structured memory improves parser-frame stability without promoting broad parser closure",
        ),
        case(
            TassadarCallStackHeapModelVariant::BaselineTransformer,
            TassadarCallStackHeapWorkloadFamily::BumpAllocatorHeap,
            TassadarCallStackHeapGeneralizationSplit::InFamily,
            7_400,
            8_200,
            7_700,
            8,
            96,
            "heap_cursor_alias",
            "baseline bump allocation keeps shallow heap state but loses later-window cursor precision",
        ),
        case(
            TassadarCallStackHeapModelVariant::StructuredMemory,
            TassadarCallStackHeapWorkloadFamily::BumpAllocatorHeap,
            TassadarCallStackHeapGeneralizationSplit::InFamily,
            8_600,
            9_100,
            8_500,
            12,
            224,
            "allocation_pointer_noise",
            "structured memory widens seeded heap-cell coverage on bump allocation",
        ),
        case(
            TassadarCallStackHeapModelVariant::BaselineTransformer,
            TassadarCallStackHeapWorkloadFamily::FreeListAllocatorHeap,
            TassadarCallStackHeapGeneralizationSplit::InFamily,
            6_900,
            7_800,
            7_600,
            7,
            128,
            "free_list_alias",
            "baseline free-list behavior stays fragile once heap reuse widens",
        ),
        case(
            TassadarCallStackHeapModelVariant::StructuredMemory,
            TassadarCallStackHeapWorkloadFamily::FreeListAllocatorHeap,
            TassadarCallStackHeapGeneralizationSplit::InFamily,
            8_200,
            8_900,
            8_400,
            12,
            256,
            "recycle_slot_noise",
            "structured memory keeps allocator reuse more stable on the seeded free-list family",
        ),
        case(
            TassadarCallStackHeapModelVariant::BaselineTransformer,
            TassadarCallStackHeapWorkloadFamily::ResumableProcessHeap,
            TassadarCallStackHeapGeneralizationSplit::InFamily,
            6_400,
            7_300,
            7_900,
            10,
            160,
            "resume_heap_state_drift",
            "baseline process-style heap handling is workable but later-window resume state still drifts",
        ),
        case(
            TassadarCallStackHeapModelVariant::StructuredMemory,
            TassadarCallStackHeapWorkloadFamily::ResumableProcessHeap,
            TassadarCallStackHeapGeneralizationSplit::InFamily,
            7_800,
            8_600,
            8_300,
            20,
            320,
            "checkpoint_rebind_noise",
            "structured memory improves resumable process heap stability while keeping refusal posture explicit",
        ),
        case(
            TassadarCallStackHeapModelVariant::BaselineTransformer,
            TassadarCallStackHeapWorkloadFamily::HeldOutContinuationMachine,
            TassadarCallStackHeapGeneralizationSplit::HeldOutFamily,
            4_200,
            6_100,
            7_200,
            6,
            48,
            "held_out_continuation_collapse",
            "baseline later-window generalization collapses on the held-out continuation machine family",
        ),
        case(
            TassadarCallStackHeapModelVariant::StructuredMemory,
            TassadarCallStackHeapWorkloadFamily::HeldOutContinuationMachine,
            TassadarCallStackHeapGeneralizationSplit::HeldOutFamily,
            6_900,
            8_100,
            8_100,
            18,
            96,
            "continuation_reentry_noise",
            "structured memory recovers a usable held-out continuation regime without closing the gap entirely",
        ),
        case(
            TassadarCallStackHeapModelVariant::BaselineTransformer,
            TassadarCallStackHeapWorkloadFamily::HeldOutAllocatorScheduler,
            TassadarCallStackHeapGeneralizationSplit::HeldOutFamily,
            3_900,
            5_700,
            7_000,
            5,
            64,
            "allocator_scheduler_collapse",
            "baseline held-out allocator scheduling remains fragile and relies on refusal calibration",
        ),
        case(
            TassadarCallStackHeapModelVariant::StructuredMemory,
            TassadarCallStackHeapWorkloadFamily::HeldOutAllocatorScheduler,
            TassadarCallStackHeapGeneralizationSplit::HeldOutFamily,
            6_500,
            7_800,
            7_900,
            16,
            144,
            "scheduler_heap_noise",
            "structured memory improves held-out allocator scheduling but still stops short of broad process ownership",
        ),
    ]
}

fn case(
    model_variant: TassadarCallStackHeapModelVariant,
    workload_family: TassadarCallStackHeapWorkloadFamily,
    split: TassadarCallStackHeapGeneralizationSplit,
    later_window_exactness_bps: u32,
    final_output_exactness_bps: u32,
    refusal_calibration_bps: u32,
    max_call_depth: u32,
    max_heap_cells: u32,
    dominant_failure_mode: &str,
    note: &str,
) -> TassadarLearnedCallStackHeapEvidenceCase {
    TassadarLearnedCallStackHeapEvidenceCase {
        case_id: format!("{}.{}", workload_family.as_str(), model_variant.as_str()),
        model_variant,
        workload_family,
        split,
        later_window_exactness_bps,
        final_output_exactness_bps,
        refusal_calibration_bps,
        max_call_depth,
        max_heap_cells,
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
        TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_OUTPUT_DIR,
        TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_REPORT_FILE,
        TassadarLearnedCallStackHeapSuiteBundle, execute_tassadar_learned_call_stack_heap_suite,
        read_json, repo_root,
    };
    use psionic_data::{
        TassadarCallStackHeapGeneralizationSplit, TassadarCallStackHeapModelVariant,
        TassadarCallStackHeapWorkloadFamily,
    };

    #[test]
    fn learned_call_stack_heap_suite_bundle_keeps_held_out_families_explicit() {
        let output_dir = tempfile::tempdir().expect("tempdir");
        let bundle =
            execute_tassadar_learned_call_stack_heap_suite(output_dir.path()).expect("bundle");

        assert_eq!(bundle.case_reports.len(), 14);
        assert!(bundle.case_reports.iter().any(|case| {
            case.workload_family == TassadarCallStackHeapWorkloadFamily::HeldOutContinuationMachine
                && case.split == TassadarCallStackHeapGeneralizationSplit::HeldOutFamily
                && case.model_variant == TassadarCallStackHeapModelVariant::StructuredMemory
        }));
    }

    #[test]
    fn learned_call_stack_heap_suite_bundle_matches_committed_truth() {
        let generated = execute_tassadar_learned_call_stack_heap_suite(
            tempfile::tempdir().expect("tempdir").path(),
        )
        .expect("bundle");
        let committed_path = repo_root()
            .join(TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_OUTPUT_DIR)
            .join(TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_REPORT_FILE);
        let committed: TassadarLearnedCallStackHeapSuiteBundle =
            read_json(committed_path).expect("committed bundle");

        assert_eq!(generated, committed);
    }
}
