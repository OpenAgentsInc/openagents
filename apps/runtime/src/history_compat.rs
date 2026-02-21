use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    artifacts::{ArtifactError, build_replay_jsonl, trajectory_hash},
    run_state_machine::{RunStateMachineError, apply_transition, transition_for_event},
    types::{RunEvent, RunStatus, RuntimeRun},
};

pub const HISTORY_FIXTURE_SCHEMA: &str = "openagents.runtime.workflow_history_compat.v1";
pub const HISTORY_FIXTURE_VERSION: u32 = 1;
pub const DEFAULT_HISTORY_FIXTURE_PATH: &str =
    "fixtures/history_compat/run_workflow_histories_v1.json";

#[derive(Debug, Error)]
pub enum HistoryCompatibilityError {
    #[error("history fixture I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("history fixture parse error: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("history fixture schema mismatch: expected={expected}, actual={actual}")]
    SchemaMismatch { expected: String, actual: String },
    #[error("history fixture version mismatch: expected={expected}, actual={actual}")]
    VersionMismatch { expected: u32, actual: u32 },
    #[error("history fixture corpus is empty")]
    EmptyCorpus,
    #[error("history fixture '{fixture_id}' has non-monotonic sequence values")]
    NonMonotonicSequence { fixture_id: String },
    #[error("history fixture state transition error in '{fixture_id}': {source}")]
    Transition {
        fixture_id: String,
        #[source]
        source: RunStateMachineError,
    },
    #[error("history fixture artifact error in '{fixture_id}': {source}")]
    Artifact {
        fixture_id: String,
        #[source]
        source: ArtifactError,
    },
    #[error(
        "history fixture expectation mismatch in '{fixture_id}' for field '{field}': expected={expected}, actual={actual}"
    )]
    ExpectationMismatch {
        fixture_id: String,
        field: &'static str,
        expected: String,
        actual: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowHistoryCorpus {
    pub schema: String,
    pub version: u32,
    pub fixtures: Vec<WorkflowHistoryFixture>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowHistoryFixture {
    pub id: String,
    pub description: String,
    pub run_id: Uuid,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub worker_id: Option<String>,
    #[serde(default = "default_fixture_metadata")]
    pub metadata: Value,
    pub events: Vec<WorkflowHistoryEvent>,
    pub expected: WorkflowHistoryExpectation,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowHistoryEvent {
    pub seq: u64,
    pub event_type: String,
    #[serde(default = "default_fixture_payload")]
    pub payload: Value,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowHistoryExpectation {
    pub final_status: RunStatus,
    pub replay_hash: String,
    pub trajectory_hash: String,
    pub runtime_event_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WorkflowHistoryOutcome {
    pub fixture_id: String,
    pub final_status: RunStatus,
    pub replay_hash: String,
    pub trajectory_hash: String,
    pub runtime_event_count: usize,
}

pub fn verify_default_history_compatibility()
-> Result<Vec<WorkflowHistoryOutcome>, HistoryCompatibilityError> {
    verify_history_compatibility(&default_fixture_path())
}

pub fn verify_history_compatibility(
    path: &Path,
) -> Result<Vec<WorkflowHistoryOutcome>, HistoryCompatibilityError> {
    let corpus = load_history_fixture_corpus(path)?;
    corpus
        .fixtures
        .iter()
        .map(verify_fixture)
        .collect::<Result<Vec<_>, _>>()
}

fn load_history_fixture_corpus(
    path: &Path,
) -> Result<WorkflowHistoryCorpus, HistoryCompatibilityError> {
    let raw = fs::read_to_string(path)?;
    let corpus = serde_json::from_str::<WorkflowHistoryCorpus>(&raw)?;
    if corpus.schema != HISTORY_FIXTURE_SCHEMA {
        return Err(HistoryCompatibilityError::SchemaMismatch {
            expected: HISTORY_FIXTURE_SCHEMA.to_string(),
            actual: corpus.schema,
        });
    }
    if corpus.version != HISTORY_FIXTURE_VERSION {
        return Err(HistoryCompatibilityError::VersionMismatch {
            expected: HISTORY_FIXTURE_VERSION,
            actual: corpus.version,
        });
    }
    if corpus.fixtures.is_empty() {
        return Err(HistoryCompatibilityError::EmptyCorpus);
    }
    Ok(corpus)
}

fn verify_fixture(
    fixture: &WorkflowHistoryFixture,
) -> Result<WorkflowHistoryOutcome, HistoryCompatibilityError> {
    if !fixture
        .events
        .windows(2)
        .all(|window| window[0].seq < window[1].seq)
    {
        return Err(HistoryCompatibilityError::NonMonotonicSequence {
            fixture_id: fixture.id.clone(),
        });
    }

    let mut status = RunStatus::Created;
    let mut events = Vec::with_capacity(fixture.events.len());

    for event in &fixture.events {
        let transition =
            transition_for_event(event.event_type.as_str(), &event.payload).map_err(|source| {
                HistoryCompatibilityError::Transition {
                    fixture_id: fixture.id.clone(),
                    source,
                }
            })?;
        if let Some(transition) = transition {
            let outcome = apply_transition(&status, &transition).map_err(|source| {
                HistoryCompatibilityError::Transition {
                    fixture_id: fixture.id.clone(),
                    source,
                }
            })?;
            status = outcome.next_status;
        }
        events.push(RunEvent {
            seq: event.seq,
            event_type: event.event_type.clone(),
            payload: event.payload.clone(),
            idempotency_key: event.idempotency_key.clone(),
            recorded_at: event.recorded_at,
        });
    }

    let updated_at = fixture
        .events
        .last()
        .map(|event| event.recorded_at)
        .unwrap_or(fixture.created_at);
    let run = RuntimeRun {
        id: fixture.run_id,
        worker_id: fixture.worker_id.clone(),
        status: status.clone(),
        metadata: fixture.metadata.clone(),
        events,
        created_at: fixture.created_at,
        updated_at,
    };

    let replay_jsonl =
        build_replay_jsonl(&run).map_err(|source| HistoryCompatibilityError::Artifact {
            fixture_id: fixture.id.clone(),
            source,
        })?;
    let trajectory_hash =
        trajectory_hash(&run.events).map_err(|source| HistoryCompatibilityError::Artifact {
            fixture_id: fixture.id.clone(),
            source,
        })?;
    let replay_hash = sha256_prefixed(replay_jsonl.as_str());
    let outcome = WorkflowHistoryOutcome {
        fixture_id: fixture.id.clone(),
        final_status: status,
        replay_hash,
        trajectory_hash,
        runtime_event_count: run.events.len(),
    };

    assert_field_match(
        fixture.id.as_str(),
        "final_status",
        format!("{:?}", fixture.expected.final_status),
        format!("{:?}", outcome.final_status),
    )?;
    assert_field_match(
        fixture.id.as_str(),
        "replay_hash",
        fixture.expected.replay_hash.clone(),
        outcome.replay_hash.clone(),
    )?;
    assert_field_match(
        fixture.id.as_str(),
        "trajectory_hash",
        fixture.expected.trajectory_hash.clone(),
        outcome.trajectory_hash.clone(),
    )?;
    assert_field_match(
        fixture.id.as_str(),
        "runtime_event_count",
        fixture.expected.runtime_event_count.to_string(),
        outcome.runtime_event_count.to_string(),
    )?;

    Ok(outcome)
}

fn assert_field_match(
    fixture_id: &str,
    field: &'static str,
    expected: String,
    actual: String,
) -> Result<(), HistoryCompatibilityError> {
    if expected == actual {
        return Ok(());
    }
    Err(HistoryCompatibilityError::ExpectationMismatch {
        fixture_id: fixture_id.to_string(),
        field,
        expected,
        actual,
    })
}

fn sha256_prefixed(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    format!("sha256:{}", hex::encode(digest))
}

fn default_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_HISTORY_FIXTURE_PATH)
}

fn default_fixture_payload() -> Value {
    Value::Object(Default::default())
}

fn default_fixture_metadata() -> Value {
    Value::Object(Default::default())
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, anyhow};

    use super::{
        HistoryCompatibilityError, WorkflowHistoryCorpus, default_fixture_path,
        load_history_fixture_corpus, verify_default_history_compatibility, verify_fixture,
    };

    #[test]
    fn history_fixture_corpus_is_replay_compatible() -> Result<()> {
        let outcomes = verify_default_history_compatibility()?;
        if outcomes.is_empty() {
            return Err(anyhow!(
                "expected non-empty history compatibility outcome set"
            ));
        }
        Ok(())
    }

    #[test]
    fn history_compat_harness_detects_hash_drift() -> Result<()> {
        let path = default_fixture_path();
        let mut corpus: WorkflowHistoryCorpus = load_history_fixture_corpus(path.as_path())?;
        let fixture = corpus
            .fixtures
            .first_mut()
            .ok_or_else(|| anyhow!("missing first fixture"))?;
        fixture.expected.replay_hash = "sha256:deadbeef".to_string();

        let result = verify_fixture(fixture);
        if !matches!(
            result,
            Err(HistoryCompatibilityError::ExpectationMismatch { field, .. }) if field == "replay_hash"
        ) {
            return Err(anyhow!("expected replay_hash mismatch detection"));
        }
        Ok(())
    }
}
