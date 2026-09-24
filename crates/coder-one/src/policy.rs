//! The policy manifest: every component's implementation and parameters,
//! resolved once per episode into an immutable configuration.
//!
//! A manifest is a JSON document with the schema
//! `openagents.coder-one.policy.v1`. It names how Jev is used, which
//! evidence the host gathers, how the episode is controlled, how the
//! briefing is built, and which executor runs it. The `protected` part
//! holds what no candidate may change: the isolation boundary, the effect
//! policy, the acceptance rule, and the resource ceilings.
//!
//! Resolution starts from the manifest `CODER_ONE_POLICY` names (a path, or
//! the JSON itself when it starts with `{`), or from the built-in default
//! when it is unset. The older environment switches, such as
//! `CODER_ONE_PROBE_V2` or `CODER_ONE_DELEGATE_EFFORT`, still work: each
//! one set is applied on top as an override, and the resolved record lists
//! it. The episode then reads its configuration only from the resolved
//! manifest, and records the manifest and its digest.
//!
//! The digest is the SHA-256 of the manifest's canonical JSON (object keys
//! sorted, no whitespace) with `name`, `note`, and `search` removed, so two
//! arms that run the same configuration share one digest. A manifest file
//! that spells out every field has the same digest as its resolution.
//!
//! A field is searchable, that is, a study may vary it, only when a canary
//! test shows that changing it reaches what the executor is invoked with.
//! [`SEARCHABLE`] lists those fields, and resolution refuses a `search`
//! entry outside it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::credentials::JEV_MODEL;
use crate::delegate::{self, Agent, Cli, Credential, Mode};

/// The manifest schema this build reads.
pub const SCHEMA: &str = "openagents.coder-one.policy.v1";

/// The revision of the question sets built into this crate. The only one
/// a manifest may name.
pub const QUESTION_SETS: &str = "builtin-v1";

/// The protected isolation boundary: the task container.
pub const ISOLATION: &str = "task-container";

/// The protected effect policy: the executor bypasses its own sandbox and
/// approval prompts because the container is the boundary.
pub const EFFECT_POLICY: &str = "executor-bypasses-its-own-sandbox";

/// The protected acceptance rule: the task's own verifier grades the final
/// state; nothing the episode says is a grade.
pub const ACCEPTANCE: &str = "task-verifier";

/// The variable that names a manifest.
pub const POLICY_VAR: &str = "CODER_ONE_POLICY";

/// Fields a study may vary. Each has a canary test in this module that
/// shows a change reaches the invoked executor or what it is given.
pub const SEARCHABLE: &[&str] = &[
    "policy.jev.mode",
    "policy.evidence.probes",
    "policy.evidence.survey_files",
    "policy.control.delegate",
    "policy.control.explore_steps",
    "policy.brief.cap",
    "policy.brief.directions",
    "policy.brief.packer",
    "policy.brief.pack.slice",
    "policy.brief.pack.item_max",
    "policy.brief.pack.instruction_share",
    "policy.executor.agent",
    "policy.executor.model",
    "policy.executor.effort",
    "policy.executor.tools",
    "policy.executor.prompt_cache_ttl",
    "policy.executor.deadline_sec",
    "policy.executor.system",
];

/// A policy manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    /// A label for people. Not part of the digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A note for people. Not part of the digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The fields a study may vary around this manifest. Not part of the
    /// digest; every entry must be in [`SEARCHABLE`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub search: Vec<String>,
    /// What a candidate may change.
    pub policy: Components,
    /// What no candidate may change.
    pub protected: Protected,
}

/// Every component's implementation and parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Components {
    pub jev: JevPolicy,
    pub evidence: EvidencePolicy,
    pub control: ControlPolicy,
    pub brief: BriefPolicy,
    pub executor: ExecutorPolicy,
    /// What runs after the executor in a Terminal-Bench episode:
    /// `verify.checks`, `verify.support`, and one `verify.repair`. Absent,
    /// nothing checks the work, and the manifest's digest is what it was
    /// before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify: Option<crate::compose::VerifyPolicy>,
}

/// How Jev is used.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JevPolicy {
    /// `off` runs the search-hit baseline; `step` asks each step; `deep`
    /// adds the survey, a readiness question, and repeated-command hints.
    pub mode: JevMode,
    /// The pinned Jev model.
    pub model: String,
    /// The revision of the built-in question sets.
    pub question_sets: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JevMode {
    Off,
    Step,
    Deep,
}

/// What evidence the host gathers before the first step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidencePolicy {
    /// `off`; `battery`, the read-only probe battery; or `v2`, the battery
    /// plus the Jev-gated setup pack, git probes in named repositories,
    /// and whole edit targets. Needs `jev.mode = deep`.
    pub probes: Probes,
    /// The most files the deep survey judges.
    pub survey_files: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Probes {
    Off,
    Battery,
    V2,
}

/// How the episode is controlled: the loop's bounds and when it delegates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlPolicy {
    /// `off`, `always`, or `auto`.
    pub delegate: DelegateMode,
    /// The explore phase's step bound.
    pub explore_steps: usize,
    /// The loop's step limit.
    pub max_steps: usize,
    /// Seconds each explorer command may run.
    pub command_timeout_sec: u64,
    /// The generation lane.
    pub lane: String,
    /// Under `auto`, consecutive Jev `error` outcomes that escalate.
    pub error_streak: usize,
    /// Under `auto`, steps with an unchanged checkout that escalate.
    pub unchanged_steps: usize,
    /// `control.monitor` over each executor session, in shadow mode: its
    /// judgments are recorded and never acted on. Absent, nothing watches,
    /// and the manifest's digest is what it was before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitor: Option<crate::monitor::Params>,
    /// `control.handoff`: escalate, plan and implement, steer, or race
    /// within the episode's one budget. Absent, one executor runs from
    /// start to finish, and the manifest's digest is unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff: Option<crate::handoff::Policy>,
    /// `control.route`: task.profile's features pick the first executor.
    /// Absent, the manifest's executor starts, and the digest is
    /// unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<crate::compose::RoutePolicy>,
    /// `control.horizon`: dispatches, checks, and effort sized from the
    /// episode deadline. Absent, each dispatch asks for
    /// `executor.deadline_sec`, and the digest is unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizon: Option<crate::compose::Horizon>,
    /// `control.persist`: fresh executor rounds while a long task has time
    /// left. Absent, the episode ends after the last verification step,
    /// and the digest is unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persist: Option<crate::compose::PersistPolicy>,
    /// `control.effort`: Jev's effort-sensitivity battery picks the effort
    /// a long task runs at, in place of `control.horizon.long_effort`.
    /// Absent, the horizon's effort holds, and the digest is unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<crate::effort::EffortPolicy>,
    /// `control.best_of`: N candidates of the first executor at once, each
    /// in its own copy of the workspace, and the one the calibrated
    /// verdict, then the checks, then cost rank first kept. Absent, one
    /// candidate runs, and the digest is unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_of: Option<crate::compose::best_of::BestOfPolicy>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DelegateMode {
    Off,
    Always,
    Auto,
}

impl DelegateMode {
    #[must_use]
    pub fn mode(self) -> Mode {
        match self {
            DelegateMode::Off => Mode::Off,
            DelegateMode::Always => Mode::Always,
            DelegateMode::Auto => Mode::Auto,
        }
    }

    fn from_mode(mode: Mode) -> Self {
        match mode {
            Mode::Off => DelegateMode::Off,
            Mode::Always => DelegateMode::Always,
            Mode::Auto => DelegateMode::Auto,
        }
    }
}

/// How the briefing is built.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BriefPolicy {
    /// The briefing's length cap in characters.
    pub cap: usize,
    /// The closing directions.
    pub directions: Directions,
    /// How the briefing is packed. Left out of a manifest that keeps the
    /// first packer, so earlier manifests keep their digests.
    #[serde(default, skip_serializing_if = "Packer::is_sections")]
    pub packer: Packer,
    /// The coverage packer's slice, span, and task-text reserve. Left out
    /// of a manifest that keeps the packer's defaults, so earlier
    /// manifests keep their digests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack: Option<PackPolicy>,
}

impl BriefPolicy {
    /// The coverage packer's parameters under this policy: the cap, plus
    /// `pack` when set, over the packer's defaults.
    #[must_use]
    pub fn pack_params(&self) -> crate::pack::Params {
        let mut params = crate::pack::Params {
            cap: self.cap,
            ..crate::pack::Params::default()
        };
        if let Some(pack) = self.pack {
            params.slice = pack.slice;
            params.item_max = pack.item_max;
            params.instruction_share = pack.instruction_share;
        }
        params
    }
}

/// The coverage packer's searchable parameters. The data-file parameters
/// (`data_head_lines` and `representatives`) stay the packer's defaults:
/// no canary reaches a data file, so a study can't vary them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackPolicy {
    /// The first slice each owed item gets, and the fill's step.
    pub slice: usize,
    /// The most characters any one item delivers.
    pub item_max: usize,
    /// The most of the cap the task text may take before it's trimmed.
    pub instruction_share: f64,
}

impl Default for PackPolicy {
    fn default() -> Self {
        let params = crate::pack::Params::default();
        PackPolicy {
            slice: params.slice,
            item_max: params.item_max,
            instruction_share: params.instruction_share,
        }
    }
}

/// How the briefing is packed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Packer {
    /// The first packer: probes, then files, each section whole or left
    /// out, in priority order.
    #[default]
    Sections,
    /// `evidence.pack` by requirement coverage: joint ranking, duplicate
    /// listings removed, representative records, trimmed spans, and every
    /// omission named.
    Coverage,
    /// The coverage packer with Jev's coverage judgments deciding which
    /// requirements each item informs.
    CoverageJev,
}

impl Packer {
    fn is_sections(&self) -> bool {
        *self == Packer::Sections
    }
}

/// The briefing's closing directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Directions {
    /// The episode contract.
    Plain,
    /// The contract plus batch mode: few, large steps and one final check.
    Batch,
    /// Batch mode that tests every changed code path.
    BatchChecked,
}

impl Directions {
    /// The directions' text.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Directions::Plain => EPISODE_DIRECTIONS,
            Directions::Batch => EPISODE_DIRECTIONS_BATCH,
            Directions::BatchChecked => EPISODE_DIRECTIONS_BATCH_CHECKED,
        }
    }
}

