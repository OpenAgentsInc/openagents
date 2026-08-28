//! Coder's turn: its own voice over the capable runtime next door.
//!
//! The session underneath is [`crate::runtime::CoderRuntimeSession`]
//! — threads, grants, the inference proxy, the live model catalog, lanes,
//! metering, and revocation — and it is used rather than copied, so there is
//! one implementation of each of those and this crate cannot drift from it.
//!
//! What this file owns is the part that is Coder's:
//!
//! - The system prompt, composed from the staged `system-prompt` surface
//!   (`surfaces/coder/system-prompt.v1.json`) and carried verbatim. It is
//!   the reason the session
//!   answers as a terminal rather than as an assistant, and a merge that
//!   reworded it would have changed the product.
//! - [`Control`], the one-way channel the TUI loop reads. Text, tool calls,
//!   tool results, the model that answered, what the turn spent, and the
//!   failures — each as its own message, so the frame shows what happened and
//!   not a summary written afterwards.
//!
//! ## Nothing here invents an answer
//!
//! Every path is the model's own words or a [`Control::Failed`] naming what
//! refused. There is no fallback model, no synthesized reply, and no invented
//! grant: the session below refuses out loud and this file carries the refusal
//! to the screen instead of painting over it.

use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::coder::turn::TurnId;
use crate::runtime::{
    ChatMessage, CoderRuntimeSession, ImageAttachment, Lane, ModelStreamEvent, ToolEvent,
    TurnProgress, TurnUsage,
};
use crate::surfaces::system_prompt as prompt;
use crate::tools::{DelegationGate, HarnessToolRegistry, ToolDefinition};

type Failure = Box<dyn std::error::Error + Send + Sync>;

/// What the runtime tells the frame, in the order it happened.
#[derive(Debug, Clone)]
pub enum Control {
    /// One event produced by a specific turn. The frame applies it only while
    /// this generation remains active.
    Turn { id: TurnId, event: Box<Control> },
    /// The transport task has stopped and the interrupted turn has been
    /// recorded locally. This is the typed completion of a cancel request.
    CancelComplete {
        id: TurnId,
        diagnostic: Option<String>,
    },
    /// A piece of the reply, as the model wrote it.
    Chunk(String),
    /// A piece of the model's reasoning summary, as it arrived.
    Reasoning(String),
    /// The current response step became a tool round, so remove its
    /// provisional assistant text.
    DiscardReply,
    /// The current response step is the final answer.
    CommitReply,
    /// A tool call started. The header goes up now; the box fills in later.
    Tool {
        call_id: String,
        name: String,
        /// The raw JSON string from the wire.
        arguments: String,
    },
    /// More of what that call has printed, appended to its box.
    ToolOutput { call_id: String, chunk: String },
    /// That call finished, and whether it worked.
    ToolDone {
        call_id: String,
        is_error: bool,
        /// Whole milliseconds the call held the session, from the tool
        /// layer. Zero for a call that never reached a tool.
        duration_ms: u64,
    },
    /// A delegated agent's activity, streamed into that agent's own box.
    ///
    /// The `call_id` is the parent `delegate` call. Child tool starts reuse
    /// [`Control::Tool`] with this same id so they nest in the box rather than
    /// opening a sibling; child text uses this variant. A child must not emit
    /// [`Control::ToolDone`] with the parent id: that would settle the box
    /// before the agent finished.
    SubagentOutput { call_id: String, line: String },
    /// The model that actually answered, as its grant pins it.
    Model(String),
    /// The thread the server opened for this session, once it has one.
    Thread(String),
    /// What the turn spent, as the server reported it.
    Usage(TurnUsage),
    /// What one read of `GET /api/v1/credit` found, and `None` when it found
    /// nothing. Read rather than subtracted from `Usage`: spend is the
    /// server's, and this session is not the only thing spending it. The
    /// failure travels too, because the frame has to stop showing the previous
    /// answer rather than leave it up.
    Credit(Option<crate::coder::credit::Credit>),
    /// What the server billed this session's thread, in tokens.
    ///
    /// Sent when the ending hands the figure back, and never otherwise: the
    /// grant's spend is a value only the server produces, and there is no
    /// route that reports it mid-thread. Distinct from [`Control::Credit`]:
    /// this is what this thread cost, that is what the account has left.
    Billed(u64),
    /// Something worth saying that is not the model talking.
    Notice(String),
    /// An in-frame device sign-in finished.
    ///
    /// Progress travels as [`Control::Output`] while the browser opens and the
    /// device authorization polls. This event lets the frame install the new
    /// authenticated session without blocking redraws during that wait.
    Login(Result<String, String>),
    /// Replace the temporary first-response status, or clear it with `None`.
    Waiting(Option<String>),
    /// What one of the session's own commands printed. Markdown, rendered the
    /// way an answer is, and exported as a notice rather than a model step.
    Output(String),
    /// The turn did not answer, and this is why. Never an answer.
    Failed(String),
    /// The turn is over, one way or the other.
    Done,
    /// The current goal after a command, tool call, or usage update.
    Goal(Option<crate::coder::goal::Goal>),
}

