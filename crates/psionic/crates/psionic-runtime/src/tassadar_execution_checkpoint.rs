use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Stable post-article profile for checkpointed multi-slice execution.
pub const TASSADAR_EXECUTION_CHECKPOINT_PROFILE_ID: &str =
    "tassadar.internal_compute.checkpoint_resume.v1";
/// Stable checkpoint-family identifier for persisted continuation artifacts.
pub const TASSADAR_EXECUTION_CHECKPOINT_FAMILY_ID: &str = "tassadar.execution_checkpoint.v1";
/// Stable run root for the committed execution-checkpoint bundle.
pub const TASSADAR_EXECUTION_CHECKPOINT_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_execution_checkpoint_v1";
/// Stable runtime-bundle filename under the committed run root.
pub const TASSADAR_EXECUTION_CHECKPOINT_BUNDLE_FILE: &str =
    "tassadar_execution_checkpoint_bundle.json";

const STATE_PAGE_BYTES: usize = 16;
const LONG_LOOP_STATE_BYTES: usize = 16;
const STATE_MACHINE_STATE_BYTES: usize = 24;
const SEARCH_FRONTIER_STATE_BYTES: usize = 24;

/// Long-running workload family exercised by the checkpointed runtime lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCheckpointWorkloadFamily {
    /// Counter-heavy loop kernel with deterministic carried state.
    LongLoopKernel,
    /// Small deterministic state machine with staged accumulation.
    StateMachineAccumulator,
    /// Synthetic search frontier with branch-like carried state.
    SearchFrontierKernel,
}

impl TassadarCheckpointWorkloadFamily {
    /// Returns the stable workload label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LongLoopKernel => "long_loop_kernel",
            Self::StateMachineAccumulator => "state_machine_accumulator",
            Self::SearchFrontierKernel => "search_frontier_kernel",
        }
    }
}

/// Resolution of one execution slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutionSliceStatus {
    /// The slice exhausted its step budget and emitted a new checkpoint.
    PausedForSliceBudget,
    /// The slice completed the workload without emitting another checkpoint.
    Completed,
}

/// Typed refusal for one attempted resume.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarResumeRefusalKind {
    /// The checkpoint was superseded by a later continuation artifact.
    StaleCheckpointSuperseded,
    /// The checkpoint state is larger than the admitted contract.
    OversizedCheckpointState,
    /// The caller requested a different execution profile.
    ProfileMismatch,
    /// The caller requested a different import/effect state digest.
    EffectStateMismatch,
    /// The continuation would exceed the explicit multi-slice budget.
    SliceLimitExceeded,
}

/// Typed refusal emitted before a resume is accepted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarResumeRefusal {
    /// Stable refusal kind.
    pub refusal_kind: TassadarResumeRefusalKind,
    /// Rejected checkpoint identifier when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_id: Option<String>,
    /// Human-readable refusal detail.
    pub detail: String,
}

/// One page-delta summary captured by the checkpoint contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCheckpointMemoryDeltaPage {
    /// Zero-based page index.
    pub page_index: u32,
    /// Digest of the page before the slice ran.
    pub before_digest: String,
    /// Digest of the page after the slice ran.
    pub after_digest: String,
    /// Number of changed bytes inside the page.
    pub changed_bytes: u32,
}

/// First-class persisted execution checkpoint for the checkpointed Tassadar lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutionCheckpoint {
    /// Stable checkpoint identifier.
    pub checkpoint_id: String,
    /// Stable workload family.
    pub workload_family: TassadarCheckpointWorkloadFamily,
    /// Stable program identifier.
    pub program_id: String,
    /// Stable profile identifier.
    pub profile_id: String,
    /// Zero-based slice index that emitted this checkpoint.
    pub slice_index: u32,
    /// Next step index to execute on resume.
    pub next_step_index: u32,
    /// Current call-frame cursor.
    pub call_frame_position: u32,
    /// Current trace cursor.
    pub trace_position: u32,
    /// Serialized state size in bytes.
    pub state_bytes: u32,
    /// Fixed page size used for the dirty-page view.
    pub page_size_bytes: u32,
    /// Digest over the full serialized state.
    pub memory_digest: String,
    /// Full serialized checkpoint state image.
    pub state_blob_hex: String,
    /// Dirty-page deltas observed across the slice.
    pub dirty_pages: Vec<TassadarCheckpointMemoryDeltaPage>,
    /// Stable import/effect-state digest.
    pub effect_state_digest: String,
    /// Stable replay identity for the continuation lineage.
    pub replay_identity: String,
    /// Previous checkpoint superseded by this checkpoint, when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes_checkpoint_id: Option<String>,
    /// Later checkpoint that superseded this one, when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by_checkpoint_id: Option<String>,
    /// Stable digest over the full checkpoint contract.
    pub checkpoint_digest: String,
}

/// Explicit multi-slice execution contract for checkpointed workloads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutionCheckpointContract {
    /// Stable contract identifier.
    pub contract_id: String,
    /// Stable execution profile identifier.
    pub profile_id: String,
    /// Stable checkpoint-family identifier.
    pub checkpoint_family_id: String,
    /// Maximum steps admitted in one slice.
    pub slice_budget_steps: u32,
    /// Maximum number of slices admitted before refusing continuation.
    pub max_total_slices: u32,
    /// Maximum serialized checkpoint state size in bytes.
    pub max_checkpoint_state_bytes: u32,
    /// Maximum dirty pages admitted in one checkpoint.
    pub max_dirty_pages: u32,
    /// Fixed dirty-page size.
    pub page_size_bytes: u32,
    /// Whether exact fresh-vs-resumed parity is required.
    pub require_exact_resume_parity: bool,
    /// Effect-state policy for the admitted workloads.
    pub effect_state_policy: String,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the contract.
    pub contract_digest: String,
}

