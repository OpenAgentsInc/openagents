//! Coder's host for the one `delegate` tool (#11511,
//! [`coder_sdk::delegation`]): which harnesses this computer can hand work
//! to, checked when a call arrives, and how a child runs on each.
//!
//! - `claude-code`: Claude Code through the Agent SDK, under the
//!   session's hooks (the fence, the floors), on the model the call named.
//! - `codex`: Codex on app-server under the same hooks, or in its
//!   read-only sandbox for a read-only task.
//! - `coder`: Coder's own loop (the Microcoder plugin), whose floors are
//!   its hook.
//! - `opencode`: the OpenCode harness (`opencode run` with Coder's
//!   plugin hook); a read-only task goes to its ACP agent.
//! - Every other installed ACP agent: the Coder SDK's ACP backend
//!   (#11508, [`crate::bundled_runtime::acp_briefed`]), the same one ACP
//!   Subagents run on, with the agent's gate from its tier
//!   ([`coder_sdk::acp_backend`]): OpenCode's ACP agent asks before every
//!   tool, Grok Build and Devin run Coder's bridge from their own hooks,
//!   and Cursor and the rest are emulated, the steps they ask about
//!   checked and the rest said plainly before the run (#11534).
//!
//! A child that may write gets a fresh worktree on its own branch (the
//! fence keeps it out of the shared checkout); a read-only child works in
//! the folder with each engine held to reading. Its events go to the
//! parent's chat as a delegation and to its transcript on disk; the parent
//! gets the short [`Report`]. A background child runs on the agent tray.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use coder_sdk::delegation::{
    Authority, Child, Delegator, Host, Lineage, Outcome, Report, Request, Target,
};
use futures::future::BoxFuture;
use serde_json::{Value, json};

use crate::bundled_runtime::{AgentTransport, RuntimeEvent};
use crate::plugin_tools::{ExecutionSettings, GenerationProvider};

/// Coder's own loop, as `delegate` names it.
pub const CODER: &str = "coder";
/// Claude Code, as `delegate` names it.
pub const CLAUDE: &str = "claude-code";
/// Codex, as `delegate` names it.
pub const CODEX: &str = "codex";
/// OpenCode, as `delegate` names it.
pub const OPENCODE: &str = "opencode";

/// One harness a child can run on here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Available {
    pub id: String,
    pub name: String,
    /// The models a call may name; empty takes none, and `None` takes any
    /// (an agent that names its own, such as Devin).
    pub models: Option<Vec<String>>,
}

/// The harnesses this chat can delegate to now, in the order `auto`
/// tries them: Claude Code, Codex, Coder's loop, then the other installed
/// agents. Read at call time, never put in the tool's description.
#[must_use]
pub fn available(execution: &ExecutionSettings, provider: bool) -> Vec<Available> {
    available_with(
        execution,
        provider,
        execution.codex && crate::codex::available(),
    )
}

