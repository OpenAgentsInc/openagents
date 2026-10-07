use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    TassadarCheckpointWorkloadFamily as WorkloadFamily, TassadarExecutionCheckpointCaseReceipt,
    TassadarResumeRefusalKind, build_tassadar_execution_checkpoint_runtime_bundle,
};

/// Stable process-oriented internal-compute profile built above checkpointed execution.
pub const TASSADAR_PROCESS_OBJECT_PROFILE_ID: &str = "tassadar.internal_compute.process_objects.v1";
/// Stable family identifier for durable process snapshots.
pub const TASSADAR_PROCESS_SNAPSHOT_FAMILY_ID: &str = "tassadar.process_snapshot.v1";
/// Stable family identifier for durable process tapes.
pub const TASSADAR_PROCESS_TAPE_FAMILY_ID: &str = "tassadar.process_tape.v1";
/// Stable family identifier for durable process work queues.
pub const TASSADAR_PROCESS_WORK_QUEUE_FAMILY_ID: &str = "tassadar.process_work_queue.v1";
/// Stable run root for the committed process-object bundle.
pub const TASSADAR_PROCESS_OBJECT_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_process_objects_v1";
/// Stable runtime-bundle filename under the committed run root.
pub const TASSADAR_PROCESS_OBJECT_BUNDLE_FILE: &str = "tassadar_process_object_bundle.json";

/// High-level tape event captured by one durable process object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarProcessTapeEntryKind {
    SliceCheckpoint,
    FrontierDrain,
    QueueReady,
}

/// Typed work-queue item carried by one durable process object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProcessWorkQueueEntry {
    pub entry_id: String,
    pub operation_id: String,
    pub payload_digest: String,
    pub enqueued_at_step: u32,
    pub priority: u8,
    pub note: String,
}

/// One durable tape entry captured from the checkpoint lineage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProcessTapeEntry {
    pub entry_id: String,
    pub kind: TassadarProcessTapeEntryKind,
    pub source_checkpoint_id: String,
    pub source_slice_index: u32,
    pub next_step_index: u32,
    pub state_digest: String,
    pub note: String,
}

/// First-class durable snapshot object for one process family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProcessSnapshotObject {
    pub process_id: String,
    pub workload_family: WorkloadFamily,
    pub profile_id: String,
    pub snapshot_family_id: String,
    pub checkpoint_id: String,
    pub next_step_index: u32,
    pub state_bytes: u32,
    pub memory_digest: String,
    pub replay_identity: String,
    pub tape_head_position: u32,
    pub work_queue_depth: u32,
    pub snapshot_digest: String,
}

/// First-class durable tape object for one process family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProcessTapeObject {
    pub process_id: String,
    pub tape_family_id: String,
    pub latest_checkpoint_id: String,
    pub head_position: u32,
    pub entry_count: u32,
    pub entries: Vec<TassadarProcessTapeEntry>,
    pub tape_digest: String,
}

/// First-class durable work-queue object for one process family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProcessWorkQueueObject {
    pub process_id: String,
    pub queue_family_id: String,
    pub latest_checkpoint_id: String,
    pub pending_entry_count: u32,
    pub drained_entry_count: u32,
    pub entries: Vec<TassadarProcessWorkQueueEntry>,
    pub queue_digest: String,
}

/// Typed refusal for malformed or stale process objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarProcessObjectRefusalKind {
    StaleProcessSnapshot,
    TapeCursorOutOfRange,
    WorkQueueProfileMismatch,
}

/// One explicit refusal case over the durable process objects.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProcessObjectRefusal {
    pub refusal_kind: TassadarProcessObjectRefusalKind,
    pub object_ref: String,
    pub detail: String,
}

/// Runtime case receipt for one durable process family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProcessObjectCaseReceipt {
    pub case_id: String,
    pub process_id: String,
    pub workload_family: WorkloadFamily,
    pub exact_process_parity: bool,
    pub snapshot: TassadarProcessSnapshotObject,
    pub tape: TassadarProcessTapeObject,
    pub work_queue: TassadarProcessWorkQueueObject,
    pub refusal_cases: Vec<TassadarProcessObjectRefusal>,
    pub note: String,
    pub receipt_digest: String,
}

/// Canonical runtime bundle for the process-object family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProcessObjectRuntimeBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub profile_id: String,
    pub snapshot_family_id: String,
    pub tape_family_id: String,
    pub work_queue_family_id: String,
    pub case_receipts: Vec<TassadarProcessObjectCaseReceipt>,
    pub exact_process_case_count: u32,
    pub refusal_case_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

