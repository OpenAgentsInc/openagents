//! The admission snapshot (plan section 4, with 13.1, 13.3, 13.6, 13.8).
//!
//! One immutable document per admitted route: what was asked, which route
//! and executor serve it, where it runs, what it may touch, who may see
//! what, what it may use, who pays, what counts as done, and which product
//! defaults filled the gaps. Its [`AdmissionSnapshot::digest`] names it.
//!
//! The four authorities stay separate fields: observation is the caller's
//! scope in [`Identity`], execution is the [`Placement`] grant, disclosure
//! is [`Disclosure`], and spending is [`Money`]. No field implies another.
//!
//! The snapshot is an internal record. Its `placement.workspace.path` and
//! exact prompts are private: a public projection (phase 2) names the
//! route, model, cost, and time (API decision D11), never paths, secrets,
//! or Jev's scores.

use serde::{Deserialize, Serialize};

use crate::digest::{Digest, digest_of};
use crate::route::RouteFamily;

/// The immutable admission snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmissionSnapshot {
    /// [`crate::SNAPSHOT_SCHEMA`].
    pub schema: String,
    pub identity: Identity,
    pub input: Input,
    pub route: Route,
    pub placement: Placement,
    pub effects: Effects,
    pub disclosure: Disclosure,
    pub resources: Resources,
    pub money: Money,
    pub evidence: Evidence,
    /// Product defaults the router applied instead of asking (13.1). Each
    /// has a regression eval that fails if its question comes back.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub defaults_applied: Vec<DefaultApplied>,
    /// The snapshot this one inherits from: a continuation of a run
    /// (13.3) or a fallback (section 5). [`AdmissionSnapshot::widens`]
    /// against that parent must be empty, or the change is a new offer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inherits: Option<Digest>,
}

impl AdmissionSnapshot {
    /// The snapshot's content digest.
    #[must_use]
    pub fn digest(&self) -> Digest {
        digest_of(self)
    }

    /// Every way this snapshot asks for more than `parent` granted: a new
    /// recipient, another computer, wider effects, a payer switched to
    /// OpenAgents, a plugin fee, or a publication effect. Empty means a
    /// fallback or continuation may run under the parent's admission;
    /// anything else needs a new offer (section 5, 13.3, 13.6).
    #[must_use]
    pub fn widens(&self, parent: &AdmissionSnapshot) -> Vec<Widening> {
        let mut out = Vec::new();
        if self.placement.computer != parent.placement.computer {
            out.push(Widening::Computer);
        }
        if self.placement.workspace != parent.placement.workspace {
            out.push(Widening::Workspace);
        }
        out.extend(self.effects.widens(&parent.effects));
        if !self.disclosure.within(&parent.disclosure) {
            out.push(Widening::Disclosure);
        }
        if self.money.switches_payer_from(&parent.money) {
            out.push(Widening::Payer);
        }
        if self
            .money
            .fees
            .iter()
            .any(|fee| !parent.money.fees.contains(fee))
        {
            out.push(Widening::PluginFee);
        }
        out
    }
}

/// What a narrower snapshot cannot add without a new offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Widening {
    Computer,
    Workspace,
    Reads,
    Writes,
    Network,
    Commands,
    Publication,
    Access,
    OsDenySet,
    Disclosure,
    Payer,
    PluginFee,
}

// ---- Identity ----

/// Who asked, from where, and which request, thread, task, and attempt
/// this is. Public references are opaque identifiers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub caller: Caller,
    pub surface: Surface,
    /// The caller's workspace or account scope, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    pub request: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<String>,
    /// The task this route created or continues; absent before dispatch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The task owner's attempt (turn) number; transport retries are not
    /// attempts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Caller {
    pub kind: CallerKind,
    pub id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallerKind {
    /// A person in OpenAgents' own apps.
    AppUser,
    /// An API key (a partner or a developer).
    ApiKey,
    /// A keyless API caller paying per call.
    Keyless,
}

/// Where the message came from. One contract for every surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    Terminal,
    Cli,
    Desktop,
    Phone,
    Web,
    Api,
}

