//! Terminal policy around the app's shared native engine protocols.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::{StreamExt, stream::BoxStream};
use openagents_harness::host::NativeOptions;
use openagents_harness::process::{LaunchBroker, SupervisedEvent, Supervision};
use openagents_harness::{
    CancellationToken, ClaudeHarness, CodexHarness, Harness, HarnessError, OpencodeHarness,
    RunControls, SteerMessage,
};
use openagents_proto::{AgentEvent, DoneStatus, RunRequest, ToolCall};
use serde_json::json;
use tokio::sync::{Notify, mpsc, oneshot};

use crate::adapter::{Retainer, STREAM_KEEP};
use crate::delegate::{self, Agent, Briefing, Cli, Report, Status, Summary, Wrap};
use crate::session::{Capabilities, Session, StopAck};
use crate::stream::{Event, Kind};

#[derive(Default)]
struct Observed {
    active: usize,
    stopped: Vec<supervise::Stopped>,
    raw: Option<Retainer>,
}

struct Run {
    stream: Option<BoxStream<'static, Result<AgentEvent, HarnessError>>>,
    steering: Option<mpsc::Sender<SteerMessage>>,
    interrupt: CancellationToken,
    broker: LaunchBroker,
    observed: Arc<Mutex<Observed>>,
    changed: Arc<Notify>,
    retainer: Option<Retainer>,
    summary: Summary,
    tools: BTreeMap<String, String>,
    turn_over: Option<u64>,
    seq: u64,
    line: usize,
    offset: u64,
    transport: crate::transport::Watch,
}

/// A terminal session using the same protocols as the desktop and phone.
/// The terminal keeps its policy wrapper, resource limits, and retained files.
pub struct CliSession<'a> {
    pub cli: &'a Cli,
    wrap: &'a Wrap<'a>,
    binary: PathBuf,
    pub deadline: Duration,
    pub steerable: bool,
    pub resume: Option<String>,
    pub transport_bound: Duration,
    name: String,
    session_id: Option<String>,
    runs: Vec<Run>,
    pending: Vec<Event>,
    clock: Instant,
    harness: Option<String>,
    stop_reason: Option<String>,
    transport: Option<(String, bool)>,
}

impl<'a> CliSession<'a> {
    #[must_use]
    pub fn new(cli: &'a Cli, binary: PathBuf, wrap: &'a Wrap<'a>, name: &str) -> Self {
        Self {
            cli,
            wrap,
            binary,
            deadline: cli.deadline,
            steerable: false,
            resume: None,
            transport_bound: crate::transport::BOUND,
            name: name.into(),
            session_id: None,
            runs: Vec::new(),
            pending: Vec::new(),
            clock: Instant::now(),
            harness: None,
            stop_reason: None,
            transport: None,
        }
    }