/// Which executor runs the briefing, and how.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutorPolicy {
    pub agent: AgentName,
    /// The CLI version the harness installs, when it pins one. The episode
    /// doctor refuses a different installed version.
    pub version: Option<String>,
    pub model: String,
    /// Reasoning effort: Claude Code's `--effort` or Codex's
    /// `model_reasoning_effort`. `null` keeps the CLI's default.
    pub effort: Option<String>,
    /// Claude Code's built-in tools, such as `Bash,Read,Edit,Write`.
    /// `null` keeps the full default set.
    pub tools: Option<String>,
    /// Claude Code's prompt-cache TTL (`5m` or `1h`). `null` keeps the
    /// CLI's default.
    pub prompt_cache_ttl: Option<String>,
    /// Seconds one dispatch may run.
    pub deadline_sec: u64,
    /// The system prompt the executor is sent (`exec.system`): sections
    /// from the library and how they reach the CLI. Absent, the executor
    /// runs its own default prompt, and the manifest's digest is what it
    /// was before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<crate::system::Policy>,
    /// Session control beyond start, observe, and the deadline stop
    /// (`exec.session`): a steer rule, an early stop rule, and a resume
    /// message. Absent, the session runs to its end or its deadline, and
    /// the manifest's digest is what it was before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionPolicy>,
    /// Microluna's loop and bounds (`exec.microluna`), for the `microluna`
    /// agent only. Absent, a Microluna executor runs the default loop, and
    /// the manifest's digest is what it was before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub microluna: Option<crate::micro::Policy>,
}

/// What a policy asks of a running executor session. Each rule needs a
/// capability its adapter has demonstrated, which [`Manifest::validate`]
/// checks against [`crate::adapter::capabilities`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steer: Option<crate::session::Steer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_when: Option<crate::session::Trigger>,
    /// After a stop, resume the same session with this message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume: Option<String>,
}

impl SessionPolicy {
    /// The capabilities the rules use.
    #[must_use]
    pub fn uses(&self) -> Vec<crate::session::Capability> {
        use crate::session::Capability;
        let mut uses = vec![Capability::Start, Capability::Observe];
        if self.steer.is_some() {
            uses.push(Capability::Steer);
        }
        if self.stop_when.is_some() || self.resume.is_some() {
            uses.push(Capability::Stop);
        }
        if self.resume.is_some() {
            uses.push(Capability::Resume);
        }
        uses
    }

    /// The host loop's controls; the deadline is set per dispatch.
    #[must_use]
    pub fn controls(&self) -> crate::session::Controls {
        crate::session::Controls {
            steer: self.steer.clone(),
            stop_when: self.stop_when.clone(),
            resume: self.resume.clone(),
            ..crate::session::Controls::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentName {
    ClaudeCode,
    Codex,
    Microluna,
}

impl AgentName {
    #[must_use]
    pub fn agent(self) -> Agent {
        match self {
            AgentName::ClaudeCode => Agent::ClaudeCode,
            AgentName::Codex => Agent::Codex,
            AgentName::Microluna => Agent::Microluna,
        }
    }

    fn from_agent(agent: Agent) -> Self {
        match agent {
            Agent::ClaudeCode => AgentName::ClaudeCode,
            Agent::Codex => AgentName::Codex,
            Agent::Microluna => AgentName::Microluna,
        }
    }
}

/// What no candidate may change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Protected {
    pub isolation: String,
    pub effect_policy: String,
    pub acceptance: String,
    pub ceilings: Ceilings,
}

/// The resource ceilings a candidate allocates within.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ceilings {
    /// The whole episode's monotonic deadline in seconds; `null` leaves the
    /// harness's own timeout as the only limit.
    pub episode_deadline_sec: Option<u64>,
    /// Seconds of the deadline kept back for final checks, cleanup, and
    /// recording.
    pub reserve_sec: u64,
    /// A spend bound checked between components. Soft: a running dispatch
    /// can pass it.
    pub spend_soft_usd: Option<f64>,
    /// A hard spend cap. Refused unless `null`: no executor adapter
    /// reserves a known maximum charge before a model call.
    pub spend_hard_usd: Option<f64>,
}

/// One environment override applied during resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Override {
    /// The variable or flag that set it.
    pub source: String,
    /// The manifest field it set.
    pub field: String,
    /// The value it set, as the manifest spells it.
    pub value: String,
}

/// A resolved manifest: where it came from, what overrode it, and the
/// immutable configuration the episode reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolution {
    pub manifest: Manifest,
    /// `builtin`, `inline`, or `file:<path>`.
    pub source: String,
    pub overrides: Vec<Override>,
    /// What resolution noticed but did not apply, such as a switch that
    /// needs another.
    pub notes: Vec<String>,
}

impl Manifest {
    /// The built-in default, equal to what an episode ran with before any
    /// environment switch.
    #[must_use]
    pub fn builtin() -> Self {
        Manifest {
            schema: SCHEMA.to_string(),
            name: Some("builtin".to_string()),
            note: None,
            search: Vec::new(),
            policy: Components {
                jev: JevPolicy {
                    mode: JevMode::Step,
                    model: JEV_MODEL.to_string(),
                    question_sets: QUESTION_SETS.to_string(),
                },
                evidence: EvidencePolicy {
                    probes: Probes::Off,
                    survey_files: crate::judge::SURVEY_FILES,
                },
                control: ControlPolicy {
                    delegate: DelegateMode::Off,
                    explore_steps: 8,
                    max_steps: 50,
                    command_timeout_sec: 300,
                    lane: "free".to_string(),
                    error_streak: 3,
                    unchanged_steps: 6,
                    monitor: None,
                    handoff: None,
                    route: None,
                    horizon: None,
                    persist: None,
                    effort: None,
                    best_of: None,
                },
                brief: BriefPolicy {
                    cap: delegate::BRIEFING_CAP,
                    directions: Directions::Plain,
                    packer: Packer::Sections,
                    pack: None,
                },
                executor: ExecutorPolicy {
                    agent: AgentName::ClaudeCode,
                    version: None,
                    model: delegate::DEFAULT_MODEL.to_string(),
                    effort: None,
                    tools: None,
                    prompt_cache_ttl: None,
                    deadline_sec: 600,
                    system: None,
                    session: None,
                    microluna: None,
                },
                verify: None,
            },
            protected: Protected {
                isolation: ISOLATION.to_string(),
                effect_policy: EFFECT_POLICY.to_string(),
                acceptance: ACCEPTANCE.to_string(),
                ceilings: Ceilings {
                    episode_deadline_sec: None,
                    reserve_sec: 30,
                    spend_soft_usd: None,
                    spend_hard_usd: None,
                },
            },
        }
    }

    /// Reads a manifest from its JSON text.
    pub fn parse(text: &str) -> Result<Self, String> {
        serde_json::from_str(text)
            .map_err(|error| format!("the policy manifest is invalid: {error}"))
    }

    /// The manifest's digest: SHA-256 of its canonical JSON without
    /// `name`, `note`, and `search`.
    #[must_use]
    pub fn digest(&self) -> String {
        digest_value(&serde_json::to_value(self).unwrap_or(Value::Null))
    }

