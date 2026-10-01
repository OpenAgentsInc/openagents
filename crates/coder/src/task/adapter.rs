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
const TRACE_LIMIT: usize = 48 * 1024 * 1024;
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
    pub max_steps: usize,
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
        if self.schema != CONFIG_SCHEMA
            || !matches!(
                self.provider.as_str(),
                "codex" | "claude" | "devin" | "opencode" | "grok" | "synthetic"
            )
            || !identifier(&self.model, true)
            || !identifier(&self.decision_model, true)
            || self
                .effort
                .as_deref()
                .is_some_and(|effort| !matches!(effort, "low" | "medium" | "high" | "xhigh"))
            || !(1..=128).contains(&self.max_steps)
            || self.acceptance
            || self.route != "never"
            || !matches!(self.knowledge.as_str(), "off" | "frozen-context")
            || self.dollar_limit_micros.is_some()
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

/// Full bounded process observation; prompt summaries are a caller's projection.
#[derive(Debug)]
pub struct CommandObservation {
    pub exit: Option<i32>,
    pub timed_out: bool,
    pub seconds: f64,
    pub output: String,
    pub group_clear: bool,
}

/// A single task owner. It is deliberately neither serializable nor cloneable.
pub struct Host {
    owner: owner::Owner,
    task: Task,
    admission: owner::Admission,
    before: Snapshot,
    /// The command boundary, for a run under it; a full-access run has
    /// none, since its commands run as the owner with no sandbox.
    boundary: Option<Boundary>,
    /// This computer's toolchains, for a run with [`Access::Toolchains`]:
    /// what its boundary reads, and its commands' `PATH` and variables.
    toolchains: Option<coder_boundary::Toolchains>,
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
    fault: RefCell<Option<String>>,
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
            let owner = owner::Owner::acquire(&store, &grant.task_id)?;
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
        if !before.is_complete()
            || grant
                .expected_source_snapshot
                .as_ref()
                .is_some_and(|pin| pin != &before.digest())
        {
            return Err(Error::InvalidCommand(
                "the granted source snapshot is unavailable or changed",
            ));
        }
        let spec = if grant.write_workspace {
            Boundary::writing(&workspace)
        } else {
            Boundary::readonly()
        };
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
        let boundary =
            if configuration.access != Access::Full || cfg!(unix) {
                let mut spec = spec
                    .readable(&workspace)
                    .readable(&program)
                    .sealed(&owner.dir)
                    .sealed(&git_directory)
                    .owned_scratch_under(std::env::temp_dir());
                for read in toolchains.iter().flat_map(|toolchains| &toolchains.reads) {
                    spec = spec.readable(&read.path);
                }
                // Git in the worktree reads the common Git directory; it stays
                // sealed against writes.
                if toolchains.is_some() {
                    spec = spec.readable(&git_directory);
                }
                if configuration.access != Access::Toolchains {
                    spec = spec.offline();
                }
                Some(spec.build().map_err(|_| {
                    Error::InvalidCommand("the repository boundary cannot be enforced")
                })?)
            } else {
                None
            };
        if let Some(container) = &configuration.container {
            container.admit(&workspace, &owner.dir).await?;
        }
        let context = checks::Context::capture(&task, &workspace, grant.requirements.as_ref())?;
        let controller = std::env::current_exe()?.canonicalize()?;
        let controller_digest = digest_bytes(&std::fs::read(&controller)?);
        if configuration
            .expected_controller_digest
            .as_ref()
            .is_some_and(|expected| expected != &controller_digest)
        {
            return Err(Error::InvalidCommand(
                "the repository controller differs from its grant",
            ));
        }
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
            .noting("controller",json!({"path":controller,"digest":controller_digest,"version":env!("CARGO_PKG_VERSION")}))
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
            .noting("toolchains", json!(toolchains)),
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
        let trace_bytes = std::fs::metadata(trace.path())?.len() as usize;
        Ok(Self {
            owner,
            task,
            admission,
            before,
            boundary,
            toolchains,
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
            fault: RefCell::new(None),
        })
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
                let environment = match reading.await {
                    Ok(environment) => environment,
                    // The reader failed; read the shell again here.
                    Err(_) => login::capture().await,
                };
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

