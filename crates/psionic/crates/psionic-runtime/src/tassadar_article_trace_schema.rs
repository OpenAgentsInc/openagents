use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::TassadarTraceAbi;

pub const TASSADAR_ARTICLE_TRACE_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_ARTICLE_TRACE_SCHEMA_ID: &str = "tassadar.article_trace_schema.v1";

/// Stable boundary markers used by the canonical article trace tokenization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleTraceBoundaryMarker {
    Bos,
    ProgramPrompt,
    TraceStart,
    Halt,
    Eos,
}

impl TassadarArticleTraceBoundaryMarker {
    /// Returns the stable boundary-marker label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Bos => "bos",
            Self::ProgramPrompt => "program_prompt",
            Self::TraceStart => "trace_start",
            Self::Halt => "halt",
            Self::Eos => "eos",
        }
    }
}

/// Stable channel families carried by the canonical article trace domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleTraceChannelKind {
    PromptScalar,
    PromptInstructionStream,
    StepScalar,
    StepInstruction,
    StepEvent,
    StackChannel,
    LocalsChannel,
    MemoryChannel,
    HaltMarker,
}

/// Runtime-owned boundary contract for the canonical article trace domain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTraceBoundaryContract {
    pub boundary_id: String,
    pub schema_version: u16,
    pub required_markers: Vec<TassadarArticleTraceBoundaryMarker>,
    pub prompt_terminator: TassadarArticleTraceBoundaryMarker,
    pub target_begins_after_prompt_terminator: bool,
    pub trace_suffix_append_only: bool,
    pub halt_marker_required_before_eos: bool,
    pub detail: String,
}

impl TassadarArticleTraceBoundaryContract {
    /// Returns a stable digest over the boundary contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_tassadar_article_trace_boundary_contract|", self)
    }
}

/// One runtime-owned channel row in the canonical article trace domain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTraceChannelRow {
    pub channel_id: String,
    pub channel_kind: TassadarArticleTraceChannelKind,
    pub stable_field_id: String,
    pub required_in_article_route: bool,
    pub runtime_owner_ref: String,
    pub detail: String,
}

/// Runtime-owned machine-step schema for the canonical article trace domain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTraceMachineStepSchema {
    pub schema_id: String,
    pub schema_version: u16,
    pub trace_abi: TassadarTraceAbi,
    pub boundary_contract: TassadarArticleTraceBoundaryContract,
    pub channel_rows: Vec<TassadarArticleTraceChannelRow>,
    pub append_only: bool,
    pub includes_stack_channel: bool,
    pub includes_locals_channel: bool,
    pub includes_memory_channel: bool,
    pub summary: String,
    pub schema_digest: String,
}

impl TassadarArticleTraceMachineStepSchema {
    /// Returns a stable digest over the schema.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_tassadar_article_trace_machine_step_schema|", self)
    }
}

/// Returns the runtime-owned prompt and terminal boundary contract.
#[must_use]
pub fn tassadar_article_trace_boundary_contract() -> TassadarArticleTraceBoundaryContract {
    TassadarArticleTraceBoundaryContract {
        boundary_id: String::from("tassadar.article_trace_boundary.v1"),
        schema_version: TASSADAR_ARTICLE_TRACE_SCHEMA_VERSION,
        required_markers: vec![
            TassadarArticleTraceBoundaryMarker::Bos,
            TassadarArticleTraceBoundaryMarker::ProgramPrompt,
            TassadarArticleTraceBoundaryMarker::TraceStart,
            TassadarArticleTraceBoundaryMarker::Halt,
            TassadarArticleTraceBoundaryMarker::Eos,
        ],
        prompt_terminator: TassadarArticleTraceBoundaryMarker::TraceStart,
        target_begins_after_prompt_terminator: true,
        trace_suffix_append_only: true,
        halt_marker_required_before_eos: true,
        detail: String::from(
            "the canonical article trace tokenization begins with a BOS marker, carries one program prompt prefix terminated by a dedicated trace-start marker, emits only the append-only trace suffix after that boundary, and requires an explicit halt marker before EOS.",
        ),
    }
}