/// A `Sender` an observer can hold: `Fn` observers are shared, and the frame
/// loop's receiver is on the other end of exactly one channel.
pub type Sink = Arc<Mutex<Sender<Control>>>;

/// Routes observer events back to the turn that created them.
///
/// Tool completions keep their original mapping after cancellation, so a late
/// completion cannot be relabeled as output from a newer turn.
#[derive(Debug, Default)]
pub struct TurnRouter {
    active: Option<TurnId>,
    calls: std::collections::HashMap<String, TurnId>,
    cancellation: Option<(TurnId, tokio::sync::watch::Sender<bool>)>,
}

pub type SharedTurnRouter = Arc<Mutex<TurnRouter>>;

impl TurnRouter {
    fn start(&mut self, id: TurnId, cancellation: tokio::sync::watch::Sender<bool>) {
        self.active = Some(id);
        self.cancellation = Some((id, cancellation));
    }

    /// Signal every tool owned by `id` and return how many are still active.
    pub fn cancel(&mut self, id: TurnId) -> usize {
        if self.active == Some(id) {
            self.active = None;
        }
        if let Some((turn, cancellation)) = &self.cancellation {
            if *turn == id {
                let _ = cancellation.send(true);
            }
        }
        self.calls.values().filter(|turn| **turn == id).count()
    }

    fn finish(&mut self, id: TurnId) {
        if self.active == Some(id) {
            self.active = None;
        }
        if self
            .cancellation
            .as_ref()
            .is_some_and(|(turn, _)| *turn == id)
        {
            self.cancellation = None;
        }
    }
}

impl Control {
    /// Whether this message settles a tool box.
    ///
    /// The live frame holds these until the next drain so a tool that starts
    /// and finishes in one channel burst still paints an active rail.
    pub fn settles_tool(&self) -> bool {
        match self {
            Control::ToolDone { .. } => true,
            Control::Turn { event, .. } => event.settles_tool(),
            _ => false,
        }
    }
}

/// Put a message on the frame's channel, or drop it if the frame is gone.
pub fn send(sink: &Sink, message: Control) {
    if let Ok(tx) = sink.lock() {
        let _ = tx.send(message);
    }
}

fn send_turn(sink: &Sink, id: TurnId, event: Control) {
    send(
        sink,
        Control::Turn {
            id,
            event: Box::new(event),
        },
    );
}

/// The system message this session opens with.
///
/// The staged instructions first and unchanged, then the tools — because a
/// model that is told it has no tools when it has some will not use them, and
/// one told it has tools it does not have will claim to have run them. The
/// list is generated from what was actually declared, so the two cannot
/// disagree.
pub fn system_prompt(tools: &[ToolDefinition]) -> String {
    let mut lines = vec![prompt::CODER_LITE_INSTRUCTIONS.to_string(), String::new()];
    if tools.is_empty() {
        lines.push(prompt::CODER_LITE_NO_TOOLS.to_string());
    } else {
        lines
            .push(prompt::CODER_LITE_TOOL_LIST_HEADER.replace("{count}", &tools.len().to_string()));
        for tool in tools {
            lines.push(format!("- `{}`", tool.name));
        }
        lines.push(String::new());
        lines.push(prompt::CODER_LITE_TOOL_LIST_CLOSING.to_string());
    }
    lines.join("\n")
}

/// Catalog ids served by the Pro door.
pub const PRO_MODEL_IDS: &[&str] = &["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"];

/// Production origin for Pro when no `--dev` base is set.
pub const PRO_PRODUCTION_ORIGIN: &str = "https://pro.openagents.com";

pub fn is_pro_model_id(id: &str) -> bool {
    PRO_MODEL_IDS.iter().any(|candidate| *candidate == id)
}

pub fn pro_origin() -> String {
    env::var("OPENAGENTS_PRO_ORIGIN")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| PRO_PRODUCTION_ORIGIN.to_string())
}

/// The `/api/v1` base this session talks to.
///
/// `OPENAGENTS_BASE_URL` first, because that is what `--dev` sets and a reader
/// who pointed the session at a server on this machine meant it. Named Pro
/// catalog ids then go to the Pro origin. Then the endpoint `oa auth`
/// selected, so Coder and `oa` agree about where they are without a second
/// configuration file.
pub fn api_base() -> String {
    api_base_for(&Lane::default())
}

pub fn api_base_for(lane: &Lane) -> String {
    for name in ["OPENAGENTS_BASE_URL", "OPENAGENTS_API_BASE"] {
        if let Ok(value) = env::var(name) {
            let value = value.trim().to_string();
            if !value.is_empty() {
                return value;
            }
        }
    }
    if let Lane::Named(id) = lane {
        if is_pro_model_id(id) {
            return format!("{}/api/v1", pro_origin());
        }
    }
    match crate::auth::resolve_endpoint(None, None) {
        Ok(endpoint) => format!("{}/api/v1", endpoint.origin),
        Err(_) => "https://openagents.com/api/v1".to_string(),
    }
}

/// The credential this session spends, or `None`.
///
/// `None` is carried rather than papered over: a thread request without one is
/// refused by the server, and that refusal is what the reader should see.
pub fn user_token() -> Option<String> {
    user_token_for(&Lane::default())
}