// ---- Input ----

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    /// The exact request bytes' digest (or the documented canonical form
    /// an idempotency key binds).
    pub request: Digest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourcePin>,
    /// Instruction files and prompts the route reads, by digest.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instructions: Vec<Digest>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Digest>,
}

/// The source a route runs against. A commit alone does not identify
/// uncommitted files; `snapshot` does.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePin {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<Digest>,
}

// ---- Route ----

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub family: RouteFamily,
    /// The route result's digest ([`crate::RouteResult::digest`]).
    pub result: Digest,
    /// The caller named this route (honored only if admitted, never
    /// silently substituted).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub explicit: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<CapabilityPin>,
    /// The adapter's digest. For a Coder route it covers the delegate
    /// settings (effort, tool set, system prompt, prompt cache, briefing)
    /// whose savings the cost audit measured (13.10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter: Option<Digest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executor_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelPin>,
    /// The route policy's version, such as `route-policy-v1`.
    pub policy: String,
    /// The Jev question set, such as `chat-router-v4`, and its digest.
    pub question_set: QuestionSet,
}

/// An admitted plugin, program, or executor, pinned.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityPin {
    pub id: String,
    pub version: String,
    pub digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelPin {
    pub provider: String,
    pub model: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestionSet {
    pub id: String,
    pub digest: Digest,
}

// ---- Placement ----

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    /// Absent for routes that run on no computer (an answer).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<WorkspaceBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grant: Option<GrantRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceBinding {
    pub project: String,
    /// Private: never in a public projection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// The execution authority, rechecked at dispatch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantRef {
    pub id: String,
    /// The grant's revocation epoch when admitted; a later epoch refuses.
    pub epoch: u64,
    pub source: GrantSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantSource {
    /// The operator granted this task (`openagents.coder.task-execution-grant.v1`).
    Operator,
    /// The host's auto-start policy, within its configured bounds.
    Autostart,
    /// A computer granted to an API key (D4).
    ApiKey,
    /// Inherited from the run this continues (13.3).
    Continuation,
}

// ---- Effects ----

/// What the route may touch. Enforced by the executor's boundary, not by
/// the prompt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effects {
    pub reads: Vec<ReadScope>,
    pub writes: WriteScope,
    pub network: Network,
    pub commands: CommandScope,
    /// Publication and other outward effects. Returning a patch does not
    /// imply permission to publish it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub publication: Vec<Publication>,
    pub access: Access,
    /// Operating-system locations and controls the executor's sandbox
    /// refuses (13.8).
    pub os_deny: OsDenySet,
}

impl Effects {
    /// Answer-only effects: nothing read, written, run, or sent.
    #[must_use]
    pub fn none() -> Self {
        Self {
            reads: Vec::new(),
            writes: WriteScope::None,
            network: Network::None,
            commands: CommandScope::None,
            publication: Vec::new(),
            access: Access::Boundary,
            os_deny: OsDenySet::macos(),
        }
    }