/// Public receipt for one execution slice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutionSliceReceipt {
    /// Stable execution identifier.
    pub execution_id: String,
    /// Workload family executed by the slice.
    pub workload_family: TassadarCheckpointWorkloadFamily,
    /// Stable program identifier.
    pub program_id: String,
    /// Stable profile identifier.
    pub profile_id: String,
    /// Zero-based slice index.
    pub slice_index: u32,
    /// Final slice status.
    pub status: TassadarExecutionSliceStatus,
    /// Number of steps executed in this slice.
    pub executed_step_count: u32,
    /// Cumulative executed step count after this slice.
    pub cumulative_step_count: u32,
    /// Final call-frame cursor after the slice.
    pub call_frame_position: u32,
    /// Digest over the slice-local trace events.
    pub trace_digest: String,
    /// Digest over the post-slice state image.
    pub memory_digest: String,
    /// Deterministic final result after the slice.
    pub final_result: i64,
    /// Emitted checkpoint when the slice paused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<TassadarExecutionCheckpoint>,
    /// Plain-language detail.
    pub detail: String,
}

/// Runtime case receipt for one checkpointed long-running workload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutionCheckpointCaseReceipt {
    /// Stable case identifier.
    pub case_id: String,
    /// Workload family.
    pub workload_family: TassadarCheckpointWorkloadFamily,
    /// Stable program identifier.
    pub program_id: String,
    /// Number of slices required to complete the workload.
    pub slice_count: u32,
    /// Number of emitted checkpoints.
    pub checkpoint_count: u32,
    /// Maximum dirty pages observed in one checkpoint.
    pub max_dirty_pages_seen: u32,
    /// Total dirty pages emitted across the checkpoint chain.
    pub total_dirty_pages_emitted: u32,
    /// Fresh-run trace digest.
    pub fresh_trace_digest: String,
    /// Resumed-run trace digest.
    pub resumed_trace_digest: String,
    /// Final state digest.
    pub final_memory_digest: String,
    /// Final deterministic result.
    pub final_result: i64,
    /// Whether resumed execution exactly matched the fresh trajectory.
    pub exact_resume_parity: bool,
    /// Latest resumable checkpoint in the chain.
    pub latest_checkpoint: TassadarExecutionCheckpoint,
    /// Full checkpoint lineage emitted during the run.
    pub checkpoint_history: Vec<TassadarExecutionCheckpoint>,
    /// Typed resume refusals exercised against the emitted checkpoints.
    pub refusal_cases: Vec<TassadarResumeRefusal>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language case note.
    pub note: String,
    /// Stable digest over the case receipt.
    pub receipt_digest: String,
}

/// Canonical runtime bundle for the checkpointed execution lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutionCheckpointRuntimeBundle {
    /// Schema version.
    pub schema_version: u16,
    /// Stable runtime bundle identifier.
    pub bundle_id: String,
    /// Shared execution contract.
    pub contract: TassadarExecutionCheckpointContract,
    /// Canonical workload receipts.
    pub case_receipts: Vec<TassadarExecutionCheckpointCaseReceipt>,
    /// Number of workloads with exact fresh-vs-resumed parity.
    pub exact_resume_parity_count: u32,
    /// Number of refusal rows exercised by the bundle.
    pub refusal_case_count: u32,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