    pub fn wall_seconds(&self) -> u64 {
        self.admission.grant.wall_seconds
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
        if self.started.elapsed() >= Duration::from_secs(self.wall_seconds()) {
            self.stopped.set(true);
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
            // reads the store again, and the wall deadline still applies.
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

    pub fn append(&self, step: &Step) -> Result<(), Error> {
        let bytes = serde_json::to_vec(step)
            .map_err(|_| Error::UnsupportedSchema)?
            .len()
            + 256;
        if bytes > STEP_LIMIT || self.trace_bytes.get().saturating_add(bytes) > TRACE_LIMIT {
            self.output_incomplete.set(true);
            self.fail("the adapter evidence limit was reached; the attempted record was omitted");
            return Err(Error::LimitExceeded);
        }
        if let Err(error) = self.trace.borrow_mut().append(step) {
            self.fail("task evidence could not be retained");
            return Err(error.into());
        }
        self.trace_bytes.set(self.trace_bytes.get() + bytes);
        Ok(())
    }

    pub fn effect(&self, kind: &str, arguments: Value) -> Result<usize, Error> {
        if self.cancelled() {
            return Err(Error::InvalidTransition);
        }
        if self.sequence.get() >= self.configuration().max_steps * 32 {
            self.fail("repository effect bound reached");
            return Err(Error::LimitExceeded);
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
            // Full access: the owner's own shell, with no sandbox, the
            // network, and the owner's login environment.
            Some(login) => {
                let mut command = std::process::Command::new(coder_boundary::plain_path(
                    &self.admission.grant.program,
                ));
                command
                    .args(&arguments)
                    .current_dir(&directory)
                    .env_clear()
                    .envs(login.variables.iter().map(|(key, value)| (key, value)))
                    .envs(script_variables);
                command
            }
            None => {
                let boundary = self.boundary.as_ref().ok_or(Error::UnsafePath)?;
                let mut command = boundary
                    .command(&self.admission.grant.program, &arguments)
                    .map_err(|_| Error::UnsafePath)?;
                let scratch = self.scratch()?;
                command
                    .current_dir(&directory)
                    .env_clear()
                    .envs(owner::base_environment())
                    .env("PATH", owner::SYSTEM_PATH)
                    .env("HOME", scratch)
                    .env("TMPDIR", scratch)
                    .env("TMP", scratch)
                    .env("TEMP", scratch);
                if let Some(toolchains) = &self.toolchains {
                    command
                        .env("PATH", command_path(boundary, toolchains))
                        .envs(toolchains.environment.iter().map(|(k, v)| (k, v)));
                    // `xcrun` keeps its lookup cache in the user's own
                    // temporary directory, not `TMPDIR`; the scratch holds it.
                    if cfg!(target_os = "macos") {
                        command.env("xcrun_db", scratch.join("xcrun_db"));
                    }
                }
                command.envs(script_variables);
                if cfg!(windows) {
                    command.env("USERPROFILE", scratch);
                }
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
                    Limits::within(
                        deadline.min(
                            Duration::from_secs(self.wall_seconds())
                                .saturating_sub(self.started.elapsed()),
                        ),
                    )
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
        self.group_clear.set(ended.group_clear);
        self.output_incomplete.set(
            self.output_incomplete.get() || ended.stderr.truncated || !ended.rest.gaps.is_empty(),
        );
        self.result(sequence,"command",json!({"ending":ended.ending.to_string(),"exit":ended.ending.code(),
            "group_clear":ended.group_clear,"requested_stop":ended.requested,"stdout_tail":ended.rest.bytes,
            "stdout_tail_offset":ended.rest.offset,"stdout_bytes":ended.stdout_bytes,"stderr":ended.stderr.text,
            "stderr_bytes":ended.stderr.bytes,"stderr_truncated":ended.stderr.truncated,
            "seconds":ended.elapsed.as_secs_f64(),"memory":format!("{:?}",ended.memory)}))?;
        if !ended.group_clear {
            self.fail("supervised process cleanup is unknown");
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
            group_clear: ended.group_clear,
        })
    }

    /// Seal the same task journal and trace; the adapter never sets checks passed.
    pub fn finish(self, ending: &str, completed: bool, summary: Value) -> Result<Task, Error> {
        let stopped = self.cancelled();
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
        self.trace.borrow_mut().append(
            &Step::said(
                Source::System,
                "Repository adapter ended; independent checks are separate.",
            )
            .noting("adapter_summary", summary)
            .noting("host_fault", json!(*self.fault.borrow())),
        )?;
        let after = Snapshot::observe(self.workspace());
        let (artifact_file, artifact_digest) =
            artifact::retain(&self.owner.dir, &self.before, &after)?;
        self.trace.borrow_mut().finish(atif::log::ENDED)?;
        let result = owner::ResultRecord {
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
            trace_digest: digest_bytes(&std::fs::read(self.trace.borrow().path())?),
            candidate_snapshot: after.is_complete().then(|| after.digest()),
            artifact_file: Some(artifact_file),
            artifact_digest: Some(artifact_digest),
            output_incomplete: self.output_incomplete.get(),
            cost_status: "unknown".into(),
        };
        self.owner.record(owner::Event::Result { result })
    }
}
