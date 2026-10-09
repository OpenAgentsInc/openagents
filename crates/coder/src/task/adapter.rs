//! An admitted adapter uses the task owner's lease, evidence, and child boundary.
//!
//! The adapter supplies its algorithm. This host retains authority and effects;
//! it does not load a model or turn a model's completion into independent checks.

use super::*;
use std::cell::{Cell, RefCell};

use atif::{Log, Session, Source, Step};
use coder_boundary::{Boundary, Snapshot};
use serde_json::{Value, json};
use supervise::{Input, Job, Limits};

pub mod container;
mod installs;
pub mod login;

pub const NAME: &str = "microcoder-repository";

/// How Microcoder takes a steer. A run reads its instructions once, when it
/// is admitted, so a correction reaches the engine only when the next turn
/// starts; the admission records the task revision it read, and the turn's
/// trace records the consumed correction as its own step. A correction
/// accepted while a run is going supersedes that run's context and stops
/// it: that is task-level CTRL cancellation, not steering. The emulated
/// operation, which a device must choose, stops the run and continues the
/// task with the message as its next turn.
pub const STEERING: coder_delegate::steering::Steering = coder_delegate::steering::Steering {
    adapter: NAME,
    native: coder_delegate::steering::Native::TurnBoundary,
    emulation: Some(coder_delegate::steering::Emulation::CancelAndContinue),
    acknowledgment: coder_delegate::steering::Acknowledgment::NextTurnStart,
    limitations: &[
        "A run reads its instructions once, at admission.",
        "Emulation stops the running turn and starts the next turn with the message.",
    ],
};
pub const CONFIG_SCHEMA: &str = "openagents.microcoder.repository-config.v1";

/// The file in the task store where task owners keep the workspace file
/// digests their snapshots took, so the next owner's first snapshot reads
/// only the files that changed ([`Snapshot::recall_digests`]).
pub const SNAPSHOT_DIGESTS: &str = "snapshot-digests.bin";
/// The ending of a run whose disk stayed full while it kept its evidence
/// (#10237): what the person reads is "disk full", not that Coder's
/// process ended.
pub const DISK_FULL: &str = "disk_full";
/// The ending of a run whose command ended (on its deadline, most often)
/// with its processes not confirmed gone (#10281): the turn ends, since
/// Coder doesn't run beside processes it can't account for, but as that
/// fault, never as a stop the person asked for.
pub const PROCESS_CLEANUP_UNKNOWN: &str = "process_cleanup_unknown";
/// The ending of a run the host ended for a fault of its own, such as
/// evidence it could not keep (#10993): the turn fails and says why, and
/// never reads as a stop the person asked for.
pub const HOST_FAULT: &str = "host_fault";
/// The transcript's soft bound. Past it, bulky records are left out and
/// the run goes on (#10993): a size bound on evidence is never a limit on
/// how long a run works.
const TRACE_LIMIT: usize = 48 * 1024 * 1024;
/// Past [`TRACE_LIMIT`], a record this small is still kept, so effect
/// intents, results, and notes keep their order.
const SMALL_RECORD: usize = 64 * 1024;
/// The transcript's hard bound, below the 64 MiB file with headroom for
/// the closing record: past it, every record is left out.
const TRACE_CEILING: usize = TRACE_LIMIT + 8 * 1024 * 1024;
/// How long a command's process group, not yet empty when the supervisor
/// ended it, is given to empty before its cleanup counts as unknown
/// (#10281).
const GROUP_SETTLE: Duration = Duration::from_secs(10);
const STEP_LIMIT: usize = 8 * 1024 * 1024;

/// The supported first repository profile is explicit about absent features.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub schema: String,
    /// `codex` (the operator's Codex login), `claude` (the operator's Claude
    /// Code login through the `claude` binary), or `synthetic` (in-process
    /// fixtures). `model` is the exact identity the provider must report.
    pub provider: String,
    pub model: String,
    pub effort: Option<String>,
    pub generation_endpoint: String,
    pub decision_endpoint: String,
    pub decision_model: String,
    /// A step limit older grants carried (1 to 128). Coder runs have no
    /// step limit: it is read and ignored, and new grants leave it out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_steps: Option<usize>,
    pub acceptance: bool,
    pub route: String,
    pub knowledge: String,
    pub dollar_limit_micros: Option<u64>,
    #[serde(default)]
    pub expected_controller_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<container::Profile>,
    /// Further admitted routes, in the owner's preference order. When the
    /// provider in use refuses for a usage or rate limit, the run switches
    /// to the first of these with capacity. Empty means no failover, and
    /// the field is then left out, so earlier grants keep their bytes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallbacks: Vec<Route>,
    /// What the engine's commands may reach. Absent means
    /// [`Access::Boundary`], so earlier grants keep their bytes and meaning.
    #[serde(default, skip_serializing_if = "Access::is_boundary")]
    pub access: Access,
    /// The Agent Studio seat whose task this is (#10542): its processes
    /// commit as the seat and Git refuses every transport for them
    /// ([`super::studio::git::confine`]). Absent for every other run, so
    /// earlier grants keep their bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub studio_seat: Option<String>,
}

/// What an admitted run's commands may reach.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    /// The filesystem boundary: writes only to the workspace and private
    /// scratch, no external network, a cleared environment with the system
    /// `PATH`, and a scratch `HOME`.
    #[default]
    Boundary,
    /// The filesystem boundary with this computer's developer tools, for
    /// a person running Coder on their own computer (`coder::task::local`):
    /// writes still only to the workspace and private scratch, and reads
    /// still confined, but the toolchains installed here are readable and
    /// on `PATH` ([`coder_boundary::toolchains`]), the network is open,
    /// and `HOME` is still the scratch. The allow list is recorded in the
    /// run's transcript.
    Toolchains,
    /// The owner's full access, for the owner's own hosts: commands run
    /// as the host's user with no sandbox, with network access, in the
    /// user's login-shell environment and real `HOME`. Credential variables
    /// (`*_API_KEY`, `*_TOKEN`, `*_SECRET`) are still left out. Only the
    /// host's owner turns it on, with `coder host autostart on
    /// --full-access`; a device cannot ask for it.
    Full,
}

impl Access {
    #[must_use]
    pub fn is_boundary(&self) -> bool {
        *self == Access::Boundary
    }

    /// The name the policy journal and the admission record use.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Access::Boundary => "boundary",
            Access::Toolchains => "toolchains",
            Access::Full => "full",
        }
    }
}

/// The most fallback routes one grant admits.
pub const MAX_FALLBACKS: usize = 4;

/// One admitted provider and model a run may generate through.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    /// `codex` or `claude`, as [`Configuration::provider`].
    pub provider: String,
    /// The exact identity the provider must report.
    pub model: String,
    pub effort: Option<String>,
    pub generation_endpoint: String,
}

impl microcoder_loop::failover::Admitted for Route {
    fn provider(&self) -> Option<super::capacity::Provider> {
        super::capacity::Provider::from_config(&self.provider)
    }

    fn model(&self) -> &str {
        &self.model
    }
}

impl Configuration {
    /// The route the run starts on: the configuration's own provider,
    /// model, effort, and endpoint.
    pub fn primary(&self) -> Route {
        Route {
            provider: self.provider.clone(),
            model: self.model.clone(),
            effort: self.effort.clone(),
            generation_endpoint: self.generation_endpoint.clone(),
        }
    }

    /// Every admitted route, the primary first.
    pub fn routes(&self) -> Vec<Route> {
        std::iter::once(self.primary())
            .chain(self.fallbacks.iter().cloned())
            .collect()
    }

    /// Whether `model` is one this grant admits. A task records the policy's
    /// first model; the grant may start on another admitted route.
    pub fn admits_model(&self, model: &str) -> bool {
        self.model == model || self.fallbacks.iter().any(|route| route.model == model)
    }

