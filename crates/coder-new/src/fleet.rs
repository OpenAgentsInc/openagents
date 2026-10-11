//! Background agents in the terminal (#11163).
//!
//! The parent chat (the model, through the `agent` tool, or a person,
//! through `/agent` and `/agents`) starts agents that run at the same time,
//! each in its own git worktree on its own branch, with its own transcript.
//! The chat keeps going. When an agent's run ends, its notice (its report,
//! time, tokens and dollars) joins the parent chat as the next input.
//!
//! The registry and lifecycle live in the shared `agent-fleet` crate so
//! the website (#11164) and the apps (#11165) use the same rows and rules.
//! This module is the terminal's host: it picks the engine, makes the
//! worktree, holds the `worktree/<id>` lease while the agent runs, points
//! its builds at the shared build folders, runs the engine through the
//! same plugin dispatch as a foreground delegation, delivers messages at
//! the end of each step, and writes the agent's ATIF transcript linked to
//! the parent session.
//!
//! No step or time limit applies to an agent. It runs until it finishes
//! or someone stops it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use agent_fleet::{Delivery, Outcome, Spec};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::bundled_runtime::RuntimeEvent;
use crate::live::{Chat, Entry};
use crate::plugin_tools::{ExecutionSettings, GenerationProvider};

/// The terminal's agent list: rows plus the engine events of each agent.
pub type Fleet = agent_fleet::Registry<RuntimeEvent>;

/// Everything a background agent needs from the machine it runs on.
#[derive(Clone)]
pub struct Host {
    pub fleet: Fleet,
    /// The chat that starts agents; their transcripts link to it.
    pub parent_session: Option<String>,
    /// Where transcripts are written: `~/.openagents/coder-new/agents`.
    pub transcripts: Option<PathBuf>,
    /// The lease table, when this machine has one.
    pub leases: Option<PathBuf>,
    /// The shared build folders' root.
    pub target_pool: Option<PathBuf>,
    pub pool_size: u64,
    pub floor_gb: u64,
    pub free_disk: fn(&Path) -> std::io::Result<u64>,
}

impl Host {
    /// This machine's host for `fleet`: transcripts and build folders
    /// under `~/.openagents`, the lease table, and the floor a person set.
    #[must_use]
    pub fn local(fleet: Fleet, parent_session: Option<String>) -> Self {
        let env = |name: &str| std::env::var(name).ok();
        let home = if cfg!(test) {
            None
        } else {
            model_access::store::openagents_dir()
        };
        Self {
            fleet,
            parent_session,
            transcripts: home.as_ref().map(|dir| dir.join("coder-new/agents")),
            leases: if cfg!(test) {
                None
            } else {
                coder_lease::root_from_env().ok()
            },
            target_pool: home.as_ref().map(|dir| dir.join("agent-targets")),
            pool_size: agent_fleet::guard::pool_size(&env),
            floor_gb: agent_fleet::guard::floor_gb(&env),
            free_disk: coder_lease::free_disk,
        }
    }
}

/// The engines an agent can run on in this chat: the enabled local agents
/// and the bundled coding loop.
#[must_use]
pub fn engines(execution: &ExecutionSettings) -> Vec<String> {
    let mut engines: Vec<String> = if execution.acp {
        execution
            .agents
            .iter()
            .filter(|agent| agent.enabled && agent.validate().is_ok())
            .map(|agent| agent.id.clone())
            .collect()
    } else {
        Vec::new()
    };
    // Claude Code in print mode, when this computer has it and no agent
    // with its id is configured.
    if execution.acp
        && !cfg!(test)
        && !execution
            .agents
            .iter()
            .any(|agent| agent.id == crate::bundled_runtime::claude_print::ID)
        && crate::bundled_runtime::claude_print::agent().is_some()
    {
        engines.push(crate::bundled_runtime::claude_print::ID.into());
    }
    if execution.microcoder {
        engines.push("microcoder".into());
    }
    engines
}