    /// Refuses a manifest this build cannot run as written.
    pub fn validate(&self) -> Result<(), String> {
        let mut problems = Vec::new();
        let policy = &self.policy;
        if self.schema != SCHEMA {
            problems.push(format!("schema must be {SCHEMA}, not {}", self.schema));
        }
        if policy.jev.model != JEV_MODEL {
            problems.push(format!(
                "jev.model must be the pinned {JEV_MODEL}, not {}",
                policy.jev.model
            ));
        }
        if policy.jev.question_sets != QUESTION_SETS {
            problems.push(format!(
                "jev.question_sets must be {QUESTION_SETS}, not {}",
                policy.jev.question_sets
            ));
        }
        if policy.evidence.probes != Probes::Off && policy.jev.mode != JevMode::Deep {
            problems.push("evidence.probes needs jev.mode = deep".to_string());
        }
        if !(1..=crate::judge::SURVEY_FILES).contains(&policy.evidence.survey_files) {
            problems.push(format!(
                "evidence.survey_files must be 1 to {}",
                crate::judge::SURVEY_FILES
            ));
        }
        if policy.control.max_steps == 0 {
            problems.push("control.max_steps must be at least 1".to_string());
        }
        if policy.control.command_timeout_sec == 0 {
            problems.push("control.command_timeout_sec must be at least 1".to_string());
        }
        if policy.control.lane.trim().is_empty() {
            problems.push("control.lane must name a lane".to_string());
        }
        if policy.brief.cap < 1_000 {
            problems.push("brief.cap must be at least 1000 characters".to_string());
        }
        if policy.brief.packer == Packer::CoverageJev && policy.jev.mode == JevMode::Off {
            problems.push("brief.packer = coverage-jev needs Jev".to_string());
        }
        if let Some(pack) = policy.brief.pack {
            if policy.brief.packer == Packer::Sections {
                problems.push("brief.pack applies only to the coverage packers".to_string());
            }
            if !(200..=policy.brief.cap).contains(&pack.slice) {
                problems.push(format!(
                    "brief.pack.slice must be 200 to brief.cap, not {}",
                    pack.slice
                ));
            }
            if pack.item_max < pack.slice {
                problems.push("brief.pack.item_max must be at least brief.pack.slice".to_string());
            }
            if !(0.1..=0.9).contains(&pack.instruction_share) {
                problems.push(format!(
                    "brief.pack.instruction_share must be 0.1 to 0.9, not {}",
                    pack.instruction_share
                ));
            }
        }
        let executor = &policy.executor;
        if executor.model.trim().is_empty() {
            problems.push("executor.model must name a model".to_string());
        }
        if executor.deadline_sec == 0 {
            problems.push("executor.deadline_sec must be at least 1".to_string());
        }
        if let Some(effort) = &executor.effort
            && (effort.is_empty() || !effort.chars().all(|c| c.is_ascii_lowercase()))
        {
            problems.push(format!(
                "executor.effort must be one lowercase word, not {effort:?}"
            ));
        }
        if executor.agent == AgentName::Microluna {
            if executor.version.is_some() {
                problems.push(
                    "executor.version pins a CLI; Microluna runs in this process, so it takes none"
                        .to_string(),
                );
            }
            if executor.system.is_some() {
                problems.push(
                    "executor.system applies to claude-code and codex; Microluna sends its own instructions"
                        .to_string(),
                );
            }
            if executor.session.is_some() {
                problems.push(
                    "executor.session needs stop, resume, or steer, which Microluna doesn't offer"
                        .to_string(),
                );
            }
        }
        if let Some(micro) = &executor.microluna {
            if executor.agent != AgentName::Microluna {
                problems.push("executor.microluna applies only to the microluna agent".to_string());
            }
            problems.extend(micro.validate());
        }
        if executor.agent != AgentName::ClaudeCode {
            if executor.tools.is_some() {
                problems.push("executor.tools applies only to claude-code".to_string());
            }
            if executor.prompt_cache_ttl.is_some() {
                problems.push("executor.prompt_cache_ttl applies only to claude-code".to_string());
            }
        }
        if let Some(tools) = &executor.tools
            && (tools.trim().is_empty()
                || !tools.split(',').all(|tool| {
                    !tool.is_empty() && tool.chars().all(|c| c.is_ascii_alphanumeric())
                }))
        {
            problems.push(format!(
                "executor.tools must be comma-separated tool names, not {tools:?}"
            ));
        }
        if let Some(ttl) = &executor.prompt_cache_ttl
            && !matches!(ttl.as_str(), "5m" | "1h")
        {
            problems.push(format!(
                "executor.prompt_cache_ttl must be 5m or 1h, not {ttl}"
            ));
        }
        if let Some(version) = &executor.version
            && !version
                .split('.')
                .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
        {
            problems.push(format!(
                "executor.version must be a dotted version, not {version}"
            ));
        }
        if let Some(system) = &executor.system {
            problems.extend(system.validate(executor.agent.agent()));
        }
        if let Some(handoff) = &self.policy.control.handoff {
            let first = crate::handoff::Tier::new(
                match executor.agent {
                    AgentName::ClaudeCode => "claude-code",
                    AgentName::Codex => "codex",
                    AgentName::Microluna => "microluna",
                },
                &executor.model,
            );
            if let Err(problem) = handoff.check(&first) {
                problems.push(problem);
            }
        }
        if let Some(route) = &self.policy.control.route {
            problems.extend(route.validate());
            if policy.control.delegate == DelegateMode::Off {
                problems.push("control.route needs a delegate mode other than off".to_string());
            }
        }
        if let Some(horizon) = &self.policy.control.horizon {
            problems.extend(horizon.validate());
        }
        if let Some(effort) = &self.policy.control.effort {
            problems.extend(effort.validate());
            if self
                .policy
                .control
                .horizon
                .as_ref()
                .is_none_or(|h| h.long_after_sec.is_none())
            {
                problems.push(
                    "control.effort needs control.horizon.long_after_sec: it picks a long task's effort"
                        .to_string(),
                );
            }
        }
        if let Some(persist) = &self.policy.control.persist {
            problems.extend(persist.validate());
            if policy.control.delegate == DelegateMode::Off {
                problems.push("control.persist needs a delegate mode other than off".to_string());
            }
            if self.policy.verify.as_ref().is_some_and(|v| !v.checks) {
                problems.push("control.persist needs verify.checks".to_string());
            }
            if persist.long_only
                && self
                    .policy
                    .control
                    .horizon
                    .as_ref()
                    .is_none_or(|h| h.long_after_sec.is_none())
            {
                problems.push(
                    "control.persist.long_only needs control.horizon.long_after_sec".to_string(),
                );
            }
        }
        if let Some(best_of) = &self.policy.control.best_of {
            problems.extend(best_of.validate());
            if policy.control.delegate == DelegateMode::Off {
                problems.push("control.best_of needs a delegate mode other than off".to_string());
            }
            if self
                .policy
                .control
                .handoff
                .as_ref()
                .is_some_and(|h| h.pattern != crate::handoff::Pattern::Single)
            {
                problems.push(
                    "control.best_of runs the first executor N times; it takes no control.handoff"
                        .to_string(),
                );
            }
            if !self
                .policy
                .verify
                .as_ref()
                .is_some_and(|v| v.checks && v.verdict)
            {
                problems.push(
                    "control.best_of needs verify.checks and verify.verdict to rank its candidates"
                        .to_string(),
                );
            }
        }
        if let Some(to) = self
            .policy
            .control
            .handoff
            .as_ref()
            .and_then(|handoff| handoff.to.as_ref())
        {
            problems.extend(to.validate("control.handoff.to"));
        }
        if let Some(verify) = &self.policy.verify {
            if verify.repair.is_some() && !verify.checks {
                problems.push("verify.repair needs verify.checks".to_string());
            }
            if verify.support && policy.jev.mode == JevMode::Off {
                problems.push("verify.support needs Jev".to_string());
            }
            if policy.control.delegate == DelegateMode::Off {
                problems.push("verify needs a delegate mode other than off".to_string());
            }
            problems.extend(verify.validate());
        }
        if let Some(session) = &executor.session {
            let (demonstrated, _) = crate::adapter::capabilities(executor.agent.agent());
            for capability in session.uses() {
                if !demonstrated.has(capability) {
                    problems.push(format!(
                        "executor.session uses {}, which the {} adapter has not demonstrated",
                        capability.word(),
                        executor.agent.agent().word()
                    ));
                }
            }
            if session.resume.is_some() && session.stop_when.is_none() {
                problems.push(
                    "executor.session.resume needs a stop_when rule: only a stopped session resumes"
                        .to_string(),
                );
            }
        }
        let protected = &self.protected;
        for (field, value, expected) in [
            ("isolation", &protected.isolation, ISOLATION),
            ("effect_policy", &protected.effect_policy, EFFECT_POLICY),
            ("acceptance", &protected.acceptance, ACCEPTANCE),
        ] {
            if value != expected {
                problems.push(format!("protected.{field} must be {expected}, not {value}"));
            }
        }
        let ceilings = &protected.ceilings;
        if ceilings.spend_hard_usd.is_some() {
            problems.push(
                "protected.ceilings.spend_hard_usd must be null: no executor adapter reserves a \
                 known maximum charge, so a hard dollar cap cannot be enforced"
                    .to_string(),
            );
        }
        if ceilings
            .spend_soft_usd
            .is_some_and(|usd| usd.is_nan() || usd < 0.0)
        {
            problems.push("protected.ceilings.spend_soft_usd must be zero or more".to_string());
        }
        if let Some(deadline) = ceilings.episode_deadline_sec
            && deadline <= ceilings.reserve_sec
        {
            problems.push(
                "protected.ceilings.episode_deadline_sec must be longer than reserve_sec"
                    .to_string(),
            );
        }
        for field in &self.search {
            if field.starts_with("protected.") {
                problems.push(format!("{field} is protected and cannot be searched"));
            } else if !SEARCHABLE.contains(&field.as_str()) {
                problems.push(format!(
                    "{field} has no passing canary, so it cannot be searched"
                ));
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "the policy manifest is refused: {}",
                problems.join("; ")
            ))
        }
    }

    /// The delegate's escalation policy.
    #[must_use]
    pub fn escalation(&self) -> delegate::Policy {
        let control = &self.policy.control;
        delegate::Policy {
            explore_steps: control.explore_steps,
            error_streak: control.error_streak,
            unchanged_steps: control.unchanged_steps,
        }
    }

    /// The delegate mode.
    #[must_use]
    pub fn mode(&self) -> Mode {
        self.policy.control.delegate.mode()
    }

    /// Whether Jev runs at all.
    #[must_use]
    pub fn jev(&self) -> bool {
        self.policy.jev.mode != JevMode::Off
    }

    /// Whether Jev runs in deep mode.
    #[must_use]
    pub fn deep(&self) -> bool {
        self.policy.jev.mode == JevMode::Deep
    }

    /// The judge this manifest configures.
    #[must_use]
    pub fn judge(
        &self,
        client: Option<jev::Client>,
        workdir: PathBuf,
        issue: &crate::state::Issue,
        recorder: crate::record::Recorder,
    ) -> crate::judge::JevJudge {
        let probes = self.policy.evidence.probes;
        crate::judge::JevJudge::new(client.filter(|_| self.jev()), workdir, issue, recorder)
            .deep(self.deep())
            .probing(self.deep() && probes != Probes::Off)
            .probe_v2(self.deep() && probes == Probes::V2)
            .survey_files(self.policy.evidence.survey_files)
    }

    /// The executor this manifest configures, given what the host found.
    #[must_use]
    pub fn executor(&self, host: ExecutorHost) -> Cli {
        let executor = &self.policy.executor;
        Cli {
            agent: executor.agent.agent(),
            binary: host.binary,
            model: executor.model.clone(),
            deadline: Duration::from_secs(executor.deadline_sec),
            workdir: host.workdir,
            artifacts: host.artifacts,
            artifacts_label: host.artifacts_label,
            env: host.env,
            credential: host.credential,
            effort: executor.effort.clone(),
            tools: executor.tools.clone(),
            prompt_cache_ttl: executor.prompt_cache_ttl.clone(),
            system: executor
                .system
                .clone()
                .map(|system| crate::system::Variant::new(executor.agent.agent(), system)),
            episode: crate::deadline::Deadline::unbounded(),
            gate: None,
            granted: None,
            runs: 0,
            control: crate::delegate::Control {
                controls: executor.session.as_ref().map(SessionPolicy::controls),
                ..crate::delegate::Control::default()
            },
        }
    }
}

/// What the host supplies to an executor besides the policy.
pub struct ExecutorHost {
    pub binary: Option<PathBuf>,
    pub credential: Credential,
    pub workdir: PathBuf,
    pub artifacts: PathBuf,
    pub artifacts_label: String,
    pub env: Vec<(String, String)>,
}

impl Resolution {
    /// Resolves the episode's configuration from `env`: the manifest
    /// `CODER_ONE_POLICY` names, or the built-in default, with each set
    /// environment switch applied on top. `lane` is the `--model` flag.
    pub fn resolve(
        env: impl Fn(&str) -> Option<String>,
        lane: Option<&str>,
    ) -> Result<Self, String> {
        let env = |name: &str| {
            env(name)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };
        let (manifest, source) = match env(POLICY_VAR) {
            None => (Manifest::builtin(), "builtin".to_string()),
            Some(text) if text.starts_with('{') => (Manifest::parse(&text)?, "inline".to_string()),
            Some(path) => {
                let text = std::fs::read_to_string(&path)
                    .map_err(|error| format!("cannot read {POLICY_VAR} {path}: {error}"))?;
                (Manifest::parse(&text)?, format!("file:{path}"))
            }
        };
        // A manifest must be valid as written, before any override.
        if source != "builtin" {
            manifest.validate()?;
        }
        let mut resolution = Resolution {
            manifest,
            source,
            overrides: Vec::new(),
            notes: Vec::new(),
        };
        resolution.apply(&env, lane)?;
        resolution.manifest.validate()?;
        Ok(resolution)
    }