    fn elapsed_ms(&self) -> u64 {
        self.clock
            .elapsed()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX)
    }

    async fn launch(&mut self, resume: Option<String>, message: &str) -> Result<(), String> {
        let mut options = NativeOptions {
            session_id: (resume.is_none() && self.cli.agent == Agent::ClaudeCode)
                .then(crate::adapter::new_session_id),
            tools: self.cli.tools.clone(),
            effort: self.cli.effort.clone(),
            codex_settings: self.cli.codex_config.clone(),
            hooks: self.cli.hooks.clone(),
            #[cfg(test)]
            claude_config_root: Some(self.cli.workdir.join(".claude")),
            #[cfg(not(test))]
            claude_config_root: self
                .cli
                .env
                .iter()
                .rev()
                .find(|(key, _)| key == "CLAUDE_CONFIG_DIR")
                .map(|(_, value)| PathBuf::from(value))
                .or_else(|| {
                    self.cli
                        .env
                        .iter()
                        .rev()
                        .find(|(key, _)| key == "HOME")
                        .map(|(_, value)| PathBuf::from(value).join(".claude"))
                }),
            ..Default::default()
        };
        if let Some(system) = &self.cli.system {
            match (self.cli.agent, system.policy.mode) {
                (Agent::ClaudeCode, crate::system::Mode::Replace)
                | (Agent::Codex, crate::system::Mode::Replace) => {
                    options.system_prompt_file = Some(self.cli.system_path())
                }
                (Agent::ClaudeCode, crate::system::Mode::Append) => {
                    options.append_system_prompt_file = Some(self.cli.system_path())
                }
                (Agent::Codex, crate::system::Mode::Append) => {
                    options.append_instructions = Some(system.text())
                }
                _ => {}
            }
        }
        self.session_id = resume.clone().or(options.session_id.clone());
        let generation = self.runs.len();
        let stem = if generation == 0 {
            self.name.clone()
        } else {
            format!("{}.resume-{generation}", self.name)
        };
        let retain = |suffix: &str| {
            let file = format!("{stem}.{suffix}.jsonl");
            Retainer::create(
                &self.cli.artifacts.join(&file),
                &format!("{}/{file}", self.cli.artifacts_label),
                STREAM_KEEP,
            )
        };
        let observed = Arc::new(Mutex::new(Observed {
            raw: Some(retain("wire")),
            ..Default::default()
        }));
        let changed = Arc::new(Notify::new());
        let seen = observed.clone();
        let notify = changed.clone();
        let (broker, mut launches) = LaunchBroker::channel();
        let broker = broker.with_host_options(options).supervised(Supervision {
            limits: supervise::Limits::within(self.deadline).keeping(STREAM_KEEP),
            observe: Arc::new(move |event| {
                let mut seen = seen.lock().unwrap_or_else(|e| e.into_inner());
                match event {
                    SupervisedEvent::Started => seen.active += 1,
                    SupervisedEvent::Output(delivery) => {
                        if let Some(raw) = &mut seen.raw {
                            for gap in delivery.gaps {
                                raw.gap(gap);
                            }
                            raw.push(&delivery.bytes);
                        }
                    }
                    SupervisedEvent::Stopped(mut stopped) => {
                        // The output callback already retained these bytes.
                        stopped.rest = Default::default();
                        seen.stopped.push(stopped);
                        seen.active = seen.active.saturating_sub(1);
                    }
                }
                notify.notify_one();
            }),
        });
        let harness: Box<dyn Harness> = match self.cli.agent {
            Agent::ClaudeCode => Box::new(
                ClaudeHarness::new()
                    .with_executable(&self.binary)
                    .with_launch_broker(broker.clone()),
            ),
            Agent::Codex => Box::new(
                CodexHarness::new()
                    .with_executable(&self.binary)
                    .with_launch_broker(broker.clone()),
            ),
            Agent::OpenCode => Box::new(
                OpencodeHarness::new()
                    .with_executable(&self.binary)
                    .with_launch_broker(broker.clone()),
            ),
            Agent::Microluna => return Err("Microluna uses its in-process executor".into()),
        };
        let request: RunRequest = serde_json::from_value(json!({
            "prompt": message, "model": (!self.cli.model.is_empty()).then_some(&self.cli.model),
            "reasoning": self.cli.effort.as_deref().and_then(|effort| serde_json::from_value::<openagents_proto::ReasoningLevel>(json!(effort)).ok()),
            "cwd": self.cli.workdir, "sandbox": "danger-full-access", "autoApprove": true, "resume": resume,
        })).map_err(|e| e.to_string())?;
        let (steering, receiver) = mpsc::channel(32);
        let interrupt = CancellationToken::new();
        let controls = RunControls {
            realtime: None,
            execution_lease: None,
            request_input: Box::new(|_| {
                let (_, rx) = oneshot::channel();
                rx
            }),
            steering: receiver,
            interrupt: interrupt.clone(),
            turn: Default::default(),
        };
        self.runs.push(Run {
            stream: None,
            steering: self.steerable.then_some(steering),
            interrupt,
            broker,
            observed,
            changed,
            retainer: Some(retain("stream")),
            summary: Summary::default(),
            tools: BTreeMap::new(),
            turn_over: None,
            seq: 0,
            line: 0,
            offset: 0,
            transport: crate::transport::Watch::new(self.transport_bound),
        });
        let result = {
            let run = harness.run(request, controls);
            tokio::pin!(run);
            loop {
                tokio::select! {
                    result = &mut run => break result.map_err(|e| e.to_string()),
                    Some(launch) = launches.recv() => launch.apply(&|mut command| {
                        self.cli.configure_command(&mut command);
                        (self.wrap)(command)
                    }),
                }
            }
        };
        match result {
            Ok(stream) => {
                self.runs.last_mut().unwrap().stream = Some(stream);
                Ok(())
            }
            Err(error) => {
                self.wait_cleanup().await;
                Err(error)
            }
        }
    }

    async fn wait_cleanup(&self) {
        let Some(run) = self.runs.last() else {
            return;
        };
        loop {
            let changed = run.changed.notified();
            if run
                .observed
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .active
                == 0
            {
                break;
            }
            changed.await;
        }
    }

    fn receive(&mut self, event: Result<AgentEvent, HarnessError>, at_ms: u64) {
        let event = match event {
            Ok(event) => event,
            Err(error) => {
                self.harness = Some(error.to_string());
                return;
            }
        };
        let run = self.runs.last_mut().unwrap();
        // Keep the shared event and its policy events together for replay.
        let exit_code = match &event {
            AgentEvent::ToolResult { id, .. } => run.broker.take_exit_code(id),
            _ => None,
        };
        let mut kinds = policy_events(&event, &mut run.tools, exit_code);
        if self.cli.agent == Agent::Codex {
            match &event {
                AgentEvent::ToolCall { .. } => {
                    kinds.retain(|kind| !matches!(kind, Kind::ArtifactChanged { .. }))
                }
                AgentEvent::ToolResult { id, .. } => match run.broker.take_changes(id) {
                    Ok(changes) => kinds.extend(
                        changes
                            .into_iter()
                            .map(|(path, change)| Kind::ArtifactChanged { path, change }),
                    ),
                    Err(error) => {
                        self.harness = Some(error);
                        run.broker.stop_processes();
                    }
                },
                _ => {}
            }
        }
        if self.cli.agent == Agent::Codex
            && matches!(
                event,
                AgentEvent::ToolResult { .. } | AgentEvent::AssistantMessageCompleted { .. }
            )
        {
            run.summary.completed_items = Some(
                run.summary
                    .completed_items
                    .unwrap_or_default()
                    .saturating_add(1),
            );
        }
        let record = json!({"type":"openagents.harness", "agent": self.cli.agent.word(), "event":event, "kinds":kinds});
        let offset = run.offset;
        run.line += 1;
        let mut line = record.to_string();
        line.push('\n');
        run.offset = run.offset.saturating_add(line.len() as u64);
        if let Some(retainer) = &mut run.retainer {
            retainer.push(line.as_bytes());
        }
        match &event {
            AgentEvent::SessionStarted {
                session_id, model, ..
            } => {
                self.session_id = Some(session_id.clone());
                run.summary.session_id = Some(session_id.clone());
                run.summary.model = Some(model.clone());
            }
            AgentEvent::TextDelta { text } => {
                let result = run.summary.result.get_or_insert_with(String::new);
                let room = 4000usize.saturating_sub(result.chars().count());
                result.extend(text.chars().take(room));
                run.transport.line(r#"{"type":"progress"}"#, at_ms);
            }
            AgentEvent::Error { message } => {
                run.transport.line(
                    &json!({"type":"error","message":message}).to_string(),
                    at_ms,
                );
                if crate::limit::says_limited(message) {
                    run.summary.limit = Some(crate::limit::Limit::from_message(
                        crate::limit::provider_of(self.cli.agent.word()),
                        message,
                        crate::limit::now(),
                    ));
                }
            }
            AgentEvent::Done {
                status,
                result,
                error,
                session_id,
            } => {
                if let Some(id) = session_id {
                    self.session_id = Some(id.clone());
                    run.summary.session_id = Some(id.clone());
                }
                run.summary.has_result = *status == DoneStatus::Completed
                    || result.is_some()
                    || run.summary.result.is_some();
                run.summary.is_error = Some(*status != DoneStatus::Completed);
                run.summary.result = result
                    .clone()
                    .or(error.clone())
                    .or(run.summary.result.clone());
                if let Some(message) = error
                    .as_ref()
                    .or(result.as_ref())
                    .filter(|m| crate::limit::says_limited(m))
                {
                    run.summary.limit = Some(crate::limit::Limit::from_message(
                        crate::limit::provider_of(self.cli.agent.word()),
                        message,
                        crate::limit::now(),
                    ));
                }
                run.turn_over = Some(at_ms);
            }
            AgentEvent::ToolCall { .. } | AgentEvent::Usage { .. } => {
                run.transport.line(r#"{"type":"progress"}"#, at_ms)
            }
            _ => {}
        }
        for kind in kinds {
            run.seq += 1;
            self.pending.push(Event {
                seq: run.seq,
                line: run.line,
                offset: Some(offset),
                kind,
            });
        }
    }

    fn summary(&self, run: &Run) -> Summary {
        let mut summary = run.summary.clone();
        let accounting = run.broker.accounting();
        summary.session_id = summary.session_id.or(self.session_id.clone());
        summary.model = accounting
            .model
            .or(summary.model)
            .or_else(|| Some(self.cli.model.clone()));
        summary.version = accounting.version;
        summary.api_key_source = accounting.api_key_source;
        summary.usage = accounting.usage;
        summary.model_usage = accounting.model_usage;
        summary.total_cost_usd = accounting.total_cost_usd;
        summary.num_turns = accounting.num_turns;
        summary.duration_ms = accounting.duration_ms;
        summary.duration_api_ms = accounting.duration_api_ms;
        summary.input_per_call = accounting.input_per_call;
        summary.api_calls = accounting.api_calls;
        if let Some(failure) = accounting.failure {
            summary.refusal = match (failure.kind.as_str(), failure.status) {
                ("ProviderAuthError" | "provider.auth", _) | ("APIError", Some(401)) => {
                    Some("not_logged_in")
                }
                ("ProviderModelNotFoundError", _) | ("APIError", Some(403 | 404)) => {
                    Some("model_unavailable")
                }
                _ => None,
            };
            if failure.status == Some(429) {
                let now = crate::limit::now();
                let mut limit = crate::limit::Limit::from_message(
                    self.cli.agent.word(),
                    summary
                        .result
                        .as_deref()
                        .unwrap_or("Provider request limit reached"),
                    now,
                );
                limit.status = failure.status;
                if let Some(seconds) = failure.retry_after_seconds {
                    limit.resets_at = Some(now.saturating_add(seconds));
                    limit.reset_source = Some("retry_after".into());
                }
                summary.limit = Some(limit);
            }
        }
        if let Some(limit) = &mut summary.limit {
            if let Some(reset) = accounting.resets_at {
                limit.resets_at = Some(reset);
                limit.reset_source = Some("rate_limit_event".into());
            }
            limit.window = accounting.limit_window;
        }
        if self.cli.agent == Agent::OpenCode {
            summary.cost_provenance = Some("cli_list_price");
            summary.cost_note = Some(delegate::OPENCODE_COST_NOTE);
        }
        if self.cli.agent == Agent::Codex
            && let Some(usage) = &summary.usage
        {
            let n = |key| usage[key].as_u64().unwrap_or_default();
            summary.total_cost_usd = delegate::codex_cost(
                summary.model.as_deref().unwrap_or(&self.cli.model),
                n("input_tokens"),
                n("cache_read_input_tokens"),
                n("output_tokens"),
            );
            summary.cost_provenance = Some("price_estimate");
            summary.cost_note = Some(delegate::CODEX_COST_NOTE);
        }
        summary
    }

    async fn drain(&mut self, now_ms: u64) {
        if let Some(run) = self.runs.last_mut() {
            run.steering.take();
            run.broker.stop_processes();
            run.interrupt.cancel();
        }
        loop {
            let event = match self.runs.last_mut().and_then(|r| r.stream.as_mut()) {
                Some(stream) => stream.next().await,
                None => None,
            };
            match event {
                Some(event) => self.receive(event, now_ms),
                None => break,
            }
            // A stopped session can emit a bounded tail; keep the host queue bounded too.
            if self.pending.len() > 4096 {
                self.harness = Some("the agent sent too many events while stopping".into());
                self.pending.truncate(4096);
            }
        }
        if let Some(run) = self.runs.last_mut() {
            run.stream = None;
        }
        self.wait_cleanup().await;
    }

    pub async fn shutdown(&mut self) {
        self.drain(self.elapsed_ms()).await;
    }
    #[must_use]
    pub fn summaries(&self) -> Vec<Summary> {
        self.runs.iter().map(|run| self.summary(run)).collect()
    }
}

impl Session for CliSession<'_> {
    fn adapter(&self) -> &str {
        self.cli.agent.word()
    }
    fn capabilities(&self) -> Capabilities {
        crate::adapter::capabilities(self.cli.agent).0
    }
    fn session_id(&self) -> Option<String> {
        self.session_id.clone()
    }
    async fn start(&mut self, briefing: &Briefing) -> Result<(), String> {
        if !self.runs.is_empty() {
            return Err("the session already started".into());
        }
        self.clock = Instant::now();
        let result = self.launch(self.resume.clone(), &briefing.text).await;
        if let Err(error) = &result {
            self.harness = Some(error.clone());
        }
        result
    }
    async fn advance(&mut self, now_ms: u64) -> bool {
        if let Some(run) = self.runs.last_mut() {
            if run.turn_over.is_some_and(|at| at < now_ms) {
                run.steering.take();
            }
        } else {
            return false;
        }
        let until = tokio::time::Instant::now()
            + Duration::from_millis(now_ms.saturating_sub(self.elapsed_ms()));
        for _ in 0..1024 {
            let Some(stream) = self.runs.last_mut().and_then(|r| r.stream.as_mut()) else {
                return false;
            };
            tokio::select! {
                event=stream.next()=>match event {
                    Some(event)=>self.receive(event,now_ms),
                    None=> { self.runs.last_mut().unwrap().stream=None; self.wait_cleanup().await; return false; }
                },
                _=tokio::time::sleep_until(until)=>break,
            }
        }
        if self.cli.agent == Agent::Codex
            && let Some(why) = self.runs.last().and_then(|r| r.transport.expired(now_ms))
        {
            let reached = self.runs.last().unwrap().transport.reached();
            self.drain(now_ms).await;
            self.transport = Some((why, reached));
            return false;
        }
        true
    }
    fn observe(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.pending)
    }
    async fn stop(&mut self, now_ms: u64, reason: &str) -> Result<StopAck, String> {
        if self.runs.last().is_none_or(|r| r.stream.is_none()) {
            return Err("no process is running".into());
        }
        self.drain(now_ms).await;
        self.stop_reason = Some(reason.into());
        let seen = self
            .runs
            .last()
            .unwrap()
            .observed
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let clear = seen.active == 0 && seen.stopped.iter().all(|s| s.group_clear);
        let cleanup = format!(
            "stopped {} processes; their groups were {} after cleanup",
            seen.stopped.len(),
            if clear { "empty" } else { "NOT empty" }
        );
        if !clear {
            return Err(cleanup);
        }
        Ok(StopAck {
            at_ms: now_ms,
            reason: reason.into(),
            pending: 1,
            cleanup,
        })
    }
    async fn resume(&mut self, _now_ms: u64, message: &str) -> Result<(), String> {
        if self.runs.last().is_some_and(|r| r.stream.is_some()) {
            return Err("the session is still running".into());
        }
        let id = self
            .session_id
            .clone()
            .ok_or("the session never reported an ID to resume")?;
        self.stop_reason = None;
        self.transport = None;
        self.harness = None;
        self.launch(Some(id), message).await
    }
    async fn steer(&mut self, _now_ms: u64, message: &str) -> Result<(), String> {
        let wait = self
            .cli
            .granted
            .unwrap_or(self.cli.deadline)
            .saturating_sub(self.clock.elapsed())
            .min(Duration::from_secs(30));
        if wait.is_zero() {
            return Err("the session's deadline already passed".into());
        }
        let run = self.runs.last_mut().ok_or("no process is running")?;
        let steering = run
            .steering
            .as_ref()
            .ok_or("this session was not launched to accept messages while it runs")?;
        steering
            .try_send(SteerMessage::text(message))
            .map_err(|e| e.to_string())?;
        run.turn_over = None;
        run.summary.has_result = false;
        run.summary.is_error = None;
        run.summary.result = None;
        if self.cli.agent == Agent::Codex {
            let receipt = async {
                loop {
                    let event = self
                        .runs
                        .last_mut()
                        .and_then(|r| r.stream.as_mut())
                        .ok_or("the session stopped before accepting the message")?
                        .next()
                        .await
                        .ok_or("the session stopped before accepting the message")?;
                    let accepted = matches!(event, Ok(AgentEvent::Steered { .. }));
                    self.receive(event, self.elapsed_ms());
                    if accepted {
                        return Ok(());
                    }
                }
            };
            return tokio::time::timeout(wait, receipt)
                .await
                .map_err(|_| "Codex did not confirm the message in time".to_string())?;
        }
        Ok(())
    }
    fn report(&mut self) -> Report {
        let summary = self
            .runs
            .last()
            .map(|r| self.summary(r))
            .unwrap_or_default();
        let mut stderr = String::new();
        let mut streams = Vec::new();
        let mut ending = supervise::Ending::Exited(Some(0));
        let last = self.runs.len().saturating_sub(1);
        for (index, run) in self.runs.iter_mut().enumerate() {
            let mut seen = run.observed.lock().unwrap_or_else(|e| e.into_inner());
            if index == last {
                ending = supervise::Ending::Exited(Some(0));
            }
            for stopped in &seen.stopped {
                stderr.push_str(&stopped.stderr.marked());
                stderr = crate::judge::clip_tail(&stderr, 8000);
                if index != last {
                    continue;
                }
                if !stopped.group_clear {
                    ending =
                        supervise::Ending::Failed("the agent's process group did not stop".into());
                } else if !stopped.ending.success()
                    && (!stopped.requested
                        || matches!(
                            stopped.ending,
                            supervise::Ending::Failed(_) | supervise::Ending::TimedOut
                        ))
                {
                    ending = stopped.ending.clone();
                }
            }
            if let Some(retain) = run.retainer.take()
                && let Some(mut record) = retain.finish()
            {
                if let Some(raw) = seen.raw.take().and_then(Retainer::finish) {
                    record["wire"] = raw;
                }
                record["schema"] = json!("openagents.harness.v1");
                streams.push(record);
            }
        }
        let status = if let Some(error) = &self.harness {
            Status::Harness(error.clone())
        } else if let Some((detail, reached)) = &self.transport {
            Status::Transport {
                detail: detail.clone(),
                reached: *reached,
            }
        } else if let Some(reason) = &self.stop_reason {
            if reason.contains("deadline") {
                Status::TimedOut
            } else {
                Status::Harness(format!("stopped by the host: {reason}"))
            }
        } else if self.runs.last().is_some_and(|r| r.stream.is_some()) {
            Status::Harness("the session was still running".into())
        } else {
            delegate::classify(&ending, &summary, &stderr)
        };
        let stream = streams.first().cloned().map(|mut first| {
            if streams.len() > 1 {
                first["resumed"] = json!(streams[1..]);
            }
            first
        });
        Report {
            status,
            summary,
            milliseconds: self.elapsed_ms(),
            stderr: crate::judge::clip_tail(&stderr, 4000),
            stream,
        }
    }
}

