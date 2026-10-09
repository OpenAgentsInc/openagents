//! A chat's working computer (CMP-01).
//!
//! A [`Computer`] joins one chat, its owner, a project and source pin, an
//! optional verified environment version it started from, a provider
//! resource, declared services, and idle/absolute bounds. After each
//! completed turn the owner checkpoints the filesystem, fenced to exactly
//! that turn's generation; the next prompt restores it, re-applies current
//! credentials, and restarts declared services.
//!
//! Every lifecycle choice — wait, skip, create, restore, checkpoint, stop —
//! comes from [`decide::decide`], a pure function over the retained record.
//! Every change goes through [`transition::apply`], also pure. Provider
//! effects sit behind [`provider::Provider`]; [`driver::Driver`] runs a
//! decision, retains the observed outcome through [`store::Store`], and
//! decides again. [`boat::BoatProvider`] wires the trait to Boat's
//! stop/resume primitives; [`provider::fake::FakeProvider`] serves tests.
//!
//! A checkpoint is not a verified environment: it belongs to one user's one
//! computer, may carry that user's own sign-ins (for example a Claude Code
//! login), and is never admitted as a reusable image, an operator copy, a
//! service read, or a restore into any other computer (see
//! [`admit_checkpoint_use`]). Selected credentials are names only; their
//! values are applied per boot and never captured.
//!
//! Checkpoint, process shutdown, resource stop, meter stop, and deletion are
//! separate retained [`Fact`]s. An unknown provider outcome stays unknown
//! until a definite observation reconciles it.

use coder_environment::{ProjectLink, Provider as ProviderKind, SourcePin, valid_id};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub mod boat;
pub mod decide;
pub mod driver;
pub mod gce;
pub mod provider;
pub mod store;
pub mod transition;

pub use decide::{Decision, Trigger, WaitReason, decide};
pub use transition::{Applied, Command, Refusal, apply};

pub const SCHEMA: &str = "openagents.working_computer.v1";
pub const MAX_BOOTS: usize = 1024;
pub const MAX_CHECKPOINTS: usize = 4096;
pub const MAX_CREATES: usize = 64;
pub const MAX_SERVICES: usize = 16;
pub const MAX_REASON_BYTES: usize = 1024;
/// How often a turn's owner says it is alive ([`Command::Heartbeat`]).
pub const HEARTBEAT_EVERY_MS: u64 = 30_000;
/// A turn whose owner has not said it is alive for this long (three missed
/// beats) is silent: its process is gone or hung, and the computer stops
/// ([`StopReason::Stale`]). This judges a dead owner, never a long turn: a
/// live owner keeps beating however long the turn runs.
pub const STALE_AFTER_MS: u64 = 90_000;
/// Failed boots within [`BREAKER_WINDOW_MS`] that stop new boots until the
/// window passes, so a broken image or a provider outage does not create
/// machines in a loop.
pub const BREAKER_FAILURES: usize = 3;
pub const BREAKER_WINDOW_MS: u64 = 5 * 60_000;
/// Boot failure times retained per computer.
const MAX_BOOT_FAILURES: usize = 8;

/// Credentials OpenAgents never accepts, stores, or injects: a user's
/// Claude.ai sign-in lives only inside that user's computer
/// (`docs/cloud/claude-code-byo.md`).
pub const REFUSED_CREDENTIAL_NAMES: &[&str] = &["CLAUDE_CODE_OAUTH_TOKEN"];
/// Files holding a user's own sign-ins inside their computer. They may
/// persist in that computer's checkpoint; no OpenAgents service reads them.
pub const USER_LOGIN_PATHS: &[&str] = &[".claude/.credentials.json"];