    /// Resolves one manifest with no environment at all.
    pub fn of(manifest: Manifest) -> Result<Self, String> {
        manifest.validate()?;
        Ok(Resolution {
            manifest,
            source: "inline".to_string(),
            overrides: Vec::new(),
            notes: Vec::new(),
        })
    }

    /// The resolved manifest's digest.
    #[must_use]
    pub fn digest(&self) -> String {
        self.manifest.digest()
    }

    /// What the episode manifest records under `policy`.
    #[must_use]
    pub fn record(&self) -> Value {
        let sha = |text: &str| hex(&Sha256::digest(text.as_bytes()));
        json!({
            "schema": SCHEMA,
            "name": self.manifest.name,
            "digest": self.digest(),
            "source": self.source,
            "overrides": self.overrides,
            "notes": self.notes,
            "manifest": self.manifest,
            "text_digests": {
                "directions": sha(self.manifest.policy.brief.directions.text()),
                "episode_instructions": sha(crate::agent::EPISODE_INSTRUCTIONS),
                "explore_prompt": sha(delegate::EXPLORE_PROMPT),
                "system": self.manifest.policy.executor.system.clone().map(|system| {
                    crate::system::Variant::new(self.manifest.policy.executor.agent.agent(), system)
                        .digest()
                }),
            },
        })
    }

    fn set(&mut self, source: &str, field: &str, value: impl Into<String>) {
        self.overrides.push(Override {
            source: source.to_string(),
            field: field.to_string(),
            value: value.into(),
        });
    }

    /// Applies each set environment switch, in the order the older
    /// episode read them.
    fn apply(
        &mut self,
        env: &impl Fn(&str) -> Option<String>,
        lane: Option<&str>,
    ) -> Result<(), String> {
        let on = |value: &str| matches!(value, "on" | "1" | "true");
        let off = |value: &str| matches!(value, "off" | "0" | "false");
        let number = |name: &str| -> Result<Option<u64>, String> {
            env(name)
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| format!("{name} must be a whole number"))
                })
                .transpose()
        };

        let lane = match lane {
            Some(lane) => Some(("--model", lane.to_string())),
            None => env("OPENAGENTS_MODEL").map(|lane| ("OPENAGENTS_MODEL", lane)),
        };
        if let Some((source, lane)) = lane
            && lane != self.manifest.policy.control.lane
        {
            self.manifest.policy.control.lane.clone_from(&lane);
            self.set(source, "policy.control.lane", lane);
        }

        let jev = &mut self.manifest.policy.jev;
        let mut changes = Vec::new();
        if let Some(value) = env("CODER_ONE_JEV") {
            if off(&value) {
                jev.mode = JevMode::Off;
                changes.push(("CODER_ONE_JEV", "off"));
            } else if jev.mode == JevMode::Off {
                jev.mode = JevMode::Step;
                changes.push(("CODER_ONE_JEV", "step"));
            }
        }
        if let Some(value) = env("CODER_ONE_DEEP") {
            if on(&value) && jev.mode != JevMode::Off {
                jev.mode = JevMode::Deep;
                changes.push(("CODER_ONE_DEEP", "deep"));
            } else if off(&value) && jev.mode == JevMode::Deep {
                jev.mode = JevMode::Step;
                changes.push(("CODER_ONE_DEEP", "step"));
            }
        }
        for (source, value) in changes {
            self.set(source, "policy.jev.mode", value);
        }

        let v2 = env("CODER_ONE_PROBE_V2");
        if let Some(value) = env("CODER_ONE_PROBES") {
            let evidence = &mut self.manifest.policy.evidence;
            if on(&value) && evidence.probes == Probes::Off {
                evidence.probes = Probes::Battery;
                self.set("CODER_ONE_PROBES", "policy.evidence.probes", "battery");
            } else if off(&value) && evidence.probes != Probes::Off {
                evidence.probes = Probes::Off;
                self.set("CODER_ONE_PROBES", "policy.evidence.probes", "off");
            }
        }
        if let Some(value) = v2 {
            let v3 = value == "v3";
            if on(&value) || v3 {
                let directions = if v3 {
                    Directions::BatchChecked
                } else {
                    Directions::Batch
                };
                self.manifest.policy.brief.directions = directions;
                self.set(
                    "CODER_ONE_PROBE_V2",
                    "policy.brief.directions",
                    if v3 { "batch-checked" } else { "batch" },
                );
                if self.manifest.policy.evidence.probes != Probes::Off {
                    self.manifest.policy.evidence.probes = Probes::V2;
                    self.manifest.policy.evidence.survey_files = crate::judge::SURVEY_FILES_V2;
                    self.set("CODER_ONE_PROBE_V2", "policy.evidence.probes", "v2");
                    self.set(
                        "CODER_ONE_PROBE_V2",
                        "policy.evidence.survey_files",
                        crate::judge::SURVEY_FILES_V2.to_string(),
                    );
                }
            } else if off(&value) {
                self.manifest.policy.brief.directions = Directions::Plain;
                self.set("CODER_ONE_PROBE_V2", "policy.brief.directions", "plain");
                if self.manifest.policy.evidence.probes == Probes::V2 {
                    self.manifest.policy.evidence.probes = Probes::Battery;
                    self.manifest.policy.evidence.survey_files = crate::judge::SURVEY_FILES;
                    self.set("CODER_ONE_PROBE_V2", "policy.evidence.probes", "battery");
                    self.set(
                        "CODER_ONE_PROBE_V2",
                        "policy.evidence.survey_files",
                        crate::judge::SURVEY_FILES.to_string(),
                    );
                }
            }
        }
        // The older switches turned probes on only in deep mode, and
        // silently: keep that for an override, and say so.
        if self.manifest.policy.jev.mode != JevMode::Deep
            && self.manifest.policy.evidence.probes != Probes::Off
        {
            self.manifest.policy.evidence.probes = Probes::Off;
            self.manifest.policy.evidence.survey_files = crate::judge::SURVEY_FILES;
            self.notes
                .push("probes are off: they run only in deep Jev mode".to_string());
        }

        if let Some(steps) = number("CODER_ONE_MAX_STEPS")? {
            self.manifest.policy.control.max_steps = usize::try_from(steps).unwrap_or(50);
            self.set(
                "CODER_ONE_MAX_STEPS",
                "policy.control.max_steps",
                steps.to_string(),
            );
        }
        if let Some(seconds) = number("CODER_ONE_COMMAND_TIMEOUT")? {
            self.manifest.policy.control.command_timeout_sec = seconds;
            self.set(
                "CODER_ONE_COMMAND_TIMEOUT",
                "policy.control.command_timeout_sec",
                seconds.to_string(),
            );
        }
        if let Some(value) = env("CODER_ONE_DELEGATE") {
            let mode = DelegateMode::from_mode(Mode::parse(&value)?);
            self.manifest.policy.control.delegate = mode;
            self.set(
                "CODER_ONE_DELEGATE",
                "policy.control.delegate",
                mode.mode().word(),
            );
        }
        if let Some(steps) = number("CODER_ONE_EXPLORE_STEPS")? {
            self.manifest.policy.control.explore_steps = usize::try_from(steps).unwrap_or(8);
            self.set(
                "CODER_ONE_EXPLORE_STEPS",
                "policy.control.explore_steps",
                steps.to_string(),
            );
        }
        if let Some(cap) = number("CODER_ONE_BRIEFING_CAP")? {
            self.manifest.policy.brief.cap = usize::try_from(cap).unwrap_or(delegate::BRIEFING_CAP);
            self.set(
                "CODER_ONE_BRIEFING_CAP",
                "policy.brief.cap",
                cap.to_string(),
            );
        }

        if let Some(value) = env("CODER_ONE_DELEGATE_AGENT") {
            let agent = AgentName::from_agent(Agent::parse(&value)?);
            if agent != self.manifest.policy.executor.agent {
                let executor = &mut self.manifest.policy.executor;
                executor.agent = agent;
                // Another agent takes its own default model, and a pinned
                // version or Claude-only settings no longer apply.
                executor.model = agent.agent().default_model().to_string();
                executor.version = None;
                if agent == AgentName::Codex {
                    executor.tools = None;
                    executor.prompt_cache_ttl = None;
                }
                self.set(
                    "CODER_ONE_DELEGATE_AGENT",
                    "policy.executor.agent",
                    agent.agent().word(),
                );
            }
        }
        if let Some(model) = env("CODER_ONE_DELEGATE_MODEL") {
            self.manifest.policy.executor.model.clone_from(&model);
            self.set("CODER_ONE_DELEGATE_MODEL", "policy.executor.model", model);
        }
        if let Some(version) = env("CODER_ONE_EXECUTOR_VERSION") {
            self.manifest.policy.executor.version = Some(version.clone());
            self.set(
                "CODER_ONE_EXECUTOR_VERSION",
                "policy.executor.version",
                version,
            );
        }
        let claude = self.manifest.policy.executor.agent == AgentName::ClaudeCode;
        if let Some(tools) = env("CODER_ONE_DELEGATE_TOOLS") {
            if claude {
                self.manifest.policy.executor.tools = Some(tools.clone());
                self.set("CODER_ONE_DELEGATE_TOOLS", "policy.executor.tools", tools);
            } else {
                self.notes.push(
                    "CODER_ONE_DELEGATE_TOOLS ignored: it applies only to claude-code".to_string(),
                );
            }
        }
        if let Some(effort) = env("CODER_ONE_DELEGATE_EFFORT") {
            // The older episode passed only a plain lowercase word.
            if effort.chars().all(|c| c.is_ascii_lowercase()) {
                self.manifest.policy.executor.effort = Some(effort.clone());
                self.set(
                    "CODER_ONE_DELEGATE_EFFORT",
                    "policy.executor.effort",
                    effort,
                );
            } else {
                self.notes.push(format!(
                    "CODER_ONE_DELEGATE_EFFORT ignored: {effort:?} is not one lowercase word"
                ));
            }
        }
        if let Some(ttl) = env("CLAUDE_CODE_PROMPT_CACHE_TTL") {
            if claude {
                self.manifest.policy.executor.prompt_cache_ttl = Some(ttl.clone());
                self.set(
                    "CLAUDE_CODE_PROMPT_CACHE_TTL",
                    "policy.executor.prompt_cache_ttl",
                    ttl,
                );
            } else {
                self.notes.push(
                    "CLAUDE_CODE_PROMPT_CACHE_TTL ignored: it applies only to claude-code"
                        .to_string(),
                );
            }
        }
        if let Some(value) = env("CODER_ONE_SYSTEM") {
            let system = if value == "default" {
                None
            } else if value.starts_with('{') {
                Some(
                    serde_json::from_str::<crate::system::Policy>(&value)
                        .map_err(|error| format!("CODER_ONE_SYSTEM is invalid: {error}"))?,
                )
            } else {
                Some(crate::system::Policy::preset(&value).ok_or_else(|| {
                    format!(
                        "CODER_ONE_SYSTEM must be default, core, core-select, or JSON, not {value}"
                    )
                })?)
            };
            self.manifest.policy.executor.system = system;
            self.set("CODER_ONE_SYSTEM", "policy.executor.system", value);
        }
        if let Some(seconds) = number("CODER_ONE_DELEGATE_TIMEOUT")? {
            self.manifest.policy.executor.deadline_sec = seconds;
            self.set(
                "CODER_ONE_DELEGATE_TIMEOUT",
                "policy.executor.deadline_sec",
                seconds.to_string(),
            );
        }
        if let Some(seconds) = number("CODER_ONE_EPISODE_DEADLINE")? {
            self.manifest.protected.ceilings.episode_deadline_sec = Some(seconds);
            self.set(
                "CODER_ONE_EPISODE_DEADLINE",
                "protected.ceilings.episode_deadline_sec",
                seconds.to_string(),
            );
        }
        if let Some(value) = env("CODER_ONE_SPEND_SOFT_USD") {
            let usd: f64 = value
                .parse()
                .map_err(|_| "CODER_ONE_SPEND_SOFT_USD must be a number of dollars".to_string())?;
            self.manifest.protected.ceilings.spend_soft_usd = Some(usd);
            self.set(
                "CODER_ONE_SPEND_SOFT_USD",
                "protected.ceilings.spend_soft_usd",
                value,
            );
        }
        Ok(())
    }
}

