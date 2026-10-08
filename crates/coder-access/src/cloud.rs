//! Operator cloud jobs under explicit native project and executor profiles.

use crate::{Code, Result, fail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_REPLY_BYTES: usize = 48 * 1024;
pub const MAX_CHUNK_BYTES: u32 = 16 * 1024;
pub const MAX_ITEMS: u16 = 64;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Projects {
    pub workspace: String,
    pub projects: Vec<String>,
    pub digest: String,
}
impl Projects {
    pub fn validate(&self) -> Result<()> {
        alias(&self.workspace)?;
        digest(&self.digest)?;
        if self.projects.len() > 64 {
            return fail(Code::Bounds, "Operator project aliases exceed their limit.");
        }
        for p in &self.projects {
            alias(p)?;
        }
        bound(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogQuery {
    pub workspace: String,
    pub project: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    pub revision: String,
    pub source_revision: String,
    pub source_digest: String,
    pub placement: String,
    pub pool: String,
    pub mode: String,
    pub executor: String,
    pub model: Option<String>,
    pub credential_names: Vec<String>,
    pub max_timeout_seconds: u64,
    pub availability: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub workspace: String,
    pub project: String,
    pub profiles: Vec<Profile>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub workspace: String,
    pub project: String,
    pub job: String,
    pub revision: String,
    pub attempt: u64,
    pub profile: String,
    pub profile_revision: String,
    pub source_digest: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListCursor {
    pub workspace: String,
    pub project: String,
    pub digest: String,
    pub next: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListQuery {
    pub workspace: String,
    pub project: String,
    pub cursor: Option<ListCursor>,
    pub limit: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub scope: Scope,
    pub state: String,
    pub executor: String,
    pub model: Option<String>,
    pub cleanup: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct List {
    pub workspace: String,
    pub project: String,
    pub digest: String,
    pub rows: Vec<Row>,
    pub next: Option<ListCursor>,
    pub more_available: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadQuery {
    pub workspace: String,
    pub project: String,
    pub job: String,
    pub revision: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Original {
    pub source: String,
    pub digest: String,
    pub bytes: u64,
    pub media_type: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub scope: Scope,
    pub state: String,
    pub placement: String,
    pub mode: String,
    pub executor: String,
    pub model: Option<String>,
    pub served_model: Option<String>,
    pub pool: String,
    pub prompt: String,
    pub prompt_omitted: bool,
    pub credential_names: Vec<String>,
    pub remote_task: Option<String>,
    pub continuation: String,
    pub cancellation: String,
    pub cleanup: String,
    pub artifact_state: String,
    pub usage: Option<Value>,
    pub error: Option<String>,
    pub details_omitted: bool,
    pub originals: Vec<Original>,
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Submit {
    pub workspace: String,
    pub project: String,
    pub profile: String,
    pub profile_revision: String,
    pub source_digest: String,
    pub prompt: String,
    pub timeout_seconds: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Continue {
    pub scope: Scope,
    pub prompt: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cancel {
    pub scope: Scope,
    pub reason: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Follow {
    pub scope: Scope,
}
impl Follow {
    pub fn validate(&self) -> Result<()> {
        self.scope.validate()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Accepted {
    pub request: String,
    pub scope: Scope,
    pub action: String,
    pub state: String,
}
/// Domain admission metadata retained for recovery, without prompts or secrets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Admission {
    pub workspace: String,
    pub project: String,
    pub profile: String,
    pub profile_revision: String,
    pub source_digest: String,
    pub job: Option<String>,
}
impl Admission {
    pub fn validate(&self) -> Result<()> {
        context(&self.workspace, &self.project)?;
        alias(&self.profile)?;
        digest(&self.profile_revision)?;
        digest(&self.source_digest)?;
        if let Some(job) = &self.job {
            alias(job)?;
        }
        Ok(())
    }
    pub fn for_operation(op: &crate::Operation) -> Option<Self> {
        match op {
            crate::Operation::CloudSubmit { intent } => Some(Self {
                workspace: intent.workspace.clone(),
                project: intent.project.clone(),
                profile: intent.profile.clone(),
                profile_revision: intent.profile_revision.clone(),
                source_digest: intent.source_digest.clone(),
                job: None,
            }),
            crate::Operation::CloudContinue { intent } => Some(Self::from(&intent.scope)),
            crate::Operation::CloudCancel { intent } => Some(Self::from(&intent.scope)),
            crate::Operation::CloudFollow { intent } => Some(Self::from(&intent.scope)),
            _ => None,
        }
    }
}
impl From<&Scope> for Admission {
    fn from(s: &Scope) -> Self {
        Self {
            workspace: s.workspace.clone(),
            project: s.project.clone(),
            profile: s.profile.clone(),
            profile_revision: s.profile_revision.clone(),
            source_digest: s.source_digest.clone(),
            job: Some(s.job.clone()),
        }
    }
}

fn text(v: &str, max: usize, empty: bool) -> Result<()> {
    if (!empty && v.is_empty()) || v.len() > max || v.contains('\0') {
        return fail(Code::Bounds, "Operator cloud text is invalid.");
    }
    Ok(())
}
pub fn alias(v: &str) -> Result<()> {
    if v.is_empty()
        || !v.bytes().next().is_some_and(|b| b.is_ascii_alphanumeric())
        || v.len() > 128
        || !v
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.:-".contains(&b))
    {
        return fail(Code::Malformed, "Operator cloud alias is invalid.");
    }
    Ok(())
}
pub fn digest(v: &str) -> Result<()> {
    if !v.strip_prefix("sha256:").is_some_and(|v| {
        v.len() == 64
            && v.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }) {
        return fail(Code::Malformed, "Operator cloud digest is invalid.");
    }
    Ok(())
}
fn context(workspace: &str, project: &str) -> Result<()> {
    alias(workspace)?;
    alias(project)
}
fn bound(v: &impl Serialize) -> Result<()> {
    crate::task_read::bounded(v, MAX_REPLY_BYTES)
}
impl CatalogQuery {
    pub fn validate(&self) -> Result<()> {
        context(&self.workspace, &self.project)
    }
}
impl Profile {
    pub fn validate(&self) -> Result<()> {
        alias(&self.name)?;
        digest(&self.revision)?;
        text(&self.source_revision, 128, false)?;
        digest(&self.source_digest)?;
        for v in [
            &self.placement,
            &self.pool,
            &self.mode,
            &self.executor,
            &self.availability,
        ] {
            text(v, 128, false)?;
        }
        if let Some(v) = &self.model {
            text(v, 128, false)?;
        }
        if self.credential_names.len() > 32
            || self.max_timeout_seconds == 0
            || self.max_timeout_seconds > 43200
        {
            return fail(Code::Bounds, "Operator cloud profile exceeds its limits.");
        }
        for v in &self.credential_names {
            alias(v)?;
        }
        if self.placement == "gce" && self.mode == "integrated" {
            return fail(Code::Unsupported, "GCE integrated agents are unavailable.");
        }
        Ok(())
    }
}
impl Catalog {
    pub fn validate(&self) -> Result<()> {
        context(&self.workspace, &self.project)?;
        if self.profiles.len() > 64 {
            return fail(Code::Bounds, "Operator cloud catalog exceeds its limit.");
        }
        for p in &self.profiles {
            p.validate()?;
        }
        bound(self)
    }
    pub fn answers(&self, q: &CatalogQuery) -> bool {
        self.workspace == q.workspace && self.project == q.project
    }
}
impl Scope {
    pub fn validate(&self) -> Result<()> {
        context(&self.workspace, &self.project)?;
        alias(&self.job)?;
        alias(&self.profile)?;
        digest(&self.revision)?;
        digest(&self.profile_revision)?;
        digest(&self.source_digest)?;
        if self.attempt == 0 {
            return fail(Code::Malformed, "Operator cloud attempt is invalid.");
        }
        Ok(())
    }
}
impl ListQuery {
    pub fn validate(&self) -> Result<()> {
        context(&self.workspace, &self.project)?;
        if self.limit == 0 || self.limit > MAX_ITEMS {
            return fail(Code::Bounds, "Operator cloud list limit is invalid.");
        }
        if let Some(c) = &self.cursor {
            context(&c.workspace, &c.project)?;
            digest(&c.digest)?;
            if c.workspace != self.workspace || c.project != self.project {
                return fail(Code::Stale, "Operator cloud cursor changed scope.");
            }
        }
        Ok(())
    }
}
impl List {
    pub fn validate(&self) -> Result<()> {
        context(&self.workspace, &self.project)?;
        digest(&self.digest)?;
        if self.rows.len() > 64 {
            return fail(Code::Bounds, "Operator cloud list exceeds its limit.");
        }
        for row in &self.rows {
            row.scope.validate()?;
            if row.scope.workspace != self.workspace || row.scope.project != self.project {
                return fail(Code::Malformed, "Operator cloud row changed scope.");
            }
            for v in [&row.state, &row.executor, &row.cleanup] {
                text(v, 128, false)?;
            }
            if let Some(model) = &row.model {
                text(model, 128, false)?;
            }
        }
        if self.more_available != self.next.is_some() {
            return fail(Code::Malformed, "Operator cloud list cursor is missing.");
        }
        if let Some(c) = &self.next {
            if c.workspace != self.workspace || c.project != self.project || c.digest != self.digest
            {
                return fail(Code::Malformed, "Operator cloud list cursor changed scope.");
            }
        }
        bound(self)
    }
    pub fn answers(&self, q: &ListQuery) -> bool {
        self.workspace == q.workspace
            && self.project == q.project
            && q.cursor.as_ref().is_none_or(|c| c.digest == self.digest)
    }
}
impl ReadQuery {
    pub fn validate(&self) -> Result<()> {
        context(&self.workspace, &self.project)?;
        alias(&self.job)?;
        if let Some(v) = &self.revision {
            digest(v)?;
        }
        Ok(())
    }
}
impl Original {
    pub fn validate(&self) -> Result<()> {
        alias(&self.source)?;
        digest(&self.digest)?;
        text(&self.media_type, 128, false)?;
        if self.bytes > 64 * 1024 * 1024 {
            return fail(Code::Bounds, "Operator cloud source exceeds its limit.");
        }
        Ok(())
    }
}
impl Job {
    pub fn validate(&self) -> Result<()> {
        self.scope.validate()?;
        for v in [
            &self.state,
            &self.placement,
            &self.mode,
            &self.executor,
            &self.pool,
            &self.continuation,
            &self.cancellation,
            &self.cleanup,
            &self.artifact_state,
        ] {
            text(v, 128, false)?;
        }
        text(&self.prompt, 16384, true)?;
        for v in [&self.model, &self.served_model, &self.remote_task]
            .into_iter()
            .flatten()
        {
            text(v, 128, false)?;
        }
        if let Some(v) = &self.error {
            text(v, 4096, true)?;
        }
        if self.originals.len() > 64 || self.credential_names.len() > 32 {
            return fail(Code::Bounds, "Operator cloud details exceed their limit.");
        }
        for name in &self.credential_names {
            alias(name)?;
        }
        for o in &self.originals {
            o.validate()?;
        }
        bound(self)
    }
    pub fn answers(&self, q: &ReadQuery) -> bool {
        self.scope.workspace == q.workspace
            && self.scope.project == q.project
            && self.scope.job == q.job
            && q.revision
                .as_ref()
                .is_none_or(|r| *r == self.scope.revision)
    }
}
impl OriginalQuery {
    pub fn validate(&self) -> Result<()> {
        self.scope.validate()?;
        self.original.validate()?;
        if self.limit == 0 || self.limit > MAX_CHUNK_BYTES {
            return fail(Code::Bounds, "Operator cloud byte limit is invalid.");
        }
        if let Some(c) = &self.cursor {
            digest(&c.prefix_digest)?;
            if c.scope != self.scope
                || c.source != self.original.source
                || c.digest != self.original.digest
                || c.next_byte > self.original.bytes
            {
                return fail(Code::Stale, "Operator cloud source cursor changed.");
            }
        }
        Ok(())
    }
}
impl OriginalChunk {
    pub fn validate(&self) -> Result<()> {
        use base64::Engine;
        self.scope.validate()?;
        self.original.validate()?;
        if self.data.len() > (MAX_CHUNK_BYTES as usize).div_ceil(3) * 4 {
            return fail(
                Code::Bounds,
                "Operator cloud encoded bytes exceed their limit.",
            );
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&self.data)
            .map_err(|_| crate::Error::new(Code::Malformed, "Operator cloud bytes are invalid."))?;
        if bytes.len() > MAX_CHUNK_BYTES as usize
            || self.start.saturating_add(bytes.len() as u64) > self.original.bytes
            || self.more_available != self.next.is_some()
        {
            return fail(Code::Bounds, "Operator cloud chunk exceeds its bounds.");
        }
        if let Some(c) = &self.next {
            digest(&c.prefix_digest)?;
            if c.scope != self.scope
                || c.source != self.original.source
                || c.digest != self.original.digest
                || c.next_byte != self.start + bytes.len() as u64
            {
                return fail(Code::Malformed, "Operator cloud chunk cursor changed.");
            }
        }
        bound(self)
    }
    pub fn answers(&self, q: &OriginalQuery) -> bool {
        use base64::Engine;
        self.scope == q.scope
            && self.original == q.original
            && self.start == q.cursor.as_ref().map_or(0, |c| c.next_byte)
            && base64::engine::general_purpose::STANDARD
                .decode(&self.data)
                .is_ok_and(|bytes| bytes.len() <= q.limit as usize)
    }
}
impl Submit {
    pub fn validate(&self) -> Result<()> {
        context(&self.workspace, &self.project)?;
        alias(&self.profile)?;
        digest(&self.profile_revision)?;
        digest(&self.source_digest)?;
        text(&self.prompt, 16384, false)?;
        if self.timeout_seconds == 0 || self.timeout_seconds > 43200 {
            return fail(Code::Bounds, "Operator cloud deadline is invalid.");
        }
        Ok(())
    }
}
impl Continue {
    pub fn validate(&self) -> Result<()> {
        self.scope.validate()?;
        text(&self.prompt, 16384, false)
    }
}
impl Cancel {
    pub fn validate(&self) -> Result<()> {
        self.scope.validate()?;
        text(&self.reason, 4096, false)
    }
}
impl Accepted {
    pub fn validate(&self) -> Result<()> {
        alias(&self.request)?;
        self.scope.validate()?;
        if !matches!(
            self.action.as_str(),
            "submit" | "continue" | "cancel" | "follow"
        ) {
            return fail(Code::Malformed, "Operator cloud action is invalid.");
        }
        text(&self.state, 128, false)?;
        bound(self)
    }
}
