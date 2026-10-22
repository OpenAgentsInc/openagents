//! NIP-DEPLOY records: a public deploy-status event for each phase of a
//! build or deploy (`3205`), and an addressable head for each lane
//! (`30205`), so a reader sees what staging and production are doing.
//!
//! Everything here is pure: [`sign`] builds a signed pair from a
//! [`Status`], and [`read`] checks an event against the one announcer key
//! a reader trusts and returns the status or a refusal. Nothing in this
//! module deploys, grants access, or proves a deploy happened; an event is
//! the announcer's claim. See `nips/openagents/NIP-DEPLOY.md`.

use serde::{Deserialize, Serialize};

use crate::domain::{Event, RelaySigner, Tag};

/// The phase event kind (regular).
pub const PHASE_KIND: u16 = crate::kinds::DEPLOY_PHASE;
/// The lane head kind (addressable, one per lane).
pub const HEAD_KIND: u16 = crate::kinds::DEPLOY_HEAD;
/// The content's version.
pub const V: &str = "openagents.deploy-status.v1";
/// The `t` marker on both kinds.
pub const MARKER: &str = "oa:deploy:v1";

/// The public key of OpenAgents' deploy announcer. Its secret stays in the
/// operator's secrets directory (`~/work/.secrets/deploy-announcer.key`).
pub const ANNOUNCER: &str = "eaf77c570b5cdfbd7056a5ebcde063446b86c767c17e50e0c7992ed078ddffb4";

/// The longest commit subject an event carries, in bytes.
pub const MAX_SUBJECT: usize = 72;
/// The longest `by` label.
pub const MAX_BY: usize = 24;
/// The longest build ID.
pub const MAX_BUILD: usize = 64;
/// The longest run ID.
pub const MAX_RUN: usize = 32;
/// The largest content a reader parses, in bytes.
pub const MAX_CONTENT: usize = 1024;
/// An event from further in the future than this is refused, s.
pub const MAX_FUTURE_SKEW_SECS: u64 = 300;

/// Where a deploy lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Env {
    Staging,
    Production,
}

impl Env {
    pub const ALL: [Env; 2] = [Env::Staging, Env::Production];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Env::Staging => "staging",
            Env::Production => "production",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|env| env.as_str() == text)
    }
}

/// What a deploy ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// A new server image.
    Image,
    /// Browser modules.
    Module,
    /// The content tier (docs, legal text, stylesheets) on livehost.
    Content,
}

impl Tier {
    pub const ALL: [Tier; 3] = [Tier::Image, Tier::Module, Tier::Content];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Image => "image",
            Tier::Module => "module",
            Tier::Content => "content",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tier| tier.as_str() == text)
    }
}

/// A step a deploy reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    BuildStarted,
    BuildDone,
    BuildFailed,
    Deploying,
    Promoted,
    DeployFailed,
    SmokeRunning,
    SmokePassed,
    SmokeFailed,
    TrafficShifted,
    RolledBack,
    Live,
}

impl Phase {
    pub const ALL: [Phase; 12] = [
        Phase::BuildStarted,
        Phase::BuildDone,
        Phase::BuildFailed,
        Phase::Deploying,
        Phase::Promoted,
        Phase::DeployFailed,
        Phase::SmokeRunning,
        Phase::SmokePassed,
        Phase::SmokeFailed,
        Phase::TrafficShifted,
        Phase::RolledBack,
        Phase::Live,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::BuildStarted => "build_started",
            Phase::BuildDone => "build_done",
            Phase::BuildFailed => "build_failed",
            Phase::Deploying => "deploying",
            Phase::Promoted => "promoted",
            Phase::DeployFailed => "deploy_failed",
            Phase::SmokeRunning => "smoke_running",
            Phase::SmokePassed => "smoke_passed",
            Phase::SmokeFailed => "smoke_failed",
            Phase::TrafficShifted => "traffic_shifted",
            Phase::RolledBack => "rolled_back",
            Phase::Live => "live",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|phase| phase.as_str() == text)
    }

    /// Whether the run stops here: live, failed, or rolled back.
    #[must_use]
    pub fn ends(self) -> bool {
        matches!(
            self,
            Phase::Live
                | Phase::BuildFailed
                | Phase::DeployFailed
                | Phase::SmokeFailed
                | Phase::RolledBack
        )
    }

    /// Whether this phase is a failure.
    #[must_use]
    pub fn failed(self) -> bool {
        matches!(
            self,
            Phase::BuildFailed | Phase::DeployFailed | Phase::SmokeFailed
        )
    }
}

/// One deploy-status record: the content of both kinds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    /// Always [`V`].
    pub v: String,
    pub env: Env,
    pub tier: Tier,
    pub phase: Phase,
    /// The deploy run this phase belongs to: lowercase hex or hyphens.
    pub run: String,
    /// The commit's abbreviated SHA, 7 to 40 lowercase hex characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// The commit's subject line, printable ASCII.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// Who started the deploy: a short label such as `agent` or `owner`.
    pub by: String,
    /// The build's ID, such as a Cloud Build ID or an image tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
    /// When the run started, Unix seconds.
    pub started: u64,
    /// A test event: readers show it only when asked to.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub test: bool,
}

/// A checked record and when its event says it happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub status: Status,
    /// The event's `created_at`: when this phase happened.
    pub at: u64,
    /// The event's ID.
    pub id: String,
    /// Whether it came from a lane head (`30205`) or a phase event.
    pub head: bool,
}

/// Why a reader refused an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    Kind,
    Signer,
    Signature,
    Content(String),
    Tags,
    Future,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::Kind => write!(f, "not a deploy-status kind"),
            Refusal::Signer => write!(f, "not signed by the deploy announcer"),
            Refusal::Signature => write!(f, "the ID or signature does not verify"),
            Refusal::Content(why) => write!(f, "content: {why}"),
            Refusal::Tags => write!(f, "the tags disagree with the content"),
            Refusal::Future => write!(f, "dated too far in the future"),
        }
    }
}