pub fn user_token_for(lane: &Lane) -> Option<String> {
    if matches!(lane, Lane::Named(id) if is_pro_model_id(id))
        && env::var("OPENAGENTS_BASE_URL").ok().is_none()
        && env::var("OPENAGENTS_API_BASE").ok().is_none()
    {
        if let Ok(value) = env::var("OPENAGENTS_PRO_API_KEY") {
            let value = value.trim().to_string();
            if !value.is_empty() {
                return Some(value);
            }
        }
        let store = crate::auth::CredentialStore::for_origin(&pro_origin());
        if let Some(token) = store.get_token() {
            return Some(token);
        }
    }
    if let Ok(value) = env::var("OPENAGENTS_API_KEY") {
        let value = value.trim().to_string();
        if !value.is_empty() {
            return Some(value);
        }
    }
    let endpoint = crate::auth::resolve_endpoint(None, None).ok()?;
    crate::auth::CredentialStore::for_origin(&endpoint.origin).get_token()
}

/// The session a Coder frame drives.
pub struct Session {
    inner: CoderRuntimeSession,
    lane: Lane,
    /// The frame's channel, kept so the ending can report what the server
    /// billed. Turns carry their own sender; this one outlives them.
    sink: Sink,
    goal: crate::coder::goal::SharedGoal,
    turn_router: SharedTurnRouter,
    next_turn: u64,
    /// Turn generations whose canceled thread has already been reported.
    settled_cancellations: std::collections::HashSet<TurnId>,
    /// Canceled turns already written to the transcript, including retries.
    recorded_cancellations: std::collections::HashSet<TurnId>,
    /// Whether the live stream observer delivered answer text for this turn.
    live_streaming: Arc<AtomicBool>,
}

impl Session {
    /// Open a session on `lane`, reporting everything it does to `tx`.
    ///
    /// The tools are the full set — `read`, `write`, `edit`, `bash`, `shell`,
    /// `skill`, `openagents`, `capability`, and `delegate` — on the same terms
    /// `oa coder` gets them: children run on this lane, on this credential, and
    /// cannot delegate again.
    pub fn open(
        lane: Lane,
        lane_name: &str,
        reasoning: Option<String>,
        agents: Vec<crate::coder::acp::Agent>,
        dev: bool,
        tx: Sender<Control>,
    ) -> Self {
        let api_base = api_base_for(&lane);
        let token = user_token_for(&lane);
        let mut session =
            Self::open_at(lane, lane_name, reasoning, agents, api_base, token, dev, tx);
        // The local-lane request controls (#293) are process-global like the
        // rest of the environment `open` reads once.
        session.inner.ollama_num_ctx = std::env::var("OPENAGENTS_OLLAMA_NUM_CTX")
            .ok()
            .and_then(|v| v.trim().parse::<u32>().ok());
        session
    }

