//! The workbench's shared contract: typed references to resources that
//! other owners keep, the capability directory an owner publishes, and the
//! intents a surface sends to an owner (`docs/terminal/workbench-resources.md`).
//!
//! A reference names a resource; it is not a copy of the record and not a
//! permission. Terminals, threads, runs, studio records, artifacts, files,
//! tools, and evidence stay in their owners. A surface (the desktop window,
//! Verse's overlay, a phone, a headless client) asks the owner through
//! [`dispatch`], which checks the owner's directory first and the owner's
//! answer after, so a surface never draws a substitute as the resource it
//! asked for.
//!
//! This crate has no Verse, window, platform, network, wallet, or scheduler
//! dependency, and it runs nothing.

use serde::{Deserialize, Serialize};

pub mod pane;

/// `v` of a resource reference.
pub const RESOURCE: &str = "openagents.workbench-resource.v1";
/// `v` of an owner's capability directory.
pub const DIRECTORY: &str = "openagents.workbench-directory.v1";
/// `v` of a surface intent.
pub const INTENT: &str = "openagents.workbench-intent.v1";
/// `v` of an owner's outcome.
pub const OUTCOME: &str = "openagents.workbench-outcome.v1";

/// The longest token ID: a thread, run, studio, or evidence ID.
pub const TOKEN_MAX: usize = 128;
/// The longest file path or fallback link.
pub const PATH_MAX: usize = 1024;
/// The longest owner-provided summary.
pub const SUMMARY_MAX: usize = 2048;
/// The largest owner-intent input, as JSON.
pub const INPUT_MAX: usize = 4096;
/// The most capabilities, intents per capability, or operations a
/// directory lists.
pub const LIST_MAX: usize = 32;

/// What kind of resource a reference names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A host terminal (NIP-TERM) or a surface's local pane.
    Terminal,
    /// An OpenAgents chat thread.
    Thread,
    /// An agent run or durable task.
    Run,
    /// An Agent Studio record: a goal, seat, task, decision, or review.
    Studio,
    /// A retained artifact, by content digest.
    Artifact,
    /// A file in an admitted workspace, at an exact content revision.
    File,
    /// An admitted component or tool, at an exact version.
    Tool,
    /// Gym or check evidence.
    Evidence,
}

/// Which Agent Studio record a `studio` reference names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StudioPart {
    Goal,
    Seat,
    Task,
    Decision,
    Review,
}

/// The owner a reference belongs to.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Host {
    /// A paired host, by its NIP-HOST public key.
    Paired { key: String },
    /// A surface's own process, such as the standalone window's local
    /// panes, by a random instance ID minted when it starts.
    Local { instance: String },
}

impl Host {
    fn check(&self) -> Result<(), Refusal> {
        match self {
            Host::Paired { key } => common_id(key, "host key"),
            Host::Local { instance } => common_id(instance, "instance"),
        }
    }
}

/// A resource's revision.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Revision {
    /// A counter the owner increases on each change.
    Counter(u64),
    /// A SHA-256 content digest, 64 lowercase hexadecimal characters.
    Sha256(String),
    /// A published version string.
    Version(String),
}

impl Revision {
    fn check(&self) -> Result<(), Refusal> {
        match self {
            Revision::Counter(_) => Ok(()),
            Revision::Sha256(digest) => common_id(digest, "sha256"),
            Revision::Version(version) => token(version, 64, "version"),
        }
    }

    fn form(&self) -> u8 {
        match self {
            Revision::Counter(_) => 0,
            Revision::Sha256(_) => 1,
            Revision::Version(_) => 2,
        }
    }
}

/// A typed reference to a resource another owner keeps.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceRef {
    pub v: String,
    pub kind: Kind,
    pub host: Host,
    /// The owner's generation, when the resource lives only as long as
    /// one: required for a terminal, absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<String>,
    /// The admitted workspace: required for a file, optional for a
    /// terminal, run, or studio record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// Which studio record: required for `studio`, absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<StudioPart>,
    /// The owner's ID for the resource; its form depends on `kind`.
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<Revision>,
}

