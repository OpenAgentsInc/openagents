//! Run keepers in the engine (#11930, `docs/desktop/move-running-agents.md`).
//!
//! Every run starts with a [`keeper::KeeperScope`], so a driver that can run
//! its agent under a keeper does. On boot the engine finds the keepers still
//! running, leaves their chats out of [`SessionsEngine::recover_stale`], and
//! picks each run back up through [`Harness::attach`]: the run continues in
//! the same transcript entry, and the keeper replays every frame after the
//! last one the old engine handled. Questions the old engine asked are
//! withdrawn, and the driver asks the ones still pending again.
//!
//! The hooks into the upstream `sessions.rs` are listed in
//! `apps/openagents/docs/downstream-patches.md`.

use super::*;

#[cfg(unix)]
use openagents_harness::keeper;

/// What the engine records about a run its keeper holds, so the next
/// engine can pick it up.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunMeta {
    chat_id: String,
    harness: HarnessId,
    user_message_id: String,
    request: RunRequest,
}

/// The binary and leading arguments that run a keeper. The app registers
/// itself at startup; without one, agents start without keepers.
static LAUNCH: OnceLock<(std::path::PathBuf, Vec<std::ffi::OsString>)> = OnceLock::new();

/// Run keepers with `exe` and `args` before `keeper`: the app passes its own
/// binary, tests their test binary. The first call wins.
pub fn set_keeper_launch(exe: std::path::PathBuf, args: Vec<std::ffi::OsString>) {
    let _ = LAUNCH.set((exe, args));
}

/// The binary and leading arguments that run keepers, if one registered.
/// Terminals use it too (#11951).
pub(crate) fn keeper_launch() -> Option<(std::path::PathBuf, Vec<std::ffi::OsString>)> {
    LAUNCH.get().cloned()
}

pub(super) struct Keepers {
    root: std::path::PathBuf,
    #[cfg(unix)]
    detach: keeper::Detach,
    /// Chats a keeper still holds that this engine is picking up. `None`
    /// while connecting; then the link, until the run takes it.
    #[cfg(unix)]
    adopting: Mutex<HashMap<String, Option<keeper::Link>>>,
    /// Runs that continue a transcript entry an earlier engine started.
    continuing: Mutex<std::collections::HashSet<String>>,
    /// The update handoff (#11946): while draining, new work is refused.
    drain: Drain,
}

/// The engine is handing its runs to a newer engine. `until` is when the
/// drain lifts by itself, so an update that never restarts the engine
/// doesn't leave it refusing work; `None` means not draining.
#[derive(Default)]
struct Drain {
    until: Mutex<Option<std::time::Instant>>,
    /// Dispatches that passed admission and haven't returned.
    admitted: std::sync::atomic::AtomicUsize,
}

/// What a dispatch holds while it runs, so a drain waits for it.
pub(super) struct Admitted(Option<Arc<Keepers>>);