    /// [`Self::open`] against a named server with a named credential.
    ///
    /// The environment is process-global and a test that set it would race
    /// every other test in the same binary, so the two values the environment
    /// supplies are parameters here and read from the environment exactly once,
    /// in `open`.
    #[allow(clippy::too_many_arguments)]
    pub fn open_at(
        lane: Lane,
        lane_name: &str,
        reasoning: Option<String>,
        agents: Vec<crate::coder::acp::Agent>,
        api_base: String,
        token: Option<String>,
        dev: bool,
        tx: Sender<Control>,
    ) -> Self {
        let mut tools = HarnessToolRegistry::with_delegation(
            None,
            DelegationGate {
                lane: lane_name.to_string(),
                user_token: token.clone(),
                api_base: Some(api_base.clone()),
                max_count: crate::delegate::MAX_DELEGATE_COUNT,
                // Coder takes no `--child-*` flags yet, so children start
                // on the defaults. Set rather than defaulted at the struct so
                // adding those flags here is a visible edit, not a silent
                // inheritance of whatever the field grows into.
                child: crate::delegate::ChildOptions::default(),
                // Discovered before the session opened: `find_agents` checks
                // each registry entry against this machine, so a program that
                // is not installed is not offered. Naming one of these in a
                // `delegate` call hands the task to it on its own bill.
                acp_agents: agents,
                acp_spent: Arc::new(AtomicBool::new(false)),
            },
        );

        let sink: Sink = Arc::new(Mutex::new(tx));
        let goal = Arc::new(Mutex::new(crate::coder::goal::GoalStore::default()));
        let observed = Arc::clone(&sink);
        let progress = Arc::clone(&sink);
        let streaming = Arc::clone(&sink);
        let turn_router = Arc::new(Mutex::new(TurnRouter::default()));
        // The turn's own sink: a tool arm that streams (delegate) emits
        // through this, and TurnRouter scopes every Control by the active
        // generation so a cancelled turn cannot paint into the next one.
        tools = tools.with_event_sink({
            let sink = Arc::clone(&sink);
            let turns = Arc::clone(&turn_router);
            Arc::new(move |event| {
                if let Some(id) = turns.lock().ok().and_then(|router| router.active) {
                    send_turn(&sink, id, event);
                }
            })
        });
        let observed_turns = Arc::clone(&turn_router);
        let progress_turns = Arc::clone(&turn_router);
        let stream_turns = Arc::clone(&turn_router);
        let live_streaming = Arc::new(AtomicBool::new(false));
        let streamed = Arc::clone(&live_streaming);

        let mut inner = CoderRuntimeSession::new(lane.clone(), Some(api_base), token, tools)
            .observing_tools(Arc::new(move |event: ToolEvent| match event {
                ToolEvent::Started {
                    call_id,
                    name,
                    arguments,
                } => {
                    let id = observed_turns.lock().ok().and_then(|mut turns| {
                        let id = turns.active?;
                        turns.calls.insert(call_id.clone(), id);
                        Some(id)
                    });
                    if let Some(id) = id {
                        send_turn(
                            &observed,
                            id,
                            Control::Tool {
                                call_id,
                                name,
                                arguments,
                            },
                        );
                    }
                }
                ToolEvent::Finished {
                    call_id,
                    output,
                    is_error,
                    duration_ms,
                    ..
                } => {
                    let id = observed_turns
                        .lock()
                        .ok()
                        .and_then(|mut turns| turns.calls.remove(&call_id));
                    if let Some(id) = id {
                        send_turn(
                            &observed,
                            id,
                            Control::ToolOutput {
                                call_id: call_id.clone(),
                                chunk: output,
                            },
                        );
                        send_turn(
                            &observed,
                            id,
                            Control::ToolDone {
                                call_id,
                                is_error,
                                duration_ms,
                            },
                        );
                    }
                }
            }))
            .observing_progress(Arc::new(move |event| {
                let message = match event {
                    TurnProgress::Waiting {
                        retry: 0,
                        max_retries: _,
                    } => Some("Waiting for the model...".to_string()),
                    TurnProgress::Waiting { retry, max_retries } => Some(format!(
                        "Waiting for the model (retry {retry} of {max_retries})..."
                    )),
                    TurnProgress::Retrying { retry, max_retries } => Some(format!(
                        "No response after 10 seconds. Retrying ({retry} of {max_retries})..."
                    )),
                    TurnProgress::Clear => None,
                };
                let id = progress_turns.lock().ok().and_then(|turns| turns.active);
                if let Some(id) = id {
                    send_turn(&progress, id, Control::Waiting(message));
                }
            }))
            .observing_stream(Arc::new(move |event| {
                let id = stream_turns.lock().ok().and_then(|turns| turns.active);
                let Some(id) = id else {
                    return;
                };
                match event {
                    ModelStreamEvent::ContentDelta(chunk) => {
                        streamed.store(true, Ordering::Relaxed);
                        send_turn(&streaming, id, Control::Chunk(chunk));
                    }
                    ModelStreamEvent::ContentCommitted => {
                        send_turn(&streaming, id, Control::CommitReply)
                    }
                    ModelStreamEvent::ContentDiscarded => {
                        streamed.store(false, Ordering::Relaxed);
                        send_turn(&streaming, id, Control::DiscardReply)
                    }
                    ModelStreamEvent::ReasoningDelta(chunk) => {
                        send_turn(&streaming, id, Control::Reasoning(chunk))
                    }
                }
            }))
            // OpenResponses is a Phoenix-only streaming surface used by tests
            // (`dev_session`). CLI `--dev` talks to the local Rust coder API
            // over the thread/grant/proxy hop and passes `false` here.
            .use_openresponses(dev);
        inner.reasoning = reasoning;
        inner.repository = repository();

        // Seeded here so the session below leaves it alone: `execute_turn`
        // writes its own system prompt only into an empty message list, and
        // this one is Coder's.
        let prompt = system_prompt(&inner.tools.list_tools());
        inner.messages.push(ChatMessage {
            role: "system".to_string(),
            content: Some(prompt),
            tool_calls: None,
            tool_call_id: None,
            images: Vec::new(),
        });

        Self {
            inner,
            lane,
            sink,
            goal,
            turn_router,
            next_turn: 1,
            settled_cancellations: std::collections::HashSet::new(),
            recorded_cancellations: std::collections::HashSet::new(),
            live_streaming,
        }
    }

    /// Attach one local session after the runtime has installed Coder's
    /// current system prompt and tools.
    pub fn with_local_session(
        mut self,
        store: crate::session_store::LocalSessionStore,
        events: &[crate::session_store::StoredEvent],
        cloud_history: bool,
    ) -> Self {
        // Long shell runs keep their whole transcript beside this record, so
        // a follow-up question reads the file instead of rerunning the job.
        let record_directory = store.directory().to_path_buf();
        self.inner
            .tools
            .keeping_session_logs_in_place(record_directory.clone());
        // The session's own record is now addressable (#159): the Tier D
        // recall host tool answers questions about past output from the
        // record and its command artifacts, so the model's way back to a
        // fact is a read, never a re-execution.
        match self
            .inner
            .tools
            .add_host_tool(crate::coder::recall::host_tool(record_directory.clone()))
        {
            Ok(()) => self.refresh_system_prompt(),
            Err(refusal) => {
                // Only a name collision could refuse here, and the registry
                // was just built — but a silent downgrade of the tool list
                // would be worse than a loud one.
                eprintln!("history_recall host tool not registered: {refusal}");
            }
        }
        let session_id = store.summary().id.clone();
        self.inner = self
            .inner
            .with_local_session(store, crate::session_store::replay_messages(events))
            .with_cloud_history(cloud_history);
        self.inner.bind_swarm(crate::swarm::SwarmBinding::new(
            crate::swarm::default_home(),
            session_id,
            record_directory,
        ));
        self
    }