/// [`available`], with whether the Codex plugin's Codex is installed and
/// signed in already read (`codex_ready`).
fn available_with(
    execution: &ExecutionSettings,
    provider: bool,
    codex_ready: bool,
) -> Vec<Available> {
    let mut harnesses = Vec::new();
    if execution.claude
        && (cfg!(test)
            || crate::bundled_runtime::claude_print::agent().is_some()
            || crate::claude::available())
    {
        harnesses.push(Available {
            id: CLAUDE.into(),
            name: "Claude Code".into(),
            models: Some(
                crate::claude::MODELS
                    .iter()
                    .map(|(id, _, _)| (*id).to_owned())
                    .collect(),
            ),
        });
    }
    let agents = || {
        execution
            .agents
            .iter()
            .filter(|agent| execution.acp && agent.enabled && agent.validate().is_ok())
    };
    // Codex is a target when its ACP agent is on, or when the Codex plugin
    // is on and Codex is installed and signed in (#11575): ACP Subagents
    // off must not hide it, as the Claude Code and OpenCode harnesses
    // aren't hidden. The check reads the path and `auth.json`, never runs
    // Codex.
    let codex = agents()
        .find(|agent| agent.id == CODEX)
        .map(|agent| agent.name.clone())
        .or_else(|| codex_ready.then(|| "Codex".to_owned()));
    if let Some(name) = codex {
        harnesses.push(Available {
            id: CODEX.into(),
            name,
            models: Some(
                crate::codex::MODELS
                    .iter()
                    .map(|(id, _, _)| (*id).to_owned())
                    .collect(),
            ),
        });
    }
    // OpenCode as a harness (#11507) when it is installed, on the models
    // its `opencode models` lists; else an enabled OpenCode ACP agent.
    let opencode_agent = agents().any(|agent| agent.id == OPENCODE);
    if crate::opencode_chat::installed() || opencode_agent {
        harnesses.push(Available {
            id: OPENCODE.into(),
            name: "OpenCode".into(),
            models: Some(if crate::opencode_chat::installed() {
                crate::opencode_chat::catalog(&crate::opencode_chat::model(None))
                    .into_iter()
                    .map(|model| model.id)
                    .collect()
            } else {
                Vec::new()
            }),
        });
    }
    if execution.microcoder {
        harnesses.push(Available {
            id: CODER.into(),
            name: "Microcoder".into(),
            models: Some(if provider {
                crate::models::SHORTLIST
                    .iter()
                    .map(|id| (*id).to_owned())
                    .collect()
            } else {
                Vec::new()
            }),
        });
    }
    for agent in agents().filter(|agent| {
        agent.id != CODEX
            && agent.id != CLAUDE
            && agent.id != OPENCODE
            && !crate::fleet::runs_claude(&agent.id)
    }) {
        harnesses.push(Available {
            id: agent.id.clone(),
            name: agent.name.clone(),
            models: if agent.id == "devin-cli" {
                None
            } else {
                Some(Vec::new())
            },
        });
    }
    harnesses
}

/// The host for one session's delegations.
#[derive(Clone)]
pub struct CoderHost {
    execution: ExecutionSettings,
    provider: Option<GenerationProvider>,
    cancel: Arc<AtomicBool>,
    /// The parent chat's events: each child shows there as a delegation.
    events: Option<tokio::sync::mpsc::UnboundedSender<RuntimeEvent>>,
    /// Where a child on an ACP agent writes its routing record (#11535),
    /// taken when the host is made, on the parent's thread.
    routing: Option<crate::routing_record::Recorder>,
}

impl CoderHost {
    #[must_use]
    pub fn new(
        execution: ExecutionSettings,
        provider: Option<GenerationProvider>,
        cancel: Arc<AtomicBool>,
        events: Option<tokio::sync::mpsc::UnboundedSender<RuntimeEvent>>,
    ) -> Self {
        CoderHost {
            execution,
            provider,
            cancel,
            events,
            routing: crate::routing_record::recorder(),
        }
    }
}

