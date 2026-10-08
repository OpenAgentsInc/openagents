//! Bounded observation of explicitly admitted resident project supervision.

use crate::{Code, Result, fail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_REPLY_BYTES: usize = 48 * 1024;
pub const MAX_CHUNK_BYTES: usize = 16 * 1024;
pub const MAX_SOURCE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub id: String,
    pub label: String,
    pub snapshot_digest: String,
    pub goals_state: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct List {
    pub workspace: String,
    pub rows: Vec<Row>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cursor {
    pub snapshot_digest: String,
    pub offset: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub workspace: String,
    pub project: String,
    pub snapshot: Option<String>,
    pub cursor: Option<Cursor>,
    pub limit: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub owner: String,
    pub attempt: String,
    pub task_digest: String,
    pub result_digest: Option<String>,
    pub cause: Option<String>,
    pub backoff_until: Option<u64>,
    pub attempts: u32,
    pub updated_unix: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub id: String,
    pub issue: u64,
    pub title: String,
    pub dependencies: Vec<String>,
    pub footprint: Value,
    pub resources: Value,
    pub status: String,
    pub content_digest: String,
    pub claim: Option<Claim>,
    pub blockers: Vec<String>,
    pub worktree_state: String,
    pub inline_state: String,
    pub worktree: Option<Worktree>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Worktree {
    pub owner: String,
    pub attempt: String,
    pub retained: bool,
    pub artifact_verified: bool,
    pub execution_status: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capacity {
    pub declared: Value,
    pub external: Value,
    pub occupied: Value,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub limit: u32,
    pub pending: u32,
    pub state: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Original {
    pub id: String,
    pub digest: String,
    pub bytes: u64,
    pub media_type: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub id: String,
    pub state: String,
    pub original: Option<Original>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub workspace: String,
    pub project: String,
    pub label: String,
    pub snapshot_digest: String,
    pub sequence: u64,
    pub goals_state: String,
    pub tracker_state: String,
    pub tracker: Value,
    pub supervisor: Value,
    pub tasks: Vec<Task>,
    pub capacity: Capacity,
    pub review: Review,
    pub external_owners: Value,
    pub excluded_issues: Vec<u64>,
    pub accepted_closed_issues: Vec<u64>,
    pub sources: Vec<Source>,
    pub next: Option<Cursor>,
    pub remaining: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OriginalCursor {
    pub snapshot_digest: String,
    pub source_digest: String,
    pub offset: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OriginalQuery {
    pub workspace: String,
    pub project: String,
    pub snapshot_digest: String,
    pub original: Original,
    pub cursor: Option<OriginalCursor>,
    pub limit: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chunk {
    pub workspace: String,
    pub project: String,
    pub snapshot_digest: String,
    pub original: Original,
    pub offset: u64,
    pub data_base64: String,
    pub next: Option<OriginalCursor>,
}

pub fn alias(value: &str) -> bool {
    (1..=128).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
pub fn digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn text(value: &str, max: usize) -> bool {
    value.len() <= max
        && !value
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
}
fn checked(valid: bool) -> Result<()> {
    if valid {
        Ok(())
    } else {
        fail(
            Code::Malformed,
            "project observation scope or bounds are invalid",
        )
    }
}
impl Query {
    pub fn validate(&self) -> Result<()> {
        checked(
            alias(&self.workspace)
                && alias(&self.project)
                && (1..=64).contains(&self.limit)
                && self.snapshot.as_deref().is_none_or(digest)
                && self.cursor.as_ref().is_none_or(|cursor| {
                    self.snapshot.as_ref() == Some(&cursor.snapshot_digest)
                        && digest(&cursor.snapshot_digest)
                }),
        )
    }
}
impl Original {
    pub fn validate(&self) -> Result<()> {
        checked(
            alias(&self.id)
                && digest(&self.digest)
                && self.bytes <= MAX_SOURCE_BYTES
                && self.media_type == "application/json",
        )
    }
}
impl OriginalQuery {
    pub fn validate(&self) -> Result<()> {
        self.original.validate()?;
        checked(
            alias(&self.workspace)
                && alias(&self.project)
                && digest(&self.snapshot_digest)
                && (1..=MAX_CHUNK_BYTES as u32).contains(&self.limit)
                && self.cursor.as_ref().is_none_or(|cursor| {
                    cursor.snapshot_digest == self.snapshot_digest
                        && cursor.source_digest == self.original.digest
                        && cursor.offset <= self.original.bytes
                }),
        )
    }
}
impl List {
    pub fn validate(&self) -> Result<()> {
        checked(
            alias(&self.workspace)
                && self.rows.len() <= 64
                && self.rows.iter().all(|row| {
                    alias(&row.id)
                        && text(&row.label, 256)
                        && digest(&row.snapshot_digest)
                        && text(&row.goals_state, 64)
                }),
        )?;
        crate::task_read::bounded(self, MAX_REPLY_BYTES)
    }
    pub fn answers(&self, workspace: &str) -> bool {
        self.workspace == workspace
    }
}
impl Page {
    pub fn validate(&self) -> Result<()> {
        checked(
            alias(&self.workspace)
                && alias(&self.project)
                && text(&self.label, 256)
                && digest(&self.snapshot_digest)
                && text(&self.goals_state, 64)
                && text(&self.tracker_state, 64)
                && self.tasks.len() <= 64
                && self.sources.len() <= 512
                && self.tasks.iter().all(|task| {
                    alias(&task.id)
                        && text(&task.title, 512)
                        && task.dependencies.len() <= 256
                        && task.dependencies.iter().all(|id| alias(id))
                        && text(&task.status, 64)
                        && digest(&task.content_digest)
                        && task.blockers.len() <= 64
                        && task.blockers.iter().all(|v| text(v, 4096))
                        && text(&task.worktree_state, 64)
                        && matches!(task.inline_state.as_str(), "complete" | "oversized")
                        && task.worktree.as_ref().is_none_or(|w| {
                            text(&w.owner, 256)
                                && text(&w.attempt, 256)
                                && w.execution_status.as_deref().is_none_or(|v| text(v, 256))
                        })
                        && task.claim.as_ref().is_none_or(|c| {
                            text(&c.owner, 256)
                                && text(&c.attempt, 256)
                                && text(&c.task_digest, 256)
                                && c.result_digest.as_deref().is_none_or(|v| text(v, 256))
                                && c.cause.as_deref().is_none_or(|v| text(v, 4096))
                        })
                })
                && self.sources.iter().all(|source| {
                    alias(&source.id)
                        && text(&source.state, 64)
                        && source
                            .original
                            .as_ref()
                            .is_none_or(|v| v.id == source.id && v.validate().is_ok())
                })
                && self
                    .next
                    .as_ref()
                    .is_none_or(|cursor| cursor.snapshot_digest == self.snapshot_digest),
        )?;
        crate::task_read::bounded(self, MAX_REPLY_BYTES)
    }
    pub fn answers(&self, query: &Query) -> bool {
        self.workspace == query.workspace
            && self.project == query.project
            && query
                .snapshot
                .as_ref()
                .is_none_or(|digest| digest == &self.snapshot_digest)
    }
}
impl Chunk {
    pub fn validate(&self) -> Result<()> {
        use base64::{Engine, engine::general_purpose::STANDARD};
        self.original.validate()?;
        checked(self.data_base64.len() <= MAX_CHUNK_BYTES.div_ceil(3) * 4)?;
        let bytes = STANDARD.decode(&self.data_base64).map_err(|_| {
            crate::Error::new(Code::Malformed, "project original encoding is invalid")
        })?;
        checked(
            alias(&self.workspace)
                && alias(&self.project)
                && digest(&self.snapshot_digest)
                && bytes.len() <= MAX_CHUNK_BYTES
                && self.offset.saturating_add(bytes.len() as u64) <= self.original.bytes
                && self.next.as_ref().is_none_or(|cursor| {
                    cursor.snapshot_digest == self.snapshot_digest
                        && cursor.source_digest == self.original.digest
                        && cursor.offset == self.offset + bytes.len() as u64
                }),
        )?;
        crate::task_read::bounded(self, MAX_REPLY_BYTES)
    }
    pub fn answers(&self, query: &OriginalQuery) -> bool {
        self.workspace == query.workspace
            && self.project == query.project
            && self.snapshot_digest == query.snapshot_digest
            && self.original == query.original
            && self.offset == query.cursor.as_ref().map_or(0, |cursor| cursor.offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_queries_pin_project_snapshot_and_exact_source() {
        let pin = format!("sha256:{}", "a".repeat(64));
        let mut query = Query {
            workspace: "repo".into(),
            project: "project".into(),
            snapshot: Some(pin.clone()),
            cursor: Some(Cursor {
                snapshot_digest: pin.clone(),
                offset: 1,
            }),
            limit: 1,
        };
        assert!(query.validate().is_ok());
        query.snapshot = None;
        assert!(query.validate().is_err());
        let original = Original {
            id: "ledger".into(),
            digest: pin.clone(),
            bytes: 20,
            media_type: "application/json".into(),
        };
        let mut query = OriginalQuery {
            workspace: "repo".into(),
            project: "project".into(),
            snapshot_digest: pin.clone(),
            original,
            cursor: Some(OriginalCursor {
                snapshot_digest: pin.clone(),
                source_digest: pin,
                offset: 2,
            }),
            limit: 16,
        };
        assert!(query.validate().is_ok());
        query.cursor.as_mut().unwrap().source_digest = format!("sha256:{}", "b".repeat(64));
        assert!(query.validate().is_err());
        query.original.id = "/private/ledger".into();
        assert!(query.validate().is_err());
    }
    #[test]
    fn chunk_bound_is_checked_before_base64_allocation() {
        let pin = format!("sha256:{}", "a".repeat(64));
        let chunk = Chunk {
            workspace: "repo".into(),
            project: "project".into(),
            snapshot_digest: pin.clone(),
            original: Original {
                id: "ledger".into(),
                digest: pin,
                bytes: MAX_SOURCE_BYTES,
                media_type: "application/json".into(),
            },
            offset: 0,
            data_base64: "A".repeat(MAX_CHUNK_BYTES * 2),
            next: None,
        };
        assert!(chunk.validate().is_err());
    }
}