/// The four tools the model drives agents with, when an engine is
/// available.
#[must_use]
pub fn tool_definitions(execution: &ExecutionSettings) -> Vec<Value> {
    let engines = engines(execution);
    if engines.is_empty() {
        return Vec::new();
    }
    vec![
        json!({"type":"function","function":{
            "name":"agent",
            "description":"Start a background agent on a task and keep working. Each agent runs at the same time as the others, in its own git worktree on its own new branch, so agents never edit the same files. Start several at once for independent tasks. You get a notice with its report as a new message when it finishes, fails, or is stopped; do not wait or poll for it. With background false it runs in the foreground in this folder and you wait for its result.",
            "parameters":{"type":"object","properties":{
                "engine":{"type":"string","enum":engines},
                "task":{"type":"string","minLength":1,"maxLength":65536,"description":"The whole task with the context it needs: it does not see this chat."},
                "name":{"type":"string","maxLength":48,"description":"A short name such as fix-login; made from the task when left out."},
                "background":{"type":"boolean","description":"Run in the background (the default)."},
                "worktree":{"type":"boolean","description":"Give it its own worktree and branch (the default). False runs it in this folder, shared with you."}
            },"required":["engine","task"],"additionalProperties":false}
        }}),
        json!({"type":"function","function":{
            "name":"agent_list",
            "description":"List background agents with their status, elapsed time, tokens, cost, branch and latest report.",
            "parameters":{"type":"object","properties":{},"additionalProperties":false}
        }}),
        json!({"type":"function","function":{
            "name":"agent_message",
            "description":"Send a message to a background agent by name or id. A running agent reads it when its current step ends; a finished or stopped agent resumes with it in the same worktree.",
            "parameters":{"type":"object","properties":{
                "agent":{"type":"string","minLength":1,"maxLength":64},
                "message":{"type":"string","minLength":1,"maxLength":65536}
            },"required":["agent","message"],"additionalProperties":false}
        }}),
        json!({"type":"function","function":{
            "name":"agent_stop",
            "description":"Stop a running background agent by name or id. Its worktree and branch are kept.",
            "parameters":{"type":"object","properties":{
                "agent":{"type":"string","minLength":1,"maxLength":64}
            },"required":["agent"],"additionalProperties":false}
        }}),
    ]
}