impl Host for CoderHost {
    fn target(&self, request: &Request) -> Result<Target, String> {
        let here = available(&self.execution, self.provider.is_some());
        let ids = || {
            here.iter()
                .map(|harness| harness.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let wanted = match request.to.as_str() {
            "auto" => here.first(),
            "coder-loop" | "microcoder" => here.iter().find(|harness| harness.id == CODER),
            "claude" => here.iter().find(|harness| harness.id == CLAUDE),
            to => here.iter().find(|harness| harness.id == to),
        };
        let Some(harness) = wanted else {
            return Err(if here.is_empty() {
                "No agent can take a delegation here: turn one on in /plugins.".to_owned()
            } else {
                format!(
                    "{} is not available here. The agents here are: {}.",
                    request.to,
                    ids()
                )
            });
        };
        if let Some(model) = &request.model
            && let Some(models) = &harness.models
            && !models.contains(model)
        {
            return Err(if models.is_empty() {
                format!("{} runs its own model here; leave model out.", harness.name)
            } else {
                format!(
                    "{} has no model {model}. Its models are: {}.",
                    harness.name,
                    models.join(", ")
                )
            });
        }
        // Microcoder with no model named runs on its own door, Gemini 4
        // Argon through the OpenAgents gateway, when signed in (#11593):
        // its card says so from the start.
        let name = if harness.id == CODER
            && request.model.is_none()
            && self.execution.microcoder_door().is_ok()
        {
            format!(
                "{} ({})",
                harness.name,
                crate::ui::rail_model_name(crate::bundled_runtime::MICROCODER_MODEL)
            )
        } else {
            harness.name.clone()
        };
        Ok(Target {
            harness: harness.id.clone(),
            name,
            model: request.model.clone(),
        })
    }

    fn run(&self, child: Child) -> BoxFuture<'static, Report> {
        let host = self.clone();
        // Coder's engine calls hold values that stay on one thread: the
        // child runs on a thread of its own and the call waits for it.
        let (answer, report) = tokio::sync::oneshot::channel();
        let fallback = Report::for_child(&child, Outcome::Failed);
        let lost = fallback.clone();
        std::thread::spawn(move || {
            let report = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime.block_on(async {
                    let events = host.events.clone();
                    let id = child.lineage.id.clone();
                    let name = label(&child.target);
                    let task = coder_sdk::delegation::clip(child.request.task.trim(), 200);
                    let mut emit = |event: RuntimeEvent| {
                        if let Some(events) = &events {
                            let _ = events.send(RuntimeEvent::Delegation {
                                id: id.clone(),
                                name: name.clone(),
                                task: task.clone(),
                                event: Box::new(event),
                            });
                        }
                    };
                    // The card opens running and closes with the child's
                    // own report: without the closing event the card stays
                    // running until the turn ends, which marked a finished
                    // child "The turn ended before this agent reported
                    // back" (#11575).
                    emit(card_event(&child, None));
                    let record = record_start(host.routing.as_ref(), &host.execution, &child);
                    let report = run_child(
                        &host.execution,
                        host.provider.clone(),
                        &child,
                        &host.cancel,
                        &mut emit,
                    )
                    .await;
                    record_end(record, &report);
                    emit(card_event(&child, Some(&report)));
                    report
                }),
                Err(error) => fallback.failed(&format!("The child could not start: {error}")),
            };
            let _ = answer.send(report);
        });
        Box::pin(async move {
            report
                .await
                .unwrap_or_else(|_| lost.failed("The child stopped without a report."))
        })
    }

    fn start(&self, child: Child) -> Result<Report, String> {
        let Some(fleet) = self.execution.fleet.clone() else {
            return Err("This session has no agent tray, so a child can't run in the background here; leave background out to wait for it.".into());
        };
        let spec = agent_fleet::Spec {
            name: Some(child.lineage.id.replace('.', "-")),
            engine: child.target.harness.clone(),
            task: child.request.task.clone(),
            place: String::new(),
            parent_session: fleet.parent_session.clone(),
            worktree: None,
            branch: None,
        };
        let control = fleet.fleet.start(spec)?;
        let execution = self.execution.clone();
        let provider = self.provider.clone();
        let running = child.clone();
        let routing = self.routing.clone();
        agent_fleet::spawn(control, move |control| {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return agent_fleet::Outcome::Failed("The agent could not start.".into());
            };
            let cancel = control.cancel_flag();
            control.set_transcript(running.transcript());
            let record = record_start(routing.as_ref(), &execution, &running);
            let report = runtime.block_on(async {
                let mut emit = |event: RuntimeEvent| control.event(event);
                run_child(&execution, provider, &running, &cancel, &mut emit).await
            });
            record_end(record, &report);
            if let Some(worktree) = &report.worktree {
                control.set_checkout(Some(worktree.clone()), report.branch.clone());
            }
            if control.stopped() {
                return agent_fleet::Outcome::Stopped;
            }
            match report.outcome {
                Outcome::Failed => agent_fleet::Outcome::Failed(
                    report
                        .error
                        .clone()
                        .unwrap_or_else(|| report.summary.clone()),
                ),
                _ => agent_fleet::Outcome::Done(report.to_value().to_string()),
            }
        })
        .map_err(|error| format!("The agent could not start: {error}"))?;
        Ok(Report::for_child(&child, Outcome::Running)
            .summarized("Started in the background; it reports in the agent tray."))
    }
}