impl ResourceRef {
    /// A reference with no generation, workspace, part, or revision.
    #[must_use]
    pub fn new(kind: Kind, host: Host, id: impl Into<String>) -> Self {
        ResourceRef {
            v: RESOURCE.into(),
            kind,
            host,
            generation: None,
            workspace: None,
            part: None,
            id: id.into(),
            revision: None,
        }
    }

    /// A terminal on `host` in `generation`.
    #[must_use]
    pub fn terminal(host: Host, generation: impl Into<String>, id: impl Into<String>) -> Self {
        ResourceRef {
            generation: Some(generation.into()),
            ..ResourceRef::new(Kind::Terminal, host, id)
        }
    }

    #[must_use]
    pub fn with_revision(mut self, revision: Revision) -> Self {
        self.revision = Some(revision);
        self
    }

    #[must_use]
    pub fn in_workspace(mut self, workspace: impl Into<String>) -> Self {
        self.workspace = Some(workspace.into());
        self
    }

    #[must_use]
    pub fn studio(mut self, part: StudioPart) -> Self {
        self.part = Some(part);
        self
    }

    /// Whether `other` names the same resource, at any revision.
    #[must_use]
    pub fn same_resource(&self, other: &ResourceRef) -> bool {
        self.kind == other.kind
            && self.host == other.host
            && self.generation == other.generation
            && self.workspace == other.workspace
            && self.part == other.part
            && self.id == other.id
    }