    pub fn validate(&self) -> Result<(), Error> {
        // Coder V1 on the OpenAgents Gateway is the one vertex route.
        let coder_on_gateway = self.provider == "vertex"
            && self.generation_endpoint == super::capacity::CODER_V1_ENDPOINT;
        if self.schema != CONFIG_SCHEMA
            || !(coder_on_gateway
                || matches!(
                    self.provider.as_str(),
                    "codex" | "claude" | "devin" | "opencode" | "grok" | "synthetic"
                ))
            || !identifier(&self.model, true)
            || !identifier(&self.decision_model, true)
            || self
                .effort
                .as_deref()
                .is_some_and(|effort| !matches!(effort, "low" | "medium" | "high" | "xhigh"))
            || self.acceptance
            || self.route != "never"
            || !matches!(self.knowledge.as_str(), "off" | "frozen-context")
            || self.dollar_limit_micros.is_some()
            || self
                .studio_seat
                .as_deref()
                .is_some_and(|seat| !super::studio::valid_name(seat))
            || self
                .expected_controller_digest
                .as_ref()
                .is_some_and(|digest| {
                    !digest.starts_with("sha256:")
                        || digest.len() != 71
                        || !digest[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
                })
        {
            return Err(Error::InvalidCommand(
                "unsupported repository adapter configuration",
            ));
        }
        if let Some(container) = &self.container {
            container.validate()?;
            match self.access {
                Access::Boundary => {}
                Access::Full => {
                    return Err(Error::InvalidCommand(
                        "full access does not apply to container commands",
                    ));
                }
                Access::Toolchains => {
                    return Err(Error::InvalidCommand(
                        "this computer's toolchains do not apply to container commands",
                    ));
                }
            }
        }
        if self.fallbacks.len() > MAX_FALLBACKS {
            return Err(Error::InvalidCommand("too many fallback routes"));
        }
        let routes = self.routes();
        for (index, route) in routes.iter().enumerate() {
            let known = if self.provider == "synthetic" {
                route.provider == "synthetic"
            } else if route.provider == "devin" {
                // Devin is a whole agent behind `devin acp`: no effort, and
                // the endpoint names the local process, not a URL.
                route.effort.is_none()
                    && route.generation_endpoint == super::capacity::DEVIN_ENDPOINT
                    && self.container.is_none()
            } else if route.provider == "opencode" {
                // OpenCode is a whole agent behind `opencode acp`, and its
                // model is OpenCode's own `provider/model`.
                route.effort.is_none()
                    && route.generation_endpoint == super::capacity::OPENCODE_ENDPOINT
                    && self.container.is_none()
                    && acp_client::opencode::Model::parse(&route.model).is_ok()
            } else if route.provider == "grok" {
                // Grok Build is a whole agent behind `grok agent stdio`.
                route.effort.is_none()
                    && route.generation_endpoint == super::capacity::GROK_ENDPOINT
                    && self.container.is_none()
                    && acp_client::grok::parse_model(&route.model).is_ok()
            } else if route.provider == "claude"
                && (route.generation_endpoint == super::capacity::CLAUDE_SESSION_ENDPOINT
                    || route.generation_endpoint == super::capacity::CLAUDE_SDK_ENDPOINT)
            {
                // A lean Claude Code session (#10246) or an Agent SDK
                // session (#10571) is the local `claude` process, like a
                // whole agent, and runs on the host itself.
                self.container.is_none()
            } else if matches!(route.provider.as_str(), "codex" | "claude" | "vertex")
                && route.generation_endpoint == super::capacity::CODER_V1_ENDPOINT
            {
                // A Coder V1 turn (#10754) is the local `openagents coder`
                // process, like a whole agent, and runs on the host itself.
                self.container.is_none()
            } else if route.provider == "codex"
                && route.generation_endpoint == super::capacity::CODEX_SESSION_ENDPOINT
            {
                // A lean Codex session (#10250) is the local `codex`
                // process, like a whole agent, and runs on the host itself.
                self.container.is_none()
            } else {
                matches!(route.provider.as_str(), "codex" | "claude")
            };
            if !known
                || !identifier(&route.model, true)
                || route
                    .effort
                    .as_deref()
                    .is_some_and(|effort| !matches!(effort, "low" | "medium" | "high" | "xhigh"))
                || routes[..index].iter().any(|earlier| {
                    earlier.provider == route.provider && earlier.model == route.model
                })
            {
                return Err(Error::InvalidCommand(
                    "unsupported or repeated fallback route",
                ));
            }
        }
        let endpoints = std::iter::once(&self.decision_endpoint)
            .chain(
                routes
                    .iter()
                    .filter(|route| {
                        !matches!(route.provider.as_str(), "devin" | "opencode" | "grok")
                            && route.generation_endpoint != super::capacity::CLAUDE_SESSION_ENDPOINT
                            && route.generation_endpoint != super::capacity::CLAUDE_SDK_ENDPOINT
                            && route.generation_endpoint != super::capacity::CODEX_SESSION_ENDPOINT
                            && route.generation_endpoint != super::capacity::CODER_V1_ENDPOINT
                    })
                    .map(|route| &route.generation_endpoint),
            )
            .collect::<Vec<_>>();
        for endpoint in endpoints {
            if self.provider == "synthetic" {
                if endpoint != "in-process" {
                    return Err(Error::InvalidCommand(
                        "synthetic fixtures require in-process models",
                    ));
                }
            } else {
                let url = reqwest::Url::parse(endpoint)
                    .map_err(|_| Error::InvalidCommand("invalid model endpoint"))?;
                if url.scheme() != "https"
                    || url.host_str().is_none()
                    || !url.username().is_empty()
                    || url.password().is_some()
                    || url.query().is_some()
                    || url.fragment().is_some()
                {
                    return Err(Error::InvalidCommand(
                        "model endpoints require HTTPS without credentials",
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn capabilities(&self) -> Value {
        json!({
            "commands":"native-supervised", "reads":"native-confined",
            "cancellation":"emulated-host-stop", "atif":"native",
            "process_cleanup":"observed", "crash_resume":"unsupported",
            "model_written_acceptance":"unsupported", "routing":"unsupported",
            "knowledge":"unsupported", "hard_dollar_limit":"unsupported",
            "billing":"unknown",
            "effort_confirmation":"not_reported",
            "cost_reporting": match self.provider.as_str() {
                "claude" => "provider-reported-list-price",
                "devin" => "provider-reported-tokens",
                "opencode" => "provider-reported-list-price",
                "grok" => "provider-reported-tokens",
                _ => "token-list-price",
            },
            "provider_artifact_attestation":"unsupported",
            "container_adapter": if self.container.is_some() { "docker-per-command-workspace-persistence" } else { "not_requested" },
            "provider_failover": if self.fallbacks.is_empty() { "not_requested" } else { "on-capacity-refusal" },
            "frozen_knowledge_context":self.knowledge == "frozen-context",
            "access":self.access.as_str(),
            "steering": match self.provider.as_str() {
                "devin" => coder_delegate::steering::DEVIN_ACP,
                "opencode" => coder_delegate::steering::OPENCODE_ACP,
                "grok" => coder_delegate::steering::GROK_ACP,
                _ => STEERING,
            }
        })
    }
}

/// The most earlier turns a later turn carries, newest kept.
pub const MAX_CARRIED_TURNS: usize = 8;
/// The most bytes of one earlier message a later turn carries.
pub const MAX_CARRIED_BYTES: usize = 4 * 1024;

/// One earlier turn of a task, as a later turn carries it: the user's
/// message and the engine's last reply, each bounded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EarlierTurn {
    /// The turn, from one.
    pub turn: usize,
    pub prompt: String,
    pub reply: Option<String>,
}

impl EarlierTurn {
    /// The trace steps that carry this turn into a later one, each marked
    /// with the turn it came from.
    pub fn steps(&self) -> Vec<Step> {
        let carried = json!({"turn": self.turn});
        let mut steps =
            vec![Step::said(Source::User, &self.prompt).noting("carried_from", carried.clone())];
        if let Some(reply) = &self.reply {
            steps.push(Step::said(Source::Agent, reply).noting("carried_from", carried));
        }
        steps
    }
}

/// The earlier turns of `task` that ran, read from their retained traces
/// in `directory`: at most [`MAX_CARRIED_TURNS`], newest kept. A trace that
/// cannot be read contributes its prompt without a reply.
pub fn earlier_turns(directory: &Path, task: &Task) -> Vec<EarlierTurn> {
    let mut turns: Vec<EarlierTurn> = task
        .earlier
        .iter()
        .enumerate()
        .map(|(index, run)| EarlierTurn {
            turn: index + 1,
            prompt: bounded(&run.admission.context.prompt),
            reply: last_reply(&directory.join(&run.admission.trace_file)),
        })
        .collect();
    let skip = turns.len().saturating_sub(MAX_CARRIED_TURNS);
    turns.drain(..skip);
    turns
}

/// The prompt an engine gets for a turn after `earlier`.
pub fn conversation_prompt(earlier: &[EarlierTurn], prompt: &str) -> String {
    if earlier.is_empty() {
        return prompt.to_owned();
    }
    let mut text = String::from(
        "This continues an earlier conversation in this repository. Earlier turns, oldest first:\n",
    );
    for turn in earlier {
        text.push_str(&format!("\nUser (turn {}):\n{}\n", turn.turn, turn.prompt));
        if let Some(reply) = &turn.reply {
            text.push_str(&format!("\nCoder (turn {}):\n{reply}\n", turn.turn));
        }
    }
    text.push_str("\nThe user's new message:\n");
    text.push_str(prompt);
    text
}

fn bounded(text: &str) -> String {
    if text.len() <= MAX_CARRIED_BYTES {
        return text.to_owned();
    }
    let mut end = MAX_CARRIED_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// Why a workspace observation cannot admit a run, or `None` when it can:
/// an incomplete observation names its first fault (the limit reached, or
/// the file that could not be read, and how many faults there were), and a
/// complete one that differs from the grant's pin says it changed. The
/// check itself is unchanged: either case refuses.
fn source_snapshot_refusal(before: &Snapshot, pin: Option<&str>) -> Option<String> {
    if let Some(first) = before.faults().first() {
        let more = before.faults().len() - 1;
        let more = if more == 0 {
            String::new()
        } else {
            format!(" (and {more} more)")
        };
        return Some(format!(
            "the workspace snapshot is incomplete: {first}{more}"
        ));
    }
    pin.is_some_and(|pin| pin != before.digest())
        .then(|| "the workspace changed since its source snapshot was granted".to_owned())
}

/// The engine's last reply in a retained trace: the last agent message, or
/// the rationale of Microcoder's last generated action.
fn last_reply(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut reply = None;
    for line in bytes.split(|byte| *byte == b'\n') {
        let Ok(record) = serde_json::from_slice::<Value>(line) else {
            continue;
        };
        if record["record"] != "step" {
            continue;
        }
        let step = &record["step"];
        let message = step["message"].as_str().unwrap_or_default();
        let agent = matches!(step["source"].as_str(), Some("Agent" | "agent"));
        let action = &step["extensions"]["microcoder"]["event"]["generated"]["action"]["Ok"];
        let said = |name: &str| action[name].as_str().filter(|text| !text.trim().is_empty());
        if agent && !message.trim().is_empty() {
            reply = Some(message.to_owned());
        } else if let Some(text) = said("reply") {
            // The engine's reply to the user; its rationale is the loop's
            // own note, carried only from a trace recorded before replies.
            reply = Some(text.to_owned());
        } else if let Some(rationale) = said("rationale") {
            reply = Some(rationale.to_owned());
        }
    }
    reply.map(|text| bounded(&text))
}

/// One retained trace line, as far as [`Host::earlier_note`] reads it.
#[derive(Deserialize)]
struct TraceLine {
    #[serde(default)]
    step: Option<TraceStep>,
}

#[derive(Deserialize)]
struct TraceStep {
    #[serde(default)]
    extensions: serde_json::Map<String, Value>,
}

/// A toolchain run's command `PATH`: the toolchains' search path, then the
/// system one, each kept only when the boundary can read it.
fn command_path(
    boundary: &Boundary,
    toolchains: &coder_boundary::Toolchains,
) -> std::ffi::OsString {
    let entries = toolchains
        .path
        .iter()
        .cloned()
        .chain(std::env::split_paths(owner::SYSTEM_PATH));
    boundary.search_path(&std::env::join_paths(entries).unwrap_or_default())
}

/// Whether a run with `access` builds, and so takes a build slot: full
/// access and this computer's toolchains do; the boundary has no
/// toolchain to build with.
fn builds(access: Access) -> bool {
    matches!(access, Access::Full | Access::Toolchains)
}

/// What a run's command boundary is built from at admission.
#[derive(Clone, Debug)]
struct CommandPolicy {
    workspace: PathBuf,
    write_workspace: bool,
    program: PathBuf,
    store: PathBuf,
    git_directory: PathBuf,
    access: Access,
    /// The run's leased build slot, which a run under this computer's
    /// toolchains may write (#10293).
    target: Option<PathBuf>,
    /// Where the run's leased builds reach (#10757): a run under this
    /// computer's toolchains may read the `cargo` shim and the binary it
    /// runs, and write the lease table. `None` for a run that doesn't
    /// build or while the shims are off.
    leases: Option<super::targets::RunLeases>,
    /// The priority the run's builds wait at.
    lease_priority: coder_lease::Priority,
}

impl CommandPolicy {
    /// The boundary's policy: writes only to the workspace (when the grant
    /// writes it) and a private scratch, the task store and the common Git
    /// directory sealed, reads confined to the workspace, the system, the
    /// granted shell, `reads`, and, under [`Access::Toolchains`], this
    /// computer's toolchains and the Git directory, and the run's build
    /// slot; no network under [`Access::Boundary`].
    fn spec(
        &self,
        toolchains: Option<&coder_boundary::Toolchains>,
        reads: &[PathBuf],
    ) -> coder_boundary::Spec {
        let mut spec = if self.write_workspace {
            Boundary::writing(&self.workspace)
        } else {
            Boundary::readonly()
        }
        .readable(&self.workspace)
        .readable(&self.program)
        .sealed(&self.store)
        .sealed(&self.git_directory)
        .owned_scratch_under(std::env::temp_dir());
        for read in toolchains.iter().flat_map(|toolchains| &toolchains.reads) {
            spec = spec.readable(&read.path);
        }
        // Git in the worktree reads the common Git directory; it stays
        // sealed against writes.
        if toolchains.is_some() {
            spec = spec.readable(&self.git_directory);
        }
        if let Some(target) = &self.target {
            spec = spec.writable(target);
        }
        if let Some(leases) = self.lease_grants() {
            spec = spec.readable(&leases.shims).writable(&leases.root);
            if let Some(bin) = &leases.bin {
                spec = spec.readable(bin);
            }
        }
        for read in reads {
            spec = spec.readable(read);
        }
        if self.access != Access::Toolchains {
            spec = spec.offline();
        }
        spec
    }
}

impl CommandPolicy {
    /// The leases a run under this computer's toolchains may reach: only
    /// when the lease root lies apart from the workspace, the task store,
    /// and the Git directory, which the boundary writes or seals.
    fn lease_grants(&self) -> Option<&super::targets::RunLeases> {
        let leases = self.leases.as_ref()?;
        if self.access != Access::Toolchains {
            return None;
        }
        let root = leases.root.canonicalize().ok()?;
        let apart = [&self.workspace, &self.store, &self.git_directory]
            .into_iter()
            .filter_map(|path| path.canonicalize().ok())
            .all(|path| !root.starts_with(&path) && !path.starts_with(&root));
        apart.then_some(leases)
    }

    /// The variables that send the run's heavy `cargo` commands through
    /// the lease shim, over `path`; empty when the run's builds aren't
    /// leased.
    fn lease_environment(
        &self,
        path: Option<&std::ffi::OsStr>,
    ) -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
        let leases = match self.access {
            Access::Full => self.leases.as_ref(),
            Access::Toolchains => self.lease_grants(),
            Access::Boundary => None,
        };
        leases
            .map(|leases| leases.environment(path, self.lease_priority))
            .unwrap_or_default()
    }
}

/// Full bounded process observation; prompt summaries are a caller's projection.
#[derive(Debug)]
pub struct CommandObservation {
    pub exit: Option<i32>,
    pub timed_out: bool,
    pub seconds: f64,
    pub output: String,
    pub group_clear: bool,
}

/// Whether the process group `group` empties within `wait`, read every
/// `every`: a killed process stays in its group until it is reaped, which
/// a loaded host can take longer than the supervisor's grace to do. A
/// group with no identifier is never confirmed clear. Off Unix, where a
/// group is a job only its supervisor sees, nothing more is learned.
async fn group_settles(group: Option<i32>, wait: Duration, every: Duration) -> bool {
    let Some(group) = group else {
        return false;
    };
    if !cfg!(unix) {
        return false;
    }
    let started = Instant::now();
    loop {
        if !supervise::running(group) {
            return true;
        }
        if started.elapsed() >= wait {
            return false;
        }
        tokio::time::sleep(every).await;
    }
}

/// A single task owner. It is deliberately neither serializable nor cloneable.
pub struct Host {
    target: Option<super::targets::Lease>,
    /// Where software the run's agent installs lands (#10336): a private
    /// prefix for a full-access run, removed when the run ends.
    installs: Option<installs::Prefix>,
    owner: owner::Owner,
    task: Task,
    admission: owner::Admission,
    before: Snapshot,
    /// The command boundary, for a run under it; a full-access run has
    /// none, since its commands run as the owner with no sandbox.
    boundary: Option<Boundary>,
    /// What the command boundary was built from, so a whole coding
    /// agent's own process gets the same policy ([`Host::engine_boundary`]).
    policy: CommandPolicy,
    /// This computer's toolchains, for a run with [`Access::Toolchains`]:
    /// what its boundary reads, and its commands' `PATH` and variables.
    toolchains: Option<coder_boundary::Toolchains>,
    /// What the run's full-access commands and whole coding agents may
    /// not write: the checkout the workspace was made from and its Git
    /// directory (#10247). `None` when the workspace is no linked
    /// worktree, in a container, or off macOS and Linux.
    guard: Option<coder_boundary::source::Guard>,
    /// The owner's login-shell environment, for a full-access run: read
    /// beside admission, and waited for by the first command that runs
    /// with it ([`Host::login_environment`]). Set to `None` at admission
    /// under the boundary.
    login: tokio::sync::OnceCell<Option<login::Environment>>,
    /// The login shell still being read.
    login_reading: RefCell<Option<tokio::task::JoinHandle<login::Environment>>>,
    earlier: Vec<EarlierTurn>,
    trace: RefCell<Log>,
    trace_bytes: Cell<usize>,
    started: Instant,
    sequence: Cell<usize>,
    stopped: Cell<bool>,
    group_clear: Cell<bool>,
    output_incomplete: Cell<bool>,
    /// Records past the transcript's bounds were left out (#10993).
    evidence_capped: Cell<bool>,
    fault: RefCell<Option<String>>,
    /// The disk filled past [`owner::STORAGE_FULL_WAIT`] while the run
    /// kept its evidence (#10237): its result says so.
    disk_full: Cell<bool>,
    /// A command ended with its processes not confirmed gone (#10281):
    /// the run ends as [`PROCESS_CLEANUP_UNKNOWN`].
    cleanup_unknown: Cell<bool>,
    /// How long, and how often, a trace write waits out a full disk.
    evidence_wait: Cell<(Duration, Duration)>,
    /// Trace writes still to fail as on a full disk ([`Host::fill_disk`]).
    full_writes: Cell<usize>,
    /// What the run cost, as the engine reported it ([`Host::cost`]).
    cost: Cell<owner::Cost>,
    /// The checks the delegate recipe froze for this turn
    /// ([`Host::freeze_checks`]), which the run's independent check runs
    /// again ([`super::local_checks`]).
    frozen_checks: RefCell<Vec<String>>,
}

impl Host {
    /// Admit exact operator bytes before any model call or workspace command.
    pub async fn admit(directory: &Path, bytes: &[u8]) -> Result<Self, Error> {
        let grant = owner::Grant::parse(bytes)?;
        let configuration = grant
            .adapter_configuration
            .as_ref()
            .ok_or(Error::InvalidCommand(
                "repository admission requires an explicit adapter configuration",
            ))?;
        configuration.validate()?;
        let has_knowledge = grant
            .requirements
            .as_ref()
            .is_some_and(|requirements| !requirements.knowledge.is_empty());
        if (configuration.knowledge == "frozen-context") != has_knowledge {
            return Err(Error::InvalidCommand(
                "frozen knowledge context differs from the execution grant",
            ));
        }
        if !grant.arguments.is_empty() {
            return Err(Error::InvalidCommand(
                "repository admission supplies no fixed command arguments",
            ));
        }
        let (owner, task) = {
            let store = Store::open_for_owner(directory)?;
            let owner = owner::Owner::acquire_waiting(&store, &grant.task_id)?;
            let task = store.show(&grant.task_id)?;
            if task.run.is_some() || task.status != Status::Queued {
                return Err(Error::InvalidTransition);
            }
            if grant.intent_digest != task.intent_digest || grant.expected_revision != task.revision
            {
                return Err(Error::RevisionMismatch);
            }
            if task.intent.configuration.adapter != NAME
                || !task
                    .intent
                    .configuration
                    .model
                    .as_deref()
                    .is_some_and(|model| configuration.admits_model(model))
            {
                return Err(Error::InvalidCommand(
                    "requested and granted adapter or model differ",
                ));
            }
            (owner, task)
        };
        let workspace = Path::new(&task.intent.workspace.path).canonicalize()?;
        if owner.dir.starts_with(&workspace) || workspace.starts_with(&owner.dir) {
            return Err(Error::UnsafePath);
        }
        let program = grant.program.canonicalize()?;
        if program != grant.program
            || !owner::SYSTEM_SHELLS
                .iter()
                .filter_map(|path| Path::new(path).canonicalize().ok())
                .any(|path| path == program)
        {
            return Err(Error::InvalidCommand(
                "the granted program must be the canonical system shell",
            ));
        }
        // The workspace is its checkout's top level: an empty or plain
        // directory inside some other repository is not a workspace.
        let top_level = owner::git(&workspace, &["rev-parse", "--show-toplevel"]).await?;
        if Path::new(&top_level).canonicalize().ok().as_deref() != Some(workspace.as_path()) {
            return Err(Error::InvalidCommand(
                "the workspace is not the top level of a Git checkout",
            ));
        }
        let source_revision = owner::git(&workspace, &["rev-parse", "HEAD"]).await?;
        if task
            .intent
            .workspace
            .source_revision
            .as_ref()
            .is_some_and(|pin| pin != &source_revision)
        {
            return Err(Error::InvalidCommand(
                "the workspace source revision changed",
            ));
        }
        let git_directory = PathBuf::from(
            owner::git(
                &workspace,
                &["rev-parse", "--path-format=absolute", "--git-common-dir"],
            )
            .await?,
        );
        // The checkout the worktree was made from stays unwritten even
        // under full access, and even by a whole coding agent that
        // approves its own tools (#10247): a guard that can't be
        // enforced here refuses the run.
        let guard = if coder_boundary::source::APPLIES && configuration.container.is_none() {
            let guard = coder_boundary::source::Guard::for_worktree(&workspace).map_err(|_| {
                Error::InvalidCommand("the workspace's source checkout cannot be found")
            })?;
            if let Some(guard) = &guard {
                guard.enforceable().map_err(|_| {
                    Error::InvalidCommand("the source checkout cannot be kept unwritten here")
                })?;
            }
            guard
        } else {
            None
        };
        // Every access that builds takes a slot, so its builds stay in the
        // slot budget instead of the worktree (#10293); a run under the
        // boundary has no toolchain to build with. A run that finds every
        // slot taken builds in its worktree, as it did before it had slots:
        // more runs than slots is normal on a busy host, and refusing it
        // read as a busy task store (#10301).
        let target = match configuration.access {
            access if builds(access) => {
                let started = Instant::now();
                loop {
                    match super::targets::Lease::acquire(&owner.dir, &git_directory) {
                        Ok(lease) => break Some(lease),
                        Err(Error::BuildDiskLow { .. })
                            if started.elapsed() < Duration::from_secs(600) =>
                        {
                            tokio::time::sleep(Duration::from_secs(2)).await;
                        }
                        Err(Error::Busy) => break None,
                        Err(error) => return Err(error),
                    }
                }
            }
            _ => None,
        };
        // The run's heavy `cargo` commands each take a counted `build`
        // lease from the host broker through the shim first on its `PATH`,
        // so Coder's builds and every other agent's share one build count
        // (#10756). The run itself holds none: a long run that isn't
        // compiling keeps no other agent's build waiting (#10757).
        let leases = if builds(configuration.access) {
            super::targets::RunLeases::for_store(&owner.dir)
                .filter(|leases| std::fs::create_dir_all(&leases.root).is_ok())
        } else {
            None
        };
        // The owner's login environment, for a full-access run, is read in
        // the background: nothing before the first command needs it, and
        // the first command waits for it ([`Host::login_environment`]).
        let login_reading = match configuration.access {
            Access::Full => Some(tokio::spawn(login::capture())),
            Access::Boundary | Access::Toolchains => None,
        };
        // The workspace is observed with the digests earlier owners kept,
        // so only files that changed since are read again.
        let digests = owner.dir.join(SNAPSHOT_DIGESTS);
        let observing = {
            let workspace = workspace.clone();
            tokio::task::spawn_blocking(move || {
                #[cfg(unix)]
                Snapshot::recall_digests(&digests);
                let before = Snapshot::observe(&workspace);
                // Off Unix the observation refuses, and there is nothing
                // to keep.
                #[cfg(unix)]
                if let Err(error) = Snapshot::remember_digests(&digests) {
                    eprintln!("coder: cannot keep the workspace digests: {error}");
                }
                #[cfg(not(unix))]
                let _ = digests;
                before
            })
        };
        let before = observing
            .await
            .map_err(|_| Error::InvalidCommand("the workspace could not be observed"))?;
        if let Some(refusal) =
            source_snapshot_refusal(&before, grant.expected_source_snapshot.as_deref())
        {
            return Err(Error::SourceSnapshot(refusal));
        }
        // A full-access run's commands run with no sandbox. Unix builds the
        // boundary for it anyway, as it always has; Windows builds none, so
        // a computer there that cannot make an AppContainer still runs the
        // owner's own full-access tasks.
        // A run with this computer's tools reads its toolchains too, never
        // a grant that would hold the task store or the Git directory.
        let toolchains = (configuration.access == Access::Toolchains).then(|| {
            coder_boundary::Toolchains::derive(&coder_boundary::toolchains::Host::this_computer())
                .clear_of(&[owner.dir.clone(), git_directory.clone()])
        });
        let policy = CommandPolicy {
            workspace: workspace.clone(),
            write_workspace: grant.write_workspace,
            program: program.clone(),
            store: owner.dir.clone(),
            git_directory: git_directory.clone(),
            access: configuration.access,
            target: (configuration.access == Access::Toolchains)
                .then(|| target.as_ref().map(|lease| lease.path.clone()))
                .flatten(),
            leases,
            lease_priority: super::targets::task_priority(),
        };
        let boundary =
            if configuration.access != Access::Full || cfg!(unix) {
                Some(policy.spec(toolchains.as_ref(), &[]).build().map_err(|_| {
                    Error::InvalidCommand("the repository boundary cannot be enforced")
                })?)
            } else {
                None
            };
        if let Some(container) = &configuration.container {
            container.admit(&workspace, &owner.dir).await?;
        }
        let context = checks::Context::capture(&task, &workspace, grant.requirements.as_ref())?;
        // The running engine, read through a path that still opens it
        // after a rebuild replaced its file (#10237), named by the path
        // it was started from.
        let (controller, image) = super::autostart::running_program()?;
        // Only a pinned controller is read and digested, as its launcher
        // does (#10115): a development build is a gigabyte, and digesting
        // it held every start for thirteen seconds before the engine ran
        // (owner, 2026-10-02). An unpinned one is named by its path, size,
        // and modification time.
        let controller_digest = match &configuration.expected_controller_digest {
            Some(expected) => {
                let digest = digest_bytes(&std::fs::read(&image)?);
                if expected != &digest {
                    return Err(Error::InvalidCommand(
                        "the repository controller differs from its grant",
                    ));
                }
                Some(digest)
            }
            None => None,
        };
        let controller_file = std::fs::metadata(&image).ok();
        let controller_bytes = controller_file.as_ref().map(std::fs::Metadata::len);
        let controller_modified = controller_file
            .and_then(|file| file.modified().ok())
            .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|at| at.as_millis());
        let admission = owner::Admission {
            grant: grant.clone(),
            grant_digest: digest_bytes(bytes),
            grant_request: String::from_utf8(bytes.to_vec())
                .map_err(|_| Error::UnsupportedSchema)?,
            workspace: workspace.clone(),
            source_revision,
            source_snapshot: before.digest(),
            program_digest: digest_bytes(&std::fs::read(&program)?),
            adapter: NAME.into(),
            network: if configuration.container.is_some() {
                "container_network_none"
            } else if configuration.access != Access::Boundary {
                "host_network"
            } else {
                owner::network_policy()
            }
            .into(),
            read_scope: if configuration.container.is_some() {
                "workspace_host_reads_and_pinned_container_image"
            } else if configuration.access == Access::Full {
                "host_user"
            } else if configuration.access == Access::Toolchains {
                "workspace_system_and_toolchains"
            } else {
                "workspace_and_system"
            }
            .into(),
            authority: "local_os_user".into(),
            trace_file: task.trace_file(task.turn()),
            context,
        };
        owner.record(owner::Event::Admitted {
            admission: Box::new(admission.clone()),
        })?;
        let mut trace = Log::create_at(
            &owner.dir.join(&admission.trace_file),
            &Session::opening(
                &format!("{}-{}", task.task_id, task.turn()),
                &configuration.model,
                NAME,
                &workspace.display().to_string(),
                env!("CARGO_PKG_VERSION"),
            ),
        )?;
        let earlier = earlier_turns(&owner.dir, &task);
        for turn in &earlier {
            for step in turn.steps() {
                trace.append(&step)?;
            }
        }
        trace.append(&Step::said(Source::User, task.effective_prompt()))?;
        for step in super::consumed_steers(&task, &STEERING) {
            trace.append(&step)?;
        }
        trace.append(
            &Step::said(
                Source::System,
                "Repository adapter admitted by the local operator.",
            )
            .noting("admission", json!(admission))
            .noting("controller",json!({"path":controller,"digest":controller_digest,"bytes":controller_bytes,"modified_ms":controller_modified,"version":env!("CARGO_PKG_VERSION")}))
            .noting("capabilities", configuration.capabilities())
            .noting(
                "command_environment",
                if login_reading.is_some() {
                    json!({"source":"login_shell","recorded":"before the first command"})
                } else if let (Some(toolchains), Some(boundary)) = (&toolchains, &boundary) {
                    json!({"source":"toolchains","path":command_path(boundary, toolchains).to_string_lossy(),
                        "variables":toolchains.environment.iter().map(|(name, _)| name).collect::<Vec<_>>()})
                } else {
                    json!({"source":"cleared","path":owner::SYSTEM_PATH})
                },
            )
            .noting("toolchains", json!(toolchains))
            .noting(
                "source_guard",
                json!(guard.as_ref().map(|guard| json!({
                    "unwritten": guard.protected().collect::<Vec<_>>(),
                    "allowed_back": guard.allowed().collect::<Vec<_>>(),
                }))),
            ),
        )?;
        if Snapshot::observe(&workspace).digest() != before.digest() {
            return Err(Error::InvalidCommand(
                "source changed after repository admission",
            ));
        }
        // The epoch effect is durable in the common journal. Individual effects
        // retain their exact intents and results in this same fsynced ATIF log.
        if Store::open_for_owner(&owner.dir)?
            .show(&task.task_id)?
            .status
            != Status::CancelRequested
        {
            owner.record(owner::Event::EffectIntent {
                effect_id: owner::effect_id_for(&task),
            })?;
        }
        let installs = if login_reading.is_some() {
            let prefix = installs::Prefix::create()?;
            trace.append(
                &Step::said(
                    Source::System,
                    "Software this run installs lands in its own prefix, not the owner's.",
                )
                .noting("install_prefix", json!({"path": prefix.path()})),
            )?;
            Some(prefix)
        } else {
            None
        };
        let trace_bytes = std::fs::metadata(trace.path())?.len() as usize;
        Ok(Self {
            target,
            installs,
            owner,
            task,
            admission,
            before,
            boundary,
            policy,
            toolchains,
            guard,
            login: if login_reading.is_some() {
                tokio::sync::OnceCell::new()
            } else {
                tokio::sync::OnceCell::new_with(Some(None))
            },
            login_reading: RefCell::new(login_reading),
            earlier,
            trace: RefCell::new(trace),
            trace_bytes: Cell::new(trace_bytes),
            started: Instant::now(),
            sequence: Cell::new(0),
            stopped: Cell::new(false),
            group_clear: Cell::new(true),
            output_incomplete: Cell::new(false),
            evidence_capped: Cell::new(false),
            fault: RefCell::new(None),
            disk_full: Cell::new(false),
            cleanup_unknown: Cell::new(false),
            evidence_wait: Cell::new((owner::STORAGE_FULL_WAIT, Duration::from_secs(2))),
            full_writes: Cell::new(0),
            cost: Cell::new(owner::Cost::default()),
            frozen_checks: RefCell::new(Vec::new()),
        })
    }

    /// Name the checks the delegate recipe froze for this turn (#10208):
    /// when the run ends, its independent check runs them again on the
    /// exact candidate, with the touched packages' tests (#10232).
    pub fn freeze_checks(&self, commands: &[String]) {
        *self.frozen_checks.borrow_mut() = commands.to_vec();
    }

    /// Whether a turn that completes is checked independently when it
    /// ends (#10232): the grant states requirements, so the frozen checks
    /// and the touched packages' tests run on the exact candidate.
    #[must_use]
    pub fn checks_follow(&self) -> bool {
        self.admission.grant.requirements.is_some()
    }

    /// The variables a local check's commands get (#10232): this run's
    /// own tool `PATH` and toolchain variables, so `cargo` resolves as it
    /// did for the engine while the check's `HOME` is scratch.
    fn check_environment(&self) -> Vec<(String, std::ffi::OsString)> {
        let mut variables: Vec<(String, std::ffi::OsString)> = Vec::new();
        if let Some(toolchains) = &self.toolchains {
            let entries = toolchains
                .path
                .iter()
                .cloned()
                .chain(std::env::split_paths(owner::SYSTEM_PATH));
            variables.push((
                "PATH".into(),
                std::env::join_paths(entries).unwrap_or_default(),
            ));
            variables.extend(
                toolchains
                    .environment
                    .iter()
                    .map(|(name, value)| (name.clone(), value.clone().into_os_string())),
            );
        } else if let Some(Some(login)) = self.login.get() {
            for (name, value) in &login.variables {
                if let Some(name) = name.to_str()
                    && matches!(name, "PATH" | "RUSTUP_HOME" | "CARGO_HOME")
                {
                    variables.push((name.to_owned(), value.clone()));
                }
            }
        }
        if !variables.iter().any(|(name, _)| name == "PATH")
            && let Some(path) = std::env::var_os("PATH")
        {
            variables.push(("PATH".into(), path));
        }
        let home = std::env::var_os("HOME").map(PathBuf::from);
        for (name, default) in [("RUSTUP_HOME", ".rustup"), ("CARGO_HOME", ".cargo")] {
            if variables.iter().any(|(held, _)| held == name) {
                continue;
            }
            let value = std::env::var_os(name)
                .map(PathBuf::from)
                .or_else(|| home.as_ref().map(|home| home.join(default)))
                .filter(|path| path.is_dir());
            if let Some(value) = value {
                variables.push((name.to_owned(), value.into_os_string()));
            }
        }
        variables
    }

    /// The private scratch directory of the command boundary.
    fn scratch(&self) -> Result<&Path, Error> {
        self.boundary
            .as_ref()
            .and_then(Boundary::scratch)
            .ok_or(Error::UnsafePath)
    }

    pub fn configuration(&self) -> &Configuration {
        self.admission
            .grant
            .adapter_configuration
            .as_ref()
            .expect("configuration admitted")
    }

    /// The task store directory this owner holds, where the capacity book
    /// lives.
    pub fn store(&self) -> &Path {
        &self.owner.dir
    }

    /// The task this run belongs to, by its 64-hex ID.
    pub fn task_id(&self) -> &str {
        &self.task.task_id
    }

    pub fn prompt(&self) -> &str {
        &self.admission.context.prompt
    }

    /// The images the person attached to this task, as its admitted intent
    /// names them, each read back from the task store and checked against
    /// its digest and type ([`super::media::load`]).
    ///
    /// # Errors
    /// An image is missing or its bytes differ.
    pub fn images(&self) -> Result<Vec<(coder_host::access::media::ImageRef, Vec<u8>)>, Error> {
        self.task
            .intent
            .images
            .iter()
            .map(|reference| {
                super::media::load(&self.owner.dir, &self.task.task_id, reference)
                    .map(|bytes| (reference.clone(), bytes))
            })
            .collect()
    }

    /// The task's earlier turns that ran, oldest first, as this turn's
    /// trace carries them.
    pub fn earlier(&self) -> &[EarlierTurn] {
        &self.earlier
    }

    /// What the engine is asked to do this turn: the admitted prompt, after
    /// the earlier turns of the conversation when there are any. The earlier
    /// turns are context the host read from its own retained traces; they
    /// grant nothing.
    pub fn engine_prompt(&self) -> String {
        conversation_prompt(&self.earlier, self.prompt())
    }
    pub fn context(&self) -> &checks::Context {
        &self.admission.context
    }
    pub fn workspace(&self) -> &Path {
        &self.admission.workspace
    }

    /// The guard that keeps this run out of the checkout its worktree
    /// was made from, when there is one.
    #[must_use]
    pub fn source_guard(&self) -> Option<&coder_boundary::source::Guard> {
        self.guard.as_ref()
    }

    /// `program` as a command with no boundary of this run's (full
    /// access, or a whole coding agent): under the [`Host::source_guard`]
    /// when there is one, so nothing it starts writes the checkout the
    /// workspace was made from, and on macOS always under the privacy
    /// rules (`coder_boundary::privacy`), with the workspace allowed
    /// back. The caller adds [`Host::guard_environment`] after its own.
    #[must_use]
    pub fn private_command(&self, program: impl AsRef<Path>) -> std::process::Command {
        let program = program.as_ref();
        match &self.guard {
            Some(guard) => guard.command(program, &[self.workspace()]),
            None => coder_boundary::privacy::command(program, &[self.workspace()]),
        }
    }

    /// [`Host::private_command`] as a program and its arguments.
    #[must_use]
    pub fn private_argv(&self, program: PathBuf, arguments: Vec<String>) -> (PathBuf, Vec<String>) {
        match &self.guard {
            Some(guard) => guard.argv(program, arguments, &[self.workspace()]),
            None => coder_boundary::privacy::argv(program, arguments, &[self.workspace()]),
        }
    }

    /// The variables a guarded process adds to its environment: Git's
    /// repository discovery stops at the worktree's parent. A studio
    /// task's process also gets [`super::studio::git::additions`]. Empty
    /// with no guard and no studio seat.
    #[must_use]
    pub fn guard_environment(&self) -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
        let mut variables = self
            .guard
            .as_ref()
            .map(coder_boundary::source::Guard::environment)
            .unwrap_or_default();
        // A studio task's process also cannot push and commits as its
        // seat (#10542).
        if let Some(seat) = &self.configuration().studio_seat {
            let added = super::studio::git::additions(seat, self.workspace());
            variables.retain(|(key, _)| !added.iter().any(|(name, _)| name == key));
            variables.extend(added);
        }
        variables
    }

    /// Confine `variables`, a whole process environment, to local Git
    /// work when this run is a studio task (#10542).
    fn confine(&self, variables: &mut Vec<(std::ffi::OsString, std::ffi::OsString)>) {
        if let Some(seat) = &self.configuration().studio_seat {
            super::studio::git::confine(variables, seat, self.workspace());
        }
    }
    pub fn execution_workspace(&self) -> &Path {
        if self.configuration().container.is_some() {
            Path::new("/workspace")
        } else {
            self.workspace()
        }
    }

    /// The owner's login-shell environment for a full-access run; `None`
    /// under the boundary. Credential variables are already left out. The
    /// shell is read beside admission; the first call waits for it and
    /// records what it read (`command_environment`) before returning.
    pub async fn login_environment(&self) -> Option<&login::Environment> {
        self.login
            .get_or_init(|| async {
                let reading = self.login_reading.borrow_mut().take()?;
                let mut environment = match reading.await {
                    Ok(environment) => environment,
                    // The reader failed; read the shell again here.
                    Err(_) => login::capture().await,
                };
                if let Some(target) = &self.target {
                    environment
                        .variables
                        .retain(|(key, _)| key != "CARGO_TARGET_DIR");
                    environment.variables.push((
                        "CARGO_TARGET_DIR".into(),
                        target.path.as_os_str().to_owned(),
                    ));
                }
                let path = environment
                    .variables
                    .iter()
                    .find(|(key, _)| key == "PATH")
                    .map(|(_, value)| value.clone());
                super::targets::apply_run_leases(
                    &mut environment.variables,
                    self.policy.lease_environment(path.as_deref()),
                );
                // Installs go to this run's own prefix (#10336).
                if let Some(prefix) = &self.installs {
                    prefix.apply(&mut environment.variables);
                }
                // A relative local remote resolves against the main
                // checkout, not this worktree (#10333).
                let already = environment
                    .variables
                    .iter()
                    .find(|(key, _)| key == "GIT_CONFIG_COUNT")
                    .and_then(|(_, value)| value.to_str()?.parse::<usize>().ok())
                    .unwrap_or(0);
                let overrides =
                    super::local::remote_override_environment(self.workspace(), already);
                if !overrides.is_empty() {
                    environment
                        .variables
                        .retain(|(key, _)| key != "GIT_CONFIG_COUNT");
                    environment.variables.extend(overrides);
                }
                self.confine(&mut environment.variables);
                let recorded = self.append(
                    &Step::said(
                        Source::System,
                        "The owner's login environment, read for the run's commands.",
                    )
                    .noting("command_environment", environment.record()),
                );
                if let Err(error) = recorded {
                    self.fail(error.to_string());
                }
                Some(environment)
            })
            .await
            .as_ref()
    }

    /// The newest value an earlier turn of this task recorded under the
    /// step extension `key`, read from the earlier turns' retained traces,
    /// newest turn first. An engine that keeps its own session across turns
    /// (the Devin CLI) finds it here.
    pub fn earlier_note(&self, key: &str) -> Option<Value> {
        self.task.earlier.iter().rev().find_map(|run| {
            let bytes = std::fs::read(self.owner.dir.join(&run.admission.trace_file)).ok()?;
            bytes
                .split(|byte| *byte == b'\n')
                .filter_map(|line| serde_json::from_slice::<TraceLine>(line).ok())
                .filter_map(|line| line.step)
                .filter_map(|mut step| step.extensions.remove(key))
                .next_back()
        })
    }

    /// The fault the host recorded, if any ([`Host::fail`]).
    pub fn fault(&self) -> Option<String> {
        self.fault.borrow().clone()
    }

    /// Whether a stop was asked for this run: the task left `Running` in
    /// the store (the person, or the flow on their behalf, stopped it).
    /// A fault of the host's own is never one (#10993).
    pub fn stop_asked(&self) -> bool {
        self.stopped.get()
            || Store::open(&self.owner.dir)
                .and_then(|store| store.show(&self.task.task_id))
                .is_ok_and(|task| task.status == Status::CancelRequested)
    }

    pub fn fail(&self, reason: impl Into<String>) {
        if self.fault.borrow().is_none() {
            *self.fault.borrow_mut() = Some(reason.into());
        }
    }

    /// Cancellation prevents future admissions; already sent model requests can
    /// still incur unknown charges after the client stops awaiting their replies.
    pub fn cancelled(&self) -> bool {
        if self.stopped.get() || self.fault.borrow().is_some() {
            return true;
        }
        match Store::open(&self.owner.dir).and_then(|store| store.show(&self.task.task_id)) {
            Ok(task) if task.status == Status::Running => false,
            Ok(_) => {
                self.stopped.set(true);
                true
            }
            // Another process held the store past the lock wait, as a slow
            // disk sync can. That says nothing about a stop; the next check
            // reads the store again.
            Err(Error::Busy) => false,
            Err(error) => {
                self.fail(error.to_string());
                true
            }
        }
    }

    pub async fn wait_cancelled(&self) {
        while !self.cancelled() {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    /// The messages the person sent this running turn since the last
    /// call ([`super::steer`]), each recorded in the trace as theirs.
    /// A message the trace cannot keep is still returned: the run reads it.
    pub fn steering(&self) -> Vec<String> {
        let messages = super::steer::take(&self.owner.dir, &self.task.task_id);
        for text in &messages {
            let _ =
                self.append(&Step::said(Source::User, text).noting("steer", json!({"read": true})));
        }
        messages
    }

    pub fn append(&self, step: &Step) -> Result<(), Error> {
        let mut bound = step.clone();
        if let Some(readings) = bound
            .extensions
            .get_mut("decision_readings")
            .and_then(Value::as_array_mut)
        {
            for reading in readings {
                reading["outcome_key"] =
                    json!(format!("{}-{}", self.task.task_id, self.task.turn()));
            }
        }
        let step = &bound;
        let bytes = serde_json::to_vec(step)
            .map_err(|_| Error::UnsupportedSchema)?
            .len()
            + 256;
        // A record past the transcript's bounds is left out and the run
        // goes on: a long run's builds once filled the transcript, and the
        // run was ended as if the person had stopped it (#10993).
        let total = self.trace_bytes.get().saturating_add(bytes);
        if bytes > STEP_LIMIT
            || total > TRACE_CEILING
            || (total > TRACE_LIMIT && bytes > SMALL_RECORD)
        {
            self.output_incomplete.set(true);
            if !self.evidence_capped.replace(true) {
                let note = Step::said(
                    Source::System,
                    "The transcript reached its size bound, so bulky records are left out from here; the run goes on.",
                )
                .noting(
                    "evidence_capped",
                    json!({"omitted_bytes": bytes, "trace_limit": TRACE_LIMIT}),
                );
                if let Err(error) = self.keep_evidence(|trace| trace.append(&note)) {
                    self.evidence_lost(&error);
                    return Err(error);
                }
            }
            return Ok(());
        }
        if let Err(error) = self.keep_evidence(|trace| trace.append(step)) {
            self.evidence_lost(&error);
            return Err(error);
        }
        self.trace_bytes.set(self.trace_bytes.get() + bytes);
        Ok(())
    }

    /// Writes to the trace, waiting out a full disk for as long as an owner
    /// does ([`owner::wait_out_full_disk`]): the host frees space, and the
    /// trace writer cuts a half-written line back off, so the write can
    /// simply be made again (#10237).
    fn keep_evidence(
        &self,
        mut write: impl FnMut(&mut Log) -> std::io::Result<()>,
    ) -> Result<(), Error> {
        let (wait, step) = self.evidence_wait.get();
        owner::wait_out_full_disk(wait, step, || {
            if let Some(left) = self.full_writes.get().checked_sub(1) {
                self.full_writes.set(left);
                return Err(Error::Io(std::io::ErrorKind::StorageFull.into()));
            }
            write(&mut self.trace.borrow_mut()).map_err(Error::Io)
        })
    }

    /// For tests: the transcript counts as at its soft size bound, as after
    /// a long run's builds (#10993).
    #[doc(hidden)]
    pub fn fill_trace(&self) {
        self.trace_bytes.set(TRACE_LIMIT);
    }

    /// For tests: the next `writes` trace writes fail as on a full disk,
    /// each waited out for at most `wait`.
    #[doc(hidden)]
    pub fn fill_disk(&self, writes: usize, wait: Duration) {
        self.full_writes.set(writes);
        self.evidence_wait.set((wait, Duration::from_millis(1)));
    }

    /// Notes that evidence could not be kept, and why: the run then ends,
    /// and still records its result (#10237).
    fn evidence_lost(&self, error: &Error) {
        self.output_incomplete.set(true);
        if owner::storage_full(error) {
            self.disk_full.set(true);
            self.fail(format!(
                "the disk is full, so task evidence could not be retained: {error}"
            ));
        } else {
            self.fail(format!("task evidence could not be retained: {error}"));
        }
    }

    pub fn effect(&self, kind: &str, arguments: Value) -> Result<usize, Error> {
        if self.cancelled() {
            return Err(Error::InvalidTransition);
        }
        let sequence = self.sequence.get() + 1;
        self.append(
            &Step::said(
                Source::System,
                "Adapter effect intent retained before dispatch.",
            )
            .noting(
                "effect",
                json!({"sequence":sequence,"kind":kind,"arguments":arguments,
                "arguments_digest":atif::digest(&arguments)}),
            ),
        )?;
        self.sequence.set(sequence);
        Ok(sequence)
    }

    pub fn result(&self, sequence: usize, kind: &str, result: Value) -> Result<(), Error> {
        self.append(
            &Step::said(Source::System, "Adapter effect observation retained.").noting(
                "effect_result",
                json!({"sequence":sequence,"kind":kind,"result":result}),
            ),
        )
    }

    /// Reads only singly linked regular files below the admitted workspace.
    pub fn read(&self, requested: &str, cap: usize) -> Result<Option<Vec<u8>>, Error> {
        if cap == 0 || cap > 1024 * 1024 {
            return Err(Error::LimitExceeded);
        }
        let requested = Path::new(requested);
        let path = if requested.is_absolute() {
            requested
                .strip_prefix(self.workspace())
                .map_err(|_| Error::UnsafePath)?
        } else {
            requested
        };
        let sequence = self.effect("read", json!({"path":path,"max_bytes":cap}))?;
        let mut bytes = Vec::new();
        let result = match artifact::confined_file(self.workspace(), path) {
            Ok(file) => {
                if let Err(error) = file.take(cap as u64 + 1).read_to_end(&mut bytes) {
                    self.output_incomplete.set(true);
                    self.fail("an admitted file read failed before a complete observation");
                    self.result(
                        sequence,
                        "read",
                        json!({
                            "status":"read_error", "error":error.to_string(),
                            "partial_bytes":bytes, "partial_digest":digest_bytes(&bytes),
                            "observation_complete":false
                        }),
                    )?;
                    return Err(error.into());
                }
                Some(bytes)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                self.result(sequence, "read", json!({"refused":error.to_string()}))?;
                return Err(Error::UnsafePath);
            }
        };
        let result = result.map(|mut bytes| {
            let truncated = bytes.len() > cap;
            bytes.truncate(cap);
            (bytes, truncated)
        });
        self.result(
            sequence,
            "read",
            json!({"observation":result.as_ref().map(|(bytes,truncated)|
            json!({"bytes":bytes,"digest":digest_bytes(bytes),"truncated":truncated}))}),
        )?;
        Ok(result.map(|(bytes, _)| bytes))
    }

    /// The whole environment of a process inside `boundary` (this run's
    /// command boundary, or one from [`Host::engine_boundary`]): the
    /// system `PATH` (under [`Access::Toolchains`], this computer's tools
    /// first, and its toolchain variables), and `HOME` and the temporary
    /// directory in the boundary's private scratch. Nothing else of this
    /// process's environment is passed.
    ///
    /// # Errors
    /// The boundary owns no scratch.
    pub fn bounded_environment(
        &self,
        boundary: &Boundary,
    ) -> Result<Vec<(std::ffi::OsString, std::ffi::OsString)>, Error> {
        let scratch = boundary.scratch().ok_or(Error::UnsafePath)?;
        let mut variables: Vec<(std::ffi::OsString, std::ffi::OsString)> =
            owner::base_environment()
                .into_iter()
                .map(|(name, value)| (name.into(), value))
                .collect();
        let path = match &self.toolchains {
            Some(toolchains) => command_path(boundary, toolchains),
            None => owner::SYSTEM_PATH.into(),
        };
        let leases = self.policy.lease_environment(Some(&path));
        variables.push(("PATH".into(), path));
        for name in ["HOME", "TMPDIR", "TMP", "TEMP"] {
            variables.push((name.into(), scratch.into()));
        }
        if let Some(toolchains) = &self.toolchains {
            variables.extend(
                toolchains
                    .environment
                    .iter()
                    .map(|(name, value)| (name.into(), value.clone().into())),
            );
            // `xcrun` keeps its lookup cache in the user's own temporary
            // directory, not `TMPDIR`; the scratch holds it.
            if cfg!(target_os = "macos") {
                variables.push(("xcrun_db".into(), scratch.join("xcrun_db").into()));
            }
        }
        if cfg!(windows) {
            variables.push(("USERPROFILE".into(), scratch.into()));
        }
        if let Some(target) = &self.policy.target {
            variables.push(("CARGO_TARGET_DIR".into(), target.as_os_str().to_owned()));
        }
        super::targets::apply_run_leases(&mut variables, leases);
        // A relative local remote resolves against the main checkout, not
        // this worktree (#10333).
        variables.extend(super::local::remote_override_environment(
            self.workspace(),
            0,
        ));
        self.confine(&mut variables);
        Ok(variables)
    }

    /// A boundary for a whole coding agent's own process (Grok Build),
    /// so the agent and every tool it runs are held to this run's access
    /// as the loop's commands are: writes only to the workspace (when the
    /// grant writes it) and a private scratch of its own, the task store
    /// and the common Git directory sealed, reads confined to the
    /// workspace, the system, this computer's toolchains under
    /// [`Access::Toolchains`], and `reads` (the agent's own program), and
    /// no network under [`Access::Boundary`]. The caller holds it until
    /// the process is reaped.
    ///
    /// # Errors
    /// The run has full access or a container (neither has this
    /// boundary), or the boundary cannot be enforced here.
    pub fn engine_boundary(&self, reads: &[PathBuf]) -> Result<Boundary, Error> {
        if self.configuration().access == Access::Full || self.configuration().container.is_some() {
            return Err(Error::InvalidCommand(
                "an engine boundary is for a run under the boundary or this computer's toolchains",
            ));
        }
        self.policy
            .spec(self.toolchains.as_ref(), reads)
            .build()
            .map_err(|_| Error::InvalidCommand("the engine boundary cannot be enforced"))
    }

    /// Commands retain bounded full streams separately from the loop's prompt cut.
    pub async fn command(
        &self,
        script: &str,
        deadline: Duration,
    ) -> Result<CommandObservation, Error> {
        if let Some(profile) = &self.configuration().container {
            return container::command(self, profile, script, deadline).await;
        }

        if script.len() > MAX_COMMAND_BYTES || script.contains('\0') {
            return Err(Error::LimitExceeded);
        }
        if digest_bytes(&std::fs::read(&self.admission.grant.program)?)
            != self.admission.program_digest
        {
            self.fail("the admitted shell changed");
            return Err(Error::InvalidTransition);
        }
        let (arguments, script_variables) = owner::shell_arguments(script)?;
        let login = self.login_environment().await;
        let sequence = self.effect("command", json!({"script":script}))?;
        // Windows programs refuse a verbatim (`\\?\`) working directory.
        let directory = coder_boundary::plain_path(self.workspace());
        let command = match login {
            // Full access: the owner's own shell, with the network and the
            // owner's login environment, and no sandbox but the privacy
            // one: on macOS the places it guards with a privacy prompt
            // (music, photos, documents, other apps' data) and Apple
            // Events are denied, so no command makes macOS ask the owner
            // about Coder (`coder_boundary::privacy`). The checkout the
            // worktree was made from stays unwritten (#10247).
            Some(login) => {
                let mut command =
                    self.private_command(coder_boundary::plain_path(&self.admission.grant.program));
                command
                    .args(&arguments)
                    .current_dir(&directory)
                    .env_clear()
                    .envs(login.variables.iter().map(|(key, value)| (key, value)));
                command.envs(self.guard_environment());
                command.envs(script_variables);
                command
            }
            None => {
                let boundary = self.boundary.as_ref().ok_or(Error::UnsafePath)?;
                let mut command = boundary
                    .command(&self.admission.grant.program, &arguments)
                    .map_err(|_| Error::UnsafePath)?;
                command
                    .current_dir(&directory)
                    .env_clear()
                    .envs(self.bounded_environment(boundary)?)
                    .envs(script_variables);
                command
            }
        };
        let live = {
            let store = Store::open_for_owner(&self.owner.dir)?;
            if store.show(&self.task.task_id)?.status != Status::Running {
                self.stopped.set(true);
                return Err(Error::InvalidTransition);
            }
            self.group_clear.set(false);
            Job::from_command(command)
                .bounded(
                    Limits::within(deadline)
                        .keeping(self.admission.grant.stream_bytes)
                        .memory(Some(self.admission.grant.memory_bytes)),
                )
                .start(Input::Null)
                .map_err(|error| {
                    self.fail(error);
                    Error::InvalidTransition
                })?
        };
        if self
            .append(
                &Step::said(Source::System, "Supervised command started.")
                    .noting("process", json!({"effect":sequence,"pid":live.pid()})),
            )
            .is_err()
        {
            let ended = live.stop().await;
            self.group_clear.set(ended.group_clear);
            return Err(Error::LimitExceeded);
        }
        let mut stdout = Vec::new();
        let ended = loop {
            let delivery = live.take();
            if !delivery.gaps.is_empty() {
                self.output_incomplete.set(true);
            }
            stdout.extend_from_slice(&delivery.bytes);
            if !delivery.is_empty()
                && self.append(&Step::said(Source::System,&String::from_utf8_lossy(&delivery.bytes))
                    .noting("stream",json!({"effect":sequence,"name":"stdout","offset":delivery.offset,
                        "bytes":delivery.bytes,"gaps":delivery.gaps.iter().map(|gap|json!({"offset":gap.offset,"bytes":gap.bytes})).collect::<Vec<_>>()}))).is_err() {
                break live.stop().await;
            }
            if stdout.len() >= 2 * 1024 * 1024 {
                self.output_incomplete.set(true);
                break live.stop().await;
            }
            if live.finished() {
                break live.wait().await;
            }
            if self.cancelled() {
                break live.stop().await;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        };
        stdout.extend_from_slice(&ended.rest.bytes);
        // The supervisor waits only its short grace for a killed group to
        // empty; on a loaded host the group's last processes can take
        // longer to be reaped (#10281), so a group not yet clear is given
        // a few seconds more before the run counts its cleanup unknown.
        let group_clear = ended.group_clear
            || group_settles(ended.group, GROUP_SETTLE, Duration::from_millis(50)).await;
        self.group_clear.set(group_clear);
        self.output_incomplete.set(
            self.output_incomplete.get() || ended.stderr.truncated || !ended.rest.gaps.is_empty(),
        );
        self.result(sequence,"command",json!({"ending":ended.ending.to_string(),"exit":ended.ending.code(),
            "group_clear":group_clear,"requested_stop":ended.requested,"stdout_tail":ended.rest.bytes,
            "stdout_tail_offset":ended.rest.offset,"stdout_bytes":ended.stdout_bytes,"stderr":ended.stderr.text,
            "stderr_bytes":ended.stderr.bytes,"stderr_truncated":ended.stderr.truncated,
            "seconds":ended.elapsed.as_secs_f64(),"memory":format!("{:?}",ended.memory)}))?;
        if !group_clear {
            self.cleanup_unknown.set(true);
            let why = if matches!(ended.ending, supervise::Ending::TimedOut) {
                format!(
                    "a command ran past its {}s deadline and its processes could not be confirmed gone",
                    deadline.as_secs()
                )
            } else {
                "a command's processes could not be confirmed gone".to_owned()
            };
            self.fail(why);
        }
        let mut output = String::from_utf8_lossy(&stdout).into_owned();
        if !ended.stderr.text.is_empty() {
            output.push('\n');
            output.push_str(&ended.stderr.marked());
        }
        Ok(CommandObservation {
            exit: ended.ending.code(),
            timed_out: matches!(ended.ending, supervise::Ending::TimedOut),
            seconds: ended.elapsed.as_secs_f64(),
            output,
            group_clear,
        })
    }

    /// Records what the run cost, engine and Jev, for its result record.
    /// Information only: nothing stops or limits a run on it.
    pub fn cost(&self, cost: owner::Cost) {
        self.cost.set(cost);
    }

    /// Seal the same task journal and trace; the adapter never sets checks passed.
    pub fn finish(self, ending: &str, completed: bool, summary: Value) -> Result<Task, Error> {
        // A full disk, or a command whose processes weren't confirmed
        // gone, ends the run as a failure, never as a stop the person
        // asked for (#10237, #10281); a stop they did ask for still reads
        // as one.
        // So does any other fault of the host's own: evidence it could not
        // keep an hour in read as "Stopped, as you asked" (#10993).
        let stopped = self.stop_asked();
        let summary = if serde_json::to_vec(&summary)
            .map_err(|_| Error::UnsupportedSchema)?
            .len()
            > 1024 * 1024
        {
            self.output_incomplete.set(true);
            self.fail("the final adapter summary exceeded its retained size bound");
            json!({"omitted":true,"digest":atif::digest(&summary)})
        } else {
            summary
        };
        // Reserved headroom preserves the final disposition even when a prior
        // record exceeded the ordinary trace cap. The file remains below 64 MiB.
        // Evidence that cannot be kept, as on a full disk, is noted and the
        // result is still recorded: an owner that exited here left its run
        // to be ended as "owner process ended" (#10237).
        let closing = Step::said(
            Source::System,
            "Repository adapter ended; independent checks are separate.",
        )
        .noting("adapter_summary", summary)
        .noting("host_fault", json!(*self.fault.borrow()));
        if let Err(error) = self.keep_evidence(|trace| trace.append(&closing)) {
            self.evidence_lost(&error);
        }
        let after = Snapshot::observe(self.workspace());
        let (artifact_file, artifact_digest) = match owner::wait_out_full_disk(
            owner::STORAGE_FULL_WAIT,
            Duration::from_secs(2),
            || artifact::retain(&self.owner.dir, &self.before, &after),
        ) {
            Ok((file, digest)) => (Some(file), Some(digest)),
            Err(error) => {
                self.evidence_lost(&error);
                (None, None)
            }
        };
        if let Err(error) = self.keep_evidence(|trace| trace.finish(atif::log::ENDED)) {
            self.evidence_lost(&error);
        }
        let ending = if self.disk_full.get() {
            DISK_FULL
        } else if self.cleanup_unknown.get() && !stopped {
            PROCESS_CLEANUP_UNKNOWN
        } else if !stopped && ending == "cancelled_or_host_refusal" && self.fault.borrow().is_some()
        {
            HOST_FAULT
        } else {
            ending
        };
        let mut result = owner::ResultRecord {
            ending: ending.into(),
            exit_code: Some(if completed && self.fault.borrow().is_none() {
                0
            } else {
                1
            }),
            stop_requested: stopped,
            group_clear: self.group_clear.get(),
            elapsed_ms: self
                .started
                .elapsed()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
            trace_digest: digest_bytes(
                &std::fs::read(self.trace.borrow().path()).unwrap_or_default(),
            ),
            candidate_snapshot: after.is_complete().then(|| after.digest()),
            artifact_file,
            artifact_digest,
            output_incomplete: self.output_incomplete.get(),
            cost_status: "unknown".into(),
            cost_microusd: None,
            engine_microusd: None,
            jev_microusd: None,
            payer: None,
            payer_keys: Vec::new(),
        };
        result.priced(self.cost.get());
        result.paid_by(&model_access::current());
        // A local run's independent check (#10232): list what it runs
        // before the result is recorded, so a reader that sees the result
        // while this owner still holds the task knows a check follows.
        let listed = if completed && self.fault.borrow().is_none() && !stopped {
            self.admission
                .grant
                .requirements
                .as_ref()
                .and_then(|requirements| {
                    super::local_checks::list(
                        requirements,
                        &self.admission.workspace,
                        &self.admission.source_revision,
                        &self.frozen_checks.borrow(),
                        &self.check_environment(),
                    )
                })
        } else {
            None
        };
        let task = self.owner.record(owner::Event::Result { result })?;
        if listed.is_some()
            && task.status == Status::Finished
            && task.execution == Execution::Finished
            && !task.context_superseded()
        {
            return self.owner.record(owner::Event::CheckIntent);
        }
        Ok(task)
    }
}

#[cfg(test)]
mod source_snapshot_tests {
    use super::*;
    use coder_boundary::snapshot::Limits;

    #[test]
    fn a_refusal_says_whether_the_snapshot_was_incomplete_or_changed() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["a", "b", "c"] {
            std::fs::write(dir.path().join(name), name).unwrap();
        }
        let whole = Snapshot::observe(dir.path());
        assert!(whole.is_complete());
        assert_eq!(source_snapshot_refusal(&whole, None), None);
        assert_eq!(source_snapshot_refusal(&whole, Some(&whole.digest())), None);
        assert_eq!(
            source_snapshot_refusal(&whole, Some("other")).as_deref(),
            Some("the workspace changed since its source snapshot was granted")
        );

        let partial = Snapshot::observe_within(dir.path(), Limits::bounded(2, u64::MAX));
        let refusal = source_snapshot_refusal(&partial, None).unwrap();
        assert!(
            refusal.starts_with("the workspace snapshot is incomplete: ")
                && refusal.contains("limit of 2 files and directories"),
            "{refusal}"
        );
        // Incomplete refuses even when the pin would match.
        assert!(source_snapshot_refusal(&partial, Some(&partial.digest())).is_some());
    }
}

#[cfg(all(test, unix))]
mod group_settle_tests {
    use super::*;

    /// #10281: a group not clear when the supervisor gave up is read again
    /// for a while before the run counts its cleanup unknown: a group that
    /// empties counts as clear, one that stays doesn't, and a group with
    /// no identifier is never confirmed.
    #[tokio::test]
    async fn a_group_is_given_time_to_empty_before_cleanup_counts_unknown() {
        let every = Duration::from_millis(10);
        // This test's own process group stays running.
        // SAFETY: getpgrp has no preconditions.
        let own = unsafe { libc::getpgrp() };
        assert!(!group_settles(Some(own), Duration::from_millis(50), every).await);
        assert!(!group_settles(None, Duration::from_millis(50), every).await);
        // A child in its own group that exits within the wait.
        let mut child = {
            use std::os::unix::process::CommandExt;
            std::process::Command::new("sleep")
                .arg("0.2")
                .process_group(0)
                .spawn()
                .unwrap()
        };
        let group = i32::try_from(child.id()).unwrap();
        let reaper = std::thread::spawn(move || child.wait());
        assert!(group_settles(Some(group), Duration::from_secs(5), every).await);
        reaper.join().unwrap().unwrap();
    }
}

#[cfg(test)]
mod slot_tests {
    use super::*;

    #[test]
    fn every_access_that_builds_takes_a_slot() {
        assert!(builds(Access::Full));
        assert!(builds(Access::Toolchains));
        assert!(!builds(Access::Boundary));
    }

    /// A run under this computer's toolchains runs `cargo` through the
    /// lease shim, which may run `openagents` and write the lease table,
    /// at the run's priority (#10757). A stand-in `openagents` records the
    /// lease it was asked for.
    #[cfg(unix)]
    #[test]
    fn a_toolchains_run_leases_each_cargo_build_through_the_shim() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().canonicalize().unwrap();
        let workspace = dir_path.join("workspace");
        let store = dir_path.join("tasks");
        let git = dir_path.join("git");
        let tools = dir_path.join("tools");
        let root = dir_path.join("leases");
        let shims = dir_path.join("bin/lease-shims");
        for path in [&workspace, &store, &git, &tools, &root] {
            std::fs::create_dir_all(path).unwrap();
        }
        coder_lease::shim::install(&shims).unwrap();
        let script = |path: &Path, body: &str| {
            std::fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        };
        script(&tools.join("cargo"), "echo \"real cargo $*\"");
        let bin = dir_path.join("bin/openagents");
        script(
            &bin,
            "echo \"$OPENAGENTS_LEASE_PRIORITY $1 $2 $3\" > \"$OPENAGENTS_LEASE_ROOT/asked\"; shift 4; OPENAGENTS_LEASES=build exec \"$@\"",
        );
        let policy = CommandPolicy {
            workspace: workspace.clone(),
            write_workspace: true,
            program: PathBuf::from("/bin/sh"),
            store,
            git_directory: git,
            access: Access::Toolchains,
            target: None,
            leases: Some(super::super::targets::RunLeases {
                shims: shims.clone(),
                bin: Some(bin),
                root: root.clone(),
            }),
            lease_priority: coder_lease::Priority::Owner,
        };
        // Where the boundary cannot be built here, there is nothing to run;
        // macOS always has `sandbox-exec`.
        let boundary = match policy.spec(None, std::slice::from_ref(&tools)).build() {
            Ok(boundary) => boundary,
            Err(error) if cfg!(target_os = "macos") => panic!("{error:?}"),
            Err(_) => return,
        };
        assert!(
            boundary.writable().contains(&root),
            "{:?}",
            boundary.writable()
        );
        let path = std::ffi::OsString::from(format!("{}:/usr/bin:/bin", tools.display()));
        let environment = policy.lease_environment(Some(&path));
        let output = boundary
            .command("/bin/sh", ["-c", "cargo test -p x; cargo fmt"])
            .unwrap()
            .env_clear()
            .envs(environment)
            .env("HOME", &workspace)
            .current_dir(&workspace)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("real cargo test -p x"), "{stdout}");
        assert!(stdout.contains("real cargo fmt"), "{stdout}");
        assert_eq!(
            std::fs::read_to_string(root.join("asked")).unwrap().trim(),
            "owner lease build --keep-target-dir"
        );
        // A full-access run gets the same variables without the boundary;
        // a run under the bare boundary builds nothing and gets none.
        let bare = CommandPolicy {
            access: Access::Boundary,
            ..policy
        };
        assert!(bare.lease_environment(Some(&path)).is_empty());
    }

    #[test]
    fn a_toolchains_run_may_write_its_build_slot() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        let store = dir.path().join("tasks");
        let git = dir.path().join("git");
        let slot = dir.path().join("targets").join("project-slot-0");
        for path in [&workspace, &store, &git, &slot] {
            std::fs::create_dir_all(path).unwrap();
        }
        let policy = CommandPolicy {
            workspace: workspace.clone(),
            write_workspace: true,
            program: PathBuf::from("/bin/sh"),
            store,
            git_directory: git,
            access: Access::Toolchains,
            target: Some(slot.clone()),
            leases: None,
            lease_priority: coder_lease::Priority::Owner,
        };
        // Where the boundary cannot be built here, there is nothing to read.
        let Ok(boundary) = policy.spec(None, &[]).build() else {
            return;
        };
        let slot = slot.canonicalize().unwrap();
        assert!(
            boundary.writable().contains(&slot),
            "{:?}",
            boundary.writable()
        );
    }
}