/// A child on an ACP agent's routing record (#11535): a `delegation` line
/// naming the agent and the version it last reported, hosted by Coder,
/// chosen by the parent's call; [`record_end`] adds its outcome. `None`
/// for a child on a harness, or with records off.
fn record_start(
    routing: Option<&crate::routing_record::Recorder>,
    execution: &ExecutionSettings,
    child: &Child,
) -> Option<(crate::routing_record::Recorder, crate::routing_record::Open)> {
    use crate::routing_record::{Choice, Features, Inputs, Kind, Rule};
    let routing = routing?;
    let agent = execution
        .agents
        .iter()
        .find(|agent| agent.id == child.target.harness && agent.transport == AgentTransport::Acp)?;
    let candidates = crate::harness_plugin::Candidates {
        admitted: Vec::new(),
        excluded: Vec::new(),
    };
    let choice = Choice {
        chat: Some(child.lineage.id.clone()),
        turn: child.lineage.depth,
        kind: Kind::Delegation,
        candidates: &candidates,
        inputs: Inputs {
            features: Features {
                local: true,
                images: 0,
                prompt_chars: u32::try_from(child.request.task.chars().count()).unwrap_or(u32::MAX),
                pinned_model: child.target.model.is_some(),
            },
            ..Inputs::default()
        },
        harness: crate::harness_plugin::Harness::CoderLoop,
        agent: Some(crate::harness_plugin::Agent {
            id: agent.id.clone(),
            version: crate::routing_record::agent_version(&agent.id),
        }),
        rule: Rule::Delegated,
        payer: crate::harness_plugin::Payer::AgentLogin,
        model: None,
    };
    let open = routing.chose(&choice, 0, 0.0);
    Some((routing.clone(), open))
}

/// The outcome of a child [`record_start`] recorded: how it ended, its
/// checks, and its error, scrubbed by the record.
fn record_end(
    record: Option<(crate::routing_record::Recorder, crate::routing_record::Open)>,
    report: &Report,
) {
    use crate::routing_record::Ended;
    let Some((routing, open)) = record else {
        return;
    };
    routing.ended(
        &open,
        crate::routing_record::Outcome {
            ended: Some(match report.outcome {
                Outcome::Done | Outcome::Running => Ended::Done,
                Outcome::Failed => Ended::Error,
                Outcome::Stopped => Ended::Stopped,
            }),
            checks_passed: (!report.checks.is_empty())
                .then(|| report.checks.iter().all(|check| check.passed)),
            model: report.model.clone(),
            error: report.error.clone(),
            ..crate::routing_record::Outcome::default()
        },
    );
}

/// The event that opens (`report` none) or closes a child's card in the
/// parent's chat: the `delegate` tool, running or with the child's report.
pub(crate) fn card_event(child: &Child, report: Option<&Report>) -> RuntimeEvent {
    RuntimeEvent::Tool {
        name: coder_sdk::delegation::TOOL.to_owned(),
        input: json!({"to": child.target.harness, "read_only": child.request.read_only}),
        output: report.map_or(Value::Null, Report::to_value),
        running: report.is_none(),
    }
}

/// The name a child shows: its harness, with the model when the call
/// named one.
pub(crate) fn label(target: &Target) -> String {
    match &target.model {
        Some(model) => format!("{} ({model})", target.name),
        None => target.name.clone(),
    }
}

/// The delegator for a session on `harness` (the parent): its place in
/// the tree from its environment, its authority, and this host.
#[must_use]
pub fn delegator(
    execution: &ExecutionSettings,
    harness: &str,
    provider: Option<GenerationProvider>,
    cancel: Arc<AtomicBool>,
    events: Option<tokio::sync::mpsc::UnboundedSender<RuntimeEvent>>,
) -> Delegator {
    let get = |name: &str| std::env::var(name).ok();
    let run = format!(
        "run-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis())
            .unwrap_or_default()
    );
    let lineage = Lineage::from_env(&get, harness, &run);
    // A gated chat asks before it writes; its children work read-only.
    let root = Authority {
        write: !crate::approval::gated(),
        ..Authority::default()
    };
    let authority = Authority::from_env(&get, root);
    Delegator::new(
        Arc::new(CoderHost::new(execution.clone(), provider, cancel, events)),
        lineage,
        authority,
        &runs_dir(execution),
    )
}