/// SHA-256 of a manifest value's canonical JSON, without `name`, `note`,
/// and `search`.
#[must_use]
pub fn digest_value(manifest: &Value) -> String {
    let mut value = manifest.clone();
    if let Some(object) = value.as_object_mut() {
        for key in ["name", "note", "search"] {
            object.remove(key);
        }
    }
    hex(&Sha256::digest(canonical(&value).as_bytes()))
}

/// JSON with object keys sorted and no whitespace.
#[must_use]
pub fn canonical(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let body: Vec<String> = keys
                .into_iter()
                .map(|key| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(key).unwrap_or_default(),
                        canonical(&map[key])
                    )
                })
                .collect();
            format!("{{{}}}", body.join(","))
        }
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(canonical).collect::<Vec<_>>().join(",")
        ),
        other => other.to_string(),
    }
}

/// The manifest files checked into this crate, by file name.
pub const REFERENCE: &[(&str, &str)] = &[
    (
        "jevprobe3-luna.json",
        include_str!("../policies/jevprobe3-luna.json"),
    ),
    (
        "jevprobe2-opus-lean-low-5m.json",
        include_str!("../policies/jevprobe2-opus-lean-low-5m.json"),
    ),
    ("pack-luna.json", include_str!("../policies/pack-luna.json")),
    (
        "handoff-escalate.json",
        include_str!("../policies/handoff-escalate.json"),
    ),
    (
        "handoff-planner-worker.json",
        include_str!("../policies/handoff-planner-worker.json"),
    ),
    (
        "handoff-steer.json",
        include_str!("../policies/handoff-steer.json"),
    ),
    (
        "handoff-race.json",
        include_str!("../policies/handoff-race.json"),
    ),
    ("tunable.json", include_str!("../policies/tunable.json")),
    (
        "tunable-opus.json",
        include_str!("../policies/tunable-opus.json"),
    ),
    (
        "tunable-luna.json",
        include_str!("../policies/tunable-luna.json"),
    ),
    (
        "tunable-v4.json",
        include_str!("../policies/tunable-v4.json"),
    ),
    (
        "tunable-v5.json",
        include_str!("../policies/tunable-v5.json"),
    ),
    (
        "tunable-luna-snapshot.json",
        include_str!("../policies/tunable-luna-snapshot.json"),
    ),
    (
        "tunable-v8.json",
        include_str!("../policies/tunable-v8.json"),
    ),
    (
        "matched-opus-medium-v8.json",
        include_str!("../policies/matched-opus-medium-v8.json"),
    ),
    (
        "tunable-v9.json",
        include_str!("../policies/tunable-v9.json"),
    ),
    (
        "tunable-v9-escalate.json",
        include_str!("../policies/tunable-v9-escalate.json"),
    ),
    (
        "tunable-v10.json",
        include_str!("../policies/tunable-v10.json"),
    ),
    (
        "tunable-luna-pack-solo.json",
        include_str!("../policies/tunable-luna-pack-solo.json"),
    ),
    (
        "microluna-v1.json",
        include_str!("../policies/microluna-v1.json"),
    ),
    (
        "microluna-v2.json",
        include_str!("../policies/microluna-v2.json"),
    ),
    (
        "microluna-v3.json",
        include_str!("../policies/microluna-v3.json"),
    ),
    (
        "luna-best-of-1.json",
        include_str!("../policies/luna-best-of-1.json"),
    ),
    (
        "luna-best-of-3.json",
        include_str!("../policies/luna-best-of-3.json"),
    ),
    (
        "luna-best-of-5.json",
        include_str!("../policies/luna-best-of-5.json"),
    ),
    (
        "microluna-best-of-1.json",
        include_str!("../policies/microluna-best-of-1.json"),
    ),
    (
        "microluna-best-of-3.json",
        include_str!("../policies/microluna-best-of-3.json"),
    ),
    (
        "microluna-best-of-3-suite.json",
        include_str!("../policies/microluna-best-of-3-suite.json"),
    ),
];

/// Where the reference manifests live in the checkout.
#[must_use]
pub fn reference_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/policies"))
}

/// The episode's closing directions under [`Directions::Plain`].
const EPISODE_DIRECTIONS: &str = "Complete the task in the current working \
directory. Nobody answers questions, so decide from the task and the \
environment. An automated checker grades the final state of the environment \
against the task, so verify every requirement, including exact paths, names, \
and formats, before you stop. The files and command outputs in this briefing \
were gathered just before you started and are current: use them instead of \
re-running those commands, and go straight to the work. End with a short \
summary of what you changed and how you checked it.";

/// Probe v2's directions: the same contract, plus batch mode. Each turn
/// costs the delegate seconds, so it should take few, large steps.
const EPISODE_DIRECTIONS_BATCH: &str = "Complete the task in the current working \
directory. Nobody answers questions, so decide from the task and the \
environment. An automated checker grades the final state of the environment \
against the task, so verify every requirement, including exact paths, names, \
and formats, before you stop. The files, command outputs, and setup results in \
this briefing were gathered just before you started and are complete and \
current: do not list, read, or run them again. Work in as few steps as \
possible: write each file whole in one command, chain related commands \
(installs, builds, tests) with && in one call, and run one final check that \
covers every requirement. End with a short summary of what you changed and how \
you checked it.";