    /// Checks the reference's shape and the rules its kind sets.
    pub fn check(&self) -> Result<(), Refusal> {
        version(&self.v, RESOURCE)?;
        self.host.check()?;
        if let Some(generation) = &self.generation {
            common_id(generation, "generation")?;
        }
        if let Some(workspace) = &self.workspace {
            common_id(workspace, "workspace")?;
        }
        if let Some(revision) = &self.revision {
            revision.check()?;
        }
        let rule = Rule::of(self.kind, self.part);
        need(self.generation.is_some(), rule.generation, "generation")?;
        need(self.workspace.is_some(), rule.workspace, "workspace")?;
        need(self.part.is_some(), rule.part, "part")?;
        match (&self.revision, rule.revision) {
            (None, Need::Required) => {
                return Err(Refusal::malformed(format!(
                    "a {:?} reference names its revision",
                    self.kind
                )));
            }
            (Some(_), Need::Absent) => {
                return Err(Refusal::malformed(format!(
                    "a {:?} reference has no revision",
                    self.kind
                )));
            }
            (Some(revision), _) if Some(revision.form()) != rule.form => {
                return Err(Refusal::malformed(format!(
                    "a {:?} reference's revision has another form",
                    self.kind
                )));
            }
            _ => {}
        }
        match self.kind {
            Kind::Terminal => slug(&self.id, 64, "terminal"),
            Kind::Artifact => common_id(&self.id, "artifact"),
            Kind::File => relative_path(&self.id),
            Kind::Tool => component(&self.id),
            Kind::Thread | Kind::Run | Kind::Studio | Kind::Evidence => {
                token(&self.id, TOKEN_MAX, "id")
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Need {
    Required,
    Optional,
    Absent,
}

struct Rule {
    generation: Need,
    workspace: Need,
    part: Need,
    revision: Need,
    /// The revision form a present revision takes.
    form: Option<u8>,
}

impl Rule {
    fn of(kind: Kind, part: Option<StudioPart>) -> Rule {
        use Need::{Absent, Optional, Required};
        let (generation, workspace, revision, form) = match kind {
            Kind::Terminal => (Required, Optional, Absent, None),
            Kind::Thread => (Absent, Absent, Optional, Some(0)),
            Kind::Run => (Absent, Optional, Optional, Some(0)),
            // A review is bound to the exact revision it reviews.
            Kind::Studio if part == Some(StudioPart::Review) => {
                (Absent, Optional, Required, Some(1))
            }
            Kind::Studio => (Absent, Optional, Optional, Some(0)),
            Kind::Artifact => (Absent, Absent, Absent, None),
            Kind::File => (Absent, Required, Required, Some(1)),
            Kind::Tool => (Absent, Absent, Required, Some(2)),
            Kind::Evidence => (Absent, Absent, Optional, Some(1)),
        };
        let part = if kind == Kind::Studio {
            Required
        } else {
            Absent
        };
        Rule {
            generation,
            workspace,
            part,
            revision,
            form,
        }
    }
}

fn need(present: bool, need: Need, what: &str) -> Result<(), Refusal> {
    match (present, need) {
        (false, Need::Required) => Err(Refusal::malformed(format!("{what} is required here"))),
        (true, Need::Absent) => Err(Refusal::malformed(format!("{what} is not allowed here"))),
        _ => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// Directories

/// What a surface may ask an owner to do with a resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// Show the resource in a pane that draws it natively.
    Open,
    /// Attach a live view (a terminal's output, a run's progress).
    Attach,
    /// End an attachment.
    Detach,
}

/// What a surface may show when it cannot draw a kind natively.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Fallback {
    /// An `https` address the owner serves for the resource; `{id}` in it
    /// is replaced with the reference's ID.
    Link { url: String },
    /// A bounded plain-text summary the owner writes per resource.
    Summary,
}

impl Fallback {
    fn check(&self) -> Result<(), Refusal> {
        match self {
            Fallback::Link { url } => link(url),
            Fallback::Summary => Ok(()),
        }
    }
}

/// What an owner serves for one kind.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capability {
    pub kind: Kind,
    pub operations: Vec<Operation>,
    /// Owner intents a surface may route, by name (`answer-decision`).
    pub intents: Vec<String>,
    /// What a surface shows instead of a native pane, when permitted.
    pub fallback: Option<Fallback>,
}

/// The kinds an owner serves and how. Discovery grants nothing: the owner
/// still checks the caller's rights on every intent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Directory {
    pub v: String,
    pub host: Host,
    /// The owner's current generation, when its resources have one.
    pub generation: Option<String>,
    pub capabilities: Vec<Capability>,
}

impl Directory {
    pub fn check(&self) -> Result<(), Refusal> {
        version(&self.v, DIRECTORY)?;
        self.host.check()?;
        if let Some(generation) = &self.generation {
            common_id(generation, "generation")?;
        }
        if self.capabilities.len() > LIST_MAX {
            return Err(Refusal::limit("a directory lists at most 32 capabilities"));
        }
        for (index, capability) in self.capabilities.iter().enumerate() {
            if self.capabilities[..index]
                .iter()
                .any(|c| c.kind == capability.kind)
            {
                return Err(Refusal::malformed("a directory lists a kind twice"));
            }
            if capability.intents.len() > LIST_MAX || capability.operations.len() > 3 {
                return Err(Refusal::limit("too many intents or operations"));
            }
            for intent in &capability.intents {
                slug(intent, 32, "intent")?;
            }
            if let Some(fallback) = &capability.fallback {
                fallback.check()?;
            }
        }
        Ok(())
    }

    /// The capability for `kind`, if the owner serves it.
    #[must_use]
    pub fn capability(&self, kind: Kind) -> Option<&Capability> {
        self.capabilities.iter().find(|c| c.kind == kind)
    }
}

// ---------------------------------------------------------------------------
// Intents and outcomes

/// Whether an attachment only reads, or also types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Observe,
    Interact,
}

/// What a surface asks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Open,
    Attach {
        mode: Mode,
    },
    Detach {
        attachment: String,
    },
    /// An operation the owner defines, routed to it unchanged. The
    /// surface never performs it itself.
    Owner {
        intent: String,
        input: serde_json::Value,
    },
}

/// One request from a surface to the resource's owner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intent {
    pub v: String,
    /// A common ID the surface chose; an owner applies it once.
    pub request: String,
    pub target: ResourceRef,
    pub action: Action,
}