/// Returns the canonical checkpointed multi-slice execution contract.
#[must_use]
pub fn tassadar_execution_checkpoint_contract() -> TassadarExecutionCheckpointContract {
    let mut contract = TassadarExecutionCheckpointContract {
        contract_id: String::from("tassadar.execution_checkpoint.contract.v1"),
        profile_id: String::from(TASSADAR_EXECUTION_CHECKPOINT_PROFILE_ID),
        checkpoint_family_id: String::from(TASSADAR_EXECUTION_CHECKPOINT_FAMILY_ID),
        slice_budget_steps: 3,
        max_total_slices: 8,
        max_checkpoint_state_bytes: 64,
        max_dirty_pages: 3,
        page_size_bytes: STATE_PAGE_BYTES as u32,
        require_exact_resume_parity: true,
        effect_state_policy: String::from("deterministic_no_imports_v1"),
        claim_boundary: String::from(
            "this contract covers one deterministic, no-import, checkpointed internal-compute lane over seeded long-loop, staged-state-machine, and search-frontier workloads. It does not imply arbitrary Wasm closure, ambient host effects, or unbounded state without explicit slice and checkpoint budgets",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = stable_digest(
        b"psionic_tassadar_execution_checkpoint_contract|",
        &contract,
    );
    contract
}

/// Executes one checkpointed workload slice from either the initial state or a
/// previously emitted checkpoint.
pub fn execute_tassadar_execution_slice(
    requested_workload_family: TassadarCheckpointWorkloadFamily,
    contract: &TassadarExecutionCheckpointContract,
    checkpoint: Option<&TassadarExecutionCheckpoint>,
    expected_profile_id: Option<&str>,
    expected_effect_state_digest: Option<&str>,
) -> Result<TassadarExecutionSliceReceipt, TassadarResumeRefusal> {
    Ok(execute_tassadar_execution_slice_internal(
        requested_workload_family,
        contract,
        checkpoint,
        expected_profile_id,
        expected_effect_state_digest,
    )?
    .receipt)
}

/// Marks one checkpoint as superseded by a later continuation artifact.
pub fn mark_tassadar_execution_checkpoint_superseded(
    checkpoint: &mut TassadarExecutionCheckpoint,
    superseding_checkpoint: &TassadarExecutionCheckpoint,
) {
    checkpoint.superseded_by_checkpoint_id = Some(superseding_checkpoint.checkpoint_id.clone());
    checkpoint.checkpoint_digest =
        stable_digest(b"psionic_tassadar_execution_checkpoint|", checkpoint);
}

/// Builds the canonical runtime bundle for the checkpointed execution lane.
#[must_use]
pub fn build_tassadar_execution_checkpoint_runtime_bundle()
-> TassadarExecutionCheckpointRuntimeBundle {
    let contract = tassadar_execution_checkpoint_contract();
    let case_receipts = [
        TassadarCheckpointWorkloadFamily::LongLoopKernel,
        TassadarCheckpointWorkloadFamily::StateMachineAccumulator,
        TassadarCheckpointWorkloadFamily::SearchFrontierKernel,
    ]
    .into_iter()
    .map(|workload_family| build_case_receipt(workload_family, &contract))
    .collect::<Vec<_>>();
    let exact_resume_parity_count = case_receipts
        .iter()
        .filter(|receipt| receipt.exact_resume_parity)
        .count() as u32;
    let refusal_case_count = case_receipts
        .iter()
        .map(|receipt| receipt.refusal_cases.len() as u32)
        .sum();
    let mut bundle = TassadarExecutionCheckpointRuntimeBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.execution_checkpoint.runtime_bundle.v1"),
        contract,
        case_receipts,
        exact_resume_parity_count,
        refusal_case_count,
        claim_boundary: String::from(
            "this runtime bundle covers one deterministic, checkpointed multi-slice execution lane over seeded long-running workload families. It demonstrates checkpoint objects, page-delta receipts, and refusal-safe continuation, but it does not widen the current promoted served claim beyond the article-closeout profile",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Execution-checkpoint runtime bundle covers {} workload receipts with exact_resume_parity_count={} and refusal_case_count={}.",
        bundle.case_receipts.len(),
        bundle.exact_resume_parity_count,
        bundle.refusal_case_count,
    );
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_execution_checkpoint_runtime_bundle|",
        &bundle,
    );
    bundle
}

struct InternalSliceOutcome {
    receipt: TassadarExecutionSliceReceipt,
    trace_events: Vec<String>,
}

#[derive(Clone)]
struct SimState {
    workload_family: TassadarCheckpointWorkloadFamily,
    step_index: u32,
    memory: Vec<u8>,
    effect_state_digest: String,
    replay_identity: String,
}

fn build_case_receipt(
    workload_family: TassadarCheckpointWorkloadFamily,
    contract: &TassadarExecutionCheckpointContract,
) -> TassadarExecutionCheckpointCaseReceipt {
    let (fresh_trace_digest, fresh_result, fresh_memory_digest) = run_fresh(workload_family);
    let mut checkpoint_history = Vec::new();
    let mut resumed_trace_events = Vec::new();
    let mut active_checkpoint: Option<TassadarExecutionCheckpoint> = None;
    let mut slice_count = 0_u32;
    let final_slice = loop {
        let expected_effect_state_digest = active_checkpoint.as_ref().map_or_else(
            || canonical_effect_state_digest(workload_family),
            |checkpoint| checkpoint.effect_state_digest.clone(),
        );
        let outcome = execute_tassadar_execution_slice_internal(
            workload_family,
            contract,
            active_checkpoint.as_ref(),
            Some(TASSADAR_EXECUTION_CHECKPOINT_PROFILE_ID),
            Some(expected_effect_state_digest.as_str()),
        )
        .expect("canonical checkpoint workload should execute");
        slice_count = slice_count.saturating_add(1);
        resumed_trace_events.extend(outcome.trace_events);
        match outcome.receipt.status {
            TassadarExecutionSliceStatus::PausedForSliceBudget => {
                let next_checkpoint = outcome
                    .receipt
                    .checkpoint
                    .clone()
                    .expect("paused slices must emit checkpoints");
                if let Some(previous_checkpoint) = checkpoint_history.last_mut() {
                    mark_tassadar_execution_checkpoint_superseded(
                        previous_checkpoint,
                        &next_checkpoint,
                    );
                }
                active_checkpoint = Some(next_checkpoint.clone());
                checkpoint_history.push(next_checkpoint);
            }
            TassadarExecutionSliceStatus::Completed => {
                break outcome.receipt;
            }
        }
    };
    let resumed_trace_digest = digest_string(
        b"psionic_tassadar_execution_checkpoint_trace|",
        &resumed_trace_events.join("\n"),
    );
    let latest_checkpoint = checkpoint_history
        .last()
        .cloned()
        .expect("canonical workloads should emit at least one checkpoint");
    let refusal_cases = build_refusal_cases(workload_family, contract, &checkpoint_history);
    let max_dirty_pages_seen = checkpoint_history
        .iter()
        .map(|checkpoint| checkpoint.dirty_pages.len() as u32)
        .max()
        .unwrap_or(0);
    let total_dirty_pages_emitted = checkpoint_history
        .iter()
        .map(|checkpoint| checkpoint.dirty_pages.len() as u32)
        .sum();
    let exact_resume_parity = final_slice.final_result == fresh_result
        && final_slice.memory_digest == fresh_memory_digest
        && resumed_trace_digest == fresh_trace_digest;
    let mut receipt = TassadarExecutionCheckpointCaseReceipt {
        case_id: String::from(workload_family.as_str()),
        workload_family,
        program_id: canonical_program_id(workload_family),
        slice_count,
        checkpoint_count: checkpoint_history.len() as u32,
        max_dirty_pages_seen,
        total_dirty_pages_emitted,
        fresh_trace_digest,
        resumed_trace_digest,
        final_memory_digest: final_slice.memory_digest,
        final_result: final_slice.final_result,
        exact_resume_parity,
        latest_checkpoint,
        checkpoint_history,
        refusal_cases,
        claim_boundary: String::from(
            "this case receipt covers one seeded long-running workload under deterministic, no-import, checkpointed multi-slice execution. It records exact fresh-vs-resumed parity plus typed refusal rows for unsupported continuation conditions instead of implying arbitrary long-running closure",
        ),
        note: workload_case_note(workload_family).to_string(),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psionic_tassadar_execution_checkpoint_case_receipt|",
        &receipt,
    );
    receipt
}

fn build_refusal_cases(
    workload_family: TassadarCheckpointWorkloadFamily,
    contract: &TassadarExecutionCheckpointContract,
    checkpoint_history: &[TassadarExecutionCheckpoint],
) -> Vec<TassadarResumeRefusal> {
    let mut refusals = Vec::new();
    if let Some(stale_checkpoint) = checkpoint_history.first() {
        refusals.push(
            execute_tassadar_execution_slice(
                workload_family,
                contract,
                Some(stale_checkpoint),
                Some(TASSADAR_EXECUTION_CHECKPOINT_PROFILE_ID),
                Some(stale_checkpoint.effect_state_digest.as_str()),
            )
            .expect_err("superseded checkpoint should refuse"),
        );
    }
    if let Some(latest_checkpoint) = checkpoint_history.last() {
        let mut oversized_contract = contract.clone();
        oversized_contract.max_checkpoint_state_bytes =
            latest_checkpoint.state_bytes.saturating_sub(1);
        refusals.push(
            execute_tassadar_execution_slice(
                workload_family,
                &oversized_contract,
                Some(latest_checkpoint),
                Some(TASSADAR_EXECUTION_CHECKPOINT_PROFILE_ID),
                Some(latest_checkpoint.effect_state_digest.as_str()),
            )
            .expect_err("oversized checkpoint should refuse"),
        );
        refusals.push(
            execute_tassadar_execution_slice(
                workload_family,
                contract,
                Some(latest_checkpoint),
                Some("tassadar.internal_compute.mismatched_profile.v1"),
                Some(latest_checkpoint.effect_state_digest.as_str()),
            )
            .expect_err("profile mismatch should refuse"),
        );
        refusals.push(
            execute_tassadar_execution_slice(
                workload_family,
                contract,
                Some(latest_checkpoint),
                Some(TASSADAR_EXECUTION_CHECKPOINT_PROFILE_ID),
                Some("effect_state://mismatched"),
            )
            .expect_err("effect-state mismatch should refuse"),
        );
    }
    refusals
}

fn execute_tassadar_execution_slice_internal(
    requested_workload_family: TassadarCheckpointWorkloadFamily,
    contract: &TassadarExecutionCheckpointContract,
    checkpoint: Option<&TassadarExecutionCheckpoint>,
    expected_profile_id: Option<&str>,
    expected_effect_state_digest: Option<&str>,
) -> Result<InternalSliceOutcome, TassadarResumeRefusal> {
    let workload_family = checkpoint
        .map(|checkpoint| checkpoint.workload_family)
        .unwrap_or(requested_workload_family);
    let program_id = canonical_program_id(workload_family);
    let mut state = if let Some(checkpoint) = checkpoint {
        validate_resume_request(
            checkpoint,
            contract,
            expected_profile_id,
            expected_effect_state_digest,
        )?;
        state_from_checkpoint(checkpoint)
    } else {
        initial_state(workload_family)
    };
    let slice_index = checkpoint.map_or(0, |checkpoint| checkpoint.slice_index + 1);
    let state_before = state.memory.clone();
    let mut trace_events = Vec::new();
    let start_step_index = state.step_index;
    while state.step_index.saturating_sub(start_step_index) < contract.slice_budget_steps
        && !state_complete(&state)
    {
        advance_state(&mut state);
        trace_events.push(trace_event(&state));
    }
    if !state_complete(&state) && slice_index + 1 >= contract.max_total_slices {
        return Err(TassadarResumeRefusal {
            refusal_kind: TassadarResumeRefusalKind::SliceLimitExceeded,
            checkpoint_id: checkpoint.map(|checkpoint| checkpoint.checkpoint_id.clone()),
            detail: format!(
                "slice `{slice_index}` exhausted the configured multi-slice budget `{}` before workload `{}` completed",
                contract.max_total_slices,
                workload_family.as_str(),
            ),
        });
    }
    let trace_digest = digest_string(
        b"psionic_tassadar_execution_checkpoint_slice_trace|",
        &trace_events.join("\n"),
    );
    let memory_digest = digest_bytes(state.memory.as_slice());
    let final_result = final_result(&state);
    let executed_step_count = state.step_index.saturating_sub(start_step_index);
    let status = if state_complete(&state) {
        TassadarExecutionSliceStatus::Completed
    } else {
        TassadarExecutionSliceStatus::PausedForSliceBudget
    };
    let checkpoint = if status == TassadarExecutionSliceStatus::PausedForSliceBudget {
        Some(build_checkpoint(
            checkpoint,
            &state_before,
            &state,
            contract,
            slice_index,
        ))
    } else {
        None
    };
    let detail = match status {
        TassadarExecutionSliceStatus::PausedForSliceBudget => format!(
            "slice `{slice_index}` paused workload `{}` after {} steps with checkpoint continuation at trace position `{}`",
            workload_family.as_str(),
            executed_step_count,
            state.step_index,
        ),
        TassadarExecutionSliceStatus::Completed => format!(
            "slice `{slice_index}` completed workload `{}` after {} steps without emitting another checkpoint",
            workload_family.as_str(),
            executed_step_count,
        ),
    };
    Ok(InternalSliceOutcome {
        receipt: TassadarExecutionSliceReceipt {
            execution_id: format!(
                "tassadar.execution_slice.{}.{}",
                workload_family.as_str(),
                slice_index
            ),
            workload_family,
            program_id,
            profile_id: String::from(TASSADAR_EXECUTION_CHECKPOINT_PROFILE_ID),
            slice_index,
            status,
            executed_step_count,
            cumulative_step_count: state.step_index,
            call_frame_position: call_frame_position(&state),
            trace_digest,
            memory_digest,
            final_result,
            checkpoint,
            detail,
        },
        trace_events,
    })
}

fn validate_resume_request(
    checkpoint: &TassadarExecutionCheckpoint,
    contract: &TassadarExecutionCheckpointContract,
    expected_profile_id: Option<&str>,
    expected_effect_state_digest: Option<&str>,
) -> Result<(), TassadarResumeRefusal> {
    if let Some(superseded_by_checkpoint_id) = &checkpoint.superseded_by_checkpoint_id {
        return Err(TassadarResumeRefusal {
            refusal_kind: TassadarResumeRefusalKind::StaleCheckpointSuperseded,
            checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
            detail: format!(
                "checkpoint `{}` was superseded by `{superseded_by_checkpoint_id}` and can no longer be resumed",
                checkpoint.checkpoint_id,
            ),
        });
    }
    let expected_profile_id =
        expected_profile_id.unwrap_or(TASSADAR_EXECUTION_CHECKPOINT_PROFILE_ID);
    if checkpoint.profile_id != expected_profile_id {
        return Err(TassadarResumeRefusal {
            refusal_kind: TassadarResumeRefusalKind::ProfileMismatch,
            checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
            detail: format!(
                "checkpoint `{}` targets profile `{}` but the caller requested `{expected_profile_id}`",
                checkpoint.checkpoint_id, checkpoint.profile_id,
            ),
        });
    }
    let expected_effect_state_digest = expected_effect_state_digest
        .map(String::from)
        .unwrap_or_else(|| checkpoint.effect_state_digest.clone());
    if checkpoint.effect_state_digest != expected_effect_state_digest {
        return Err(TassadarResumeRefusal {
            refusal_kind: TassadarResumeRefusalKind::EffectStateMismatch,
            checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
            detail: format!(
                "checkpoint `{}` carries effect-state digest `{}` but the caller requested `{expected_effect_state_digest}`",
                checkpoint.checkpoint_id, checkpoint.effect_state_digest,
            ),
        });
    }
    if checkpoint.state_bytes > contract.max_checkpoint_state_bytes
        || checkpoint.dirty_pages.len() as u32 > contract.max_dirty_pages
    {
        return Err(TassadarResumeRefusal {
            refusal_kind: TassadarResumeRefusalKind::OversizedCheckpointState,
            checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
            detail: format!(
                "checkpoint `{}` needs state_bytes={} and dirty_pages={} but the contract only admits state_bytes<={} and dirty_pages<={}",
                checkpoint.checkpoint_id,
                checkpoint.state_bytes,
                checkpoint.dirty_pages.len(),
                contract.max_checkpoint_state_bytes,
                contract.max_dirty_pages,
            ),
        });
    }
    Ok(())
}

fn build_checkpoint(
    previous_checkpoint: Option<&TassadarExecutionCheckpoint>,
    state_before: &[u8],
    state: &SimState,
    contract: &TassadarExecutionCheckpointContract,
    slice_index: u32,
) -> TassadarExecutionCheckpoint {
    let state_blob_hex = hex::encode(state.memory.as_slice());
    let dirty_pages = memory_delta_pages(
        state_before,
        state.memory.as_slice(),
        contract.page_size_bytes as usize,
    );
    let mut checkpoint = TassadarExecutionCheckpoint {
        checkpoint_id: format!(
            "tassadar.{}.checkpoint.slice_{slice_index:04}",
            state.workload_family.as_str(),
        ),
        workload_family: state.workload_family,
        program_id: canonical_program_id(state.workload_family),
        profile_id: String::from(TASSADAR_EXECUTION_CHECKPOINT_PROFILE_ID),
        slice_index,
        next_step_index: state.step_index,
        call_frame_position: call_frame_position(state),
        trace_position: state.step_index,
        state_bytes: state.memory.len() as u32,
        page_size_bytes: contract.page_size_bytes,
        memory_digest: digest_bytes(state.memory.as_slice()),
        state_blob_hex,
        dirty_pages,
        effect_state_digest: state.effect_state_digest.clone(),
        replay_identity: state.replay_identity.clone(),
        supersedes_checkpoint_id: previous_checkpoint
            .map(|checkpoint| checkpoint.checkpoint_id.clone()),
        superseded_by_checkpoint_id: None,
        checkpoint_digest: String::new(),
    };
    checkpoint.checkpoint_digest =
        stable_digest(b"psionic_tassadar_execution_checkpoint|", &checkpoint);
    checkpoint
}

fn run_fresh(workload_family: TassadarCheckpointWorkloadFamily) -> (String, i64, String) {
    let mut state = initial_state(workload_family);
    let mut trace_events = Vec::new();
    while !state_complete(&state) {
        advance_state(&mut state);
        trace_events.push(trace_event(&state));
    }
    (
        digest_string(
            b"psionic_tassadar_execution_checkpoint_trace|",
            &trace_events.join("\n"),
        ),
        final_result(&state),
        digest_bytes(state.memory.as_slice()),
    )
}

fn initial_state(workload_family: TassadarCheckpointWorkloadFamily) -> SimState {
    let mut memory = match workload_family {
        TassadarCheckpointWorkloadFamily::LongLoopKernel => vec![0_u8; LONG_LOOP_STATE_BYTES],
        TassadarCheckpointWorkloadFamily::StateMachineAccumulator => {
            vec![0_u8; STATE_MACHINE_STATE_BYTES]
        }
        TassadarCheckpointWorkloadFamily::SearchFrontierKernel => {
            vec![0_u8; SEARCH_FRONTIER_STATE_BYTES]
        }
    };
    match workload_family {
        TassadarCheckpointWorkloadFamily::LongLoopKernel => {
            write_u32(&mut memory, 8, 14);
        }
        TassadarCheckpointWorkloadFamily::StateMachineAccumulator => {}
        TassadarCheckpointWorkloadFamily::SearchFrontierKernel => {
            write_u32(&mut memory, 0, 13);
            write_u32(&mut memory, 8, 2);
            write_u32(&mut memory, 12, 100);
        }
    }
    let effect_state_digest = canonical_effect_state_digest(workload_family);
    let replay_identity = digest_string(
        b"psionic_tassadar_execution_checkpoint_replay_identity|",
        &format!(
            "{}|{}|{}",
            canonical_program_id(workload_family),
            TASSADAR_EXECUTION_CHECKPOINT_PROFILE_ID,
            effect_state_digest
        ),
    );
    SimState {
        workload_family,
        step_index: 0,
        memory,
        effect_state_digest,
        replay_identity,
    }
}

fn state_from_checkpoint(checkpoint: &TassadarExecutionCheckpoint) -> SimState {
    SimState {
        workload_family: checkpoint.workload_family,
        step_index: checkpoint.next_step_index,
        memory: hex::decode(&checkpoint.state_blob_hex)
            .expect("checkpoint state blob should always be valid hex"),
        effect_state_digest: checkpoint.effect_state_digest.clone(),
        replay_identity: checkpoint.replay_identity.clone(),
    }
}

fn canonical_program_id(workload_family: TassadarCheckpointWorkloadFamily) -> String {
    format!("tassadar.program.{}.v1", workload_family.as_str())
}

fn canonical_effect_state_digest(workload_family: TassadarCheckpointWorkloadFamily) -> String {
    digest_string(
        b"psionic_tassadar_execution_checkpoint_effect_state|",
        workload_family.as_str(),
    )
}

fn workload_case_note(workload_family: TassadarCheckpointWorkloadFamily) -> &'static str {
    match workload_family {
        TassadarCheckpointWorkloadFamily::LongLoopKernel => {
            "long-loop kernel needs repeated bounded slices to cross the fixed 14-step horizon without losing carried accumulator state"
        }
        TassadarCheckpointWorkloadFamily::StateMachineAccumulator => {
            "state-machine accumulator preserves staged state and parity bits across repeated continuation checkpoints"
        }
        TassadarCheckpointWorkloadFamily::SearchFrontierKernel => {
            "search-frontier kernel carries remaining-work, frontier-width, and best-score state across repeated resumptions instead of flattening the run into one oversized trace"
        }
    }
}