/// The variable naming where children's run folders and the delegation
/// records go; `--state DIR` sets it to `DIR/delegations`.
pub const DIR_ENV: &str = "CODER_DELEGATIONS_DIR";

/// Where children's run folders go: [`DIR_ENV`], else
/// `~/.openagents/coder/delegations`, or under the working folder's
/// `.coder` when there is no home.
#[must_use]
pub fn runs_dir(execution: &ExecutionSettings) -> PathBuf {
    if let Some(dir) = std::env::var_os(DIR_ENV).filter(|dir| !dir.is_empty()) {
        return PathBuf::from(dir);
    }
    let home = if cfg!(test) {
        None
    } else {
        crate::home::data_dir()
    };
    home.unwrap_or_else(|| execution.cwd.join(".coder"))
        .join("delegations")
}

/// Runs one `delegate` call for a session on `harness`, drawing the
/// child's events through `emit` while it waits: the native Coder loop
/// tool and the tool server's tool both come here.
///
/// # Errors
/// The call was refused before a child started.
pub async fn execute(
    execution: &ExecutionSettings,
    harness: &str,
    arguments: Value,
    provider: Option<GenerationProvider>,
    cancel: &Arc<AtomicBool>,
    emit: &mut (dyn FnMut(RuntimeEvent) + Send),
) -> Result<Value, String> {
    let (sender, mut events) = tokio::sync::mpsc::unbounded_channel();
    let delegator = delegator(execution, harness, provider, cancel.clone(), Some(sender));
    let call = delegator.delegate(arguments);
    tokio::pin!(call);
    let result = loop {
        tokio::select! {
            result = &mut call => break result,
            Some(event) = events.recv() => emit(event),
        }
    };
    while let Ok(event) = events.try_recv() {
        emit(event);
    }
    result.map(|report| report.to_value())
}

/// Runs `child` to its end and reports.
pub async fn run_child(
    execution: &ExecutionSettings,
    provider: Option<GenerationProvider>,
    child: &Child,
    cancel: &Arc<AtomicBool>,
    emit: &mut (dyn FnMut(RuntimeEvent) + Send),
) -> Report {
    let report = Report::for_child(child, Outcome::Done);
    let _ = std::fs::create_dir_all(&child.run_dir);
    // Its own worktree and branch, unless it only reads.
    let checkout = if child.request.read_only {
        None
    } else if !in_git(&execution.cwd) {
        return report.failed("A child that changes files gets its own worktree, and this folder is not in a Git repository. Ask again with read_only.");
    } else {
        let name = format!("delegate-{}", child.lineage.id.replace('.', "-"));
        let branch = format!("delegate/{name}");
        let start = child.start.as_deref().unwrap_or("HEAD");
        match coder::branch_checkout::add_at(&execution.cwd, &name, &branch, start).await {
            Ok(checkout) => Some(checkout),
            Err(error) => return report.failed(&error),
        }
    };
    let cwd = checkout
        .as_ref()
        .map_or_else(|| execution.cwd.clone(), |checkout| checkout.path.clone());
    // Its process and everything it starts know where it stands.
    let before = crate::bundled_runtime::child_env();
    let mut vars = before.clone();
    for (name, value) in child.env() {
        vars.retain(|(existing, _)| existing.to_string_lossy() != name);
        vars.push((name.into(), value.into()));
    }
    crate::bundled_runtime::set_child_env(vars);
    // Who ran it, first (#11593): the transcript, its export and the
    // card name the same agent and model.
    let mut transcript = vec![
        json!({"role": "agent", "harness": child.target.harness, "name": label(&child.target)}),
        json!({"role": "user", "text": child.briefing}),
    ];
    let mut ran_model: Option<String> = None;
    let mut record = |event: RuntimeEvent| {
        if let RuntimeEvent::Model(model) = &event
            && !model.is_empty()
        {
            ran_model = Some(model.clone());
        }
        if let Some(line) = transcript_line(&event) {
            transcript.push(line);
        }
        emit(event);
    };
    let (result, ran_on) = engine(execution, provider, child, &cwd, cancel, &mut record).await;
    crate::bundled_runtime::set_child_env(before);
    let reply = match &result {
        Ok(reply) => reply.clone(),
        Err(error) => String::new() + error,
    };
    transcript.push(json!({"role": "assistant", "text": reply, "error": result.is_err()}));
    write_transcript(&child.transcript(), &transcript);
    let files = checkout.as_ref().map_or_else(Vec::new, |checkout| {
        coder_sdk::delegation::changed_files(&checkout.path, Some(&checkout.base))
    });
    let (summary, checks, done) = coder_sdk::delegation::closing(&reply);
    let mut report = report
        .summarized(&summary)
        .with_checks(checks)
        .with_files(files);
    // The model that answered last, as the child's own events named it
    // (#11593), over the one the call asked for.
    if let Some(model) = ran_model {
        report.model = Some(model);
    }
    // The model the child actually ran on, and why, when Coder moved it
    // off the configured one (#11575).
    if let Some(fallback) = ran_on {
        report.model = Some(fallback.to.clone());
        report.summary = coder_sdk::delegation::clip(
            &format!("{} {}", fallback.said(), report.summary)
                .trim()
                .to_owned(),
            coder_sdk::delegation::SUMMARY_MAX,
        );
    }
    if let Err(error) = result {
        report = report.failed(&error);
    } else if reply.trim().is_empty() && report.changed_files.is_empty() {
        // A child that ended saying and changing nothing did not do the
        // task (#11575: OpenCode stopped silently after a refused step and
        // was reported done).
        let failed = format!("{} ended without an answer.", child.target.name);
        let said = format!("{} {failed}", report.summary).trim().to_owned();
        report = report.failed(&said);
    } else if done == Some(false) {
        report.outcome = Outcome::Failed;
    }
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        report.outcome = Outcome::Stopped;
    }
    if let Some(checkout) = checkout {
        // A worktree with nothing in it goes, with its branch; one with
        // work stays for the parent to review.
        if report.changed_files.is_empty() {
            let _ = coder::branch_checkout::remove(&checkout, true).await;
        } else {
            report.worktree = Some(checkout.path.clone());
            report.branch = Some(checkout.branch.clone());
        }
    }
    report
}