impl Intent {
    #[must_use]
    pub fn new(request: impl Into<String>, target: ResourceRef, action: Action) -> Self {
        Intent {
            v: INTENT.into(),
            request: request.into(),
            target,
            action,
        }
    }

    pub fn check(&self) -> Result<(), Refusal> {
        version(&self.v, INTENT)?;
        common_id(&self.request, "request")?;
        self.target.check()?;
        match &self.action {
            Action::Open | Action::Attach { .. } => Ok(()),
            Action::Detach { attachment } => common_id(attachment, "attachment"),
            Action::Owner { intent, input } => {
                slug(intent, 32, "intent")?;
                if !input.is_object() {
                    return Err(Refusal::malformed("an owner intent's input is an object"));
                }
                if serde_json::to_vec(input).map_or(true, |json| json.len() > INPUT_MAX) {
                    return Err(Refusal::limit("an owner intent's input is over 4096 bytes"));
                }
                Ok(())
            }
        }
    }

    fn operation(&self) -> Option<Operation> {
        match self.action {
            Action::Open => Some(Operation::Open),
            Action::Attach { .. } => Some(Operation::Attach),
            Action::Detach { .. } => Some(Operation::Detach),
            Action::Owner { .. } => None,
        }
    }
}

/// What a surface shows as a permitted fallback.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Shown {
    Link { url: String },
    Summary { text: String },
}

/// What happened to an intent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum State {
    /// The surface may draw the resource natively.
    Opened,
    Attached {
        attachment: String,
    },
    Detached,
    /// The owner accepted an owner intent; `receipt` is its own record.
    Routed {
        receipt: String,
    },
    /// The surface cannot draw the kind; the owner permits this instead.
    Fallback {
        shown: Shown,
    },
    /// The owner cannot answer now. Nothing changed.
    Unavailable,
    /// The reference's revision is not current. The owner names the
    /// current one; the surface asks again only if the person chooses.
    Stale {
        current: Option<Revision>,
    },
    /// The resource ended in this generation and the owner keeps it no
    /// longer.
    Closed,
    /// The resource belonged to an earlier owner generation and did not
    /// survive it. Nothing replaces it.
    Lost,
    /// The owner does not serve this kind or operation.
    Unsupported,
    /// The owner refused, with a shared refusal code (`not_admitted`).
    Refused {
        reason: String,
    },
}

/// An owner's answer to one intent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outcome {
    pub v: String,
    pub request: String,
    /// Exactly the intent's target. An outcome about anything else is a
    /// substitution and is refused.
    pub target: ResourceRef,
    pub state: State,
}

impl Outcome {
    #[must_use]
    pub fn new(intent: &Intent, state: State) -> Self {
        Outcome {
            v: OUTCOME.into(),
            request: intent.request.clone(),
            target: intent.target.clone(),
            state,
        }
    }