/// Returns the runtime-owned machine-step schema for the canonical article route.
#[must_use]
pub fn tassadar_article_trace_machine_step_schema() -> TassadarArticleTraceMachineStepSchema {
    let trace_abi = TassadarTraceAbi::article_i32_compute_v1();
    let boundary_contract = tassadar_article_trace_boundary_contract();
    let channel_rows = vec![
        channel_row(
            "prompt.locals",
            TassadarArticleTraceChannelKind::PromptScalar,
            "locals",
            "crates/psionic-runtime/src/tassadar_article_trace_schema.rs",
            "the program prompt declares the bounded local-count scalar before the trace suffix begins",
        ),
        channel_row(
            "prompt.memory_slots",
            TassadarArticleTraceChannelKind::PromptScalar,
            "memory_slots",
            "crates/psionic-runtime/src/tassadar_article_trace_schema.rs",
            "the program prompt declares the bounded memory-slot scalar before trace decoding begins",
        ),
        channel_row(
            "prompt.initial_memory",
            TassadarArticleTraceChannelKind::MemoryChannel,
            "initial_memory",
            "crates/psionic-runtime/src/tassadar_article_trace_schema.rs",
            "the program prompt carries the initial memory image needed to interpret later memory snapshots",
        ),
        channel_row(
            "prompt.instructions",
            TassadarArticleTraceChannelKind::PromptInstructionStream,
            "instruction_stream",
            "crates/psionic-runtime/src/tassadar_article_trace_schema.rs",
            "the program prompt carries the ordered instruction stream for the realized execution",
        ),
        channel_row(
            "step.step_index",
            TassadarArticleTraceChannelKind::StepScalar,
            "step_index",
            "crates/psionic-runtime/src/tassadar_article_trace_schema.rs",
            "each trace step declares one append-only step index",
        ),
        channel_row(
            "step.pc",
            TassadarArticleTraceChannelKind::StepScalar,
            "pc",
            "crates/psionic-runtime/src/tassadar_article_trace_schema.rs",
            "each trace step declares the program counter before execution",
        ),
        channel_row(
            "step.next_pc",
            TassadarArticleTraceChannelKind::StepScalar,
            "next_pc",
            "crates/psionic-runtime/src/tassadar_article_trace_schema.rs",
            "each trace step declares the program counter after execution",
        ),
        channel_row(
            "step.instruction",
            TassadarArticleTraceChannelKind::StepInstruction,
            "instruction",
            "crates/psionic-runtime/src/tassadar_article_trace_schema.rs",
            "each trace step carries the realized instruction at the current pc",
        ),
        channel_row(
            "step.event",
            TassadarArticleTraceChannelKind::StepEvent,
            "event",
            "crates/psionic-runtime/src/tassadar_article_trace_schema.rs",
            "each trace step carries the realized machine event",
        ),
        channel_row(
            "step.stack_before",
            TassadarArticleTraceChannelKind::StackChannel,
            "stack_before",
            "crates/psionic-runtime/src/tassadar_article_trace_schema.rs",
            "each trace step carries the pre-step operand-stack channel",
        ),
        channel_row(
            "step.stack_after",
            TassadarArticleTraceChannelKind::StackChannel,
            "stack_after",
            "crates/psionic-runtime/src/tassadar_article_trace_schema.rs",
            "each trace step carries the post-step operand-stack channel",
        ),
        channel_row(
            "step.locals_after",
            TassadarArticleTraceChannelKind::LocalsChannel,
            "locals_after",
            "crates/psionic-runtime/src/tassadar_article_trace_schema.rs",
            "each trace step carries the post-step locals channel",
        ),
        channel_row(
            "step.memory_after",
            TassadarArticleTraceChannelKind::MemoryChannel,
            "memory_after",
            "crates/psionic-runtime/src/tassadar_article_trace_schema.rs",
            "each trace step carries the post-step memory channel",
        ),
        channel_row(
            "terminal.halt_reason",
            TassadarArticleTraceChannelKind::HaltMarker,
            "halt_reason",
            "crates/psionic-runtime/src/tassadar_article_trace_schema.rs",
            "the trace suffix terminates through one explicit halt marker bound to the runtime halt reason",
        ),
    ];
    let mut schema = TassadarArticleTraceMachineStepSchema {
        schema_id: String::from(TASSADAR_ARTICLE_TRACE_SCHEMA_ID),
        schema_version: TASSADAR_ARTICLE_TRACE_SCHEMA_VERSION,
        append_only: trace_abi.append_only,
        includes_stack_channel: trace_abi.includes_stack_snapshots,
        includes_locals_channel: trace_abi.includes_local_snapshots,
        includes_memory_channel: trace_abi.includes_memory_snapshots,
        trace_abi,
        boundary_contract,
        channel_rows,
        summary: String::new(),
        schema_digest: String::new(),
    };
    schema.summary = format!(
        "Article trace schema now records channel_rows={}, append_only={}, includes_stack_channel={}, includes_locals_channel={}, includes_memory_channel={}, and boundary_markers={}.",
        schema.channel_rows.len(),
        schema.append_only,
        schema.includes_stack_channel,
        schema.includes_locals_channel,
        schema.includes_memory_channel,
        schema.boundary_contract.required_markers.len(),
    );
    schema.schema_digest = schema.stable_digest();
    schema
}