    fn widens(&self, parent: &Effects) -> Vec<Widening> {
        let mut out = Vec::new();
        if self.reads.iter().any(|read| !parent.reads.contains(read)) {
            out.push(Widening::Reads);
        }
        if self.writes > parent.writes {
            out.push(Widening::Writes);
        }
        if !self.network.within(&parent.network) {
            out.push(Widening::Network);
        }
        if self.commands > parent.commands {
            out.push(Widening::Commands);
        }
        if self
            .publication
            .iter()
            .any(|effect| !parent.publication.contains(effect))
        {
            out.push(Widening::Publication);
        }
        if self.access > parent.access {
            out.push(Widening::Access);
        }
        if !self.os_deny.covers(&parent.os_deny) {
            out.push(Widening::OsDenySet);
        }
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReadScope {
    Workspace,
    /// This computer's developer toolchains (`Access::Toolchains`).
    Toolchains,
    /// The boundary's documented system directories.
    SystemDirectories,
    /// The task store's retained artifacts.
    Artifacts,
    /// One explicitly granted path.
    Path {
        path: String,
    },
}

/// Ordered from least to most.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteScope {
    None,
    /// An isolated Git worktree whose common Git directory is outside it.
    IsolatedWorktree,
    /// The granted workspace itself.
    Workspace,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Network {
    None,
    Localhost,
    /// Only these hosts.
    Destinations {
        hosts: Vec<String>,
    },
    Open,
}

impl Network {
    fn within(&self, parent: &Network) -> bool {
        match (self, parent) {
            (Network::None, _) | (_, Network::Open) => true,
            (Network::Localhost, Network::Localhost) => true,
            (Network::Destinations { hosts }, Network::Destinations { hosts: allowed }) => {
                hosts.iter().all(|host| allowed.contains(host))
            }
            _ => false,
        }
    }
}

/// Ordered from least to most.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandScope {
    None,
    /// Read-only commands from the computer's own command tree (13.4).
    ReadOnlyTree,
    /// One exact admitted command (`bounded-command`).
    Bounded,
    /// A coding engine's own tools inside the boundary.
    EngineTools,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Publication {
    Commit,
    Push,
    OpenPullRequest,
    Merge,
    /// Comment on, claim, or close an issue (13.5).
    IssueUpdate,
    /// Sign and send a Nostr event.
    PublishEvent,
    Deploy,
    /// Install or enable a plugin or standing rule.
    Install,
}

/// Ordered from least to most. `Full` requires an explicit operator choice
/// and is visible here (section 9).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    Boundary,
    Toolchains,
    Full,
}

/// The operating-system deny set (13.8). On macOS, executors run in a
/// sandbox that refuses the privacy-protected locations and control of
/// other apps, so the system never shows a permission dialog. A route that
/// needs one of them is a new explicit grant, never a dialog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OsDenySet {
    pub platform: Platform,
    pub locations: Vec<Protected>,
    /// Refuse Apple Events and other control of other apps.
    pub app_control: bool,
}

impl OsDenySet {
    /// The default macOS set: every protected location and app control.
    #[must_use]
    pub fn macos() -> Self {
        Self {
            platform: Platform::Macos,
            locations: Protected::ALL.to_vec(),
            app_control: true,
        }
    }

    /// Whether this set refuses at least what `parent` refuses.
    #[must_use]
    pub fn covers(&self, parent: &OsDenySet) -> bool {
        parent
            .locations
            .iter()
            .all(|location| self.locations.contains(location))
            && (self.app_control || !parent.app_control)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Macos,
    Linux,
}

/// macOS privacy-protected locations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protected {
    Music,
    Photos,
    Documents,
    Desktop,
    Downloads,
    Mail,
    Messages,
    Contacts,
    Calendars,
    IcloudDrive,
}

impl Protected {
    pub const ALL: [Protected; 10] = [
        Protected::Music,
        Protected::Photos,
        Protected::Documents,
        Protected::Desktop,
        Protected::Downloads,
        Protected::Mail,
        Protected::Messages,
        Protected::Contacts,
        Protected::Calendars,
        Protected::IcloudDrive,
    ];
}

// ---- Disclosure ----

/// Who may receive what. Repository text, plugin output, and model text
/// cannot edit this list (section 9).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Disclosure {
    pub recipients: Vec<Recipient>,
    pub context: Vec<ContentClass>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ContentClass>,
}

