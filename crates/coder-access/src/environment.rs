//! Project environments (ENV-07): the native projection a browser panel
//! reads, paged retained evidence, and the reviewed Promote, Select, and
//! setup steering intents an operator owner answers.
//!
//! These DTOs carry identities, states, digests, and user-visible text
//! only. They never carry credential values, recipe scripts, or command
//! environments; command arguments stay in the retained evidence, which is
//! redacted before it is spooled.

use crate::{Code, Result, fail};
use serde::{Deserialize, Serialize};

/// Rows of one kind a view carries; older rows page or stay in the record.
pub const MAX_ROWS: usize = 16;
pub const MAX_STEPS: usize = 8;
pub const MAX_CALLS: usize = 48;
pub const MAX_GAPS: usize = 24;
pub const MAX_CHUNK_BYTES: u32 = 16 * 1024;
pub const MAX_STEER_BYTES: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub workspace: String,
    pub project: String,
    /// The environment to show; `None` shows the project's one live
    /// environment, if any.
    pub environment: Option<String>,
    /// Page saved versions older than this version number.
    pub before: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Listed {
    pub id: String,
    pub retired: bool,
    pub active: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub repository: Option<String>,
    pub revision: String,
    pub digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub image_id: String,
    pub snapshot_id: Option<String>,
    pub manifest_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub revision: u64,
    pub digest: String,
    pub created_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub state: String,
    pub at_ms: u64,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Build {
    pub id: String,
    pub recipe_revision: u64,
    pub recipe_digest: String,
    pub state: String,
    /// A later recipe edit made this build unusable for verify or save.
    pub stale: bool,
    /// An outcome the owner could not observe; it must be reconciled.
    pub unresolved: Option<String>,
    pub job: Option<String>,
    pub image: Option<Image>,
    pub steps: Vec<Step>,
    pub created_ms: u64,
}

/// Exactly what a reviewed Save of one passed verification would record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub digest: String,
    pub recipe_revision: u64,
    pub recipe_digest: String,
    pub source_revision: String,
    pub image: Image,
    pub plan_digest: String,
    pub evidence_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Verification {
    pub id: String,
    pub build_id: String,
    pub state: String,
    pub unresolved: Option<String>,
    pub job: Option<String>,
    pub evidence_status: Option<String>,
    pub evidence_digest: Option<String>,
    /// The owner can page this verification's retained evidence.
    pub evidence_readable: bool,
    /// Present only while a reviewed Save of this attempt is admissible.
    pub candidate: Option<Candidate>,
    pub steps: Vec<Step>,
    pub created_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Version {
    pub id: String,
    pub number: u64,
    pub created_ms: u64,
    pub recipe_revision: u64,
    pub source_revision: String,
    pub image: Image,
    pub evidence_digest: String,
    pub selected: bool,
    pub reviewer: Option<String>,
    pub selected_at: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub revision: u64,
    pub kind: String,
    pub version_id: String,
    pub previous: Option<String>,
    pub at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Steering {
    pub at_ms: u64,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupCommand {
    pub id: String,
    pub purpose: String,
    pub state: String,
}

/// One setup session: its state, the user's steering, and command states.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    pub id: String,
    pub state: String,
    /// The question the setup paused on, while it awaits input.
    pub question: Option<String>,
    pub reason: Option<String>,
    pub objective: String,
    pub steering: Vec<Steering>,
    pub commands: Vec<SetupCommand>,
    pub recipe_revisions: Vec<u64>,
    pub updated_ms: u64,
    pub steerable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detail {
    pub id: String,
    /// Record revision; it increases on every retained change.
    pub revision: u64,
    pub retired: bool,
    pub source: Source,
    pub draft_revision: u64,
    pub draft_digest: String,
    pub recipes: Vec<Recipe>,
    pub builds: Vec<Build>,
    pub verifications: Vec<Verification>,
    pub selection_revision: u64,
    pub active: Option<String>,
    pub history: Vec<Version>,
    /// Continue history with `Query::before` set to this version number.
    pub history_before: Option<u64>,
    pub changes: Vec<Change>,
    /// `None` when no setup owner is composed with this operator.
    pub setup: Option<Vec<Setup>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub workspace: String,
    pub project: String,
    pub environments: Vec<Listed>,
    /// `None` is the empty state: no environment for this project yet.
    pub detail: Option<Box<Detail>>,
}

/// The saved version a Cloud job started with (ENV-06).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    pub environment: String,
    pub version_id: String,
    pub number: u64,
    pub selection_revision: u64,
    pub recipe_revision: u64,
    pub source_revision: String,
    pub image: Image,
    pub evidence_digest: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stream {
    Stdout,
    Stderr,
}

/// A byte position in one retained stream, bound to the record that
/// produced it; a cursor from a rewritten or other record is refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceCursor {
    pub evidence_id: String,
    pub call: String,
    pub stream: Stream,
    pub offset: u64,
    pub chunk_seq: Option<u64>,
    pub chunk_digest: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceQuery {
    pub workspace: String,
    pub project: String,
    pub environment: String,
    pub verification: String,
    /// An archived child record (one verifier machine), by evidence ID.
    pub child: Option<String>,
    /// The call and stream to page; `None` reads the summary only.
    pub call: Option<String>,
    pub stream: Option<Stream>,
    pub cursor: Option<EvidenceCursor>,
    pub limit: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Call {
    pub call: String,
    pub tool: String,
    pub outcome: String,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chunk {
    pub call: String,
    pub stream: Stream,
    pub start: u64,
    /// Base64 of verified original retained bytes.
    pub data: String,
    pub digest: String,
    /// Retained bytes the log accounts for now.
    pub length: u64,
    pub closed: bool,
    pub next: Option<EvidenceCursor>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidencePage {
    pub workspace: String,
    pub project: String,
    pub environment: String,
    pub verification: String,
    pub child: Option<String>,
    pub evidence_id: String,
    pub status: String,
    pub complete: bool,
    pub sealed_digest: Option<String>,
    pub head_seq: u64,
    pub calls: Vec<Call>,
    pub calls_omitted: bool,
    pub children: Vec<String>,
    /// Disclosed gaps, each a compact typed description.
    pub gaps: Vec<String>,
    pub gaps_omitted: bool,
    pub chunk: Option<Chunk>,
}

/// Reviewed Save plus selection of the exact displayed candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Promote {
    pub workspace: String,
    pub project: String,
    pub environment: String,
    pub verification: String,
    pub candidate_digest: String,
    pub expected_selection_revision: u64,
}

/// Select a saved version; an earlier one is a rollback.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Select {
    pub workspace: String,
    pub project: String,
    pub environment: String,
    pub version: String,
    pub expected_selection_revision: u64,
}

/// User steering for one setup session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Steer {
    pub workspace: String,
    pub project: String,
    pub environment: String,
    pub session: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Accepted {
    pub request: String,
    pub workspace: String,
    pub project: String,
    pub environment: String,
    pub action: String,
    pub state: String,
    pub version: Option<String>,
    pub selection_revision: Option<u64>,
}

/// Opaque environment identity, as `coder-environment` admits it.
pub fn id(v: &str) -> Result<()> {
    if v.is_empty()
        || v.len() > 128
        || v.starts_with('.')
        || !v
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
    {
        return fail(Code::Malformed, "Environment identity is invalid.");
    }
    Ok(())
}
/// Lowercase hex SHA-256, as environment records retain it.
pub fn hex(v: &str) -> Result<()> {
    if v.len() != 64
        || !v
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return fail(Code::Malformed, "Environment digest is invalid.");
    }
    Ok(())
}
fn text(v: &str, max: usize) -> Result<()> {
    if v.len() > max || v.contains('\0') {
        return fail(Code::Bounds, "Environment text exceeds its bounds.");
    }
    Ok(())
}
fn context(workspace: &str, project: &str) -> Result<()> {
    crate::cloud::alias(workspace)?;
    crate::cloud::alias(project)
}
fn rows<T>(v: &[T], max: usize) -> Result<()> {
    if v.len() > max {
        return fail(Code::Bounds, "Environment rows exceed their limit.");
    }
    Ok(())
}
fn bound(v: &impl Serialize) -> Result<()> {
    crate::task_read::bounded(v, crate::cloud::MAX_REPLY_BYTES)
}

impl Query {
    pub fn validate(&self) -> Result<()> {
        context(&self.workspace, &self.project)?;
        if let Some(e) = &self.environment {
            id(e)?;
        }
        Ok(())
    }
}
impl Image {
    pub fn validate(&self) -> Result<()> {
        text(&self.image_id, 256)?;
        if let Some(s) = &self.snapshot_id {
            text(s, 256)?;
        }
        hex(&self.manifest_digest)
    }
}
impl Source {
    fn validate(&self) -> Result<()> {
        if let Some(r) = &self.repository {
            text(r, 512)?;
        }
        text(&self.revision, 64)?;
        hex(&self.digest)
    }
}
impl Step {
    fn validate(&self) -> Result<()> {
        text(&self.state, 64)?;
        if let Some(r) = &self.reason {
            text(r, 1024)?;
        }
        Ok(())
    }
}
impl Detail {
    fn validate(&self) -> Result<()> {
        id(&self.id)?;
        self.source.validate()?;
        hex(&self.draft_digest)?;
        rows(&self.recipes, MAX_ROWS)?;
        rows(&self.builds, MAX_ROWS)?;
        rows(&self.verifications, MAX_ROWS)?;
        rows(&self.history, MAX_ROWS)?;
        rows(&self.changes, MAX_ROWS)?;
        for r in &self.recipes {
            hex(&r.digest)?;
        }
        for b in &self.builds {
            id(&b.id)?;
            hex(&b.recipe_digest)?;
            text(&b.state, 64)?;
            rows(&b.steps, MAX_STEPS)?;
            for s in &b.steps {
                s.validate()?;
            }
            if let Some(i) = &b.image {
                i.validate()?;
            }
            for v in [&b.unresolved, &b.job].into_iter().flatten() {
                text(v, 1024)?;
            }
        }
        for v in &self.verifications {
            id(&v.id)?;
            id(&v.build_id)?;
            text(&v.state, 64)?;
            rows(&v.steps, MAX_STEPS)?;
            for s in &v.steps {
                s.validate()?;
            }
            for t in [&v.unresolved, &v.job, &v.evidence_status]
                .into_iter()
                .flatten()
            {
                text(t, 1024)?;
            }
            if let Some(d) = &v.evidence_digest {
                hex(d)?;
            }
            if let Some(c) = &v.candidate {
                hex(&c.digest)?;
                hex(&c.recipe_digest)?;
                hex(&c.plan_digest)?;
                hex(&c.evidence_digest)?;
                c.image.validate()?;
            }
        }
        for v in &self.history {
            id(&v.id)?;
            v.image.validate()?;
            hex(&v.evidence_digest)?;
            rows(&v.selected_at, MAX_ROWS)?;
        }
        if let Some(a) = &self.active {
            id(a)?;
        }
        for c in &self.changes {
            id(&c.version_id)?;
            text(&c.kind, 32)?;
        }
        if let Some(setup) = &self.setup {
            rows(setup, MAX_ROWS)?;
            for s in setup {
                id(&s.id)?;
                text(&s.state, 64)?;
                text(&s.objective, 4096)?;
                for t in [&s.question, &s.reason].into_iter().flatten() {
                    text(t, 4096)?;
                }
                rows(&s.steering, MAX_ROWS)?;
                rows(&s.commands, MAX_ROWS)?;
                rows(&s.recipe_revisions, MAX_ROWS)?;
                for t in &s.steering {
                    text(&t.text, MAX_STEER_BYTES)?;
                }
                for c in &s.commands {
                    id(&c.id)?;
                    text(&c.purpose, 64)?;
                    text(&c.state, 64)?;
                }
            }
        }
        Ok(())
    }
}
impl View {
    pub fn validate(&self) -> Result<()> {
        context(&self.workspace, &self.project)?;
        rows(&self.environments, MAX_ROWS)?;
        for e in &self.environments {
            id(&e.id)?;
            if let Some(a) = &e.active {
                id(a)?;
            }
        }
        if let Some(d) = &self.detail {
            d.validate()?;
            if !self.environments.iter().any(|e| e.id == d.id) {
                return fail(Code::Malformed, "The environment detail is not listed.");
            }
        }
        bound(self)
    }
    pub fn answers(&self, q: &Query) -> bool {
        self.workspace == q.workspace
            && self.project == q.project
            && q.environment
                .as_ref()
                .is_none_or(|e| self.detail.as_ref().is_some_and(|d| d.id == *e))
    }
}
impl Pin {
    pub fn validate(&self) -> Result<()> {
        id(&self.environment)?;
        id(&self.version_id)?;
        text(&self.source_revision, 64)?;
        self.image.validate()?;
        hex(&self.evidence_digest)
    }
}
impl EvidenceCursor {
    fn validate(&self) -> Result<()> {
        id(&self.evidence_id)?;
        id(&self.call)?;
        if let Some(d) = &self.chunk_digest {
            hex(d)?;
        }
        Ok(())
    }
}
impl EvidenceQuery {
    pub fn validate(&self) -> Result<()> {
        context(&self.workspace, &self.project)?;
        id(&self.environment)?;
        id(&self.verification)?;
        if let Some(c) = &self.child {
            id(c)?;
        }
        if let Some(c) = &self.call {
            id(c)?;
        }
        if self.call.is_some() != self.stream.is_some()
            || (self.cursor.is_some() && self.call.is_none())
        {
            return fail(Code::Malformed, "An evidence page names a call and stream.");
        }
        if self.limit == 0 || self.limit > MAX_CHUNK_BYTES {
            return fail(Code::Bounds, "Evidence byte limit is invalid.");
        }
        if let Some(c) = &self.cursor {
            c.validate()?;
            if Some(&c.call) != self.call.as_ref() || Some(c.stream) != self.stream {
                return fail(Code::Stale, "The evidence cursor names another stream.");
            }
        }
        Ok(())
    }
}
impl EvidencePage {
    pub fn validate(&self) -> Result<()> {
        use base64::Engine;
        context(&self.workspace, &self.project)?;
        id(&self.environment)?;
        id(&self.verification)?;
        id(&self.evidence_id)?;
        text(&self.status, 64)?;
        if let Some(d) = &self.sealed_digest {
            hex(d)?;
        }
        rows(&self.calls, MAX_CALLS)?;
        rows(&self.children, MAX_CALLS)?;
        rows(&self.gaps, MAX_GAPS)?;
        for c in &self.calls {
            id(&c.call)?;
            text(&c.tool, 128)?;
            text(&c.outcome, 64)?;
        }
        for c in &self.children {
            id(c)?;
        }
        for g in &self.gaps {
            text(g, 512)?;
        }
        if let Some(c) = &self.chunk {
            id(&c.call)?;
            hex(&c.digest)?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&c.data)
                .map_err(|_| crate::Error::new(Code::Malformed, "Evidence bytes are invalid."))?;
            if bytes.len() > MAX_CHUNK_BYTES as usize
                || c.start.saturating_add(bytes.len() as u64) > c.length
            {
                return fail(Code::Bounds, "Evidence chunk exceeds its bounds.");
            }
            if let Some(n) = &c.next {
                n.validate()?;
                if n.call != c.call || n.stream != c.stream || n.evidence_id != self.evidence_id {
                    return fail(Code::Malformed, "Evidence chunk cursor changed.");
                }
            }
        }
        bound(self)
    }
    pub fn answers(&self, q: &EvidenceQuery) -> bool {
        use base64::Engine;
        self.workspace == q.workspace
            && self.project == q.project
            && self.environment == q.environment
            && self.verification == q.verification
            && self.child == q.child
            && match (&self.chunk, &q.call) {
                (None, None) => true,
                (Some(c), Some(call)) => {
                    c.call == *call
                        && Some(c.stream) == q.stream
                        && c.start == q.cursor.as_ref().map_or(c.start, |k| k.offset)
                        && q.cursor
                            .as_ref()
                            .is_none_or(|k| k.evidence_id == self.evidence_id)
                        && base64::engine::general_purpose::STANDARD
                            .decode(&c.data)
                            .is_ok_and(|b| b.len() <= q.limit as usize)
                }
                _ => false,
            }
    }
}
impl Promote {
    pub fn validate(&self) -> Result<()> {
        context(&self.workspace, &self.project)?;
        id(&self.environment)?;
        id(&self.verification)?;
        hex(&self.candidate_digest)
    }
}
impl Select {
    pub fn validate(&self) -> Result<()> {
        context(&self.workspace, &self.project)?;
        id(&self.environment)?;
        id(&self.version)
    }
}
impl Steer {
    pub fn validate(&self) -> Result<()> {
        context(&self.workspace, &self.project)?;
        id(&self.environment)?;
        id(&self.session)?;
        if self.text.trim().is_empty() || self.text.len() > MAX_STEER_BYTES {
            return fail(Code::Bounds, "Steering needs 1 to 4096 bytes.");
        }
        text(&self.text, MAX_STEER_BYTES)
    }
}
impl Accepted {
    pub fn validate(&self) -> Result<()> {
        crate::cloud::alias(&self.request)?;
        context(&self.workspace, &self.project)?;
        id(&self.environment)?;
        if !matches!(self.action.as_str(), "promote" | "select" | "steer") {
            return fail(Code::Malformed, "Environment action is invalid.");
        }
        text(&self.state, 128)?;
        if let Some(v) = &self.version {
            id(v)?;
        }
        bound(self)
    }
    /// Whether this accepts `op` under its original request identity.
    pub fn answers(&self, op: &crate::Operation) -> bool {
        use crate::Operation;
        let (workspace, project, environment, action) = match op {
            Operation::EnvironmentPromote { intent } => (
                &intent.workspace,
                &intent.project,
                &intent.environment,
                "promote",
            ),
            Operation::EnvironmentSelect { intent } => (
                &intent.workspace,
                &intent.project,
                &intent.environment,
                "select",
            ),
            Operation::EnvironmentSteer { intent } => (
                &intent.workspace,
                &intent.project,
                &intent.environment,
                "steer",
            ),
            _ => return false,
        };
        self.workspace == *workspace
            && self.project == *project
            && self.environment == *environment
            && self.action == action
            && match op {
                Operation::EnvironmentSelect { intent } => {
                    self.version.as_deref() == Some(intent.version.as_str())
                }
                _ => true,
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Operation, Outcome};

    fn select(version: &str) -> Operation {
        Operation::EnvironmentSelect {
            intent: Select {
                workspace: "checkout".into(),
                project: "synthetic".into(),
                environment: "env-1".into(),
                version: version.into(),
                expected_selection_revision: 1,
            },
        }
    }

    #[test]
    fn reads_need_observe_and_effects_need_operate_and_retain_replies() {
        let read = Operation::EnvironmentRead {
            query: Query {
                workspace: "checkout".into(),
                project: "synthetic".into(),
                environment: None,
                before: None,
            },
        };
        assert!(read.reads_only() && read.validate().is_ok());
        assert_eq!(read.required(), Some(crate::Right::Observe));
        assert!(!select("v1").reads_only());
        assert_eq!(select("v1").required(), Some(crate::Right::Operate));
        let json = serde_json::to_value(select("v1")).unwrap();
        assert_eq!(json["kind"], "environment.select");
    }

    #[test]
    fn accepted_answers_only_its_own_action_and_version() {
        let accepted = Accepted {
            request: "req-1".into(),
            workspace: "checkout".into(),
            project: "synthetic".into(),
            environment: "env-1".into(),
            action: "select".into(),
            state: "rolled_back".into(),
            version: Some("v1".into()),
            selection_revision: Some(2),
        };
        let outcome = Outcome::EnvironmentAccepted {
            accepted: accepted.clone(),
        };
        assert!(outcome.validate().is_ok());
        assert!(outcome.answers(&select("v1")));
        assert!(!outcome.answers(&select("v2")));
        let mut wrong = accepted;
        wrong.action = "promote".into();
        assert!(!Outcome::EnvironmentAccepted { accepted: wrong }.answers(&select("v1")));
    }

    #[test]
    fn steering_and_evidence_queries_are_bounded() {
        let steer = |text: &str| Steer {
            workspace: "checkout".into(),
            project: "synthetic".into(),
            environment: "env-1".into(),
            session: "setup-1".into(),
            text: text.into(),
        };
        assert!(steer("Use the locked toolchain.").validate().is_ok());
        assert!(steer("  ").validate().is_err());
        assert!(steer(&"x".repeat(MAX_STEER_BYTES + 1)).validate().is_err());
        let mut q = EvidenceQuery {
            workspace: "checkout".into(),
            project: "synthetic".into(),
            environment: "env-1".into(),
            verification: "verify-1".into(),
            child: None,
            call: Some("check-1".into()),
            stream: None,
            cursor: None,
            limit: 1024,
        };
        // A call needs its stream; a cursor must name the same stream.
        assert!(q.validate().is_err());
        q.stream = Some(Stream::Stdout);
        assert!(q.validate().is_ok());
        q.cursor = Some(EvidenceCursor {
            evidence_id: "verify".into(),
            call: "check-1".into(),
            stream: Stream::Stderr,
            offset: 4,
            chunk_seq: None,
            chunk_digest: None,
        });
        assert!(q.validate().is_err());
        q.cursor = None;
        q.limit = MAX_CHUNK_BYTES + 1;
        assert!(q.validate().is_err());
    }
}