    pub fn local_session_summary(&self) -> Option<&crate::session_store::SessionSummary> {
        self.inner.local_session_summary()
    }

    /// The lane this session was opened on. What was asked for, not what
    /// answered — [`Control::Model`] carries that.
    pub fn lane(&self) -> &Lane {
        &self.lane
    }

    /// Move this session onto `lane`, as shift+tab does.
    ///
    /// The thread the session was holding is dropped rather than carried
    /// over, because its grant pinned the *old* lane's model for the thread's
    /// whole life. The next turn opens its own thread on the new lane.
    pub fn set_lane(&mut self, lane: Lane) {
        self.lane = lane.clone();
        self.inner.set_lane(lane);
    }

    /// Change the first-response watchdog on an existing session.
    #[doc(hidden)]
    pub fn set_first_response_policy(
        &mut self,
        waiting_after: std::time::Duration,
        timeout_after: std::time::Duration,
    ) {
        self.inner
            .set_first_response_policy(waiting_after, timeout_after);
    }

    /// Apply one `/goal` command and return its notice.
    pub fn goal_command(&mut self, line: &str) -> String {
        use crate::coder::goal::{GoalCommand, GoalStatus};

        let Some(command) = crate::coder::goal::parse_command(line) else {
            return "There is no `/goal` command in that input.".to_string();
        };
        let mut store = match self.goal.lock() {
            Ok(store) => store,
            Err(_) => return "This session does not have goal storage available.".to_string(),
        };
        match command {
            GoalCommand::Status => crate::coder::goal::format_notice(store.get().as_ref()),
            GoalCommand::Clear => {
                let cleared = store.clear();
                drop(store);
                if cleared {
                    self.inner.tools.remove_host_tool("goal");
                    self.refresh_system_prompt();
                    "Cleared active task goal.".to_string()
                } else {
                    "No active task goal to clear.".to_string()
                }
            }
            GoalCommand::Pause => match store.update_status(GoalStatus::Paused) {
                Some(goal) => format!("Paused task goal: \"{}\"", goal.objective),
                None => "No active task goal to pause.".to_string(),
            },
            GoalCommand::Resume => match store.update_status(GoalStatus::Active) {
                Some(goal) => format!("Resumed task goal: \"{}\"", goal.objective),
                None => "No task goal to resume.".to_string(),
            },
            GoalCommand::Set {
                objective,
                token_budget,
            } => {
                let goal = store.set(&objective, token_budget);
                drop(store);
                if !self
                    .inner
                    .tools
                    .list_tools()
                    .iter()
                    .any(|tool| tool.name == "goal")
                {
                    if let Err(refusal) = self
                        .inner
                        .tools
                        .add_host_tool(crate::coder::goal::host_tool(Arc::clone(&self.goal)))
                    {
                        return refusal;
                    }
                    self.refresh_system_prompt();
                }
                format!(
                    "Set active goal: \"{}\"{}\nCall /goal for details, or /goal clear to remove.",
                    goal.objective,
                    goal.token_budget
                        .map(|budget| {
                            format!(" (Budget: {} tokens)", crate::coder::goal::grouped(budget))
                        })
                        .unwrap_or_default()
                )
            }
        }
    }

    pub fn goal(&self) -> Option<crate::coder::goal::Goal> {
        self.goal.lock().ok().and_then(|store| store.get())
    }

    fn refresh_system_prompt(&mut self) {
        let content = system_prompt(&self.inner.tools.list_tools());
        if let Some(message) = self
            .inner
            .messages
            .first_mut()
            .filter(|message| message.role == "system")
        {
            message.content = Some(content);
        }
    }

    /// Run one turn, streaming everything it does down `tx`.
    ///
    /// Always ends with exactly one [`Control::Done`], so a frame cannot be
    /// left spinning over a turn that has finished.
    /// Run a turn for callers that do not own a UI generation counter.
    pub async fn execute_turn(&mut self, prompt: &str, tx: Sender<Control>) {
        let id = TurnId::new(self.next_turn);
        self.next_turn = self.next_turn.saturating_add(1);
        self.execute_turn_with_id(id, prompt, tx).await;
    }

    /// Run one turn under a generation assigned by the frame reducer.
    pub async fn execute_turn_with_id(&mut self, id: TurnId, prompt: &str, tx: Sender<Control>) {
        self.execute_turn_with_id_and_images(id, prompt, &[], tx)
            .await;
    }