/// Builds the committed durable process-object bundle from the checkpoint lane.
#[must_use]
pub fn build_tassadar_process_object_runtime_bundle() -> TassadarProcessObjectRuntimeBundle {
    let checkpoint_bundle = build_tassadar_execution_checkpoint_runtime_bundle();
    let case_receipts = checkpoint_bundle
        .case_receipts
        .iter()
        .map(build_process_case_receipt)
        .collect::<Vec<_>>();
    let exact_process_case_count = case_receipts
        .iter()
        .filter(|case| case.exact_process_parity)
        .count() as u32;
    let refusal_case_count = case_receipts
        .iter()
        .map(|case| case.refusal_cases.len() as u32)
        .sum();
    let mut bundle = TassadarProcessObjectRuntimeBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.process_object.bundle.v1"),
        profile_id: String::from(TASSADAR_PROCESS_OBJECT_PROFILE_ID),
        snapshot_family_id: String::from(TASSADAR_PROCESS_SNAPSHOT_FAMILY_ID),
        tape_family_id: String::from(TASSADAR_PROCESS_TAPE_FAMILY_ID),
        work_queue_family_id: String::from(TASSADAR_PROCESS_WORK_QUEUE_FAMILY_ID),
        case_receipts,
        exact_process_case_count,
        refusal_case_count,
        claim_boundary: String::from(
            "this runtime bundle freezes one durable process-object family over the committed checkpointed workloads only. It keeps snapshots, tapes, work queues, and malformed-or-stale continuation-object refusals explicit instead of implying arbitrary process semantics, async effects, or general served internal compute",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Process-object bundle covers {} process rows with exact_process_cases={} and refusal_cases={}.",
        bundle.case_receipts.len(),
        bundle.exact_process_case_count,
        bundle.refusal_case_count,
    );
    bundle.bundle_digest = stable_digest(b"psionic_tassadar_process_object_bundle|", &bundle);
    bundle
}