fn state_complete(state: &SimState) -> bool {
    match state.workload_family {
        TassadarCheckpointWorkloadFamily::LongLoopKernel => {
            read_u32(state.memory.as_slice(), 12) != 0
        }
        TassadarCheckpointWorkloadFamily::StateMachineAccumulator => {
            read_u32(state.memory.as_slice(), 20) != 0
        }
        TassadarCheckpointWorkloadFamily::SearchFrontierKernel => {
            read_u32(state.memory.as_slice(), 20) != 0
        }
    }
}

fn call_frame_position(state: &SimState) -> u32 {
    match state.workload_family {
        TassadarCheckpointWorkloadFamily::LongLoopKernel => {
            read_u32(state.memory.as_slice(), 0) % 4
        }
        TassadarCheckpointWorkloadFamily::StateMachineAccumulator => {
            read_u32(state.memory.as_slice(), 0)
        }
        TassadarCheckpointWorkloadFamily::SearchFrontierKernel => {
            read_u32(state.memory.as_slice(), 16)
        }
    }
}

fn final_result(state: &SimState) -> i64 {
    match state.workload_family {
        TassadarCheckpointWorkloadFamily::LongLoopKernel => {
            i64::from(read_u32(state.memory.as_slice(), 4))
        }
        TassadarCheckpointWorkloadFamily::StateMachineAccumulator => {
            i64::from(read_u32(state.memory.as_slice(), 8))
        }
        TassadarCheckpointWorkloadFamily::SearchFrontierKernel => {
            i64::from(read_u32(state.memory.as_slice(), 12))
        }
    }
}