/// An OpenCode child moved off the model its configuration names onto
/// OpenCode's free default (#11575).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Fallback {
    /// The configured model that failed.
    pub from: String,
    /// The free model it ran on instead.
    pub to: String,
}

impl Fallback {
    /// The free model's name as OpenCode's picker shows it ("Cat Jam").
    fn name(&self) -> String {
        crate::opencode_chat::rail_label(&self.to)
            .trim_start_matches("OpenCode (")
            .trim_end_matches(')')
            .to_owned()
    }

    /// One short line for the report, after the run.
    pub(crate) fn said(&self) -> String {
        format!(
            "OpenCode's {} failed, so it ran on OpenCode's free {} instead.",
            self.from,
            self.name()
        )
    }

    /// One short line for the rail, as the second run starts.
    pub(crate) fn retrying(&self) -> String {
        format!(
            "OpenCode's {} failed; trying again on OpenCode's free {}.",
            self.from,
            self.name()
        )
    }
}

/// Whether an OpenCode child failed because the provider behind its model
/// refused: a version it no longer supports ("Update Required"), a model
/// that is gone, or a provider that would not start or sign in. OpenCode's
/// free model runs through OpenCode Zen, so the same task can still run.
#[must_use]
pub(crate) fn opencode_provider_refused(error: &str) -> bool {
    coder_delegate::delegate::opencode_model_gone(error)
        || [
            " refused this version",
            "Update Required",
            "no longer supported",
            "ProviderAuthError",
            "ProviderInitError",
        ]
        .iter()
        .any(|marker| error.contains(marker))
}