/// Probe v3's directions: batch mode, without the single final check that
/// let v2's delegate stop before its checks reached every change.
const EPISODE_DIRECTIONS_BATCH_CHECKED: &str = "Complete the task in the current \
working directory. Nobody answers questions, so decide from the task and the \
environment. An automated checker grades the final state of the environment \
against the task, so verify every requirement, including exact paths, names, \
and formats, before you stop. The files, command outputs, and setup results in \
this briefing were gathered just before you started and are complete and \
current: do not list, read, or run them again. Work in few, large steps: write \
each file whole in one command, and chain related commands (installs, builds) \
with && in one call. Before you stop, run the checks the task names and \
exercise every code path you changed, not only the example the task gives. \
After a bulk find-and-replace, search the result for occurrences it missed or \
changed twice. End with a short summary of what you changed and how you \
checked it.";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::agent::{Generate, Shell};
    use crate::delegate::Plan;
    use crate::record::Recorder;
    use crate::state::{Environment, Issue, Observation, State};

    fn reference(name: &str) -> Manifest {
        let (_, text) = REFERENCE
            .iter()
            .find(|(file, _)| *file == name)
            .expect("a reference manifest");
        Manifest::parse(text).expect("parses")
    }

    fn luna() -> Manifest {
        reference("jevprobe3-luna.json")
    }

    fn opus() -> Manifest {
        reference("jevprobe2-opus-lean-low-5m.json")
    }

    #[test]
    fn a_policy_may_use_only_the_session_capabilities_its_adapter_demonstrated() {
        use crate::session::{Steer, Trigger};
        let steer = SessionPolicy {
            steer: Some(Steer {
                when: Trigger::CommandFailed,
                message: "Read the failing test first.".to_string(),
            }),
            stop_when: None,
            resume: None,
        };
        // Claude Code has demonstrated steering; the manifest is valid and
        // its controls reach the executor.
        let mut claude = opus();
        claude.policy.executor.session = Some(steer.clone());
        claude.validate().unwrap();
        let cli = claude.executor(ExecutorHost {
            binary: None,
            credential: crate::delegate::Credential::OauthToken,
            workdir: std::env::temp_dir(),
            artifacts: std::env::temp_dir(),
            artifacts_label: "artifacts".to_string(),
            env: Vec::new(),
        });
        assert_eq!(cli.control.controls.unwrap().steer, steer.steer);
        // Codex has not, so the same session policy is refused.
        let mut codex = luna();
        codex.policy.executor.session = Some(steer);
        let error = codex.validate().unwrap_err();
        assert!(
            error.contains(
                "executor.session uses steer, which the codex adapter has not demonstrated"
            ),
            "{error}"
        );
        // Stop and resume are demonstrated by both.
        codex.policy.executor.session = Some(SessionPolicy {
            steer: None,
            stop_when: Some(Trigger::After { ms: 600_000 }),
            resume: Some("Finish the task.".to_string()),
        });
        codex.validate().unwrap();
        // An absent session policy leaves the digest as it was.
        let mut plain = opus();
        plain.policy.executor.session = None;
        assert_eq!(plain.digest(), opus().digest());
    }

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |name: &str| map.get(name).cloned()
    }

    /// Every field path and value of a manifest's `policy` part, flattened.
    fn flat(manifest: &Manifest) -> BTreeMap<String, Value> {
        fn walk(prefix: &str, value: &Value, out: &mut BTreeMap<String, Value>) {
            match value {
                Value::Object(map) => {
                    for (key, child) in map {
                        walk(&format!("{prefix}.{key}"), child, out);
                    }
                }
                other => {
                    out.insert(prefix.to_string(), other.clone());
                }
            }
        }
        let value = serde_json::to_value(manifest).unwrap();
        let mut out = BTreeMap::new();
        walk("policy", &value["policy"], &mut out);
        out
    }

    #[test]
    fn the_builtin_default_is_what_an_unconfigured_episode_ran() {
        let resolved = Resolution::resolve(|_: &str| None, None).unwrap();
        assert_eq!(resolved.source, "builtin");
        assert!(resolved.overrides.is_empty());
        let policy = &resolved.manifest.policy;
        assert_eq!(policy.jev.mode, JevMode::Step);
        assert_eq!(policy.evidence.probes, Probes::Off);
        assert_eq!(policy.control.delegate, DelegateMode::Off);
        assert_eq!(policy.control.max_steps, 50);
        assert_eq!(policy.control.explore_steps, 8);
        assert_eq!(policy.control.command_timeout_sec, 300);
        assert_eq!(policy.brief.cap, 12_000);
        assert_eq!(policy.brief.directions, Directions::Plain);
        assert_eq!(policy.executor.deadline_sec, 600);
        assert_eq!(policy.executor.model, "claude-opus-5-5");
        assert_eq!(resolved.digest().len(), 64);
    }

    #[test]
    fn reference_manifests_are_complete_and_valid() {
        for (file, text) in REFERENCE {
            let manifest = Manifest::parse(text).unwrap();
            manifest.validate().unwrap();
            // Every field is spelled out, so the file's digest is its
            // resolution's digest.
            let raw: Value = serde_json::from_str(text).unwrap();
            assert_eq!(digest_value(&raw), manifest.digest(), "{file}");
            let on_disk = std::fs::read_to_string(reference_dir().join(file)).unwrap();
            assert_eq!(&on_disk, text);
        }
    }

    #[test]
    fn the_luna_arm_resolves_to_its_manifest_exactly() {
        // The switches the arm's profile and adapter set before manifests.
        let env = env_of(&[
            ("CODER_ONE_DEEP", "on"),
            ("CODER_ONE_PROBES", "on"),
            ("CODER_ONE_PROBE_V2", "v3"),
            ("CODER_ONE_DELEGATE", "always"),
            ("CODER_ONE_DELEGATE_AGENT", "codex"),
            ("CODER_ONE_DELEGATE_MODEL", "gpt-6-luna"),
            ("CODER_ONE_EXPLORE_STEPS", "0"),
            ("CODER_ONE_EXECUTOR_VERSION", "0.155.1"),
        ]);
        let resolved = Resolution::resolve(env, Some("free")).unwrap();
        assert_eq!(resolved.manifest.policy, luna().policy);
        assert_eq!(resolved.manifest.protected, luna().protected);
        assert_eq!(resolved.digest(), luna().digest());
    }

    #[test]
    fn the_opus_arm_resolves_to_its_manifest_exactly() {
        let env = env_of(&[
            ("CODER_ONE_DEEP", "on"),
            ("CODER_ONE_DELEGATE_TOOLS", "Bash,Read,Edit,Write,Glob,Grep"),
            ("CODER_ONE_PROBES", "on"),
            ("CODER_ONE_DELEGATE_EFFORT", "low"),
            ("CODER_ONE_PROBE_V2", "on"),
            ("CLAUDE_CODE_PROMPT_CACHE_TTL", "5m"),
            ("CODER_ONE_DELEGATE", "always"),
            ("CODER_ONE_DELEGATE_AGENT", "claude-code"),
            ("CODER_ONE_DELEGATE_MODEL", "claude-opus-5-5"),
            ("CODER_ONE_EXPLORE_STEPS", "0"),
            ("CODER_ONE_EXECUTOR_VERSION", "2.1.280"),
        ]);
        let resolved = Resolution::resolve(env, Some("free")).unwrap();
        assert_eq!(resolved.manifest.policy, opus().policy);
        assert_eq!(resolved.digest(), opus().digest());
    }

    #[test]
    fn the_two_reference_manifests_differ_only_in_executor_and_directions() {
        let (a, b) = (flat(&luna()), flat(&opus()));
        let differing: Vec<&str> = a
            .keys()
            .filter(|key| a[*key] != b[*key])
            .map(String::as_str)
            .collect();
        assert_eq!(
            differing,
            [
                "policy.brief.directions",
                "policy.executor.agent",
                "policy.executor.effort",
                "policy.executor.model",
                "policy.executor.prompt_cache_ttl",
                "policy.executor.tools",
                "policy.executor.version",
            ]
        );
        assert_ne!(luna().digest(), opus().digest());
        // The Gym computes the same digest from the file
        // (`gym::coder_policy`); both crates pin it.
        assert_eq!(
            luna().digest(),
            "bdefda51a03c05ed4322f668c6ea8e1695b3102f50261afb649ce87dd5411f85"
        );
    }

    #[test]
    fn a_manifest_names_its_source_and_env_switches_override_it_on_record() {
        let text = REFERENCE[0].1;
        let resolved = Resolution::resolve(env_of(&[(POLICY_VAR, text)]), None).unwrap();
        assert_eq!(resolved.source, "inline");
        assert!(resolved.overrides.is_empty());
        assert_eq!(resolved.digest(), luna().digest());

        let path = reference_dir().join("jevprobe3-luna.json");
        let path = path.to_string_lossy();
        let resolved = Resolution::resolve(
            env_of(&[(POLICY_VAR, &path), ("CODER_ONE_DELEGATE_EFFORT", "high")]),
            Some("free"),
        )
        .unwrap();
        assert!(resolved.source.starts_with("file:"));
        assert_eq!(
            resolved.manifest.policy.executor.effort.as_deref(),
            Some("high")
        );
        assert_eq!(
            resolved.overrides,
            [Override {
                source: "CODER_ONE_DELEGATE_EFFORT".to_string(),
                field: "policy.executor.effort".to_string(),
                value: "high".to_string(),
            }]
        );
        assert_ne!(resolved.digest(), luna().digest());
        let record = resolved.record();
        assert_eq!(record["digest"], resolved.digest());
        assert_eq!(record["manifest"]["policy"]["executor"]["effort"], "high");
        assert_eq!(
            record["overrides"][0]["source"],
            "CODER_ONE_DELEGATE_EFFORT"
        );
        assert_eq!(
            record["text_digests"]["directions"].as_str().unwrap().len(),
            64
        );
    }

    #[test]
    fn older_switches_keep_their_meaning() {
        // Probes without deep mode never ran; batch directions did.
        let resolved = Resolution::resolve(
            env_of(&[("CODER_ONE_PROBES", "on"), ("CODER_ONE_PROBE_V2", "on")]),
            None,
        )
        .unwrap();
        assert_eq!(resolved.manifest.policy.evidence.probes, Probes::Off);
        assert_eq!(resolved.manifest.policy.evidence.survey_files, 100);
        assert_eq!(resolved.manifest.policy.brief.directions, Directions::Batch);
        assert!(resolved.notes.iter().any(|note| note.contains("deep")));
        // Jev off wins over deep.
        let resolved = Resolution::resolve(
            env_of(&[("CODER_ONE_JEV", "off"), ("CODER_ONE_DEEP", "on")]),
            None,
        )
        .unwrap();
        assert_eq!(resolved.manifest.policy.jev.mode, JevMode::Off);
        // An effort that is not one lowercase word was never passed.
        let resolved =
            Resolution::resolve(env_of(&[("CODER_ONE_DELEGATE_EFFORT", "High!")]), None).unwrap();
        assert_eq!(resolved.manifest.policy.executor.effort, None);
        // A Codex delegate takes its own default model.
        let resolved =
            Resolution::resolve(env_of(&[("CODER_ONE_DELEGATE_AGENT", "codex")]), None).unwrap();
        assert_eq!(resolved.manifest.policy.executor.model, "gpt-6-luna");
        assert!(Resolution::resolve(env_of(&[("CODER_ONE_MAX_STEPS", "many")]), None).is_err());
    }

    #[test]
    fn unsupported_combinations_and_protected_searches_are_refused() {
        let refused = |edit: &dyn Fn(&mut Manifest), needle: &str| {
            let mut manifest = luna();
            edit(&mut manifest);
            let error = manifest.validate().expect_err(needle);
            assert!(error.contains(needle), "{error}");
        };
        refused(
            &|m| m.policy.jev.mode = JevMode::Step,
            "needs jev.mode = deep",
        );
        refused(
            &|m| m.policy.executor.tools = Some("Bash".into()),
            "only to claude-code",
        );
        refused(
            &|m| m.policy.executor.prompt_cache_ttl = Some("5m".into()),
            "only to claude-code",
        );
        refused(&|m| m.policy.jev.model = "jev-2".into(), "pinned");
        refused(
            &|m| m.protected.isolation = "none".into(),
            "protected.isolation",
        );
        refused(
            &|m| m.protected.ceilings.spend_hard_usd = Some(1.0),
            "cannot be enforced",
        );
        refused(
            &|m| m.search = vec!["protected.ceilings.reserve_sec".into()],
            "is protected",
        );
        refused(
            &|m| m.search = vec!["policy.control.max_steps".into()],
            "no passing canary",
        );
        refused(&|m| m.policy.evidence.survey_files = 0, "survey_files");
        let mut manifest = luna();
        manifest.search = SEARCHABLE.iter().map(|f| (*f).to_string()).collect();
        manifest.validate().unwrap();
        // The digest ignores labels and the search space.
        assert_eq!(manifest.digest(), luna().digest());

        let mut raw: Value = serde_json::from_str(REFERENCE[0].1).unwrap();
        raw["policy"]["executor"]["temperature"] = json!(0.2);
        assert!(Manifest::parse(&raw.to_string()).is_err());
        let error = Resolution::resolve(env_of(&[(POLICY_VAR, "{\"schema\":1}")]), None)
            .expect_err("not a manifest");
        assert!(error.contains("invalid"), "{error}");
    }

    // Canaries: one per searchable field. Each shows that changing the
    // field changes what the executor is invoked with, what it reads on
    // standard input, or what the host does before it, with a scripted
    // executor binary and no model call.

    /// The searchable fields the canaries below cover.
    const CANARIES: &[&str] = &[
        "policy.jev.mode",
        "policy.evidence.probes",
        "policy.evidence.survey_files",
        "policy.control.delegate",
        "policy.control.explore_steps",
        "policy.brief.cap",
        "policy.brief.directions",
        "policy.brief.packer",
        "policy.brief.pack.slice",
        "policy.brief.pack.item_max",
        "policy.brief.pack.instruction_share",
        "policy.executor.agent",
        "policy.executor.model",
        "policy.executor.effort",
        "policy.executor.tools",
        "policy.executor.prompt_cache_ttl",
        "policy.executor.deadline_sec",
        "policy.executor.system",
    ];

    #[test]
    fn every_searchable_field_has_a_canary() {
        assert_eq!(CANARIES, SEARCHABLE);
    }

    /// A scripted executor: it records its arguments, environment, and
    /// standard input, then prints a stream both parsers read as an answer.
    const FAKE_AGENT: &str = r#"#!/bin/sh