/// A retained outcome of one provider effect. Unknown stays unknown until a
/// definite observation replaces it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Fact {
    Requested {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        operation: Option<String>,
        at_ms: u64,
    },
    Done {
        evidence: String,
        at_ms: u64,
    },
    Failed {
        reason: String,
        at_ms: u64,
    },
    Unknown {
        reason: String,
        at_ms: u64,
    },
}
impl Fact {
    pub fn is_done(&self) -> bool {
        matches!(self, Self::Done { .. })
    }
    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown { .. })
    }
    pub fn is_settled(&self) -> bool {
        matches!(self, Self::Done { .. } | Self::Failed { .. })
    }
    pub fn evidence(&self) -> Option<&str> {
        match self {
            Self::Done { evidence, .. } => Some(evidence),
            _ => None,
        }
    }
}
pub(crate) fn done(fact: &Option<Fact>) -> bool {
    fact.as_ref().is_some_and(Fact::is_done)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Principal {
    pub workspace: String,
    pub principal: String,
}

/// The verified environment version a computer started from. A checkpoint
/// never becomes one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseLink {
    pub environment: String,
    pub version: String,
}

/// What a computer is for. A setup computer is dedicated to one environment
/// setup session: it never serves a chat, and a chat computer never runs a
/// setup session (`docs/cloud/example-cursor-cloud-agent-onboarding/environment-onboarding.md`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Purpose {
    #[default]
    Chat,
    /// The `chat` field holds the setup session identity.
    EnvironmentSetup { environment: String },
    /// A clean builder for one build attempt (ENV-04): created fresh from
    /// the recipe's pinned base, never a setup or chat computer, and never
    /// restored from any checkpoint. The `chat` field holds the build job.
    EnvironmentBuild { environment: String, build: String },
    /// An independent verifier (ENV-05): created fresh from one sealed
    /// output image (`image`, the provider name), never the setup or
    /// builder computer and never restored from a checkpoint. It carries no
    /// credentials; the verifier starts declared services itself once the
    /// restored files are hydrated. The `chat` field holds the verify job.
    EnvironmentVerify {
        environment: String,
        build: String,
        verification: String,
        image: String,
        role: VerifyRole,
    },
}

/// Which verifier machine this is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifyRole {
    /// The untouched candidate the frozen checks run on first.
    Baseline,
    /// A disposable second boot of the same image where the recipe is
    /// rerun to prove it changes nothing.
    Fork,
}
impl Purpose {
    pub fn is_chat(&self) -> bool {
        matches!(self, Self::Chat)
    }
    /// The output image a verifier computer boots from.
    pub fn verify_image(&self) -> Option<&str> {
        match self {
            Self::EnvironmentVerify { image, .. } => Some(image),
            _ => None,
        }
    }
}

/// How a declared service proves readiness. A reachable port alone is not
/// application readiness, so an HTTP rule names a path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Health {
    Http { port: u16, path: String },
    Command { command: String },
}

/// One process restarted on every boot (fresh or restored).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceDecl {
    pub name: String,
    pub command: String,
    /// Source-relative working directory.
    pub cwd: String,
    pub health: Health,
    pub ready_within_seconds: u32,
}

/// Admitted idle and absolute bounds for one boot. Observation extends the
/// idle deadline, never past the absolute one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    pub idle_ms: u64,
    pub observed_extension_ms: u64,
    pub absolute_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// The provider quiesced a turn checkpoint by stopping (Boat).
    Checkpoint,
    Idle,
    Absolute,
    FailedBoot,
    Owner,
    /// The running turn's owner stopped saying it is alive.
    Stale,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BootOrigin {
    Created,
    /// Resumed from the provider's retained stopped filesystem, optionally
    /// naming the turn checkpoint it carries.
    Restored {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        checkpoint: Option<String>,
    },
}

/// One wake of the provider resource and its separate facts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Boot {
    pub number: u32,
    pub resource: String,
    pub origin: BootOrigin,
    pub started_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restore: Option<Fact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials: Option<Fact>,
    /// Declared service name to readiness.
    #[serde(default)]
    pub services: BTreeMap<String, Fact>,
    pub idle_deadline_ms: u64,
    pub absolute_deadline_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<StopReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shutdown: Option<Fact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_stop: Option<Fact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meter_stop: Option<Fact>,
}