    /// Run one turn with the images attached in the composer.
    pub async fn execute_turn_with_id_and_images(
        &mut self,
        id: TurnId,
        prompt: &str,
        images: &[ImageAttachment],
        tx: Sender<Control>,
    ) {
        let (cancel_tools, tool_cancellation) = tokio::sync::watch::channel(false);
        if let Ok(mut turns) = self.turn_router.lock() {
            turns.start(id, cancel_tools);
        }
        self.inner.set_tool_cancellation(tool_cancellation);
        // A fresh turn may hand work to an external agent again. The limit is
        // per user turn, not per session; the flag lives in the delegation
        // gate and is claimed by the `delegate` tool's `agent` path.
        if let Some(gate) = &self.inner.tools.delegation {
            gate.acp_spent.store(false, Ordering::SeqCst);
        }
        self.inner.drain_swarm_inbox().await;
        let sink: Sink = Arc::new(Mutex::new(tx));
        self.live_streaming.store(false, Ordering::Relaxed);
        let started = Instant::now();
        let goal_prompt = self
            .goal()
            .as_ref()
            .and_then(crate::coder::goal::continuation_prompt);
        let outgoing = goal_prompt
            .map(|goal| format!("{prompt}\n\n{goal}"))
            .unwrap_or_else(|| prompt.to_string());
        let result = self
            .inner
            // The rich stream observer above carries each wire delta. This
            // committed-answer callback remains for non-interactive callers
            // and would repeat the same text here.
            .execute_turn_with_images(&outgoing, images, |_| {})
            .await;

        if let Some(model) = &self.inner.last_model {
            send_turn(&sink, id, Control::Model(model.clone()));
        }
        if let Some(thread) = self.inner.thread() {
            send_turn(&sink, id, Control::Thread(thread.to_string()));
        }
        if self.inner.last_usage.reported() {
            send_turn(&sink, id, Control::Usage(self.inner.last_usage));
        }
        if let Ok(mut goal) = self.goal.lock() {
            goal.add_usage(
                self.inner.last_usage.total_tokens,
                u64::try_from((started.elapsed().as_millis() + 500) / 1_000).unwrap_or(u64::MAX),
            );
            send_turn(&sink, id, Control::Goal(goal.get()));
        }
        match result {
            Ok(answer) => {
                // The fallback for a path that answered without streaming, and
                // nothing else: an empty answer stays empty rather than
                // becoming a sentence somebody could read as a reply.
                if !answer.is_empty() && !self.live_streaming.load(Ordering::Relaxed) {
                    send_turn(&sink, id, Control::Chunk(answer));
                }
            }
            Err(error) => send_turn(&sink, id, Control::Failed(error.to_string())),
        }
        for failure in self.inner.record_failures.drain(..) {
            send_turn(&sink, id, Control::Notice(failure));
        }
        if let Ok(mut turns) = self.turn_router.lock() {
            turns.finish(id);
        }
        send_turn(&sink, id, Control::Done);
    }

    /// The observer fence shared with the terminal loop.
    pub fn turn_router(&self) -> SharedTurnRouter {
        Arc::clone(&self.turn_router)
    }

    /// Record and settle one interrupted turn after its transport has stopped.
    ///
    /// The server accepts an identical report more than once. This method
    /// records the generation after that idempotent report succeeds, so a lost
    /// response can retry and a later key or event cannot settle it again.
    pub async fn settle_cancellation(&mut self, id: TurnId) -> Result<Option<String>, Failure> {
        if self.settled_cancellations.contains(&id) {
            return Ok(None);
        }
        if self.recorded_cancellations.insert(id) {
            self.inner
                .note_interruption("The turn was canceled before it finished.")
                .await;
        }
        let spent = self.inner.finish().await?;
        self.settled_cancellations.insert(id);
        if let Some(spent) = spent {
            send(&self.sink, Control::Billed(spent.total_tokens));
        }
        Ok(self.inner.spend_line(spent))
    }

    /// End this session's thread by saying what it did, and say what the
    /// server billed.
    ///
    /// A report rather than a `DELETE`: a session that answered and left is not
    /// a cancellation, and filing it as one is what made every thread in the
    /// account's history read as cancelled (issue #106). It also leaves the
    /// thread resumable, which `DELETE` does not.
    ///
    /// Awaited by the caller rather than left to `Drop`: a thread left open
    /// holds its grant's remaining budget, and the `Drop` backstop can only
    /// spawn an ending this process may exit before polling.
    pub async fn finish(&mut self) -> Result<Option<String>, Failure> {
        let spent = self.inner.finish().await?;
        if let Some(spent) = spent {
            send(&self.sink, Control::Billed(spent.total_tokens));
        }
        Ok(self.inner.spend_line(spent))
    }

    /// The failures the runtime recorded and nothing has shown yet.
    ///
    /// [`Self::execute_turn`] drains these into notices; [`Self::finish`]
    /// cannot, because its caller is not a turn. `/logout` reads them so an
    /// ending that fell back to a cancellation says so on screen rather than
    /// only in the runtime's field.
    pub fn take_record_failures(&mut self) -> Vec<String> {
        self.inner.record_failures.drain(..).collect()
    }
}

/// The repository this session was opened in, as `owner/name`, when it is one.
///
/// Recorded on the thread so `oa coder --resume` has something to filter on. A
/// directory that is not an OpenAgents checkout has none, which is not an
/// error — the thread is simply not attributable to a repository.
fn repository() -> Option<String> {
    let endpoint = crate::auth::resolve_endpoint(None, None).ok()?;
    crate::repo::infer_repository(&endpoint.origin, None).ok()
}