printf '%s\n' "$@" > "$CANARY_DIR/args"
env > "$CANARY_DIR/env"
cat > "$CANARY_DIR/stdin"
for a in "$@"; do
  case "$a" in
    *.system.md) cp "$a" "$CANARY_DIR/system" ;;
    model_instructions_file=*) f=${a#model_instructions_file=\"}; cp "${f%\"}" "$CANARY_DIR/system" ;;
  esac
done
[ -n "$CANARY_SLEEP" ] && sleep "$CANARY_SLEEP"
echo '{"type":"thread.started","thread_id":"t-canary"}'
echo '{"type":"item.completed","item":{"id":"i1","type":"agent_message","text":"done"}}'
echo '{"type":"turn.completed","usage":{"input_tokens":10,"cached_input_tokens":0,"output_tokens":1}}'
echo '{"type":"result","subtype":"success","is_error":false,"num_turns":1,"result":"done","total_cost_usd":0.0,"usage":{"input_tokens":10,"cache_read_input_tokens":0,"cache_creation_input_tokens":0,"output_tokens":1}}'
"#;

    struct Seen {
        invoked: bool,
        args: Vec<String>,
        env: BTreeMap<String, String>,
        stdin: String,
        /// The system prompt file the executor was pointed at, if any.
        system: String,
        generations: usize,
        status: Option<String>,
        switches: (bool, bool, bool, bool, usize),
    }

    /// Counts generations; with its flag set, each one runs a distinct
    /// long command until the explore bound stops it.
    struct Counting(usize, bool);

    impl Generate for Counting {
        async fn generate(&mut self, _prompt: &str) -> Result<String, String> {
            self.0 += 1;
            if self.1 {
                let command = format!("cat report-{:02}.txt # {}", self.0, "x".repeat(170));
                return Ok(json!({ "action": "shell", "command": command }).to_string());
            }
            Ok(
                r#"{"action":"finished","title":"Explored","summary":"Nothing to run."}"#
                    .to_string(),
            )
        }
    }

    /// A shell whose every command prints the same output.
    struct Echo(String);

    impl Shell for Echo {
        async fn run(&mut self, _command: &str) -> Observation {
            Observation {
                exit: Some(0),
                output: self.0.clone(),
                truncated: false,
            }
        }
    }

    pub(crate) fn scratch() -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "coder-one-canary-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(dir.join("work")).unwrap();
        std::fs::create_dir_all(dir.join("artifacts")).unwrap();
        dir
    }

    /// Runs one delegated episode under `manifest` with the scripted
    /// executor, and reports what the executor saw.
    async fn observe(manifest: &Manifest, extra_env: &[(&str, &str)]) -> Seen {
        observe_with(manifest, extra_env, None).await
    }

    /// Runs one delegated episode as [`observe`] does; with `output`, the
    /// explorer first runs one command whose output is `output`, so the
    /// briefing carries evidence for the packer to slice.
    async fn observe_with(
        manifest: &Manifest,
        extra_env: &[(&str, &str)],
        output: Option<&str>,
    ) -> Seen {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch();
        let fake = dir.join("fake-agent");
        std::fs::write(&fake, FAKE_AGENT).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let workdir = dir.join("work");
        let issue = Issue {
            url: String::new(),
            title: "Canary task".to_string(),
            body: format!("Write the canary file.\n\n{}", "context ".repeat(600)),
            labels: vec![],
        };
        let mut state = State::new(
            Environment {
                repository: String::new(),
                workdir: workdir.to_string_lossy().into_owned(),
                os: "linux".to_string(),
            },
            issue.clone(),
        );
        let recorder = Recorder::default();
        let client = jev::Client::new(
            jev::Config::new()
                .api_key("canary")
                .base_url("http://127.0.0.1:9")
                .default_model(JEV_MODEL),
        )
        .unwrap();
        let switches = manifest
            .judge(Some(client), workdir.clone(), &issue, recorder.clone())
            .switches();
        // Jev stays off for the run itself: the canary makes no request.
        let mut judge = manifest.judge(None, workdir.clone(), &issue, recorder.clone());
        let mut env = vec![("CANARY_DIR".to_string(), dir.to_string_lossy().into_owned())];
        env.extend(
            extra_env
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string())),
        );
        let mut executor = manifest.executor(ExecutorHost {
            binary: Some(fake),
            credential: Credential::OauthToken,
            workdir: workdir.clone(),
            artifacts: dir.join("artifacts"),
            artifacts_label: "artifacts".to_string(),
            env,
        });
        let plan = Plan {
            mode: manifest.mode(),
            policy: manifest.escalation(),
            max_steps: manifest.policy.control.max_steps,
            prompt: "Complete this task.",
            instruction: &issue.body,
            directions: manifest.policy.brief.directions.text(),
            cap: manifest.policy.brief.cap,
            packer: manifest.policy.brief.packer,
            pack: manifest.policy.brief.pack_params(),
            isolation: "none",
            base: None,
        };
        let mut generator = Counting(0, output.is_some());
        let mut shell = Echo(output.unwrap_or("").to_string());
        let (_, delegated) = crate::delegate::explore_then_delegate(
            &mut state,
            &plan,
            &mut judge,
            &mut generator,
            &mut shell,
            &mut executor,
            &recorder,
            &mut |_| {},
        )
        .await;
        let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap_or_default();
        let seen = Seen {
            invoked: dir.join("args").exists(),
            args: read("args").lines().map(str::to_string).collect(),
            env: read("env")
                .lines()
                .filter_map(|line| line.split_once('='))
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            stdin: read("stdin"),
            system: read("system"),
            generations: generator.0,
            status: delegated.map(|d| d.report.status.word().to_string()),
            switches,
        };
        let _ = std::fs::remove_dir_all(&dir);
        seen
    }

    fn with(edit: impl Fn(&mut Manifest)) -> Manifest {
        let mut manifest = opus();
        edit(&mut manifest);
        manifest.validate().unwrap();
        manifest
    }

    fn follows(args: &[String], flag: &str) -> Option<String> {
        args.iter()
            .position(|arg| arg == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    }

    #[tokio::test]
    async fn canary_policy_executor_agent() {
        let claude = observe(&opus(), &[]).await;
        assert_eq!(claude.args.first().map(String::as_str), Some("-p"));
        let codex = observe(&luna(), &[]).await;
        assert_eq!(codex.args.first().map(String::as_str), Some("exec"));
        assert!(codex.args.contains(&"--json".to_string()));
        assert_eq!(codex.status.as_deref(), Some("answered"));
    }

    #[tokio::test]
    async fn canary_policy_executor_model() {
        let seen = observe(
            &with(|m| m.policy.executor.model = "claude-canary-1".into()),
            &[],
        )
        .await;
        assert_eq!(
            follows(&seen.args, "--model").as_deref(),
            Some("claude-canary-1")
        );
        let seen = observe(&luna(), &[]).await;
        assert_eq!(follows(&seen.args, "-m").as_deref(), Some("gpt-6-luna"));
    }

    #[tokio::test]
    async fn canary_policy_executor_effort() {
        let seen = observe(
            &with(|m| m.policy.executor.effort = Some("high".into())),
            &[],
        )
        .await;
        assert_eq!(follows(&seen.args, "--effort").as_deref(), Some("high"));
        let seen = observe(&with(|m| m.policy.executor.effort = None), &[]).await;
        assert!(!seen.args.contains(&"--effort".to_string()));
        let mut codex = luna();
        codex.policy.executor.effort = Some("low".into());
        let seen = observe(&codex, &[]).await;
        assert_eq!(
            follows(&seen.args, "-c").as_deref(),
            Some("model_reasoning_effort=low")
        );
    }

    #[tokio::test]
    async fn canary_policy_executor_tools() {
        let seen = observe(
            &with(|m| m.policy.executor.tools = Some("Bash,Read".into())),
            &[],
        )
        .await;
        assert_eq!(follows(&seen.args, "--tools").as_deref(), Some("Bash,Read"));
        let seen = observe(&with(|m| m.policy.executor.tools = None), &[]).await;
        assert!(!seen.args.contains(&"--tools".to_string()));
    }

    #[tokio::test]
    async fn canary_policy_executor_prompt_cache_ttl() {
        let seen = observe(
            &with(|m| m.policy.executor.prompt_cache_ttl = Some("1h".into())),
            &[],
        )
        .await;
        assert_eq!(
            seen.env
                .get("CLAUDE_CODE_PROMPT_CACHE_TTL")
                .map(String::as_str),
            Some("1h")
        );
        let seen = observe(&with(|m| m.policy.executor.prompt_cache_ttl = None), &[]).await;
        assert!(!seen.env.contains_key("CLAUDE_CODE_PROMPT_CACHE_TTL"));
    }

    #[tokio::test]
    async fn canary_policy_executor_deadline_sec() {
        let seen = observe(
            &with(|m| m.policy.executor.deadline_sec = 1),
            &[("CANARY_SLEEP", "3")],
        )
        .await;
        assert_eq!(seen.status.as_deref(), Some("timed_out"));
        let seen = observe(&opus(), &[]).await;
        assert_eq!(seen.status.as_deref(), Some("answered"));
    }

    #[tokio::test]
    async fn canary_policy_executor_system() {
        use crate::system::{Mode as SystemMode, Policy as SystemPolicy};
        let security = crate::system::section(Agent::ClaudeCode, "security")
            .unwrap()
            .text
            .trim();
        let seen = observe(
            &with(|m| m.policy.executor.system = SystemPolicy::preset("core")),
            &[],
        )
        .await;
        assert!(follows(&seen.args, "--system-prompt-file").is_some());
        assert!(seen.system.contains(security));
        assert!(seen.system.contains("working headless on one task"));
        let seen = observe(&opus(), &[]).await;
        assert!(!seen.args.iter().any(|arg| arg.contains("system-prompt")));
        assert!(seen.system.is_empty());
        let seen = observe(
            &with(|m| {
                m.policy.executor.system = Some(SystemPolicy {
                    mode: SystemMode::Append,
                    sections: vec!["authority".into()],
                    select: Vec::new(),
                });
            }),
            &[],
        )
        .await;
        assert!(follows(&seen.args, "--append-system-prompt-file").is_some());
        assert!(seen.system.contains("You are authorized"));

        let mut codex = luna();
        codex.policy.executor.system = SystemPolicy::preset("core-select");
        codex.validate().unwrap();
        let seen = observe(&codex, &[]).await;
        assert!(
            seen.args
                .iter()
                .any(|arg| arg.starts_with("model_instructions_file=\"")),
            "{:?}",
            seen.args
        );
        assert!(seen.system.contains(security));
        codex.policy.executor.system = Some(SystemPolicy {
            mode: SystemMode::Append,
            sections: vec!["security".into(), "verify".into()],
            select: Vec::new(),
        });
        codex.validate().unwrap();
        let seen = observe(&codex, &[]).await;
        let developer = seen
            .args
            .iter()
            .find_map(|arg| arg.strip_prefix("developer_instructions="))
            .expect("developer_instructions reaches codex");
        let text: String = serde_json::from_str(developer).unwrap();
        assert!(text.contains(security));
        assert!(text.contains("An automated checker grades"));
    }

    #[tokio::test]
    async fn canary_policy_brief_cap() {
        let seen = observe(&with(|m| m.policy.brief.cap = 2_000), &[]).await;
        assert!(seen.stdin.chars().count() <= 2_000, "{}", seen.stdin.len());
        let seen = observe(&opus(), &[]).await;
        assert!(seen.stdin.chars().count() > 2_000);
    }

    #[tokio::test]
    async fn canary_policy_brief_directions() {
        for directions in [
            Directions::Plain,
            Directions::Batch,
            Directions::BatchChecked,
        ] {
            let seen = observe(&with(|m| m.policy.brief.directions = directions), &[]).await;
            assert!(seen.stdin.contains(directions.text()), "{directions:?}");
        }
    }

    #[tokio::test]
    async fn canary_policy_brief_packer() {
        let first = observe(&opus(), &[]).await;
        assert!(first.stdin.contains("complete and current"));
        assert!(
            !first
                .stdin
                .contains("each says whether it is complete or trimmed")
        );
        let coverage = observe(&with(|m| m.policy.brief.packer = Packer::Coverage), &[]).await;
        assert!(!coverage.stdin.contains("complete and current"));
        assert!(
            coverage
                .stdin
                .contains("each says whether it is complete or trimmed")
        );
        assert!(
            coverage
                .stdin
                .contains("## Requirements from the task's own words")
        );
    }

    /// A manifest with the coverage packer, one explore step, and `pack`.
    fn packed(cap: usize, pack: PackPolicy) -> Manifest {
        with(|m| {
            m.policy.brief.packer = Packer::Coverage;
            m.policy.brief.cap = cap;
            m.policy.brief.pack = Some(pack);
            m.policy.control.explore_steps = 12;
            m.policy.control.unchanged_steps = 100;
            m.policy.control.error_streak = 100;
        })
    }

    /// Numbered report lines: 1,500 characters, the most of a command's
    /// output the explorer keeps.
    fn report() -> String {
        (1..=100)
            .map(|n| format!("line {n:03} of the report."))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The characters of the report the briefing delivered.
    fn report_shown(stdin: &str) -> usize {
        stdin
            .lines()
            .filter(|line| line.starts_with("line ") && line.ends_with("of the report."))
            .count()
    }

    #[tokio::test]
    async fn canary_policy_brief_pack_item_max() {
        let output = report();
        let whole = observe_with(&packed(12_000, PackPolicy::default()), &[], Some(&output)).await;
        let small = PackPolicy {
            slice: 200,
            item_max: 300,
            ..PackPolicy::default()
        };
        let capped = observe_with(&packed(12_000, small), &[], Some(&output)).await;
        assert!(report_shown(&whole.stdin) > 50, "{}", whole.stdin);
        assert!(report_shown(&capped.stdin) < 20, "{}", capped.stdin);
    }

    #[tokio::test]
    async fn canary_policy_brief_pack_slice() {
        // Twelve long commands and the report compete for a tight cap.
        // The fill grants a slice at a time in rank order, so a large
        // slice lets the command list take the room first, and a small one
        // splits it.
        let output = report();
        let tight = |slice: usize| PackPolicy {
            slice,
            item_max: 8_000,
            instruction_share: 0.2,
        };
        let small = observe_with(&packed(4_000, tight(200)), &[], Some(&output)).await;
        let large = observe_with(&packed(4_000, tight(1_400)), &[], Some(&output)).await;
        assert_ne!(
            report_shown(&small.stdin),
            report_shown(&large.stdin),
            "{}\n---\n{}",
            small.stdin,
            large.stdin
        );
    }

    #[tokio::test]
    async fn canary_policy_brief_pack_instruction_share() {
        let share = |instruction_share: f64| PackPolicy {
            instruction_share,
            ..PackPolicy::default()
        };
        let narrow = observe_with(&packed(3_000, share(0.2)), &[], None).await;
        let wide = observe_with(&packed(3_000, share(0.8)), &[], None).await;
        let context = |stdin: &str| stdin.matches("context").count();
        assert!(
            context(&wide.stdin) > context(&narrow.stdin) + 50,
            "{} vs {}",
            context(&wide.stdin),
            context(&narrow.stdin)
        );
    }

    #[test]
    fn brief_pack_is_refused_outside_its_bounds_and_leaves_digests_alone() {
        let base = opus();
        assert!(base.policy.brief.pack.is_none());
        let mut sections = base.clone();
        sections.policy.brief.pack = Some(PackPolicy::default());
        let error = sections.validate().unwrap_err();
        assert!(error.contains("only to the coverage packers"), "{error}");
        let mut wide = base.clone();
        wide.policy.brief.packer = Packer::Coverage;
        wide.policy.brief.pack = Some(PackPolicy {
            slice: 100,
            item_max: 50,
            instruction_share: 0.95,
        });
        let error = wide.validate().unwrap_err();
        assert!(error.contains("slice must be 200"), "{error}");
        assert!(error.contains("item_max must be at least"), "{error}");
        assert!(
            error.contains("instruction_share must be 0.1 to 0.9"),
            "{error}"
        );
        // A manifest without `pack` serializes without it.
        let value = serde_json::to_value(&base).unwrap();
        assert!(value["policy"]["brief"].get("pack").is_none());
        assert_eq!(
            base.policy.brief.pack_params(),
            crate::pack::Params {
                cap: base.policy.brief.cap,
                ..crate::pack::Params::default()
            }
        );
    }

    #[tokio::test]
    async fn canary_policy_control_delegate() {
        let seen = observe(
            &with(|m| m.policy.control.delegate = DelegateMode::Off),
            &[],
        )
        .await;
        assert!(!seen.invoked);
        let seen = observe(&opus(), &[]).await;
        assert!(seen.invoked);
    }

    #[tokio::test]
    async fn canary_policy_control_explore_steps() {
        let seen = observe(&opus(), &[]).await;
        assert_eq!(seen.generations, 0);
        let seen = observe(&with(|m| m.policy.control.explore_steps = 2), &[]).await;
        assert_eq!(seen.generations, 1);
        // The explorer's conclusion reaches the executor's briefing.
        assert!(seen.stdin.contains("Nothing to run."));
    }

    #[tokio::test]
    async fn canary_policy_jev_mode() {
        let seen = observe(
            &with(|m| {
                m.policy.jev.mode = JevMode::Step;
                m.policy.evidence.probes = Probes::Off;
            }),
            &[],
        )
        .await;
        assert_eq!(seen.switches, (true, false, false, false, 40));
        let seen = observe(
            &with(|m| {
                m.policy.jev.mode = JevMode::Off;
                m.policy.evidence.probes = Probes::Off;
            }),
            &[],
        )
        .await;
        assert!(!seen.switches.0, "Jev off drops the client");
        let seen = observe(&opus(), &[]).await;
        assert_eq!(seen.switches, (true, true, true, true, 40));
    }

    #[tokio::test]
    async fn canary_policy_evidence_probes() {
        let seen = observe(&with(|m| m.policy.evidence.probes = Probes::Battery), &[]).await;
        assert_eq!((seen.switches.2, seen.switches.3), (true, false));
        let seen = observe(&with(|m| m.policy.evidence.probes = Probes::Off), &[]).await;
        assert_eq!((seen.switches.2, seen.switches.3), (false, false));
    }

    #[test]
    fn canary_policy_evidence_survey_files() {
        let dir = scratch();
        let workdir = dir.join("work");
        for i in 0..60 {
            std::fs::write(workdir.join(format!("file_{i:02}.txt")), "x").unwrap();
        }
        let issue = Issue {
            url: String::new(),
            title: "Survey".to_string(),
            body: "Look at the files.".to_string(),
            labels: vec![],
        };
        let pool = |files: usize| {
            with(|m| m.policy.evidence.survey_files = files)
                .judge(None, workdir.clone(), &issue, Recorder::default())
                .survey_pool(&issue)
                .len()
        };
        assert_eq!(pool(40), 40);
        assert_eq!(pool(10), 10);
        assert_eq!(pool(100), 60);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