/// What the model reads about agents each turn.
pub const INSTRUCTIONS: &str = "Background agents: the agent tool starts one and returns at once; start several in the same reply to run them in parallel, each in its own worktree and branch. Give each a complete task, since it does not see this chat. Keep talking with the user or end your turn; each agent's report arrives later as a message that starts with \"Background agent\". Use agent_list, agent_message and agent_stop to see, steer or stop them. Never wait or poll in a loop for an agent.\n";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StartArguments {
    pub engine: String,
    pub task: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub background: Option<bool>,
    #[serde(default)]
    pub worktree: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MessageArguments {
    agent: String,
    message: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StopArguments {
    agent: String,
}

/// The plugin tool and its arguments that run `engine` on `task`.
///
/// # Errors
/// The engine is not available in this chat.
pub fn engine_call(
    execution: &ExecutionSettings,
    engine: &str,
    task: &str,
) -> Result<(String, Value), String> {
    if engine == "microcoder" {
        return if execution.microcoder {
            Ok(("microcoder".into(), json!({"task":task})))
        } else {
            Err("The Coder loop plugin is turned off. Turn it on in /plugins.".into())
        };
    }
    let configured = execution
        .agents
        .iter()
        .any(|agent| agent.id == engine && agent.enabled);
    let builtin = engine == crate::bundled_runtime::claude_print::ID
        && !execution.agents.iter().any(|agent| agent.id == engine);
    if execution.acp && (configured || builtin) {
        Ok(("acp_subagent".into(), json!({"agent":engine,"task":task})))
    } else {
        Err(format!(
            "{engine} is not available here. Choose one of: {}.",
            engines(execution).join(", ")
        ))
    }
}

/// Runs one of the agent tools.
///
/// # Errors
/// Bad arguments, an unavailable engine, a missing repository, the disk
/// floor, or an unknown agent.
pub fn execute(
    host: &Host,
    execution: &ExecutionSettings,
    name: &str,
    arguments: Value,
    provider: Option<GenerationProvider>,
) -> Result<Value, String> {
    match name {
        "agent" => {
            let args: StartArguments = serde_json::from_value(arguments).map_err(
                |_| "agent takes engine and task, and optionally name, background and worktree.",
            )?;
            let row = start(host, execution, args, provider)?;
            Ok(json!({
                "agent": row.id,
                "name": row.name,
                "engine": row.engine,
                "status": "running",
                "worktree": row.worktree.is_some() || row.branch.is_some(),
                "note": "It is working now. Its report arrives as a new message when it ends; do not wait for it."
            }))
        }
        "agent_list" => Ok(json!({"agents": host.fleet.snapshot_json()})),
        "agent_message" => {
            let args: MessageArguments = serde_json::from_value(arguments)
                .map_err(|_| "agent_message takes agent and message.")?;
            match host.fleet.message(&args.agent, &args.message)? {
                Delivery::Queued(row) => Ok(json!({
                    "agent": row.id, "name": row.name, "delivered": "queued",
                    "note": "It reads the message when its current step ends."
                })),
                Delivery::Resume(row) => {
                    resume(host, execution, &row.id, &args.message, provider)?;
                    Ok(json!({"agent": row.id, "name": row.name, "delivered": "resumed"}))
                }
            }
        }
        "agent_stop" => {
            let args: StopArguments =
                serde_json::from_value(arguments).map_err(|_| "agent_stop takes agent.")?;
            let row = host.fleet.stop(&args.agent)?;
            Ok(json!({"agent": row.id, "name": row.name, "status": "stopping"}))
        }
        _ => Err("This tool is unknown.".into()),
    }
}

/// Whether `directory` is inside a Git checkout.
fn in_git(directory: &Path) -> bool {
    directory
        .ancestors()
        .any(|folder| folder.join(".git").exists())
}

/// Starts a background agent and returns its row at once; the worktree is
/// made and the engine runs on the agent's own thread.
///
/// # Errors
/// An unavailable engine, a folder outside Git (with a worktree), the disk
/// floor, or an unusable name.
pub(crate) fn start(
    host: &Host,
    execution: &ExecutionSettings,
    args: StartArguments,
    provider: Option<GenerationProvider>,
) -> Result<agent_fleet::AgentRow, String> {
    if args.background == Some(false) {
        return Err("Use acp_subagent or microcoder for a foreground run.".into());
    }
    let (tool, _) = engine_call(execution, &args.engine, &args.task)?;
    let worktree = args.worktree.unwrap_or(true);
    if worktree {
        if !in_git(&execution.cwd) {
            return Err("Background agents get their own copy of a Git repository, and this folder is not in one. Pass worktree false to run it here, shared with you.".into());
        }
        agent_fleet::guard::check_disk(&execution.cwd, host.floor_gb, &host.free_disk)?;
    }
    let spec = Spec {
        name: args.name.filter(|name| !name.trim().is_empty()),
        engine: args.engine.clone(),
        task: args.task.clone(),
        place: String::new(),
        parent_session: host.parent_session.clone(),
        worktree: None,
        branch: None,
    };
    let control = host.fleet.start(spec)?;
    let row = control.row().ok_or("The agent could not be registered.")?;
    let run = Run {
        host: host.clone(),
        execution: execution.clone(),
        engine: args.engine,
        tool,
        task: args.task,
        provider,
        worktree,
        checkout: None,
        history: Vec::new(),
    };
    launch(control, run)?;
    Ok(row)
}

/// Resumes an ended agent with `message`, in its worktree when it still
/// has one.
///
/// # Errors
/// No such agent, it is running, or its engine is gone.
pub(crate) fn resume(
    host: &Host,
    execution: &ExecutionSettings,
    key: &str,
    message: &str,
    provider: Option<GenerationProvider>,
) -> Result<(), String> {
    let row = host
        .fleet
        .get(key)
        .ok_or_else(|| format!("There is no agent called {key}."))?;
    let (tool, _) = engine_call(execution, &row.engine, message)?;
    let checkout = match (&row.worktree, &row.branch) {
        (Some(path), Some(branch)) if path.exists() => {
            Some(coder::branch_checkout::BranchCheckout {
                repository: execution.cwd.clone(),
                path: path.clone(),
                branch: branch.clone(),
                base: String::new(),
            })
        }
        _ => None,
    };
    let control = host.fleet.resume(key)?;
    let history = vec![
        format!("Your task was: {}", row.task),
        format!(
            "Your last report was: {}",
            row.report
                .as_deref()
                .or(row.error.as_deref())
                .unwrap_or("(none)")
        ),
    ];
    let run = Run {
        host: host.clone(),
        execution: execution.clone(),
        engine: row.engine.clone(),
        tool,
        task: format!(
            "{}\n\nNew message from the person you work for: {message}",
            history.join("\n\n")
        ),
        provider,
        worktree: row.worktree.is_some() || checkout.is_some(),
        checkout,
        history,
    };
    launch(control, run)
}

struct Run {
    host: Host,
    execution: ExecutionSettings,
    engine: String,
    tool: String,
    task: String,
    provider: Option<GenerationProvider>,
    worktree: bool,
    checkout: Option<coder::branch_checkout::BranchCheckout>,
    history: Vec<String>,
}

fn launch(control: agent_fleet::Control<RuntimeEvent>, run: Run) -> Result<(), String> {
    agent_fleet::spawn(control, move |control| {
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return Outcome::Failed("The agent could not start.".into());
        };
        runtime.block_on(run.drive(control))
    })
    .map(|_| ())
    .map_err(|error| format!("The agent could not start: {error}"))
}

impl Run {
    async fn drive(mut self, control: &agent_fleet::Control<RuntimeEvent>) -> Outcome {
        let id = control.id().to_owned();
        let row = control.row();
        let name = row.as_ref().map_or(id.clone(), |row| row.name.clone());
        // Its own worktree and branch, unless it already has one.
        if self.worktree && self.checkout.is_none() {
            match make_checkout(&self.execution.cwd, &name).await {
                Ok(checkout) => {
                    control
                        .set_checkout(Some(checkout.path.clone()), Some(checkout.branch.clone()));
                    self.checkout = Some(checkout);
                }
                Err(error) => return Outcome::Failed(error),
            }
        }
        let cwd = self
            .checkout
            .as_ref()
            .map_or(self.execution.cwd.clone(), |checkout| checkout.path.clone());
        // Nothing removes the worktree while this lease is held.
        let _lease = match &self.host.leases {
            Some(root) if self.checkout.is_some() => agent_fleet::lease::hold(root, &id).ok(),
            _ => None,
        };
        crate::bundled_runtime::set_child_env(child_env(&self.host, &id));
        let mut execution = self.execution.clone();
        execution.cwd = cwd.clone();
        execution.prompt_inbox = None;
        execution.fleet = None;
        let transcript = self.host.transcripts.as_ref().map(|root| {
            agent_fleet::transcript_path(root, self.host.parent_session.as_deref(), &id)
        });
        if let Some(path) = &transcript {
            control.set_transcript(path.clone());
        }
        let mut chat = Chat::default();
        for line in &self.history {
            chat.entries.push(Entry::User(line.clone()));
        }
        let cancel = control.cancel_flag();
        let mut task = self.task.clone();
        let outcome = loop {
            chat.entries.push(Entry::User(task.clone()));
            let arguments = match self.tool.as_str() {
                "microcoder" => json!({"task":task}),
                _ => json!({"agent":self.engine,"task":task}),
            };
            control.event(RuntimeEvent::Tool {
                name: self.tool.clone(),
                input: arguments.clone(),
                output: Value::Null,
                running: true,
            });
            write_transcript(transcript.as_deref(), &chat, &id, &name, &self, &cwd, true);
            let mut emit = |event: RuntimeEvent| {
                record(&mut chat, &event);
                control.event(event);
            };
            let result = execution
                .execute(
                    &self.tool,
                    arguments,
                    self.provider.clone(),
                    &cancel,
                    &mut emit,
                )
                .await;
            let output = match &result {
                Ok(value) => value.clone(),
                Err(error) => json!({"error":error}),
            };
            let (tokens, cost) = usage(&output);
            control.add_usage(tokens, cost);
            control.event(RuntimeEvent::Tool {
                name: self.tool.clone(),
                input: Value::Null,
                output: output.clone(),
                running: false,
            });
            chat.finish_partial();
            if control.stopped() {
                break Outcome::Stopped;
            }
            let report = match result {
                Ok(value) => value["reply"].as_str().unwrap_or_default().to_owned(),
                Err(error) => break Outcome::Failed(error),
            };
            if !chat
                .entries
                .iter()
                .rev()
                .take_while(|entry| !matches!(entry, Entry::User(_)))
                .any(|entry| matches!(entry, Entry::Assistant { .. }))
                && !report.is_empty()
            {
                chat.entries.push(Entry::Assistant {
                    text: report.clone(),
                    model: output["model"].as_str().map(str::to_owned),
                    elapsed_ms: None,
                });
            }
            // Messages sent while it worked: one more step with them, so
            // a steer is never lost and one finished agent sends one notice.
            let messages = control.take_messages();
            if messages.is_empty() {
                break Outcome::Done(report);
            }
            task = format!(
                "You were working on: {}\n\nYou reported: {}\n\nNew message from the person you work for:\n{}",
                self.task,
                if report.is_empty() {
                    "(nothing yet)"
                } else {
                    &report
                },
                messages.join("\n")
            );
        };
        chat.stop_tools("The agent ended before this tool returned.");
        chat.busy = false;
        // A worktree with no changes is removed, with its branch; one with
        // work stays for review.
        if let Some(checkout) = &self.checkout {
            let unchanged = !checkout.base.is_empty()
                && matches!(coder::branch_checkout::changed(checkout).await, Ok(false));
            if unchanged && coder::branch_checkout::remove(checkout, true).await.is_ok() {
                control.set_checkout(None, None);
            }
        }
        write_transcript(transcript.as_deref(), &chat, &id, &name, &self, &cwd, false);
        outcome
    }
}

async fn make_checkout(
    cwd: &Path,
    name: &str,
) -> Result<coder::branch_checkout::BranchCheckout, String> {
    let mut last = String::new();
    for attempt in 1..=20 {
        let suffix = if attempt == 1 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        match coder::branch_checkout::add(
            cwd,
            &format!("agent-{name}{suffix}"),
            &format!("agent/{name}{suffix}"),
        )
        .await
        {
            Ok(checkout) => return Ok(checkout),
            Err(error) if error.contains("already exists") => last = error,
            Err(error) => return Err(error),
        }
    }
    Err(last)
}

/// The variables an agent's engine and commands get: its shared build
/// folder, and the lease shims when this process turned them on.
fn child_env(host: &Host, id: &str) -> Vec<(OsString, OsString)> {
    let mut vars = coder_lease::shim::delegate_vars_here();
    if let Some(pool) = &host.target_pool {
        let index = id
            .rsplit('-')
            .next()
            .and_then(|n| n.parse::<u64>().ok())
            .unwrap_or(0);
        let slot = agent_fleet::guard::target_slot(pool, index, host.pool_size);
        vars.push(("CARGO_TARGET_DIR".into(), slot.into_os_string()));
    }
    vars.push(("OPENAGENTS_AGENT_ID".into(), id.into()));
    vars
}

/// Tokens and user API charges reported by an engine.
#[must_use]
pub fn usage(output: &Value) -> (u64, Option<f64>) {
    let tokens = output["tokens"]
        .as_u64()
        .or_else(|| output["usage"]["total_tokens"].as_u64())
        .or_else(|| {
            Some(
                output["usage"]["input_tokens"]
                    .as_u64()?
                    .saturating_add(output["usage"]["output_tokens"].as_u64()?),
            )
        })
        .unwrap_or(0);
    let cost = if matches!(
        output["transport"].as_str(),
        Some("codex-cli" | "claude-cli")
    ) {
        None
    } else {
        output["cost_usd"]
            .as_f64()
            .or_else(|| output["usage"]["cost"].as_f64())
    };
    (tokens, cost)
}

/// Adds one engine event to the agent's own transcript.
fn record(chat: &mut Chat, event: &RuntimeEvent) {
    match event {
        RuntimeEvent::Text(text) => chat.partial.push_str(text),
        RuntimeEvent::Model(model) => chat.partial_model = crate::live::model_slug(model),
        RuntimeEvent::Tokens(tokens) => chat.tokens = *tokens,
        RuntimeEvent::Tool {
            name,
            input,
            output,
            running,
        } => chat.tool(name.clone(), input.clone(), output.clone(), *running),
        RuntimeEvent::Delegation { .. } | RuntimeEvent::Progress { .. } => {}
    }
}

fn write_transcript(
    path: Option<&Path>,
    chat: &Chat,
    id: &str,
    name: &str,
    run: &Run,
    cwd: &Path,
    running: bool,
) {
    let Some(path) = path else { return };
    let model = chat
        .entries
        .iter()
        .rev()
        .find_map(|entry| match entry {
            Entry::Assistant {
                model: Some(model), ..
            } => Some(model.as_str()),
            _ => None,
        })
        .unwrap_or(run.engine.as_str());
    let mut document = crate::trajectory::document(chat, id, model, cwd);
    document["extra"]["agent"] = json!(name);
    document["extra"]["engine"] = json!(run.engine);
    document["extra"]["background"] = json!(true);
    document["extra"]["parent_session_id"] = json!(run.host.parent_session);
    document["extra"]["branch"] = json!(run.checkout.as_ref().map(|c| c.branch.clone()));
    if running {
        document["extra"]["running"] = json!(true);
    }
    run.execution.redact(&mut document);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let temporary = path.with_extension("json.tmp");
    if std::fs::write(
        &temporary,
        serde_json::to_vec_pretty(&document).unwrap_or_default(),
    )
    .is_ok()
    {
        let _ = std::fs::rename(&temporary, path);
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod user_cost_tests {
    #[test]
    fn subscriptions_never_report_dollars() {
        for transport in ["codex-cli", "claude-cli"] {
            let output = serde_json::json!({"transport":transport,"model":"gpt-6.1-sol","tokens":42,"cost_usd":1.2,"usage":{"cost":1.2}});
            assert_eq!(super::usage(&output), (42, None));
        }
        assert_eq!(
            super::usage(&serde_json::json!({"tokens":42,"usage":{"cost":0.25}})),
            (42, Some(0.25))
        );
    }
}