/// The model an OpenCode child runs when the call names none: on the
/// `opencode run` path the person's OpenCode pick, else (its ACP agent,
/// which takes a read-only task) what their OpenCode configuration names.
fn opencode_configured(read_only: bool) -> Option<String> {
    if !read_only && crate::opencode_chat::installed() {
        return Some(crate::opencode_chat::model(None));
    }
    #[cfg(test)]
    {
        TEST_OPENCODE_MODEL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
    #[cfg(not(test))]
    acp_client::opencode::configured_model(&|name| std::env::var_os(name))
}

/// A test's stand-in for the model the person's OpenCode configuration
/// names; a unit test never reads the computer's.
#[cfg(test)]
pub(crate) static TEST_OPENCODE_MODEL: std::sync::Mutex<Option<String>> =
    std::sync::Mutex::new(None);

/// The child's reply from its harness, or why it failed, and the model it
/// fell back to when its configured OpenCode model was refused: the same
/// task runs once more on OpenCode's free default, said in the child's
/// chat, and the person's configuration is left as it is (#11575).
async fn engine(
    execution: &ExecutionSettings,
    provider: Option<GenerationProvider>,
    child: &Child,
    cwd: &Path,
    cancel: &Arc<AtomicBool>,
    emit: &mut (dyn FnMut(RuntimeEvent) + Send),
) -> (Result<String, String>, Option<Fallback>) {
    let first = engine_once(execution, provider.clone(), child, cwd, None, cancel, emit).await;
    let Err(error) = &first else {
        return (first, None);
    };
    if child.target.harness != OPENCODE
        || child.target.model.is_some()
        || !opencode_provider_refused(error)
        || cancel.load(std::sync::atomic::Ordering::Relaxed)
    {
        return (first, None);
    }
    let free = crate::opencode_chat::auto_model().to_owned();
    let from = opencode_configured(child.request.read_only).unwrap_or_else(|| "its model".into());
    if from == free {
        return (first, None);
    }
    let fallback = Fallback { from, to: free };
    emit(RuntimeEvent::Tool {
        name: crate::claude_stream::NOTICE.to_owned(),
        input: Value::Null,
        output: json!({"text": fallback.retrying()}),
        running: false,
    });
    emit(RuntimeEvent::Model(fallback.to.clone()));
    let second = engine_once(
        execution,
        provider,
        child,
        cwd,
        Some(&fallback.to),
        cancel,
        emit,
    )
    .await;
    (second, Some(fallback))
}

/// One run of the child on its harness; `opencode_model` runs an OpenCode
/// child on that model instead of its configured one.
async fn engine_once(
    execution: &ExecutionSettings,
    provider: Option<GenerationProvider>,
    child: &Child,
    cwd: &Path,
    opencode_model: Option<&str>,
    cancel: &Arc<AtomicBool>,
    emit: &mut (dyn FnMut(RuntimeEvent) + Send),
) -> Result<String, String> {
    let model = child.target.model.as_deref();
    let read_only = child.request.read_only;
    let task = child.briefing.as_str();
    match child.target.harness.as_str() {
        CLAUDE => crate::claude::run_delegated(task, cwd, model, read_only, cancel, emit)
            .await
            .map(|streamed| streamed.text),
        CODER => {
            let mut execution = execution.clone();
            execution.cwd = cwd.to_owned();
            execution.fleet = None;
            // A model the call named runs on the chat's own door; else
            // Microcoder runs on its own, Gemini 4 Argon through the
            // OpenAgents gateway (#11593), not the parent chat's model.
            let provider = match (provider, model) {
                (Some(mut provider), Some(model)) => {
                    model.clone_into(&mut provider.model);
                    Some(provider)
                }
                _ => None,
            };
            execution
                .execute(
                    "microcoder",
                    json!({"task": task, "read_only": read_only}),
                    provider,
                    cancel,
                    emit,
                )
                .await
                .map(|output| output["reply"].as_str().unwrap_or_default().to_owned())
        }
        // OpenCode as a harness (#11507): `opencode run` under Coder's
        // hooks, on the model the call named or the person's OpenCode pick.
        // A read-only task goes to its ACP agent, which has a plan mode.
        OPENCODE if !read_only && crate::opencode_chat::installed() => {
            let turn = crate::opencode_chat::Turn {
                messages: vec![openrouter::Message::user(task)],
                cwd: cwd.to_owned(),
                session: None,
                last_model: None,
                briefing: None,
                model: model
                    .or(opencode_model)
                    .map_or_else(|| crate::opencode_chat::model(None), str::to_owned),
                tools: false,
            };
            crate::opencode_chat::run(turn, cancel, emit, &mut |_| {})
                .await
                .map(|streamed| streamed.text)
        }
        harness => {
            let installed = execution.agents.iter().find(|agent| agent.id == harness);
            // Codex taken through its plugin has no ACP agent entry: it runs
            // on the native Codex bridge.
            let native;
            let agent = match installed {
                Some(agent) => agent,
                None if harness == CODEX => {
                    native = codex_agent()
                        .ok_or_else(|| format!("{harness} is no longer installed here."))?;
                    &native
                }
                // OpenCode as a harness without its ACP agent listed: the
                // installed program's own ACP agent.
                None if harness == OPENCODE => {
                    native = opencode_agent()
                        .ok_or_else(|| format!("{harness} is no longer installed here."))?;
                    &native
                }
                None => return Err(format!("{harness} is no longer installed here.")),
            };
            let program = agent.program.clone();
            let output = if agent.transport == AgentTransport::CodexCli {
                crate::bundled_runtime::codex_delegated(
                    &program, task, cwd, model, read_only, cancel, emit,
                )
                .await?
            } else if agent.id == OPENCODE
                && let Some(named) = model.or(opencode_model)
            {
                // OpenCode's ACP agent takes its model from the inline
                // configuration, merged under the gate's permission; the
                // person's own configuration file is left alone.
                let before = crate::bundled_runtime::child_env();
                let mut vars = before.clone();
                vars.retain(|(name, _)| name.to_string_lossy() != acp_client::opencode::CONFIG_VAR);
                vars.push((
                    acp_client::opencode::CONFIG_VAR.into(),
                    json!({"model": named}).to_string().into(),
                ));
                crate::bundled_runtime::set_child_env(vars);
                let output = crate::bundled_runtime::acp_briefed(
                    agent, task, cwd, None, read_only, None, cancel, emit,
                )
                .await;
                crate::bundled_runtime::set_child_env(before);
                let mut output = output?;
                if output.get("model").is_none_or(Value::is_null) {
                    output["model"] = json!(named);
                }
                output
            } else {
                crate::bundled_runtime::acp_briefed(
                    agent, task, cwd, model, read_only, None, cancel, emit,
                )
                .await?
            };
            Ok(output["reply"].as_str().unwrap_or_default().to_owned())
        }
    }
}

/// OpenCode's own ACP agent, from the installed program.
fn opencode_agent() -> Option<crate::bundled_runtime::AcpAgent> {
    Some(crate::bundled_runtime::AcpAgent {
        id: OPENCODE.into(),
        name: "OpenCode".into(),
        program: crate::opencode_chat::binary()?,
        transport: AgentTransport::Acp,
        arguments: acp_client::opencode::arguments(),
        mode: None,
        enabled: true,
    })
}

/// Codex on the native bridge, from the program the Codex plugin runs.
fn codex_agent() -> Option<crate::bundled_runtime::AcpAgent> {
    Some(crate::bundled_runtime::AcpAgent {
        id: CODEX.into(),
        name: "Codex".into(),
        program: crate::codex::program()?,
        transport: AgentTransport::CodexCli,
        arguments: Vec::new(),
        mode: None,
        enabled: true,
    })
}

fn in_git(directory: &Path) -> bool {
    directory
        .ancestors()
        .any(|folder| folder.join(".git").exists())
}

/// One transcript line for an engine event: what it said, ran and got.
fn transcript_line(event: &RuntimeEvent) -> Option<Value> {
    match event {
        RuntimeEvent::Text(text) => Some(json!({"role": "assistant_delta", "text": text})),
        RuntimeEvent::Model(model) => Some(json!({"role": "model", "model": model})),
        RuntimeEvent::Tool {
            name,
            input,
            output,
            running,
        } => Some(
            json!({"role": "tool", "name": name, "input": input, "output": output, "running": running}),
        ),
        RuntimeEvent::Delegation {
            id, name, event, ..
        } => transcript_line(event)
            .map(|inner| json!({"role": "delegation", "id": id, "name": name, "event": inner})),
        _ => None,
    }
}

fn write_transcript(path: &Path, lines: &[Value]) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
    let _ = std::fs::write(path, text);
}

#[cfg(test)]
#[path = "delegate_host_tests.rs"]
mod tests;