    /// Checks that the outcome answers `intent` and nothing else.
    pub fn check(&self, intent: &Intent) -> Result<(), Refusal> {
        version(&self.v, OUTCOME)?;
        if self.request != intent.request {
            return Err(Refusal::new(
                Reason::IdentityMismatch,
                "an outcome for another request",
            ));
        }
        if self.target != intent.target {
            return Err(Refusal::new(
                Reason::IdentityMismatch,
                "an outcome names another resource, host, generation, or revision",
            ));
        }
        let fits = match (&self.state, &intent.action) {
            (State::Opened, Action::Open)
            | (State::Attached { .. }, Action::Attach { .. })
            | (State::Detached, Action::Detach { .. })
            | (State::Routed { .. }, Action::Owner { .. })
            | (State::Fallback { .. }, Action::Open) => true,
            (
                State::Opened | State::Attached { .. } | State::Detached | State::Routed { .. },
                _,
            )
            | (State::Fallback { .. }, _) => false,
            _ => true,
        };
        if !fits {
            return Err(Refusal::malformed(
                "an outcome that does not answer its action",
            ));
        }
        match &self.state {
            State::Attached { attachment } => common_id(attachment, "attachment"),
            State::Routed { receipt } => token(receipt, TOKEN_MAX, "receipt"),
            State::Fallback {
                shown: Shown::Link { url },
            } => link(url),
            State::Fallback {
                shown: Shown::Summary { text },
            } => {
                if text.len() > SUMMARY_MAX || text.chars().any(|c| c.is_control() && c != '\n') {
                    return Err(Refusal::malformed(
                        "a summary is plain text up to 2048 bytes",
                    ));
                }
                Ok(())
            }
            State::Stale { current } => match (current, &self.target.revision) {
                (Some(current), Some(asked)) if current == asked => Err(Refusal::malformed(
                    "a stale outcome names the same revision",
                )),
                (Some(current), Some(asked)) if current.form() != asked.form() => {
                    Err(Refusal::malformed("a current revision of another form"))
                }
                (Some(current), _) => current.check(),
                (None, _) => Ok(()),
            },
            State::Lost if self.target.generation.is_none() => Err(Refusal::malformed(
                "only a generation-bound resource is lost",
            )),
            State::Refused { reason } => slug(reason, 32, "reason"),
            _ => Ok(()),
        }
    }
}

/// An owner of resources: it publishes a directory and answers intents.
/// An owner never answers about a resource other than the one asked for,
/// and never creates a resource to stand in for a lost one.
pub trait Owner {
    fn directory(&self) -> Directory;
    fn resolve(&mut self, intent: &Intent) -> Outcome;
}

/// Sends `intent` to `owner` the way every surface does: it checks the
/// intent, answers from the directory alone when the owner does not serve
/// the kind or operation or when the reference names an earlier
/// generation, and checks that the owner's answer is about the target.
pub fn dispatch(owner: &mut dyn Owner, intent: &Intent) -> Result<Outcome, Refusal> {
    intent.check()?;
    let directory = owner.directory();
    directory.check()?;
    if intent.target.host != directory.host {
        return Err(Refusal::new(
            Reason::IdentityMismatch,
            "the target belongs to another host",
        ));
    }
    let Some(capability) = directory.capability(intent.target.kind) else {
        return Ok(Outcome::new(intent, State::Unsupported));
    };
    let served = match (&intent.action, intent.operation()) {
        (Action::Owner { intent: name, .. }, _) => capability.intents.contains(name),
        (_, Some(operation)) => capability.operations.contains(&operation),
        (_, None) => false,
    };
    if !served {
        if let (Action::Open, Some(Fallback::Link { url })) = (&intent.action, &capability.fallback)
        {
            let url = url.replace("{id}", &intent.target.id);
            return Ok(Outcome::new(
                intent,
                State::Fallback {
                    shown: Shown::Link { url },
                },
            ));
        }
        let summary = matches!(capability.fallback, Some(Fallback::Summary));
        if !(summary && intent.action == Action::Open) {
            return Ok(Outcome::new(intent, State::Unsupported));
        }
    }
    if let (Some(asked), Some(current)) = (&intent.target.generation, &directory.generation)
        && asked != current
    {
        return Ok(Outcome::new(intent, State::Lost));
    }
    let outcome = owner.resolve(intent);
    outcome.check(intent)?;
    match (&outcome.state, &capability.fallback) {
        // Open is not served natively, so only the declared summary may
        // stand in for it.
        (State::Opened, _) if !served => {
            return Err(Refusal::malformed(
                "an owner opened a kind it does not serve",
            ));
        }
        // An owner that serves Open may still answer with its declared
        // fallback (a resource too large to draw, say), and only that.
        (
            State::Fallback {
                shown: Shown::Link { .. },
            },
            Some(Fallback::Link { .. }),
        )
        | (
            State::Fallback {
                shown: Shown::Summary { .. },
            },
            Some(Fallback::Summary),
        ) => {}
        (State::Fallback { .. }, _) => {
            return Err(Refusal::malformed(
                "a fallback the directory does not permit",
            ));
        }
        _ => {}
    }
    Ok(outcome)
}