fn advance_state(state: &mut SimState) {
    match state.workload_family {
        TassadarCheckpointWorkloadFamily::LongLoopKernel => advance_long_loop_kernel(state),
        TassadarCheckpointWorkloadFamily::StateMachineAccumulator => {
            advance_state_machine_accumulator(state)
        }
        TassadarCheckpointWorkloadFamily::SearchFrontierKernel => {
            advance_search_frontier_kernel(state)
        }
    }
    state.step_index = state.step_index.saturating_add(1);
}

fn advance_long_loop_kernel(state: &mut SimState) {
    let counter = read_u32(state.memory.as_slice(), 0).saturating_add(1);
    let accumulator = read_u32(state.memory.as_slice(), 4)
        .saturating_add(counter.saturating_mul(3) + (counter % 2));
    let limit = read_u32(state.memory.as_slice(), 8);
    write_u32(&mut state.memory, 0, counter);
    write_u32(&mut state.memory, 4, accumulator);
    if counter >= limit {
        write_u32(&mut state.memory, 12, 1);
    }
}

fn advance_state_machine_accumulator(state: &mut SimState) {
    const VALUES: [u32; 8] = [2, 5, 1, 4, 3, 6, 8, 7];
    let stage = read_u32(state.memory.as_slice(), 0);
    let cursor = read_u32(state.memory.as_slice(), 4);
    if cursor as usize >= VALUES.len() {
        write_u32(&mut state.memory, 20, 1);
        return;
    }
    let value = VALUES[cursor as usize];
    let mut accumulator = read_u32(state.memory.as_slice(), 8);
    let transitions = read_u32(state.memory.as_slice(), 12).saturating_add(1);
    let mut parity = read_u32(state.memory.as_slice(), 16);
    match stage % 3 {
        0 => {
            accumulator = accumulator.saturating_add(value.saturating_mul(2));
            parity ^= value & 1;
        }
        1 => {
            accumulator =
                accumulator.saturating_add(value.saturating_add(parity).saturating_add(1));
            parity = (parity + value) & 1;
        }
        _ => {
            accumulator =
                accumulator.saturating_add(value.saturating_mul(cursor.saturating_add(1)));
        }
    }
    let next_cursor = cursor.saturating_add(1);
    let mut next_stage = stage;
    if next_cursor % 2 == 0 {
        next_stage = next_stage.saturating_add(1);
    }
    write_u32(&mut state.memory, 0, next_stage);
    write_u32(&mut state.memory, 4, next_cursor);
    write_u32(&mut state.memory, 8, accumulator);
    write_u32(&mut state.memory, 12, transitions);
    write_u32(&mut state.memory, 16, parity);
    if next_cursor as usize >= VALUES.len() {
        write_u32(&mut state.memory, 20, 1);
    }
}