impl Drop for Admitted {
    fn drop(&mut self) {
        if let Some(keepers) = &self.0 {
            keepers
                .drain
                .admitted
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

/// What a person sees when they send while the engine hands off.
pub const HANDOFF_REFUSAL: &str =
    "OpenAgents is restarting to finish an update. Send it again in a moment.";

/// Admit one dispatch, or refuse it while the engine hands off.
pub(super) fn admit(inner: &Inner) -> Result<Admitted, EngineError> {
    let Some(keepers) = inner.keepers.get() else {
        return Ok(Admitted(None));
    };
    // Count first, then look: a drain raises its flag first, then counts.
    keepers
        .drain
        .admitted
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let admitted = Admitted(Some(keepers.clone()));
    if keepers.drain.active() {
        return Err(EngineError::Other(HANDOFF_REFUSAL.into()));
    }
    Ok(admitted)
}

impl Drain {
    fn active(&self) -> bool {
        lock(&self.until).is_some_and(|until| std::time::Instant::now() < until)
    }

    fn raise(&self, lasts: std::time::Duration) {
        *lock(&self.until) = Some(std::time::Instant::now() + lasts);
    }

    fn lower(&self) {
        *lock(&self.until) = None;
    }
}

impl SessionsEngine {
    /// Keep agents under keepers in `root`, and pick up the runs whose
    /// keepers are still running. Call it before [`Self::recover_stale`].
    pub fn start_keepers(&self, root: std::path::PathBuf) {
        let keepers = Arc::new(Keepers {
            root,
            #[cfg(unix)]
            detach: keeper::Detach::default(),
            #[cfg(unix)]
            adopting: Mutex::new(HashMap::new()),
            continuing: Mutex::new(std::collections::HashSet::new()),
            drain: Drain::default(),
        });
        if self.inner.keepers.set(keepers.clone()).is_err() {
            return;
        }
        #[cfg(unix)]
        for found in keeper::client::discover(&keepers.root) {
            let Ok(meta) = serde_json::from_value::<RunMeta>(found.meta) else {
                continue;
            };
            if !alive(&found.dir) {
                continue; // its journal stays for whoever looks; recovery takes the chat
            }
            lock(&keepers.adopting).insert(meta.chat_id.clone(), None);
            let engine = self.clone();
            tokio::spawn(async move {
                let chat_id = meta.chat_id.clone();
                if let Err(error) = engine.adopt(found.dir, meta).await {
                    tracing::warn!(chat = %chat_id, %error, "could not pick up a running agent");
                    if let Some(keepers) = engine.inner.keepers.get() {
                        lock(&keepers.adopting).remove(&chat_id);
                    }
                    // Fall back to the restart recovery for this chat.
                    if let Err(error) = engine.recover_stale() {
                        tracing::warn!(chat = %chat_id, %error, "recovery after a failed pickup failed");
                    }
                }
            });
        }
    }

    #[cfg(unix)]
    async fn adopt(&self, dir: std::path::PathBuf, meta: RunMeta) -> Result<(), EngineError> {
        let link = keeper::client::connect(&dir, None).await?;
        tracing::info!(chat = %meta.chat_id, pid = link.pid, "picking up a running agent");
        // The old engine's questions can't be answered here; the driver asks
        // the ones the agent still waits on again.
        for request_id in open_questions(&self.inner.journal, &meta.chat_id) {
            self.inner
                .publish(&meta.chat_id, &AgentEvent::InputResolved { request_id });
        }
        // Between turns, the agent is a parked session waiting for its next
        // prompt: pick it up idle, or let it go if it has already exited.
        let between_turns = matches!(
            self.inner.journal.last_event(&meta.chat_id),
            Ok(Some((_, AgentEvent::Done { .. })))
        );
        if between_turns && link.exited {
            link.sender().finish()?;
            if let Some(keepers) = self.inner.keepers.get() {
                lock(&keepers.adopting).remove(&meta.chat_id);
            }
            return Ok(());
        }
        if let Some(keepers) = self.inner.keepers.get() {
            lock(&keepers.adopting).insert(meta.chat_id.clone(), Some(link));
        }
        self.dispatch_inner(
            &meta.chat_id,
            meta.harness,
            meta.request,
            Some(meta.user_message_id),
            false,
            between_turns,
        )
        .await?;
        Ok(())
    }

    /// True while a keeper holds this chat's agent or this engine is picking it up.
    pub(super) fn kept(&self, chat_id: &str) -> bool {
        let Some(keepers) = self.inner.keepers.get() else {
            return false;
        };
        #[cfg(unix)]
        {
            if lock(&keepers.adopting).contains_key(chat_id) {
                return true;
            }
            keeper::client::discover(&keepers.root)
                .into_iter()
                .any(|found| {
                    serde_json::from_value::<RunMeta>(found.meta)
                        .is_ok_and(|meta| meta.chat_id == chat_id)
                        && alive(&found.dir)
                })
        }
        #[cfg(not(unix))]
        {
            let _ = (keepers, chat_id);
            false
        }
    }

    /// Whether a newer engine could take over now without interrupting
    /// anything: no dispatch is starting, and every working run's agent is
    /// under a keeper the next engine picks up. Without keepers, whether no
    /// run is working.
    pub fn handoff_ready(&self) -> bool {
        let Some(keepers) = self.inner.keepers.get() else {
            return !self.any_active();
        };
        if keepers
            .drain
            .admitted
            .load(std::sync::atomic::Ordering::SeqCst)
            > 0
        {
            return false;
        }
        if lock(&self.inner.runs)
            .values()
            .any(|run| run.voice_active.live())
        {
            return false;
        }
        let active: Vec<String> = lock(&self.inner.statuses)
            .iter()
            .filter(|(_, session)| is_active(session))
            .map(|(chat_id, _)| chat_id.clone())
            .collect();
        if active.is_empty() {
            return true;
        }
        #[cfg(unix)]
        {
            let mut kept: std::collections::HashSet<String> =
                lock(&keepers.adopting).keys().cloned().collect();
            for found in keeper::client::discover(&keepers.root) {
                if let Ok(meta) = serde_json::from_value::<RunMeta>(found.meta)
                    && active.contains(&meta.chat_id)
                    && alive(&found.dir)
                {
                    kept.insert(meta.chat_id);
                }
            }
            active.iter().all(|chat_id| kept.contains(chat_id))
        }
        #[cfg(not(unix))]
        false
    }

    /// Refuse new work for `lasts` if [`Self::handoff_ready`]; the caller
    /// restarts the engine within that time. Returns false, refusing
    /// nothing, when a run would be interrupted.
    pub fn hold_for_handoff(&self, lasts: std::time::Duration) -> bool {
        let Some(keepers) = self.inner.keepers.get() else {
            return self.handoff_ready();
        };
        keepers.drain.raise(lasts);
        if self.handoff_ready() {
            return true;
        }
        keepers.drain.lower();
        false
    }

    /// Refuse new work and wait up to `within` for the runs a newer engine
    /// can't pick up to finish. True when the engine can hand off now; the
    /// drain then holds until [`Self::end_drain`] or the engine exits. False,
    /// with the drain lifted, when a run is still working.
    pub async fn drain(&self, within: std::time::Duration) -> bool {
        let Some(keepers) = self.inner.keepers.get() else {
            return self.handoff_ready();
        };
        // Long enough to outlast the wait and the restart that follows.
        keepers.drain.raise(within + HANDOFF_LEASE);
        let deadline = tokio::time::Instant::now() + within;
        loop {
            if self.handoff_ready() {
                return true;
            }
            if tokio::time::Instant::now() >= deadline {
                keepers.drain.lower();
                return false;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }

    /// Take new work again after a handoff that didn't happen.
    pub fn end_drain(&self) {
        if let Some(keepers) = self.inner.keepers.get() {
            keepers.drain.lower();
        }
    }

    /// The engine is going away on purpose: agents under keepers keep running.
    pub(super) fn detach_keepers(&self) {
        #[cfg(unix)]
        if let Some(keepers) = self.inner.keepers.get() {
            keepers.detach.raise();
        }
    }
}

/// How long a drain holds after the engine is ready to hand off: the time a
/// service manager has to restart it.
pub const HANDOFF_LEASE: std::time::Duration = std::time::Duration::from_secs(30);

/// Whether the run folder's keeper accepts connections.
#[cfg(unix)]
fn alive(dir: &std::path::Path) -> bool {
    let Ok(launch) = std::fs::read(dir.join("launch.json")) else {
        return false;
    };
    let Some(socket) = serde_json::from_slice::<serde_json::Value>(&launch)
        .ok()
        .and_then(|launch| launch["socket"].as_str().map(std::path::PathBuf::from))
    else {
        return false;
    };
    std::os::unix::net::UnixStream::connect(socket).is_ok()
}

/// Request ids of questions asked since the chat's last turn end and not resolved.
fn open_questions(journal: &RunJournal, chat_id: &str) -> Vec<String> {
    let mut resolved = std::collections::HashSet::new();
    let mut open = Vec::new();
    let Ok(events) = journal.events_rev(chat_id) else {
        return open;
    };
    for event in events.flatten() {
        match event.1 {
            AgentEvent::Done { .. } => break,
            AgentEvent::InputResolved { request_id } => {
                resolved.insert(request_id);
            }
            AgentEvent::InputRequested { request_id, .. } if !resolved.contains(&request_id) => {
                open.push(request_id);
            }
            _ => {}
        }
    }
    open
}

/// Start the harness for one run: pick up the chat's kept agent if this
/// engine is adopting it, else start a new run with a keeper scope.
pub(super) async fn start(
    inner: &Arc<Inner>,
    chat_id: &str,
    run_id: &str,
    user_message_id: &str,
    harness: &Arc<dyn Harness>,
    request: RunRequest,
    controls: RunControls,
    idle: bool,
) -> Result<
    futures::stream::BoxStream<'static, Result<AgentEvent, openagents_harness::HarnessError>>,
    openagents_harness::HarnessError,
> {
    #[cfg(unix)]
    if let Some(keepers) = inner.keepers.get() {
        let adopted = lock(&keepers.adopting).remove(chat_id).flatten();
        let meta = RunMeta {
            chat_id: chat_id.to_owned(),
            harness: harness.id(),
            user_message_id: user_message_id.to_owned(),
            request: request.clone(),
        };
        // Only a binary that registered itself runs keepers; any other
        // host (a test, a tool) starts its agents directly.
        let launch = LAUNCH.get().cloned();
        let mut meta = serde_json::to_value(&meta).unwrap_or_default();
        // A driver that reconnects to its agent, rather than replaying its
        // journal, learns what of the turn the earlier engine handed over.
        if adopted.is_some() {
            meta["handed"] = handed(&inner.journal, chat_id);
        }
        let scope = launch.map(|(keeper_exe, keeper_args)| keeper::KeeperScope {
            root: keepers.root.clone(),
            keeper_exe,
            keeper_args,
            meta,
            orphan_after: keeper::ORPHAN_AFTER,
            detach: keepers.detach.clone(),
        });
        if let Some(link) = adopted {
            lock(&keepers.continuing).insert(run_id.to_owned());
            let attach = harness.attach(request, controls, link);
            let attached = match scope {
                Some(scope) => keeper::scope(scope, attach).await,
                None => attach.await,
            };
            return attached.unwrap_or_else(|| {
                Err(openagents_harness::HarnessError::Protocol(
                    "this agent can't be picked up after a restart".into(),
                ))
            });
        }
        if let Some(scope) = scope {
            let start = async {
                if idle {
                    harness.start_idle(request, controls).await
                } else {
                    harness.run(request, controls).await
                }
            };
            return keeper::scope(scope, start).await;
        }
    }
    let _ = (inner, chat_id, run_id, user_message_id);
    if idle {
        harness.start_idle(request, controls).await
    } else {
        harness.run(request, controls).await
    }
}

/// What the run journal has of the chat's current turn: its text and
/// reasoning, the tool calls and results, and whether the turn had ended.
fn handed(journal: &RunJournal, chat_id: &str) -> serde_json::Value {
    let mut events = Vec::new();
    let mut between_turns = None;
    if let Ok(rev) = journal.events_rev(chat_id) {
        for event in rev.flatten() {
            if matches!(
                event.1,
                AgentEvent::Done { .. } | AgentEvent::Steered { .. }
            ) {
                between_turns.get_or_insert(matches!(event.1, AgentEvent::Done { .. }));
                break;
            }
            between_turns.get_or_insert(false);
            events.push(event.1);
        }
    }
    let (mut text, mut reasoning) = (String::new(), String::new());
    let (mut tools, mut results) = (Vec::new(), Vec::new());
    for event in events.iter().rev() {
        match event {
            AgentEvent::TextDelta { text: delta } => text.push_str(delta),
            AgentEvent::ReasoningDelta { text: delta } => reasoning.push_str(delta),
            AgentEvent::ToolCall { id, .. } => tools.push(id.clone()),
            AgentEvent::ToolResult { id, .. } => results.push(id.clone()),
            _ => {}
        }
    }
    serde_json::json!({
        "text": text,
        "reasoning": reasoning,
        "tools": tools,
        "results": results,
        "betweenTurns": between_turns.unwrap_or(false),
    })
}

/// Where a run that picks up a kept agent continues its transcript.
pub(super) struct Continued {
    /// The earlier engine's streaming entry, when the saved transcript has
    /// it: `(index, id, created_at, parts written)`.
    pub entry: Option<(usize, String, i64, Vec<MessagePart>)>,
    /// The turn's parts so far, folded from the run journal, which has every
    /// event the earlier engine handled, including ones its last save missed.
    pub parts: Vec<MessagePart>,
}

/// For a run that picks up a kept agent, where its transcript continues.
pub(super) fn continued_segment(
    inner: &Inner,
    chat_id: &str,
    run_id: &str,
    doc: &SessionDoc,
) -> Option<Continued> {
    let keepers = inner.keepers.get()?;
    if !lock(&keepers.continuing).remove(run_id) {
        return None;
    }
    let mut events: Vec<AgentEvent> = Vec::new();
    for event in inner.journal.events_rev(chat_id).ok()?.flatten() {
        if matches!(
            event.1,
            AgentEvent::Done { .. } | AgentEvent::Steered { .. }
        ) {
            break;
        }
        events.push(event.1);
    }
    let mut parts = Vec::new();
    for event in events.iter().rev() {
        match event {
            AgentEvent::Subagent { .. } => {}
            AgentEvent::SessionStarted { .. } if !parts.is_empty() => {}
            _ => fold_event_into_parts(&mut parts, event),
        }
    }
    let entries = doc.read_entries().unwrap_or_default();
    let entry = entries
        .iter()
        .enumerate()
        .rev()
        .find(|(_, e)| {
            e.role == MessageRole::Assistant
                && e.device_id == inner.device_id
                && e.status == Some(MessageStatus::Streaming)
        })
        .map(|(index, e)| (index, e.id.clone(), e.created_at, e.parts.clone()));
    if parts.is_empty()
        && let Some((_, _, _, written)) = &entry
    {
        parts = written.clone();
    }
    Some(Continued { entry, parts })
}