/// The one-line header a tool call shows above its output box.
///
/// Built from the call's own arguments, so it says what was actually asked
/// for. A tool this does not know by name still gets a header rather than a
/// blank one, because a call with no header is a call the reader cannot see.
pub fn tool_title(name: &str, arguments: &str) -> String {
    let parsed: serde_json::Value =
        serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null);
    let string = |key: &str| {
        parsed
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };

    let detail = match name {
        "shell" | "bash" => string("command"),
        // The path, never the content: `write` carries a whole file in its
        // arguments, and the fallthrough below would put it in the header.
        "read" | "write" | "edit" => string("path"),
        "skill" => string("name"),
        "openagents" => parsed.get("args").and_then(|v| v.as_array()).map(|args| {
            args.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        }),
        "capability" => string("name").or_else(|| string("query")),
        "delegate" => {
            let verb = match string("agent") {
                Some(agent) => format!("delegate: {agent}"),
                None => "delegate".to_string(),
            };
            if let Some(description) = string("description") {
                return format!("{verb} {}", one_line(&description));
            }
            let count = parsed
                .get("count")
                .and_then(|v| v.as_u64())
                .filter(|n| *n > 1)
                .map(|n| format!("×{n} "))
                .unwrap_or_default();
            return match string("prompt") {
                Some(prompt) => format!("{verb} {count}{}", one_line(&prompt)),
                None => verb,
            };
        }
        // The note itself, never the JSON envelope. The fallthrough below
        // would have put `{"text":"..."}` in the header — the one tool whose
        // payload is prose read as the one header that was unreadable.
        // A plugin loaded through `capability` declares a tool under its own
        // name and over its own schema, so there is nothing general to read
        // out of it but the arguments themselves.
        "checkpoint" => string("text"),
        "swarm.inbox" | "swarm_inbox" => {
            let from = string("from");
            let kind = string("kind");
            let count = parsed
                .get("count")
                .and_then(|value| value.as_u64())
                .unwrap_or(1);
            return match (from, kind) {
                (Some(from), Some(kind)) if count > 1 => {
                    format!("swarm ← {from} [{kind}] +{}", count - 1)
                }
                (Some(from), Some(kind)) => format!("swarm ← {from} [{kind}]"),
                _ => "swarm inbox".to_string(),
            };
        }
        "swarm_send" => {
            return match string("to") {
                Some(to) => format!("swarm → {to}"),
                None => "swarm send".to_string(),
            };
        }
        "swarm_list" => return "swarm list".to_string(),
        _ => (parsed != serde_json::Value::Null).then(|| parsed.to_string()),
    };

    match detail {
        Some(detail) => format!("{name} {}", one_line(&detail)),
        None => name.to_string(),
    }
}