fn advance_search_frontier_kernel(state: &mut SimState) {
    let remaining = read_u32(state.memory.as_slice(), 0);
    if remaining == 0 {
        write_u32(&mut state.memory, 20, 1);
        write_u32(&mut state.memory, 8, 0);
        return;
    }
    let visited = read_u32(state.memory.as_slice(), 4).saturating_add(1);
    let mut frontier_width =
        read_u32(state.memory.as_slice(), 8).saturating_add(1 + (state.step_index % 2));
    if state.step_index % 3 == 0 {
        frontier_width = frontier_width.saturating_sub(1);
    }
    let branch_depth = (read_u32(state.memory.as_slice(), 16).saturating_add(1)) % 5;
    let candidate_best = 100_u32
        .saturating_sub(visited.saturating_mul(4))
        .saturating_sub(frontier_width)
        .saturating_sub(branch_depth.saturating_mul(3));
    let best_score = read_u32(state.memory.as_slice(), 12).min(candidate_best);
    let decrement = if state.step_index % 4 == 3 { 2 } else { 1 };
    let next_remaining = remaining.saturating_sub(decrement);
    write_u32(&mut state.memory, 0, next_remaining);
    write_u32(&mut state.memory, 4, visited);
    write_u32(
        &mut state.memory,
        8,
        if next_remaining == 0 {
            0
        } else {
            frontier_width
        },
    );
    write_u32(&mut state.memory, 12, best_score);
    write_u32(&mut state.memory, 16, branch_depth);
    if next_remaining == 0 {
        write_u32(&mut state.memory, 20, 1);
    }
}