fn build_process_case_receipt(
    case: &TassadarExecutionCheckpointCaseReceipt,
) -> TassadarProcessObjectCaseReceipt {
    let process_id = format!("tassadar.process.{}.v1", case.case_id);
    let tape_entries = case
        .checkpoint_history
        .iter()
        .enumerate()
        .map(|(index, checkpoint)| TassadarProcessTapeEntry {
            entry_id: format!("{}::tape::{index:02}", process_id),
            kind: tape_entry_kind(case.workload_family, index),
            source_checkpoint_id: checkpoint.checkpoint_id.clone(),
            source_slice_index: checkpoint.slice_index,
            next_step_index: checkpoint.next_step_index,
            state_digest: checkpoint.memory_digest.clone(),
            note: format!(
                "process tape entry captures slice {} for `{}`",
                checkpoint.slice_index,
                case.workload_family.as_str()
            ),
        })
        .collect::<Vec<_>>();
    let work_queue_entries = work_queue_entries(case);
    let latest_checkpoint = &case.latest_checkpoint;
    let mut snapshot = TassadarProcessSnapshotObject {
        process_id: process_id.clone(),
        workload_family: case.workload_family,
        profile_id: String::from(TASSADAR_PROCESS_OBJECT_PROFILE_ID),
        snapshot_family_id: String::from(TASSADAR_PROCESS_SNAPSHOT_FAMILY_ID),
        checkpoint_id: latest_checkpoint.checkpoint_id.clone(),
        next_step_index: latest_checkpoint.next_step_index,
        state_bytes: latest_checkpoint.state_bytes,
        memory_digest: latest_checkpoint.memory_digest.clone(),
        replay_identity: latest_checkpoint.replay_identity.clone(),
        tape_head_position: tape_entries.len() as u32,
        work_queue_depth: work_queue_entries.len() as u32,
        snapshot_digest: String::new(),
    };
    snapshot.snapshot_digest = stable_digest(b"psionic_tassadar_process_snapshot|", &snapshot);

    let mut tape = TassadarProcessTapeObject {
        process_id: process_id.clone(),
        tape_family_id: String::from(TASSADAR_PROCESS_TAPE_FAMILY_ID),
        latest_checkpoint_id: latest_checkpoint.checkpoint_id.clone(),
        head_position: tape_entries.len() as u32,
        entry_count: tape_entries.len() as u32,
        entries: tape_entries,
        tape_digest: String::new(),
    };
    tape.tape_digest = stable_digest(b"psionic_tassadar_process_tape|", &tape);

    let mut work_queue = TassadarProcessWorkQueueObject {
        process_id: process_id.clone(),
        queue_family_id: String::from(TASSADAR_PROCESS_WORK_QUEUE_FAMILY_ID),
        latest_checkpoint_id: latest_checkpoint.checkpoint_id.clone(),
        pending_entry_count: work_queue_entries.len() as u32,
        drained_entry_count: case.slice_count.saturating_sub(1),
        entries: work_queue_entries,
        queue_digest: String::new(),
    };
    work_queue.queue_digest = stable_digest(b"psionic_tassadar_process_work_queue|", &work_queue);

    let refusal_cases = build_process_object_refusals(case, &process_id, &tape, &work_queue);
    let mut receipt = TassadarProcessObjectCaseReceipt {
        case_id: case.case_id.clone(),
        process_id,
        workload_family: case.workload_family,
        exact_process_parity: case.exact_resume_parity,
        snapshot,
        tape,
        work_queue,
        refusal_cases,
        note: format!(
            "durable process objects preserve checkpointed continuation for `{}` while keeping stale or malformed continuation objects explicit",
            case.case_id
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(b"psionic_tassadar_process_case_receipt|", &receipt);
    receipt
}

fn build_process_object_refusals(
    case: &TassadarExecutionCheckpointCaseReceipt,
    process_id: &str,
    tape: &TassadarProcessTapeObject,
    work_queue: &TassadarProcessWorkQueueObject,
) -> Vec<TassadarProcessObjectRefusal> {
    let stale_snapshot_ref = case
        .checkpoint_history
        .first()
        .map(|checkpoint| checkpoint.checkpoint_id.clone())
        .unwrap_or_else(|| case.latest_checkpoint.checkpoint_id.clone());
    let mut refusals = vec![
        TassadarProcessObjectRefusal {
            refusal_kind: TassadarProcessObjectRefusalKind::StaleProcessSnapshot,
            object_ref: stale_snapshot_ref,
            detail: String::from(
                "older process snapshots remain stale once a later durable continuation object supersedes them",
            ),
        },
        TassadarProcessObjectRefusal {
            refusal_kind: TassadarProcessObjectRefusalKind::TapeCursorOutOfRange,
            object_ref: format!("{process_id}#tape"),
            detail: format!(
                "process tape for `{}` refuses cursors beyond head_position={}",
                case.case_id, tape.head_position
            ),
        },
        TassadarProcessObjectRefusal {
            refusal_kind: TassadarProcessObjectRefusalKind::WorkQueueProfileMismatch,
            object_ref: format!("{process_id}#queue"),
            detail: format!(
                "work queue for `{}` remains profile-bound to `{}` and refuses mismatched profile resumes",
                case.case_id, TASSADAR_PROCESS_OBJECT_PROFILE_ID
            ),
        },
    ];
    if case
        .refusal_cases
        .iter()
        .any(|refusal| refusal.refusal_kind == TassadarResumeRefusalKind::StaleCheckpointSuperseded)
    {
        refusals[0].detail.push_str(
            " The underlying execution-checkpoint lineage already exercises stale-checkpoint refusal truth.",
        );
    }
    if work_queue.pending_entry_count == 0 {
        refusals[2]
            .detail
            .push_str(" Empty queues are kept out of scope for this bounded family.");
    }
    refusals
}

fn tape_entry_kind(workload_family: WorkloadFamily, index: usize) -> TassadarProcessTapeEntryKind {
    match (workload_family, index == 0) {
        (WorkloadFamily::SearchFrontierKernel, _) => TassadarProcessTapeEntryKind::FrontierDrain,
        (_, true) => TassadarProcessTapeEntryKind::SliceCheckpoint,
        _ => TassadarProcessTapeEntryKind::QueueReady,
    }
}

fn work_queue_entries(
    case: &TassadarExecutionCheckpointCaseReceipt,
) -> Vec<TassadarProcessWorkQueueEntry> {
    match case.workload_family {
        WorkloadFamily::LongLoopKernel => vec![queue_entry(
            case,
            "resume_loop_slice",
            case.latest_checkpoint.next_step_index,
            1,
            "resume the next bounded loop slice from the durable process snapshot",
        )],
        WorkloadFamily::StateMachineAccumulator => vec![
            queue_entry(
                case,
                "load_state_stage",
                case.latest_checkpoint.next_step_index,
                1,
                "load the staged accumulator state from the durable snapshot",
            ),
            queue_entry(
                case,
                "emit_final_sum",
                case.latest_checkpoint.next_step_index.saturating_add(1),
                2,
                "emit the final deterministic accumulator value after queue drain",
            ),
        ],
        WorkloadFamily::SearchFrontierKernel => vec![
            queue_entry(
                case,
                "expand_frontier_head",
                case.latest_checkpoint.next_step_index,
                1,
                "expand the frontier head selected by the durable queue",
            ),
            queue_entry(
                case,
                "drain_terminal_goal",
                case.latest_checkpoint.next_step_index.saturating_add(1),
                2,
                "drain the terminal goal entry after deterministic frontier replay",
            ),
        ],
    }
}

fn queue_entry(
    case: &TassadarExecutionCheckpointCaseReceipt,
    operation_id: &str,
    enqueued_at_step: u32,
    priority: u8,
    note: &str,
) -> TassadarProcessWorkQueueEntry {
    let payload_digest = stable_digest(
        b"psionic_tassadar_process_work_queue_payload|",
        &(
            case.case_id.as_str(),
            operation_id,
            enqueued_at_step,
            priority,
            note,
        ),
    );
    TassadarProcessWorkQueueEntry {
        entry_id: format!("{}::{operation_id}", case.case_id),
        operation_id: String::from(operation_id),
        payload_digest,
        enqueued_at_step,
        priority,
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
mod tests {
    use super::{
        TASSADAR_PROCESS_OBJECT_PROFILE_ID, TASSADAR_PROCESS_SNAPSHOT_FAMILY_ID,
        TASSADAR_PROCESS_TAPE_FAMILY_ID, TASSADAR_PROCESS_WORK_QUEUE_FAMILY_ID,
        TassadarProcessObjectRefusalKind, build_tassadar_process_object_runtime_bundle,
    };

    #[test]
    fn process_object_bundle_keeps_snapshot_tape_and_queue_families_explicit() {
        let bundle = build_tassadar_process_object_runtime_bundle();

        assert_eq!(bundle.profile_id, TASSADAR_PROCESS_OBJECT_PROFILE_ID);
        assert_eq!(
            bundle.snapshot_family_id,
            TASSADAR_PROCESS_SNAPSHOT_FAMILY_ID
        );
        assert_eq!(bundle.tape_family_id, TASSADAR_PROCESS_TAPE_FAMILY_ID);
        assert_eq!(
            bundle.work_queue_family_id,
            TASSADAR_PROCESS_WORK_QUEUE_FAMILY_ID
        );
        assert_eq!(bundle.exact_process_case_count, 3);
        assert_eq!(bundle.case_receipts.len(), 3);
        assert!(
            bundle
                .case_receipts
                .iter()
                .all(|case| !case.refusal_cases.is_empty())
        );
        assert!(bundle.case_receipts.iter().any(|case| {
            case.refusal_cases.iter().any(|refusal| {
                refusal.refusal_kind == TassadarProcessObjectRefusalKind::StaleProcessSnapshot
            })
        }));
    }

    #[test]
    fn process_snapshot_tape_and_queue_stay_consistent() {
        let bundle = build_tassadar_process_object_runtime_bundle();

        for case in &bundle.case_receipts {
            assert_eq!(case.snapshot.tape_head_position, case.tape.entry_count);
            assert_eq!(
                case.snapshot.work_queue_depth,
                case.work_queue.pending_entry_count
            );
            assert_eq!(case.snapshot.checkpoint_id, case.tape.latest_checkpoint_id);
            assert_eq!(
                case.snapshot.checkpoint_id,
                case.work_queue.latest_checkpoint_id
            );
        }
    }

    #[test]
    fn process_work_queue_operations_stay_profile_bound_and_replay_ordered() {
        let bundle = build_tassadar_process_object_runtime_bundle();
        let search_case = bundle
            .case_receipts
            .iter()
            .find(|case| case.case_id == "search_frontier_kernel")
            .expect("search frontier case");
        let state_case = bundle
            .case_receipts
            .iter()
            .find(|case| case.case_id == "state_machine_accumulator")
            .expect("state machine case");

        assert_eq!(
            search_case
                .work_queue
                .entries
                .iter()
                .map(|entry| entry.operation_id.as_str())
                .collect::<Vec<_>>(),
            vec!["expand_frontier_head", "drain_terminal_goal"]
        );
        assert_eq!(
            state_case
                .work_queue
                .entries
                .iter()
                .map(|entry| entry.operation_id.as_str())
                .collect::<Vec<_>>(),
            vec!["load_state_stage", "emit_final_sum"]
        );
    }
}
