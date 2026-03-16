#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::unwrap_used
    )
)]
//! ARC local and remote client wrappers plus REST-facing transport models.

pub mod local;
pub mod models;
pub mod parity;
pub mod recording;
pub mod remote;
pub mod server;

use arc_core::{ArcActionError, ArcFrameDataError, ArcRecordingError, ArcTaskId, ArcTaskIdError};
use arc_engine::ArcEngineError;
use reqwest::StatusCode;
use thiserror::Error;

pub use local::LocalArcEnvironment;
pub use models::{
    ArcCloseScorecardRequest, ArcCompatibilityActionInput, ArcCompatibilityFrameResponse,
    ArcComplexActionCommand, ArcEnvironmentInfo, ArcOpenScorecardRequest, ArcOpenScorecardResponse,
    ArcRemoteSession, ArcResetCommand, ArcScorecardEnvironment, ArcScorecardRunSummary,
    ArcScorecardSummary, ArcSessionFrame, ArcSimpleActionCommand, ArcTagScore,
};
pub use parity::{
    ArcLocalRemoteParityField, ArcLocalRemoteParityMismatch, ArcLocalRemoteParityOutcome,
    ArcLocalRemoteParityReport, ArcParityTraceStep, compare_local_remote_traces,
};
pub use recording::{
    ArcJsonlImportContext, ArcJsonlRecordingData, ArcJsonlRecordingEntry,
    ArcRecordingTransportPolicy, jsonl_entries_to_recording, read_jsonl_recording_file,
    recording_to_jsonl_entries, session_frames_to_recording, write_jsonl_recording_file,
};
pub use remote::{ArcRemoteArcade, ArcRemoteClient, ArcRemoteRetryPolicy, RemoteArcEnvironment};
pub use server::{ArcCompatibilityServer, ArcCompatibilityServerConfig, ArcRegisteredEnvironment};

/// Human-readable ownership summary for this crate.
pub const CRATE_ROLE: &str =
    "ARC local and remote wrappers, REST models, and cookie-affine session behavior";

/// Stable boundary summary for downstream ARC crates.
pub const CLIENT_BOUNDARY_SUMMARY: &str = "arc-client owns local and remote wrappers, REST transport models, and cookie-affine session handling";

#[derive(Debug, Error)]
pub enum ArcClientError {
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Engine(#[from] ArcEngineError),
    #[error(transparent)]
    TaskId(#[from] ArcTaskIdError),
    #[error(transparent)]
    Action(#[from] ArcActionError),
    #[error(transparent)]
    Frame(#[from] ArcFrameDataError),
    #[error(transparent)]
    Recording(#[from] ArcRecordingError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("ARC remote API returned {status} for `{path}`: {body}")]
    UnexpectedStatus {
        status: StatusCode,
        path: String,
        body: String,
    },
    #[error("ARC remote response for `{game_id}` did not include a session guid")]
    MissingGuid { game_id: ArcTaskId },
    #[error("ARC local environment `{game_id}` is missing a package path")]
    MissingLocalPackagePath { game_id: ArcTaskId },
    #[error("ARC client state `{state}` lock poisoned")]
    StatePoisoned { state: &'static str },
    #[error(
        "ARC remote retry budget for `{path}` exhausted after {attempts} attempts with {status}: {body}"
    )]
    RetryBudgetExhausted {
        path: String,
        attempts: u32,
        status: StatusCode,
        body: String,
    },
    #[error("ARC remote session for `{game_id}` is not initialized; call reset first")]
    MissingSessionGuid { game_id: ArcTaskId },
    #[error("ARC remote response used unsupported action id {id}")]
    UnsupportedActionId { id: u8 },
    #[error("ARC ACTION6 response is missing coordinate `{axis}`")]
    MissingAction6Coordinate { axis: &'static str },
    #[error("ARC session frame step {step_index} is missing an observation frame")]
    MissingObservationFrame { step_index: usize },
    #[error("ARC JSONL recording must contain at least one entry")]
    JsonlRecordingEmpty,
    #[error("ARC JSONL recording is missing action input at line {line_index}")]
    JsonlActionInputMissing { line_index: usize },
    #[error(
        "ARC JSONL recording changed task id from `{expected}` to `{actual}` at line {line_index}"
    )]
    JsonlTaskMismatch {
        expected: ArcTaskId,
        actual: ArcTaskId,
        line_index: usize,
    },
    #[error("ARC JSONL recording for `{task_id}` omitted frame data at line {line_index}")]
    JsonlFrameDataMissing {
        task_id: ArcTaskId,
        line_index: usize,
    },
    #[error(
        "ARC remote frame {frame_index} used ragged row widths: expected {expected}, got {actual} at row {row_index}"
    )]
    RaggedRemoteFrame {
        frame_index: usize,
        row_index: usize,
        expected: usize,
        actual: usize,
    },
}