fn memory_delta_pages(
    before: &[u8],
    after: &[u8],
    page_size_bytes: usize,
) -> Vec<TassadarCheckpointMemoryDeltaPage> {
    let page_size_bytes = page_size_bytes.max(1);
    let page_count = before.len().max(after.len()).div_ceil(page_size_bytes);
    let mut pages = Vec::new();
    for page_index in 0..page_count {
        let start = page_index * page_size_bytes;
        let end = (start + page_size_bytes).min(before.len().max(after.len()));
        let before_page = page_slice(before, start, end);
        let after_page = page_slice(after, start, end);
        let changed_bytes = before_page
            .iter()
            .zip(after_page.iter())
            .filter(|(left, right)| left != right)
            .count() as u32;
        if changed_bytes == 0 {
            continue;
        }
        pages.push(TassadarCheckpointMemoryDeltaPage {
            page_index: page_index as u32,
            before_digest: digest_bytes(before_page.as_slice()),
            after_digest: digest_bytes(after_page.as_slice()),
            changed_bytes,
        });
    }
    pages
}

fn page_slice(bytes: &[u8], start: usize, end: usize) -> Vec<u8> {
    let mut page = Vec::with_capacity(end.saturating_sub(start));
    for index in start..end {
        page.push(*bytes.get(index).unwrap_or(&0));
    }
    page
}

