//! The policy sections a CLI turn reads: how Jev is used, which evidence
//! the host gathers, how the briefing is built, and which executor runs
//! it, with the builders that turn them into a judge and an executor.
//!
//! Coder One's policy manifest (`coder_one::policy::Manifest`) holds these
//! sections beside the ones only its episodes read (control, verify, and
//! Microluna's loop), and re-exports them. [`TurnPolicy`] reads the same
//! sections from a manifest file and ignores the rest, which is how Coder's
//! terminal turn reads its reference manifest without Coder One.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::delegate::{Agent, Cli, Credential};

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
    /// The digest of the decision settings this build reads Jev's answers
    /// under, where any differs from its default ([`crate::decision`]).
    /// Absent while every setting is at its default, so a manifest written
    /// before settings existed keeps its digest. Resolution fills it in,
    /// and a manifest that names another digest is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,
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
    /// `evidence.guests`: the snapshot-read Wasm guests in
    /// `programs/evidence-guests.json`, run by the probe stage, their
    /// outputs offered to the probe keep question beside the probes'.
    /// Needs `probes`. Absent, no guest runs, and the manifest's digest is
    /// what it was before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guests: Option<crate::guests::Policy>,
    /// `evidence.environment`: presence probes for a fixed program set and
    /// the programs the task's files imply, delivered as one briefing line
    /// under this template. Needs `probes`. Absent, as in every manifest
    /// before it, nothing is probed and the digest is unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<crate::environment::Params>,
    /// `evidence.data_profile`: code profiles every data file the task
    /// ships and puts each profile in the survey as an evidence item the
    /// coverage packer ranks with the rest ([`crate::data_profile`], issue
    /// #9654). Needs `probes`. Absent, as in every manifest before it,
    /// nothing is profiled and the digest is unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_profile: Option<crate::data_profile::Params>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Probes {
    Off,
    Battery,
    V2,
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
    /// Whether this is the first packer, which a manifest leaves out.
    #[must_use]
    pub fn is_sections(&self) -> bool {
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
///
/// `M` is the type of the Microluna section, which Coder One's manifest
/// reads (`coder_one::micro::Policy`); a turn that runs a CLI reads it as
/// `()`, and a manifest with the section is then refused.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutorPolicy<M = ()> {
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
    pub microluna: Option<M>,
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

    /// The manifest name of `agent`.
    #[must_use]
    pub fn from_agent(agent: Agent) -> Self {
        match agent {
            Agent::ClaudeCode => AgentName::ClaudeCode,
            Agent::Codex => AgentName::Codex,
            Agent::Microluna => AgentName::Microluna,
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

/// The sections of a policy manifest a turn reads: Jev, evidence, the
/// briefing, and the executor. Other sections of the manifest, and its
/// `protected` part, are ignored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurnPolicy {
    pub jev: JevPolicy,
    pub evidence: EvidencePolicy,
    pub brief: BriefPolicy,
    pub executor: ExecutorPolicy,
}

/// A manifest's `policy` object, as [`TurnPolicy`] reads it.
#[derive(Deserialize)]
struct TurnManifest {
    policy: TurnPolicy,
}

impl TurnPolicy {
    /// The turn sections of the manifest `text`.
    ///
    /// # Errors
    ///
    /// Returns why the manifest's turn sections don't parse.
    pub fn parse(text: &str) -> Result<Self, String> {
        serde_json::from_str::<TurnManifest>(text)
            .map(|manifest| manifest.policy)
            .map_err(|error| format!("the manifest doesn't parse: {error}"))
    }

    /// Whether Jev runs at all.
    #[must_use]
    pub fn jev(&self) -> bool {
        self.jev.mode != JevMode::Off
    }

    /// Whether Jev runs in deep mode.
    #[must_use]
    pub fn deep(&self) -> bool {
        self.jev.mode == JevMode::Deep
    }

    /// The judge these sections configure.
    #[must_use]
    pub fn judge(
        &self,
        client: Option<jev::Client>,
        workdir: PathBuf,
        issue: &crate::state::Issue,
        recorder: crate::record::Recorder,
    ) -> crate::judge::JevJudge {
        judge(&self.jev, &self.evidence, client, workdir, issue, recorder)
    }

    /// The executor these sections configure, given what the host found.
    #[must_use]
    pub fn executor(&self, host: ExecutorHost) -> Cli {
        executor(&self.executor, host)
    }
}

/// The judge `jev` and `evidence` configure.
#[must_use]
pub fn judge(
    jev: &JevPolicy,
    evidence: &EvidencePolicy,
    client: Option<jev::Client>,
    workdir: PathBuf,
    issue: &crate::state::Issue,
    recorder: crate::record::Recorder,
) -> crate::judge::JevJudge {
    let on = jev.mode != JevMode::Off;
    let deep = jev.mode == JevMode::Deep;
    let probes = evidence.probes;
    crate::judge::JevJudge::new(client.filter(|_| on), workdir, issue, recorder)
        .deep(deep)
        .probing(deep && probes != Probes::Off)
        .probe_v2(deep && probes == Probes::V2)
        .environment(
            evidence
                .environment
                .clone()
                .filter(|_| deep && probes != Probes::Off),
        )
        .data_profile(
            evidence
                .data_profile
                .clone()
                .filter(|_| deep && probes != Probes::Off),
        )
        .survey_files(evidence.survey_files)
        .guests(evidence.guests.clone())
}

/// The executor `executor` configures, given what the host found.
#[must_use]
pub fn executor<M>(executor: &ExecutorPolicy<M>, host: ExecutorHost) -> Cli {
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
    Sha256::digest(canonical(&value).as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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

/// A manifest's `name`, from its text.
#[must_use]
pub fn manifest_name(text: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct Named {
        name: Option<String>,
    }
    serde_json::from_str::<Named>(text)
        .ok()
        .and_then(|named| named.name)
}