// ---------------------------------------------------------------------------
// Refusals and helpers

/// Why a body was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    Malformed,
    UnsupportedVersion,
    IdentityMismatch,
    LimitExceeded,
}

impl Reason {
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Reason::Malformed => "malformed",
            Reason::UnsupportedVersion => "unsupported_version",
            Reason::IdentityMismatch => "identity_mismatch",
            Reason::LimitExceeded => "limit_exceeded",
        }
    }
}

/// A refused body. The detail is data, never instructions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub reason: Reason,
    pub detail: String,
}

impl Refusal {
    #[must_use]
    pub fn new(reason: Reason, detail: impl Into<String>) -> Self {
        Refusal {
            reason,
            detail: detail.into(),
        }
    }

    fn malformed(detail: impl Into<String>) -> Self {
        Refusal::new(Reason::Malformed, detail)
    }

    fn limit(detail: impl Into<String>) -> Self {
        Refusal::new(Reason::LimitExceeded, detail)
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.reason.code(), self.detail)
    }
}

impl std::error::Error for Refusal {}

fn version(v: &str, expected: &str) -> Result<(), Refusal> {
    if v == expected {
        Ok(())
    } else if v.starts_with("openagents.workbench-") {
        Err(Refusal::new(
            Reason::UnsupportedVersion,
            format!("expected {expected}"),
        ))
    } else {
        Err(Refusal::malformed(format!("expected {expected}")))
    }
}

/// Whether `id` is 64 lowercase hexadecimal characters.
#[must_use]
pub fn is_common_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn common_id(id: &str, what: &str) -> Result<(), Refusal> {
    if is_common_id(id) {
        Ok(())
    } else {
        Err(Refusal::malformed(format!(
            "{what} is 64 lowercase hexadecimal characters"
        )))
    }
}

/// 1 to `max` bytes of lowercase letters, digits, `-`, and `_`.
fn slug(text: &str, max: usize, what: &str) -> Result<(), Refusal> {
    let valid = !text.is_empty()
        && text.len() <= max
        && text
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
    if valid {
        Ok(())
    } else {
        Err(Refusal::malformed(format!(
            "{what} is 1 to {max} lowercase letters, digits, - or _"
        )))
    }
}

/// 1 to `max` bytes of ASCII letters, digits, `.`, `_`, `:`, and `-`.
fn token(text: &str, max: usize, what: &str) -> Result<(), Refusal> {
    let valid = !text.is_empty()
        && text.len() <= max
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'));
    if valid {
        Ok(())
    } else {
        Err(Refusal::malformed(format!(
            "{what} is 1 to {max} letters, digits, ., _, :, or -"
        )))
    }
}

fn relative_path(path: &str) -> Result<(), Refusal> {
    if path.is_empty() || path.len() > PATH_MAX {
        return Err(Refusal::malformed("a file path is 1 to 1024 bytes"));
    }
    if path.starts_with('/')
        || path.contains('\\')
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(Refusal::malformed(
            "a file path is relative to its workspace and stays inside it",
        ));
    }
    Ok(())
}

/// `<publisher-pubkey>:<package-slug>/<component-slug>`.
fn component(id: &str) -> Result<(), Refusal> {
    let parsed = id.split_once(':').and_then(|(publisher, rest)| {
        let (package, component) = rest.split_once('/')?;
        Some((publisher, package, component))
    });
    let Some((publisher, package, component)) = parsed else {
        return Err(Refusal::malformed(
            "a tool is <publisher>:<package>/<component>",
        ));
    };
    common_id(publisher, "publisher")?;
    slug(package, 64, "package")?;
    slug(component, 64, "component")
}

fn link(url: &str) -> Result<(), Refusal> {
    if !url.starts_with("https://")
        || url.len() > PATH_MAX
        || url.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(Refusal::malformed("a fallback link is an https address"));
    }
    Ok(())
}