fn channel_row(
    channel_id: &str,
    channel_kind: TassadarArticleTraceChannelKind,
    stable_field_id: &str,
    runtime_owner_ref: &str,
    detail: &str,
) -> TassadarArticleTraceChannelRow {
    TassadarArticleTraceChannelRow {
        channel_id: String::from(channel_id),
        channel_kind,
        stable_field_id: String::from(stable_field_id),
        required_in_article_route: true,
        runtime_owner_ref: String::from(runtime_owner_ref),
        detail: String::from(detail),
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
    use crate::TassadarTraceAbi;

    use super::{
        tassadar_article_trace_boundary_contract, tassadar_article_trace_machine_step_schema,
        TassadarArticleTraceBoundaryMarker, TassadarArticleTraceChannelKind,
        TASSADAR_ARTICLE_TRACE_SCHEMA_ID, TASSADAR_ARTICLE_TRACE_SCHEMA_VERSION,
    };

    #[test]
    fn article_trace_boundary_contract_requires_trace_start_halt_and_eos() {
        let contract = tassadar_article_trace_boundary_contract();

        assert_eq!(
            contract.schema_version,
            TASSADAR_ARTICLE_TRACE_SCHEMA_VERSION
        );
        assert_eq!(
            contract.prompt_terminator,
            TassadarArticleTraceBoundaryMarker::TraceStart
        );
        assert!(contract.target_begins_after_prompt_terminator);
        assert!(contract.trace_suffix_append_only);
        assert!(contract.halt_marker_required_before_eos);
        assert_eq!(
            contract.required_markers,
            vec![
                TassadarArticleTraceBoundaryMarker::Bos,
                TassadarArticleTraceBoundaryMarker::ProgramPrompt,
                TassadarArticleTraceBoundaryMarker::TraceStart,
                TassadarArticleTraceBoundaryMarker::Halt,
                TassadarArticleTraceBoundaryMarker::Eos,
            ]
        );
    }

    #[test]
    fn article_trace_machine_step_schema_tracks_prompt_step_and_terminal_channels() {
        let schema = tassadar_article_trace_machine_step_schema();

        assert_eq!(schema.schema_id, TASSADAR_ARTICLE_TRACE_SCHEMA_ID);
        assert_eq!(schema.schema_version, TASSADAR_ARTICLE_TRACE_SCHEMA_VERSION);
        assert_eq!(schema.trace_abi, TassadarTraceAbi::article_i32_compute_v1());
        assert!(schema.append_only);
        assert!(schema.includes_stack_channel);
        assert!(schema.includes_locals_channel);
        assert!(schema.includes_memory_channel);
        assert_eq!(schema.channel_rows.len(), 14);
        assert!(schema.channel_rows.iter().any(
            |row| row.channel_kind == TassadarArticleTraceChannelKind::PromptInstructionStream
        ));
        assert!(schema.channel_rows.iter().any(|row| row.channel_kind
            == TassadarArticleTraceChannelKind::StackChannel
            && row.stable_field_id == "stack_before"));
        assert!(schema.channel_rows.iter().any(|row| row.channel_kind
            == TassadarArticleTraceChannelKind::LocalsChannel
            && row.stable_field_id == "locals_after"));
        assert!(schema.channel_rows.iter().any(|row| row.channel_kind
            == TassadarArticleTraceChannelKind::MemoryChannel
            && row.stable_field_id == "memory_after"));
        assert!(schema
            .channel_rows
            .iter()
            .any(|row| row.channel_kind == TassadarArticleTraceChannelKind::HaltMarker));
        assert!(!schema.schema_digest.is_empty());
    }
}
