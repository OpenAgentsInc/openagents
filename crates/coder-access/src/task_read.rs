//! Observe-only, bounded views of the resident task owner's original records.

use crate::{Code, Error, Result, fail};
use base64::{Engine, engine::general_purpose::STANDARD};
use nostr::activity_summary::Phase;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, Write};

pub const MAX_REPLY_BYTES: usize = 48 * 1024;
pub const MAX_INLINE_STEP_BYTES: usize = 16 * 1024;
pub const MAX_CHUNK_BYTES: usize = 16 * 1024;
pub const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_ITEMS: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub workspace: String,
    pub task: String,
    pub revision: u64,
    pub attempt: Option<u64>,
    pub intent_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub task: String,
    pub revision: u64,
    pub title: String,
    #[serde(with = "phase")]
    pub phase: Phase,
    pub attempt: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListCursor {
    pub workspace: String,
    pub snapshot_digest: String,
    pub next: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListQuery {
    pub workspace: String,
    pub cursor: Option<ListCursor>,
    pub limit: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct List {
    pub workspace: String,
    pub snapshot_digest: String,
    pub rows: Vec<Row>,
    pub next: Option<ListCursor>,
    pub more_available: bool,
}

/// A prefix reconnect also pins the previous raw snapshot before new appends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cursor {
    pub scope: Scope,
    pub source: String,
    pub source_digest: String,
    pub source_bytes: u64,
    pub next_step: u64,
    pub prefix_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageQuery {
    pub workspace: String,
    pub task: String,
    pub revision: Option<u64>,
    pub cursor: Option<Cursor>,
    pub limit: u16,
}

/// An opaque source alias under one task scope, never a caller-provided path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Original {
    pub source: String,
    pub digest: String,
    pub bytes: u64,
    pub media_type: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapReason {
    Oversized,
}

/// Original step objects stay intact; a large object points to its raw source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Step {
    Original {
        index: u64,
        step: Value,
    },
    Gap {
        index: u64,
        reason: GapReason,
        original: Original,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fault {
    pub line: u64,
    pub kind: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub state: String,
    pub original: Option<Original>,
    pub total_steps: u64,
    pub steps: Vec<Step>,
    pub faults: Vec<Fault>,
    pub more_faults: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub label: String,
    pub state: String,
    pub original: Option<Original>,
}

/// An original structured delegation reference, without a task or control grant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Child {
    pub source_step: u64,
    pub call_id: String,
    pub agent: String,
    pub reference: Option<String>,
    pub state: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub scope: Scope,
    pub title: String,
    pub prompt: String,
    #[serde(with = "phase")]
    pub phase: Phase,
    pub execution: String,
    pub verification: String,
    pub integration: String,
    pub termination: String,
    pub delivery: String,
    pub cleanup: String,
    pub cost_microusd: Option<u64>,
    pub cost_status: String,
    pub evidence: Evidence,
    pub artifacts: Vec<Artifact>,
    pub children: Vec<Child>,
    pub more_children: bool,
    pub next: Option<Cursor>,
    pub more_available: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OriginalCursor {
    pub scope: Scope,
    pub source: String,
    pub digest: String,
    pub next_byte: u64,
    pub prefix_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OriginalQuery {
    pub scope: Scope,
    pub original: Original,
    pub cursor: Option<OriginalCursor>,
    pub limit: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OriginalChunk {
    pub scope: Scope,
    pub original: Original,
    pub start: u64,
    pub data: String,
    pub next: Option<OriginalCursor>,
    pub more_available: bool,
}

mod phase {
    use super::Phase;
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn serialize<S: Serializer>(phase: &Phase, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match phase {
            Phase::Queued => "queued",
            Phase::Running => "running",
            Phase::Waiting => "waiting",
            Phase::Completed => "completed",
            Phase::Failed => "failed",
            Phase::Cancelled => "cancelled",
            Phase::Unknown => "unknown",
        })
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Phase, D::Error> {
        match String::deserialize(deserializer)?.as_str() {
            "queued" => Ok(Phase::Queued),
            "running" => Ok(Phase::Running),
            "waiting" => Ok(Phase::Waiting),
            "completed" => Ok(Phase::Completed),
            "failed" => Ok(Phase::Failed),
            "cancelled" => Ok(Phase::Cancelled),
            "unknown" => Ok(Phase::Unknown),
            _ => Err(D::Error::custom("unknown task phase")),
        }
    }
}

fn number(value: u64) -> Result<()> {
    if value > crate::protocol::MAX_SAFE {
        return fail(Code::Malformed, "task read integer exceeds its bound");
    }
    Ok(())
}
fn digest(value: &str) -> Result<()> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return fail(Code::Malformed, "task read digest is malformed");
    };
    crate::protocol::identity(hex).map_err(Error::from)
}
fn label(value: &str, maximum: usize, empty: bool) -> Result<()> {
    if (!empty && value.is_empty()) || value.len() > maximum || value.chars().any(char::is_control)
    {
        return fail(Code::Bounds, "task read label exceeds its bound");
    }
    Ok(())
}
fn workspace(value: &str) -> Result<()> {
    label(value, 128, false)?;
    if value.contains(['/', '\\']) || matches!(value, "." | "..") {
        return fail(
            Code::Malformed,
            "task read workspace must be an admitted label",
        );
    }
    Ok(())
}
fn source(value: &str) -> Result<()> {
    label(value, 128, false)?;
    let allowed = value == "task"
        || value
            .strip_prefix("artifact:")
            .is_some_and(|hex| crate::protocol::identity(hex).is_ok())
        || ["trace:", "manifest:"].iter().any(|prefix| {
            value.strip_prefix(prefix).is_some_and(|number| {
                !number.is_empty()
                    && number.bytes().all(|byte| byte.is_ascii_digit())
                    && number
                        .parse::<u64>()
                        .is_ok_and(|epoch| epoch > 0 && epoch <= crate::protocol::MAX_SAFE)
            })
        });
    if !allowed {
        return fail(Code::Malformed, "task read source must be an opaque alias");
    }
    Ok(())
}
fn limit(value: usize, maximum: usize) -> Result<()> {
    if value == 0 || value > maximum {
        return fail(Code::Bounds, "task read limit exceeds its bound");
    }
    Ok(())
}
fn attempt(value: Option<u64>) -> Result<()> {
    if let Some(value) = value {
        number(value)?;
        if value == 0 {
            return fail(Code::Malformed, "task read attempt is malformed");
        }
    }
    Ok(())
}

/// Count serialized bytes without allocating an oversized response buffer.
pub fn bounded(value: &impl Serialize, maximum: usize) -> Result<()> {
    struct Count(usize, usize);
    impl Write for Count {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > self.1.saturating_sub(self.0) {
                return Err(io::Error::other("bounded task answer"));
            }
            self.0 += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(Count(0, maximum), value)
        .map_err(|_| Error::new(Code::Bounds, "task answer exceeds its encoded bound"))
}

impl Scope {
    pub fn validate(&self) -> Result<()> {
        workspace(&self.workspace)?;
        crate::studio::id(&self.task)?;
        number(self.revision)?;
        attempt(self.attempt)?;
        digest(&self.intent_digest)
    }
}
impl Row {
    pub fn validate(&self) -> Result<()> {
        crate::studio::id(&self.task)?;
        number(self.revision)?;
        label(&self.title, 200, false)?;
        attempt(self.attempt)
    }
}
impl ListCursor {
    pub fn validate(&self) -> Result<()> {
        workspace(&self.workspace)?;
        digest(&self.snapshot_digest)?;
        number(self.next)
    }
}
impl ListQuery {
    pub fn validate(&self) -> Result<()> {
        workspace(&self.workspace)?;
        limit(self.limit.into(), MAX_ITEMS)?;
        if let Some(cursor) = &self.cursor {
            cursor.validate()?;
            if cursor.workspace != self.workspace {
                return fail(Code::Malformed, "task list cursor names another workspace");
            }
        }
        Ok(())
    }
}
impl List {
    pub fn validate(&self) -> Result<()> {
        workspace(&self.workspace)?;
        digest(&self.snapshot_digest)?;
        if self.rows.len() > MAX_ITEMS {
            return fail(Code::Bounds, "too many task rows");
        }
        let mut tasks = std::collections::BTreeSet::new();
        for row in &self.rows {
            row.validate()?;
            if !tasks.insert(&row.task) {
                return fail(Code::Malformed, "duplicate task row");
            }
        }
        if let Some(cursor) = &self.next {
            cursor.validate()?;
            if cursor.workspace != self.workspace || cursor.snapshot_digest != self.snapshot_digest
            {
                return fail(
                    Code::Malformed,
                    "task list continuation changes its snapshot",
                );
            }
        }
        if self.more_available && (self.next.is_none() || self.rows.is_empty()) {
            return fail(Code::Malformed, "task list continuation makes no progress");
        }
        bounded(self, MAX_REPLY_BYTES)
    }
    pub fn answers(&self, query: &ListQuery) -> bool {
        self.workspace == query.workspace
            && self.rows.len() <= usize::from(query.limit)
            && query
                .cursor
                .as_ref()
                .is_none_or(|cursor| cursor.snapshot_digest == self.snapshot_digest)
            && self.next.as_ref().is_none_or(|next| {
                next.next
                    == query.cursor.as_ref().map_or(0, |cursor| cursor.next)
                        + self.rows.len() as u64
            })
    }
}
impl Cursor {
    pub fn validate(&self) -> Result<()> {
        self.scope.validate()?;
        source(&self.source)?;
        digest(&self.source_digest)?;
        if self.source_bytes > MAX_SOURCE_BYTES {
            return fail(Code::Bounds, "task trace snapshot exceeds its bound");
        }
        number(self.next_step)?;
        digest(&self.prefix_digest)
    }
}
impl PageQuery {
    pub fn validate(&self) -> Result<()> {
        workspace(&self.workspace)?;
        crate::studio::id(&self.task)?;
        if let Some(revision) = self.revision {
            number(revision)?;
        }
        limit(self.limit.into(), MAX_ITEMS)?;
        if let Some(cursor) = &self.cursor {
            cursor.validate()?;
            if cursor.scope.workspace != self.workspace
                || cursor.scope.task != self.task
                || Some(cursor.scope.revision) != self.revision
            {
                return fail(Code::Malformed, "task page cursor changes its scope");
            }
        }
        Ok(())
    }
}
impl Original {
    pub fn validate(&self) -> Result<()> {
        source(&self.source)?;
        digest(&self.digest)?;
        if self.bytes > MAX_SOURCE_BYTES {
            return fail(Code::Bounds, "task original exceeds its bound");
        }
        label(&self.media_type, 128, false)
    }
}
impl Page {
    pub fn validate(&self) -> Result<()> {
        self.scope.validate()?;
        label(&self.title, 200, false)?;
        if self.prompt.len() > 16 * 1024 * 1024 {
            return fail(Code::Bounds, "task prompt exceeds its bound");
        }
        for state in [
            &self.execution,
            &self.verification,
            &self.integration,
            &self.termination,
            &self.delivery,
            &self.cleanup,
            &self.cost_status,
            &self.evidence.state,
        ] {
            label(state, 128, false)?;
        }
        if let Some(cost) = self.cost_microusd {
            number(cost)?;
        }
        number(self.evidence.total_steps)?;
        if self.evidence.steps.len() > MAX_ITEMS
            || self.evidence.faults.len() > MAX_ITEMS
            || self.artifacts.len() > MAX_ITEMS
            || self.children.len() > MAX_ITEMS
        {
            return fail(Code::Bounds, "too many task evidence entries");
        }
        if let Some(original) = &self.evidence.original {
            original.validate()?;
        }
        let mut previous = None;
        for step in &self.evidence.steps {
            let index = match step {
                Step::Original { index, step } => {
                    if !step.is_object() {
                        return fail(Code::Malformed, "task step must be an original object");
                    }
                    bounded(step, MAX_INLINE_STEP_BYTES)?;
                    *index
                }
                Step::Gap {
                    index, original, ..
                } => {
                    original.validate()?;
                    if Some(original) != self.evidence.original.as_ref() {
                        return fail(Code::Malformed, "task step gap changes its original source");
                    }
                    *index
                }
            };
            number(index)?;
            if index >= self.evidence.total_steps
                || previous.is_some_and(|previous| index != previous + 1)
            {
                return fail(Code::Malformed, "task steps must form one original prefix");
            }
            previous = Some(index);
        }
        for fault in &self.evidence.faults {
            number(fault.line)?;
            label(&fault.kind, 128, false)?;
        }
        for artifact in &self.artifacts {
            label(&artifact.label, 1024, false)?;
            label(&artifact.state, 128, false)?;
            if let Some(original) = &artifact.original {
                original.validate()?;
            }
        }
        for child in &self.children {
            number(child.source_step)?;
            label(&child.call_id, 128, false)?;
            label(&child.agent, 128, false)?;
            if let Some(reference) = &child.reference {
                label(reference, 128, false)?;
            }
            if child.source_step >= self.evidence.total_steps
                || !matches!(child.state.as_str(), "reference_only" | "unavailable")
                || (child.state == "reference_only") != child.reference.is_some()
            {
                return fail(
                    Code::Malformed,
                    "task child must be an original reference without authority",
                );
            }
        }
        if let Some(next) = &self.next {
            next.validate()?;
            if next.scope != self.scope
                || self.evidence.original.as_ref().is_none_or(|original| {
                    next.source != original.source
                        || next.source_digest != original.digest
                        || next.source_bytes != original.bytes
                })
                || previous.is_some_and(|index| next.next_step != index + 1)
            {
                return fail(
                    Code::Malformed,
                    "task continuation changes its source or scope",
                );
            }
        }
        if self.more_available && (self.next.is_none() || self.evidence.steps.is_empty()) {
            return fail(Code::Malformed, "task continuation makes no progress");
        }
        bounded(self, MAX_REPLY_BYTES)
    }
    pub fn answers(&self, query: &PageQuery) -> bool {
        let start = query.cursor.as_ref().map_or(0, |cursor| cursor.next_step);
        let first = self.evidence.steps.first().map(|step| match step {
            Step::Original { index, .. } | Step::Gap { index, .. } => *index,
        });
        self.scope.workspace == query.workspace
            && self.scope.task == query.task
            && query
                .revision
                .is_none_or(|revision| self.scope.revision == revision)
            && self.evidence.steps.len() <= usize::from(query.limit)
            && first.is_none_or(|first| first == start)
            && query.cursor.as_ref().is_none_or(|cursor| {
                cursor.scope == self.scope
                    && self
                        .evidence
                        .original
                        .as_ref()
                        .is_some_and(|original| original.source == cursor.source)
            })
    }
}
impl OriginalCursor {
    pub fn validate(&self) -> Result<()> {
        self.scope.validate()?;
        source(&self.source)?;
        digest(&self.digest)?;
        if self.next_byte > MAX_SOURCE_BYTES {
            return fail(Code::Bounds, "task original cursor exceeds its bound");
        }
        digest(&self.prefix_digest)
    }
}
impl OriginalQuery {
    pub fn validate(&self) -> Result<()> {
        self.scope.validate()?;
        self.original.validate()?;
        limit(self.limit as usize, MAX_CHUNK_BYTES)?;
        if let Some(cursor) = &self.cursor {
            cursor.validate()?;
            if cursor.scope != self.scope
                || cursor.source != self.original.source
                || cursor.digest != self.original.digest
                || cursor.next_byte > self.original.bytes
            {
                return fail(Code::Malformed, "task original cursor changes its pin");
            }
        }
        Ok(())
    }
}
impl OriginalChunk {
    pub fn validate(&self) -> Result<()> {
        self.scope.validate()?;
        self.original.validate()?;
        if self.data.len() > MAX_CHUNK_BYTES.div_ceil(3) * 4 {
            return fail(Code::Bounds, "task original chunk exceeds its bound");
        }
        let bytes = STANDARD
            .decode(&self.data)
            .map_err(|_| Error::new(Code::Malformed, "task original chunk is not base64"))?;
        if bytes.len() > MAX_CHUNK_BYTES
            || self.start > self.original.bytes
            || bytes.len() as u64 > self.original.bytes.saturating_sub(self.start)
        {
            return fail(Code::Bounds, "task original chunk exceeds its pin");
        }
        let end = self.start + bytes.len() as u64;
        if self.more_available != (end < self.original.bytes)
            || self.more_available && self.next.is_none()
            || self.more_available && bytes.is_empty()
        {
            return fail(
                Code::Malformed,
                "task original continuation makes no progress",
            );
        }
        if let Some(next) = &self.next {
            next.validate()?;
            if next.scope != self.scope
                || next.source != self.original.source
                || next.digest != self.original.digest
                || next.next_byte != end
            {
                return fail(
                    Code::Malformed,
                    "task original continuation changes its pin",
                );
            }
        }
        bounded(self, MAX_REPLY_BYTES)
    }
    pub fn answers(&self, query: &OriginalQuery) -> bool {
        self.scope == query.scope
            && self.original == query.original
            && self.start == query.cursor.as_ref().map_or(0, |cursor| cursor.next_byte)
            && STANDARD
                .decode(&self.data)
                .is_ok_and(|bytes| bytes.len() <= query.limit as usize)
    }
}

#[cfg(test)]
mod tests;