/// One provider create attempt, identified before the call so a lost reply
/// reconciles the same resource instead of creating another.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateAttempt {
    pub operation: String,
    pub fact: Fact,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deletion: Option<Fact>,
}

/// Who may ever use a checkpoint: only its own computer, for its owner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Custody {
    UserPrivate {
        principal: Principal,
        computer: String,
    },
}

/// A filesystem checkpoint of exactly one completed turn.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub id: String,
    pub turn_generation: u64,
    pub boot: u32,
    pub resource: String,
    pub fact: Fact,
    pub custody: Custody,
    /// It may hold the user's own sign-ins; it is never a reusable image.
    pub may_hold_user_logins: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnFence {
    /// Last generation dispatched to the engine.
    pub dispatched: u64,
    /// Last generation whose turn completed.
    pub completed: u64,
    /// When the running turn's owner last said it is alive (its start
    /// counts). Zero for a record from before heartbeats: never judged.
    #[serde(default)]
    pub heartbeat_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
pub enum Phase {
    New,
    Creating,
    /// Restore, credentials, and services for the current boot.
    Booting,
    Awake,
    Turn {
        generation: u64,
    },
    /// Quiesced for one completed turn; no prompt dispatches until settled.
    Checkpointing {
        generation: u64,
    },
    Stopping {
        reason: StopReason,
    },
    Stopped,
    /// A provider outcome is unknown; reconcile before anything else.
    Unknown {
        reason: String,
    },
    /// A create or boot failed; cleanup runs before any new attempt.
    Failed {
        reason: String,
    },
    Deleting,
    Deleted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Computer {
    pub schema: String,
    pub id: String,
    pub revision: u64,
    pub created_ms: u64,
    pub updated_ms: u64,
    pub owner: Principal,
    pub chat: String,
    #[serde(default, skip_serializing_if = "Purpose::is_chat")]
    pub purpose: Purpose,
    pub project: ProjectLink,
    pub source: SourcePin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<BaseLink>,
    pub provider: ProviderKind,
    pub size: String,
    /// Names only. Values are applied per boot and never retained.
    #[serde(default)]
    pub credential_names: BTreeSet<String>,
    #[serde(default)]
    pub services: Vec<ServiceDecl>,
    pub bounds: Bounds,
    pub phase: Phase,
    #[serde(default)]
    pub turn: TurnFence,
    #[serde(default)]
    pub creates: Vec<CreateAttempt>,
    #[serde(default)]
    pub boots: Vec<Boot>,
    #[serde(default)]
    pub checkpoints: Vec<Checkpoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deletion: Option<Fact>,
    /// How long a running turn may go without a heartbeat before it is
    /// silent ([`STALE_AFTER_MS`] for chat and setup computers, whose turn
    /// owner beats). Zero never judges: builders and verifiers, whose job
    /// owners follow each command's own exit, and records from before
    /// heartbeats.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub stale_ms: u64,
    /// When recent boots failed, newest last (at most a few).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub boot_failures: Vec<u64>,
}

fn is_zero(v: &u64) -> bool {
    *v == 0
}

/// What a new computer is asked to be.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub id: String,
    pub owner: Principal,
    pub chat: String,
    pub project: ProjectLink,
    pub source: SourcePin,
    pub base: Option<BaseLink>,
    pub size: String,
    pub credential_names: BTreeSet<String>,
    pub services: Vec<ServiceDecl>,
    pub bounds: Bounds,
}

fn valid_text(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
fn valid_relative(value: &str) -> bool {
    value == "."
        || (valid_text(value, 1024)
            && !value.starts_with('/')
            && value
                .split('/')
                .all(|p| !p.is_empty() && p != "." && p != ".."))
}

/// Refuse a credential name OpenAgents may not carry.
pub fn credential_name_allowed(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
        && !REFUSED_CREDENTIAL_NAMES.contains(&name)
}

/// Whether an OpenAgents service may read `path` (home-relative or
/// absolute) out of a user's computer or checkpoint. Sign-in files never.
pub fn service_may_read(path: &str) -> bool {
    let trimmed = path.trim_start_matches('/');
    !USER_LOGIN_PATHS.iter().any(|login| {
        trimmed == *login
            || trimmed.ends_with(&format!("/{login}"))
            || trimmed.split('/').any(|part| part == ".credentials.json")
    })
}

/// A requested use of a retained checkpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckpointUse<'a> {
    /// Restore into a computer for a principal.
    Restore {
        computer: &'a str,
        principal: &'a Principal,
    },
    /// Seal as a reusable or shared environment image.
    EnvironmentImage,
    /// Copy for an operator or support tooling.
    OperatorCopy,
    /// Read its contents from an OpenAgents service.
    ServiceRead,
}

/// Only the computer that made a checkpoint, for its own owner, may restore
/// it. Everything else is refused, so a user's sign-ins never leave it.
pub fn admit_checkpoint_use(
    computer: &Computer,
    checkpoint: &str,
    use_: &CheckpointUse<'_>,
) -> Result<(), Refusal> {
    let found = computer
        .checkpoints
        .iter()
        .find(|c| c.id == checkpoint)
        .ok_or_else(|| Refusal::UnknownCheckpoint(checkpoint.into()))?;
    let Custody::UserPrivate {
        principal,
        computer: owner_computer,
    } = &found.custody;
    match use_ {
        CheckpointUse::Restore {
            computer: target,
            principal: who,
        } if *target == owner_computer && *who == principal => {
            if found.fact.is_done() {
                Ok(())
            } else {
                Err(Refusal::CheckpointNotDone(checkpoint.into()))
            }
        }
        CheckpointUse::Restore { .. } => Err(Refusal::CheckpointCustody),
        CheckpointUse::EnvironmentImage
        | CheckpointUse::OperatorCopy
        | CheckpointUse::ServiceRead => Err(Refusal::CheckpointCustody),
    }
}

impl Computer {
    pub fn new(spec: Spec, now_ms: u64) -> Result<Self, &'static str> {
        let c = Self {
            schema: SCHEMA.into(),
            id: spec.id,
            revision: 0,
            created_ms: now_ms,
            updated_ms: now_ms,
            owner: spec.owner,
            chat: spec.chat,
            purpose: Purpose::Chat,
            project: spec.project,
            source: spec.source,
            base: spec.base,
            provider: ProviderKind::Boat,
            size: spec.size,
            credential_names: spec.credential_names,
            services: spec.services,
            bounds: spec.bounds,
            phase: Phase::New,
            turn: TurnFence::default(),
            creates: vec![],
            boots: vec![],
            checkpoints: vec![],
            deletion: None,
            stale_ms: STALE_AFTER_MS,
            boot_failures: vec![],
        };
        c.validate()?;
        Ok(c)
    }

    /// A clean builder for one build attempt (`spec.chat` names the build
    /// job). Like a setup computer, it starts from no saved version and
    /// runs no declared services.
    pub fn for_build(
        spec: Spec,
        environment: &str,
        build: &str,
        now_ms: u64,
    ) -> Result<Self, &'static str> {
        if spec.base.is_some() || !spec.services.is_empty() {
            return Err("A builder starts from its pinned base and declares no services.");
        }
        let mut c = Self::new(spec, now_ms)?;
        c.purpose = Purpose::EnvironmentBuild {
            environment: environment.into(),
            build: build.into(),
        };
        c.stale_ms = 0;
        c.validate()?;
        Ok(c)
    }

    /// An independent verifier booted from one sealed output image
    /// (`spec.chat` names the verify job). It starts from no saved version,
    /// declares no services, and carries no credentials.
    pub fn for_verify(spec: Spec, purpose: Purpose, now_ms: u64) -> Result<Self, &'static str> {
        if !matches!(purpose, Purpose::EnvironmentVerify { .. }) {
            return Err("A verifier needs a verify purpose.");
        }
        if spec.base.is_some() || !spec.services.is_empty() || !spec.credential_names.is_empty() {
            return Err(
                "A verifier boots its image with no saved version, services, or credentials.",
            );
        }
        let mut c = Self::new(spec, now_ms)?;
        c.purpose = purpose;
        c.stale_ms = 0;
        c.validate()?;
        Ok(c)
    }

    /// A computer dedicated to one environment setup session (`spec.chat`
    /// names the session). It starts from no saved environment version and
    /// runs no declared services.
    pub fn for_setup(spec: Spec, environment: &str, now_ms: u64) -> Result<Self, &'static str> {
        if spec.base.is_some() || !spec.services.is_empty() {
            return Err("A setup computer starts from its pinned base and declares no services.");
        }
        let mut c = Self::new(spec, now_ms)?;
        c.purpose = Purpose::EnvironmentSetup {
            environment: environment.into(),
        };
        c.validate()?;
        Ok(c)
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != SCHEMA {
            return Err("The working computer schema is not supported.");
        }
        if !valid_id(&self.id) || self.id.len() > 96 || !valid_id(&self.chat) {
            return Err("The computer and chat need opaque identities.");
        }
        if !valid_id(&self.owner.workspace) || !valid_id(&self.owner.principal) {
            return Err("The owner needs a workspace and principal.");
        }
        if !valid_id(&self.project.workspace) || !valid_id(&self.project.project) {
            return Err("The project link is invalid.");
        }
        self.source.validate()?;
        if self
            .base
            .as_ref()
            .is_some_and(|b| !valid_id(&b.environment) || !valid_id(&b.version))
        {
            return Err("The base environment link is invalid.");
        }
        if !valid_id(&self.size) {
            return Err("The computer size is invalid.");
        }
        if let Purpose::EnvironmentSetup { environment } = &self.purpose
            && (!valid_id(environment) || self.base.is_some() || !self.services.is_empty())
        {
            return Err("A setup computer is invalid.");
        }
        if let Purpose::EnvironmentBuild { environment, build } = &self.purpose
            && (!valid_id(environment)
                || !valid_id(build)
                || self.base.is_some()
                || !self.services.is_empty())
        {
            return Err("A builder computer is invalid.");
        }
        if let Purpose::EnvironmentVerify {
            environment,
            build,
            verification,
            image,
            ..
        } = &self.purpose
            && (!valid_id(environment)
                || !valid_id(build)
                || !valid_id(verification)
                || !valid_id(image)
                || self.base.is_some()
                || !self.services.is_empty()
                || !self.credential_names.is_empty())
        {
            return Err("A verifier computer is invalid.");
        }
        if self.credential_names.len() > 32
            || !self
                .credential_names
                .iter()
                .all(|n| credential_name_allowed(n))
        {
            return Err(
                "Credential names must be admitted uppercase names; a Claude sign-in is never one.",
            );
        }
        if self.services.len() > MAX_SERVICES {
            return Err("The computer declares too many services.");
        }
        let mut names = BTreeSet::new();
        for s in &self.services {
            let health_ok = match &s.health {
                Health::Http { port, path } => {
                    *port > 0 && path.starts_with('/') && valid_text(path, 512)
                }
                Health::Command { command } => valid_text(command, 4096),
            };
            if !valid_id(&s.name)
                || !names.insert(&s.name)
                || !valid_text(&s.command, 4096)
                || !valid_relative(&s.cwd)
                || !health_ok
                || s.ready_within_seconds == 0
                || s.ready_within_seconds > 900
            {
                return Err("A declared service is invalid.");
            }
        }
        let b = &self.bounds;
        if b.idle_ms == 0
            || b.absolute_ms == 0
            || b.idle_ms > b.absolute_ms
            || b.observed_extension_ms > b.absolute_ms
            || b.absolute_ms > 24 * 3600 * 1000
        {
            return Err("The idle and absolute bounds are invalid.");
        }
        if self.creates.len() > MAX_CREATES
            || self.boots.len() > MAX_BOOTS
            || self.checkpoints.len() > MAX_CHECKPOINTS
        {
            return Err("The computer retains too much history.");
        }
        if self.turn.completed > self.turn.dispatched {
            return Err("A turn cannot complete before it is dispatched.");
        }
        for c in &self.checkpoints {
            let Custody::UserPrivate {
                principal,
                computer,
            } = &c.custody;
            if principal != &self.owner || computer != &self.id {
                return Err("A checkpoint belongs only to its own computer and owner.");
            }
        }
        Ok(())
    }

    /// The live provider resource, if one was created and not deleted.
    /// Whether the running turn has gone silent at `now_ms`.
    pub fn turn_silent(&self, now_ms: u64) -> bool {
        matches!(self.phase, Phase::Turn { .. })
            && self.stale_ms > 0
            && self.turn.heartbeat_ms > 0
            && now_ms >= self.turn.heartbeat_ms.saturating_add(self.stale_ms)
    }

    /// While [`BREAKER_FAILURES`] boots failed within the last
    /// [`BREAKER_WINDOW_MS`], when a new boot may be tried again.
    pub fn breaker_open_until(&self, now_ms: u64) -> Option<u64> {
        let recent: Vec<u64> = self
            .boot_failures
            .iter()
            .copied()
            .filter(|t| now_ms < t.saturating_add(BREAKER_WINDOW_MS))
            .collect();
        if recent.len() < BREAKER_FAILURES {
            return None;
        }
        // The window closes when enough of these failures age out.
        let pivot = recent[recent.len() - BREAKER_FAILURES];
        Some(pivot.saturating_add(BREAKER_WINDOW_MS))
    }

    pub fn resource(&self) -> Option<&str> {
        self.creates
            .iter()
            .rev()
            .find(|c| c.resource.is_some() && !done(&c.deletion))
            .and_then(|c| c.resource.as_deref())
    }
    pub fn boot(&self) -> Option<&Boot> {
        self.boots.last()
    }
    pub(crate) fn boot_mut(&mut self) -> Option<&mut Boot> {
        self.boots.last_mut()
    }
    /// The latest completed checkpoint of this computer's live resource.
    pub fn latest_checkpoint(&self) -> Option<&Checkpoint> {
        let resource = self.resource()?;
        self.checkpoints
            .iter()
            .rev()
            .find(|c| c.fact.is_done() && c.resource == resource)
    }
    /// Declared services and their readiness for the current boot.
    pub fn service_readiness(&self) -> Vec<(&str, Option<&Fact>)> {
        let boot = self.boot();
        self.services
            .iter()
            .map(|s| (s.name.as_str(), boot.and_then(|b| b.services.get(&s.name))))
            .collect()
    }
    /// Checkpoints, boots, and creates are append-only history.
    pub fn preserves_history_of(&self, next: &Computer) -> bool {
        next.id == self.id
            && next.owner == self.owner
            && next.purpose == self.purpose
            && next.created_ms == self.created_ms
            && next.creates.len() >= self.creates.len()
            && next.boots.len() >= self.boots.len()
            && next.checkpoints.len() >= self.checkpoints.len()
            && self
                .checkpoints
                .iter()
                .zip(&next.checkpoints)
                .all(|(a, b)| {
                    a.id == b.id
                        && a.turn_generation == b.turn_generation
                        && (!a.fact.is_done() || a == b)
                })
            && self
                .creates
                .iter()
                .zip(&next.creates)
                .all(|(a, b)| a.operation == b.operation)
    }
}

#[cfg(test)]
mod tests;