/// The first line of `text`, marked when there was more.
fn one_line(text: &str) -> String {
    let first = text.lines().next().unwrap_or_default().trim();
    if text.lines().nth(1).is_some() {
        format!("{first} …")
    } else {
        first.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_swarm_inbox_title_names_the_sender_and_kind() {
        let title = tool_title(
            "swarm.inbox",
            r#"{"from":"session-b","kind":"question","count":1}"#,
        );
        assert_eq!(title, "swarm ← session-b [question]");
        assert!(!title.contains('>'));
    }

    /// The prompt is the product. A merge that reworded it would pass every
    /// other test in this crate.
    #[test]
    fn the_system_prompt_opens_with_the_terse_instructions_unchanged() {
        let prompt = system_prompt(&[]);
        assert!(
            prompt.starts_with(prompt::CODER_LITE_INSTRUCTIONS),
            "the instructions were not carried verbatim: {prompt}"
        );
        assert!(prompt::CODER_LITE_INSTRUCTIONS.contains("no greetings"));
        assert!(prompt::CODER_LITE_INSTRUCTIONS.contains("no unnecessary padding"));
    }

    /// A model told it has tools it does not have will claim to have run them.
    #[test]
    fn the_prompt_names_every_declared_tool_and_claims_no_others() {
        let tools = vec![
            ToolDefinition {
                name: "shell".to_string(),
                description: String::new(),
                parameters: serde_json::json!({}),
            },
            ToolDefinition {
                name: "skill".to_string(),
                description: String::new(),
                parameters: serde_json::json!({}),
            },
        ];
        let prompt = system_prompt(&tools);
        assert!(
            prompt.contains("You have 2 tools, and no others:"),
            "{prompt}"
        );
        assert!(prompt.contains("- `shell`"), "{prompt}");
        assert!(prompt.contains("- `skill`"), "{prompt}");

        let none = system_prompt(&[]);
        assert!(none.contains("You have no tools in this session"), "{none}");
    }

    #[test]
    fn a_tool_header_says_what_the_call_asked_for() {
        assert_eq!(
            tool_title("shell", r#"{"command":"cargo test -p Coder"}"#),
            "shell cargo test -p Coder"
        );
        assert_eq!(tool_title("skill", r#"{"name":"effect"}"#), "skill effect");
        assert_eq!(
            tool_title("openagents", r#"{"args":["issue","list"]}"#),
            "openagents issue list"
        );
        assert_eq!(
            tool_title("delegate", r#"{"prompt":"read it","count":3}"#),
            "delegate ×3 read it"
        );
        assert_eq!(
            tool_title(
                "delegate",
                r#"{"prompt":"read it","count":3,"description":"Audit auth module"}"#
            ),
            "delegate Audit auth module"
        );
        assert_eq!(
            tool_title("delegate", r#"{"prompt":"read it","agent":"grok"}"#),
            "delegate: grok read it"
        );
        assert_eq!(
            tool_title("delegate", r#"{"agent":"grok"}"#),
            "delegate: grok"
        );
        // A multi-line command is one line on the header, and says so.
        assert_eq!(
            tool_title("shell", "{\"command\":\"one\\ntwo\"}"),
            "shell one …"
        );
        // Arguments that will not parse are not a reason to draw no header.
        assert_eq!(tool_title("shell", "not json"), "shell");
        // The checkpoint header is the note, not the JSON envelope carrying
        // it — the note is prose, and `{"text":"..."}` in a header is the
        // one line a reader cannot use.
        assert_eq!(
            tool_title("checkpoint", r#"{"text":"issue 228 done. Next: none."}"#),
            "checkpoint issue 228 done. Next: none."
        );
        // No note, no detail: the bare name, never an empty envelope.
        assert_eq!(tool_title("checkpoint", "{}"), "checkpoint");
    }

    #[test]
    fn a_goal_adds_its_tool_and_clear_removes_it() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut session = Session::open_at(
            Lane::Flash,
            "flash",
            None,
            Vec::new(),
            "http://127.0.0.1:1/api/v1".to_string(),
            Some("test-token".to_string()),
            false,
            tx,
        );
        assert!(
            session
                .inner
                .tools
                .list_tools()
                .iter()
                .all(|tool| tool.name != "goal")
        );

        let notice = session.goal_command("/goal --budget 500 finish the port");
        assert!(notice.contains("Set active goal"), "{notice}");
        assert!(
            session
                .inner
                .tools
                .list_tools()
                .iter()
                .any(|tool| tool.name == "goal")
        );
        assert!(
            session.inner.messages[0]
                .content
                .as_deref()
                .unwrap_or_default()
                .contains("- `goal`")
        );

        assert_eq!(
            session.goal_command("/goal clear"),
            "Cleared active task goal."
        );
        assert!(session.goal().is_none());
        assert!(
            session
                .inner
                .tools
                .list_tools()
                .iter()
                .all(|tool| tool.name != "goal")
        );
    }

    #[tokio::test]
    async fn the_model_can_complete_the_active_goal() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut session = Session::open_at(
            Lane::Flash,
            "flash",
            None,
            Vec::new(),
            "http://127.0.0.1:1/api/v1".to_string(),
            Some("test-token".to_string()),
            false,
            tx,
        );
        session.goal_command("/goal finish the port");
        let output = session
            .inner
            .tools
            .execute_tool(&crate::tools::ToolCall {
                id: "goal-call".to_string(),
                name: "goal".to_string(),
                arguments: serde_json::json!({"action": "complete"}),
            })
            .await;
        assert!(!output.is_error, "{}", output.output);
        assert!(output.output.contains("marked as completed"));
        assert_eq!(
            session.goal().unwrap().status,
            crate::coder::goal::GoalStatus::Completed
        );
    }

    #[test]
    fn cancel_signals_the_matching_turn_once_and_counts_its_tools() {
        let mut router = TurnRouter::default();
        let id = TurnId::new(7);
        let (stop, cancel) = tokio::sync::watch::channel(false);
        router.start(id, stop);
        router.calls.insert("one".to_string(), id);
        router.calls.insert("two".to_string(), id);

        assert_eq!(router.cancel(id), 2);
        assert!(*cancel.borrow());
        assert_eq!(router.cancel(id), 2, "a repeated cancel is idempotent");
    }

    #[test]
    fn named_sol_uses_the_pro_origin_unless_a_dev_base_is_set() {
        unsafe {
            env::remove_var("OPENAGENTS_BASE_URL");
            env::remove_var("OPENAGENTS_API_BASE");
            env::remove_var("OPENAGENTS_PRO_ORIGIN");
        }
        assert_eq!(
            api_base_for(&Lane::Named("gpt-5.6-sol".into())),
            "https://pro.openagents.com/api/v1"
        );
        assert_eq!(
            api_base_for(&Lane::Named("gpt-5.6-terra".into())),
            "https://pro.openagents.com/api/v1"
        );
        assert_eq!(
            api_base_for(&Lane::Named("gpt-5.6-luna".into())),
            "https://pro.openagents.com/api/v1"
        );
        assert_ne!(
            api_base_for(&Lane::Flash),
            "https://pro.openagents.com/api/v1"
        );
        unsafe {
            env::set_var("OPENAGENTS_BASE_URL", "http://127.0.0.1:4100/api/v1");
        }
        assert_eq!(
            api_base_for(&Lane::Named("gpt-5.6-sol".into())),
            "http://127.0.0.1:4100/api/v1"
        );
        unsafe {
            env::remove_var("OPENAGENTS_BASE_URL");
        }
    }
}