fn trace_event(state: &SimState) -> String {
    format!(
        "{}|step={}|frame={}|memory={}",
        state.workload_family.as_str(),
        state.step_index,
        call_frame_position(state),
        digest_bytes(state.memory.as_slice()),
    )
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    let mut raw = [0_u8; 4];
    raw.copy_from_slice(&bytes[offset..offset + 4]);
    u32::from_le_bytes(raw)
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_tassadar_execution_checkpoint_bytes|");
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn digest_string(prefix: &[u8], value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
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
        TassadarCheckpointWorkloadFamily, TassadarExecutionSliceStatus, TassadarResumeRefusalKind,
        build_tassadar_execution_checkpoint_runtime_bundle, execute_tassadar_execution_slice,
        mark_tassadar_execution_checkpoint_superseded, tassadar_execution_checkpoint_contract,
    };

    #[test]
    fn checkpointed_execution_preserves_resume_parity_for_long_loop_kernel() {
        let contract = tassadar_execution_checkpoint_contract();
        let first_slice = execute_tassadar_execution_slice(
            TassadarCheckpointWorkloadFamily::LongLoopKernel,
            &contract,
            None,
            None,
            None,
        )
        .expect("first slice");
        assert_eq!(
            first_slice.status,
            TassadarExecutionSliceStatus::PausedForSliceBudget
        );
        let second_slice = execute_tassadar_execution_slice(
            TassadarCheckpointWorkloadFamily::LongLoopKernel,
            &contract,
            first_slice.checkpoint.as_ref(),
            None,
            None,
        )
        .expect("second slice");
        assert_eq!(
            second_slice.status,
            TassadarExecutionSliceStatus::PausedForSliceBudget
        );
        let mut stale_checkpoint = first_slice.checkpoint.expect("checkpoint");
        let latest_checkpoint = second_slice.checkpoint.as_ref().expect("checkpoint");
        mark_tassadar_execution_checkpoint_superseded(&mut stale_checkpoint, latest_checkpoint);
        let err = execute_tassadar_execution_slice(
            TassadarCheckpointWorkloadFamily::LongLoopKernel,
            &contract,
            Some(&stale_checkpoint),
            None,
            None,
        )
        .expect_err("superseded checkpoint should refuse");
        assert_eq!(
            err.refusal_kind,
            TassadarResumeRefusalKind::StaleCheckpointSuperseded
        );
    }

    #[test]
    fn checkpointed_execution_refuses_profile_effect_and_state_mismatches() {
        let contract = tassadar_execution_checkpoint_contract();
        let first_slice = execute_tassadar_execution_slice(
            TassadarCheckpointWorkloadFamily::StateMachineAccumulator,
            &contract,
            None,
            None,
            None,
        )
        .expect("first slice");
        let checkpoint = first_slice.checkpoint.expect("checkpoint");

        let profile_err = execute_tassadar_execution_slice(
            TassadarCheckpointWorkloadFamily::StateMachineAccumulator,
            &contract,
            Some(&checkpoint),
            Some("tassadar.internal_compute.mismatch.v1"),
            None,
        )
        .expect_err("profile mismatch should refuse");
        assert_eq!(
            profile_err.refusal_kind,
            TassadarResumeRefusalKind::ProfileMismatch
        );

        let effect_err = execute_tassadar_execution_slice(
            TassadarCheckpointWorkloadFamily::StateMachineAccumulator,
            &contract,
            Some(&checkpoint),
            None,
            Some("effect_state://wrong"),
        )
        .expect_err("effect mismatch should refuse");
        assert_eq!(
            effect_err.refusal_kind,
            TassadarResumeRefusalKind::EffectStateMismatch
        );

        let mut tight_contract = contract.clone();
        tight_contract.max_checkpoint_state_bytes = checkpoint.state_bytes.saturating_sub(1);
        let size_err = execute_tassadar_execution_slice(
            TassadarCheckpointWorkloadFamily::StateMachineAccumulator,
            &tight_contract,
            Some(&checkpoint),
            None,
            None,
        )
        .expect_err("oversized checkpoint should refuse");
        assert_eq!(
            size_err.refusal_kind,
            TassadarResumeRefusalKind::OversizedCheckpointState
        );
    }

    #[test]
    fn execution_checkpoint_runtime_bundle_is_machine_legible() {
        let bundle = build_tassadar_execution_checkpoint_runtime_bundle();

        assert_eq!(bundle.case_receipts.len(), 3);
        assert_eq!(bundle.exact_resume_parity_count, 3);
        assert_eq!(bundle.refusal_case_count, 12);
        assert!(
            bundle
                .case_receipts
                .iter()
                .all(|receipt| receipt.exact_resume_parity)
        );
        assert!(
            bundle
                .case_receipts
                .iter()
                .all(|receipt| receipt.checkpoint_count >= 2)
        );
    }
}