fn policy_events(
    event: &AgentEvent,
    tools: &mut BTreeMap<String, String>,
    exit_code: Option<i64>,
) -> Vec<Kind> {
    match event {
        AgentEvent::SessionStarted { session_id, .. } => vec![Kind::SessionStarted {
            session_id: Some(session_id.clone()),
        }],
        AgentEvent::TextDelta { text } if !text.trim().is_empty() => vec![Kind::AssistantClaim {
            text: text.chars().take(crate::stream::CLAIM_CHARS).collect(),
        }],
        AgentEvent::ToolCall { id, call } => {
            if let ToolCall::Exec { command } = call
                && id.len() <= 256
                && (tools.len() < 4096 || tools.contains_key(id))
            {
                tools.insert(id.clone(), command.chars().take(4000).collect());
            }
            match call {
                ToolCall::Exec { command } => vec![Kind::CommandStarted {
                    command: command.chars().take(4000).collect(),
                }],
                ToolCall::WriteFile { path, .. } | ToolCall::EditFile { path, .. } => {
                    vec![Kind::ArtifactChanged {
                        path: path.clone(),
                        change: "write".into(),
                    }]
                }
                ToolCall::ApplyPatch { path: Some(path) } => vec![Kind::ArtifactChanged {
                    path: path.clone(),
                    change: "update".into(),
                }],
                _ => Vec::new(),
            }
        }
        AgentEvent::ToolResult {
            id,
            is_error,
            output,
            ..
        } => match tools.remove(id) {
            Some(command) => vec![Kind::CommandCompleted {
                command,
                exit_code: exit_code.or_else(|| (*is_error).then_some(1)),
                output: output
                    .as_deref()
                    .unwrap_or_default()
                    .chars()
                    .take(crate::stream::OUTPUT_CHARS)
                    .collect(),
            }],
            _ => Vec::new(),
        },
        AgentEvent::Usage {
            input_tokens,
            output_tokens,
        } => vec![Kind::UsageUpdate {
            usage: json!({"input_tokens":input_tokens,"output_tokens":output_tokens}),
        }],
        AgentEvent::Done {
            status,
            result,
            error,
            ..
        } => vec![Kind::SessionEnded {
            error: *status != DoneStatus::Completed,
            result: result.clone().or(error.clone()),
        }],
        _ => Vec::new(),
    }
}