impl Disclosure {
    /// Whether every recipient and class here is in `parent`.
    #[must_use]
    pub fn within(&self, parent: &Disclosure) -> bool {
        self.recipients
            .iter()
            .all(|recipient| parent.recipients.contains(recipient))
            && self
                .context
                .iter()
                .all(|class| parent.context.contains(class))
            && self
                .artifacts
                .iter()
                .all(|class| parent.artifacts.contains(class))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipient {
    pub kind: RecipientKind,
    /// `anthropic`, `openai`, `openrouter`, `typesafe`, a plugin id, ...
    pub id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecipientKind {
    ModelProvider,
    DecisionProvider,
    ToolProvider,
    Plugin,
    OpenAgents,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentClass {
    Message,
    Thread,
    RepositorySource,
    CommandOutput,
    Attachments,
    Patch,
    RunSummary,
}

// ---- Resources ----

/// The operator's execution constraints, the observed capacity, and the
/// caller's own ceilings (D16: the caller's choices; we impose none on our
/// apps' users).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_bytes: Option<u64>,
    /// Concurrent runs the operator allows this route (a dispatch plan's
    /// N is bounded by it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_parallel: Option<u32>,
    pub capacity: Capacity,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caller_limits: Vec<CallerLimit>,
}

/// Unknown capacity is not evidence of availability.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Capacity {
    Available,
    Limited { until: Option<u64> },
    Unknown,
    NotNeeded,
}

/// One of the caller's own limits (D16), named as the `limit_reached`
/// error names it, such as `spend_day` or `max_price_sats`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallerLimit {
    pub limit: String,
    pub value: u64,
}

// ---- Money ----

/// Who pays for each resource, and the price terms (13.6, section 8).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Money {
    /// The person's provider-key mode (BYOK, #10176).
    pub byok: ByokMode,
    pub payers: Vec<PayerEntry>,
    pub funding: Funding,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_book: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<Quote>,
    /// Plugin authors' declared fees (D7, D9).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fees: Vec<Fee>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reservation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement: Option<String>,
    /// Show the cost to the caller: API callers yes; our own apps record
    /// costs without showing them (13.6).
    pub shown: bool,
}

impl Money {
    /// Whether any resource the caller paid for in `parent` is paid by
    /// someone else here. Under `mine`, nothing falls back to ours; it
    /// fails plainly instead.
    #[must_use]
    pub fn switches_payer_from(&self, parent: &Money) -> bool {
        (parent.byok == ByokMode::Mine && self.byok != ByokMode::Mine)
            || parent.payers.iter().any(|before| {
                before.payer != Payer::OpenAgents
                    && self
                        .payers
                        .iter()
                        .any(|now| now.resource == before.resource && now.payer != before.payer)
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ByokMode {
    Ours,
    Mine,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayerEntry {
    pub resource: Resource,
    pub payer: Payer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resource {
    ChatModel,
    Decision,
    Embedding,
    Executor,
    Judge,
    PluginFee,
    Routing,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Payer {
    OpenAgents,
    /// The caller's own provider key: `openrouter`, `vercel`, or
    /// `typesafe`.
    CallerKey {
        provider: String,
    },
    /// A coding engine on the person's own login (Claude Code, Codex, ...).
    CallerLogin {
        engine: String,
    },
}

/// How the caller's charges are funded (D3, D8, D13).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Funding {
    /// OpenAgents' own apps: nothing charged.
    None,
    FreeTier,
    Balance,
    PerCall,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quote {
    pub id: String,
    pub max_sats: u64,
    /// `fixed` or `reservation`; a route-dependent task has no exact
    /// upfront price.
    pub basis: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fee {
    pub plugin: String,
    pub author: String,
    pub sats: u64,
}

// ---- Evidence ----

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub deliverables: Vec<Deliverable>,
    pub check: CheckScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checker: Option<CapabilityPin>,
    /// Days the artifacts are retained; absent keeps the store's policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_days: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Deliverable {
    Answer,
    CommandOutput,
    PluginOutput,
    Patch,
    RetainedArtifacts,
    /// One result per run of a dispatch plan, plus a summary composed from
    /// them when asked.
    RunResults,
    LandedCommit,
    Rule,
    PluginPackage,
}

/// What a check establishes. These are different facts; none proves every
/// kind of outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckScope {
    None,
    ExecutorExit,
    ModelJudgment,
    /// The task owner's independent suites (`coder task check`).
    IndependentSuite,
    /// The issue flow's gate: tests retried, then compared with the base
    /// (13.5).
    IssueGate,
    BuyerAcceptance,
}

// ---- Defaults applied ----

/// A product default the router used instead of a question (13.1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefaultApplied {
    pub default: DefaultKind,
    /// What it resolved to, such as `spark` or `codex,claude`.
    pub value: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultKind {
    /// The built-in wallet (Spark), never "which wallet?".
    BuiltInWallet,
    ThisComputer,
    CurrentProject,
    /// The thread's running or last session.
    CurrentSession,
    /// Every signed-in, enabled coding agent (opt-out).
    SignedInEngines,
    /// The measured delegate settings (13.10).
    DelegateSettings,
}