impl std::error::Error for Refusal {}

fn label(text: &str, max: usize) -> bool {
    !text.is_empty()
        && text.len() <= max
        && text
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn build_id(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_BUILD
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// Makes `text` a subject an event may carry: control characters and
/// anything outside printable ASCII become spaces, runs of spaces collapse,
/// and it is cut to [`MAX_SUBJECT`] bytes.
#[must_use]
pub fn clean_subject(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        let c = if c.is_ascii_graphic() { c } else { ' ' };
        if c == ' ' && (out.is_empty() || out.ends_with(' ')) {
            continue;
        }
        if out.len() == MAX_SUBJECT {
            break;
        }
        out.push(c);
    }
    out.trim_end().to_owned()
}

impl Status {
    /// Checks every field against the NIP's bounds.
    ///
    /// # Errors
    ///
    /// Names the first field out of bounds.
    pub fn check(&self) -> Result<(), String> {
        if self.v != V {
            return Err(format!("v must be {V}"));
        }
        if !label(&self.run, MAX_RUN) {
            return Err("run must be 1-32 lowercase letters, digits, or hyphens".into());
        }
        if !label(&self.by, MAX_BY) {
            return Err("by must be 1-24 lowercase letters, digits, or hyphens".into());
        }
        if let Some(commit) = &self.commit
            && !(7..=40).contains(&commit.len())
        {
            return Err("commit must be 7-40 lowercase hex characters".into());
        }
        if let Some(commit) = &self.commit
            && !commit
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("commit must be 7-40 lowercase hex characters".into());
        }
        if let Some(subject) = &self.subject
            && (subject.is_empty() || clean_subject(subject) != *subject)
        {
            return Err("subject must be 1-72 printable ASCII characters".into());
        }
        if let Some(build) = &self.build
            && !build_id(build)
        {
            return Err("build must be 1-64 letters, digits, '-', '_', or '.'".into());
        }
        Ok(())
    }

    /// The lane head's `d` tag: the environment, under `test/` for a test.
    #[must_use]
    pub fn lane(&self) -> String {
        lane(self.env, self.test)
    }
}

/// A lane head's `d` tag.
#[must_use]
pub fn lane(env: Env, test: bool) -> String {
    if test {
        format!("test/{}", env.as_str())
    } else {
        env.as_str().to_owned()
    }
}

fn tags(status: &Status, head: bool) -> Vec<Tag> {
    let tag = |name: &str, value: &str| Tag::new(vec![name.to_owned(), value.to_owned()]);
    let mut tags = Vec::new();
    if head {
        tags.push(tag("d", &status.lane()));
    }
    tags.push(tag("t", MARKER));
    tags.push(tag("env", status.env.as_str()));
    tags.push(tag("phase", status.phase.as_str()));
    tags
}

/// Signs `status` at `created_at` as a phase event and its lane head.
///
/// # Errors
///
/// Refuses a status out of bounds.
pub fn sign(signer: &RelaySigner, status: &Status, created_at: u64) -> Result<[Event; 2], String> {
    status.check()?;
    let content = serde_json::to_string(status).map_err(|e| e.to_string())?;
    Ok([
        signer.sign(created_at, PHASE_KIND, tags(status, false), content.clone()),
        signer.sign(created_at, HEAD_KIND, tags(status, true), content),
    ])
}

/// Checks `event` as a deploy status signed by `announcer`, at the
/// reader's clock `now` (Unix seconds).
///
/// # Errors
///
/// Refuses another kind, another signer, a bad ID or signature, content
/// out of bounds, tags that disagree with the content, or a date more than
/// [`MAX_FUTURE_SKEW_SECS`] ahead.
pub fn read(event: &Event, announcer: &str, now: u64) -> Result<Record, Refusal> {
    let head = match event.kind {
        PHASE_KIND => false,
        HEAD_KIND => true,
        _ => return Err(Refusal::Kind),
    };
    if event.pubkey != announcer {
        return Err(Refusal::Signer);
    }
    event.validate_crypto().map_err(|_| Refusal::Signature)?;
    if event.created_at > now.saturating_add(MAX_FUTURE_SKEW_SECS) {
        return Err(Refusal::Future);
    }
    if event.content.len() > MAX_CONTENT {
        return Err(Refusal::Content("too long".into()));
    }
    let status: Status =
        serde_json::from_str(&event.content).map_err(|e| Refusal::Content(e.to_string()))?;
    status.check().map_err(Refusal::Content)?;
    if event.created_at < status.started {
        return Err(Refusal::Content("dated before its run started".into()));
    }
    let one = |name: &'static str| {
        let mut values = event.tag_values(name);
        let first = values.next();
        if values.next().is_some() { None } else { first }
    };
    let agrees = one("t") == Some(MARKER)
        && one("env") == Some(status.env.as_str())
        && one("phase") == Some(status.phase.as_str())
        && if head {
            one("d") == Some(status.lane().as_str())
        } else {
            event.tag_values("d").next().is_none()
        };
    if !agrees {
        return Err(Refusal::Tags);
    }
    Ok(Record {
        status,
        at: event.created_at,
        id: event.id.clone(),
        head,
    })
}

/// The NIP-01 filters a reader subscribes with: every lane head, and the
/// newest `history` phase events.
#[must_use]
pub fn filters(announcer: &str, history: usize) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({"kinds": [HEAD_KIND], "authors": [announcer]}),
        serde_json::json!({"kinds": [PHASE_KIND], "authors": [announcer], "limit": history}),
    ]
}

#[cfg(test)]
mod tests;
