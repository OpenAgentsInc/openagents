//! Agent wakes on the host that runs each agent's chat (#11965, #12044).
//! Every saved agent whose definition declares wakes gets its own engine,
//! keyed by its definition ID. Its account schedule is an ordinary scheduled
//! prompt for this computer, kept through the SDK; a check-in is a message
//! queued into the agent's chat the way the app sends one, so the existing
//! ledger runs it and a retry never doubles it. Grants come from the account
//! and are enforced on every command the agent's chat sends. See
//! `crate::agent_wakes` for the state, log and coalescing.
use super::*;
use crate::agent_wakes::{
    Adopt, AgentWakes, CLAIMS_QUERY, Declaring, GhIssue, LocalProgress, Wake, WakeEngine,
    claim_issues, tool_label,
};
use openagents_account::{
    Lease,
    openagents::schedules::{self, Schedule},
};
use openagents_proto::agent_wakes::{
    AgentWakeStatus, DecisionEntry, Grant, GrantRequest, MAX_EVERY_MINUTES, MIN_EVERY_MINUTES,
    NEVER_REFUSAL, Over, RecordDecision, RevokeGrant, Scope, SetWakes, Wake as WakeOn,
    WakeCandidate, WakeSettings, WakesStatus,
};
use std::sync::Arc;

const SCHEDULE_EVERY_MS: i64 = 60_000;
const GRANTS_EVERY_MS: i64 = 60_000;
const DECISIONS_EVERY_MS: i64 = 20_000;
const GITHUB_EVERY_MS: i64 = 5 * 60_000;
const REPO: &str = "OpenAgentsInc/openagents";

fn error(message: &str) -> EngineError {
    EngineError::Other(message.into())
}

/// `record_decision` from a chat that doesn't run a saved agent.
const NOT_AN_AGENT: &str = "Only a saved agent's own chat can write its decision log.";
/// `record_decision` from an agent without `log.write` (#12045).
pub(crate) const NO_LOG_GRANT: &str = "This agent can't keep a decision log yet. Turn on Decision log on its card in Settings, then try again.";

/// A chat's saved agent: its definition ID, from the chat config the host
/// stamps from the definition.
fn chat_agent(chat: &openagents_proto::Chat) -> Option<String> {
    chat.config
        .as_ref()
        .filter(|c| openagents_proto::saved_agent::is_agent_chat(Some(c)))?
        .model_options
        .get(openagents_proto::saved_agent::AGENT)?
        .as_str()
        .map(str::to_owned)
}

fn title_of(chat: &openagents_proto::Chat) -> String {
    chat.title
        .clone()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "Untitled".into())
}

/// An account grant in the app's terms; the two share one JSON shape.
fn from_account(grant: openagents_account::openagents::agents::AgentGrant) -> Option<Grant> {
    serde_json::from_value(serde_json::to_value(grant).ok()?).ok()
}

/// What the agent's definition declares to wake on, from the chat config.
fn declared_wakes(chat: &openagents_proto::Chat) -> (Vec<WakeOn>, Option<i64>) {
    openagents_proto::saved_agent::declared(chat.config.as_ref())
        .map(|d| (d.wakes, d.coalesce_secs.map(|s| i64::from(s) * 1000)))
        .unwrap_or_default()
}

impl DocHost {
    /// Open every agent's wake state under `root` (`<profile>/agent-wakes`), move
    /// state saved before #12044 from `legacy` (`<profile>/foreman`) once its
    /// agent is known, and watch every device's chat status for the chats
    /// the agents manage. Called once at engine assembly.
    pub fn start_agent_wakes(&self, root: std::path::PathBuf, legacy: std::path::PathBuf) {
        if self
            .inner
            .agent_wakes
            .set(Arc::new(AgentWakes::open(&root)))
            .is_err()
        {
            return;
        }
        let _ = self.inner.agent_wakes_legacy.set(legacy);
        self.start_working_state(&root);
        // Remote run commands (#12054) act for an agent only within the
        // same account grants its own chat's commands are held to.
        let weak = Arc::downgrade(&self.inner);
        self.set_run_command_authority(Some(Arc::new(
            move |agent: &str, scope: &str, target: &str| {
                let inner = weak
                    .upgrade()
                    .ok_or_else(|| "This computer is shutting down.".to_owned())?;
                DocHost { inner }.agent_allows(agent, Scope::grantable(scope)?, target)
            },
        )));
        let weak = Arc::downgrade(&self.inner);
        self.spawn_worker(async move {
            // The workspace and sessions attach after assembly.
            let (workspace, sessions) = loop {
                let Some(inner) = weak.upgrade() else { return };
                let host = DocHost { inner };
                if let (Some(w), Some(s)) = (host.workspace().cloned(), host.sessions()) {
                    break (w, s);
                }
                drop(host);
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            };
            let mut rows = workspace.merged_sessions_watch(sessions.watch_sessions());
            let chats = workspace.watch_chats();
            loop {
                let Some(inner) = weak.upgrade() else { return };
                let host = DocHost { inner };
                if let Some(wakes) = host.agent_wakes() {
                    let rows = rows.borrow_and_update().clone();
                    let chats = chats.borrow().clone();
                    let title = |id: &str| {
                        chats
                            .iter()
                            .find(|c| c.id == id)
                            .map(title_of)
                            .unwrap_or_else(|| "Untitled".into())
                    };
                    for engine in wakes.all() {
                        engine.observe_claim_chats(&chats, &rows, now_ms());
                        let chat = engine.chat();
                        let is_child = |id: &str| {
                            chats
                                .iter()
                                .any(|c| c.id == id && chat.is_some() && c.parent_chat_id == chat)
                        };
                        if let Err(e) = engine.observe(&rows, is_child, title, now_ms()) {
                            tracing::warn!(error = %e, "agent wakes: session look failed");
                        }
                    }
                }
                drop(host);
                if rows.changed().await.is_err() {
                    return;
                }
            }
        });
        let weak = Arc::downgrade(&self.inner);
        self.spawn_worker(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                let Some(inner) = weak.upgrade() else { return };
                let host = DocHost { inner };
                let Some(wakes) = host.agent_wakes().cloned() else {
                    return;
                };
                // One GitHub look feeds every agent that's due and wakes on
                // issues or claims.
                let mut due = Vec::new();
                for engine in wakes.all() {
                    let state = engine.state();
                    let wants = [
                        WakeOn::IssueOpened,
                        WakeOn::IssueClosed,
                        WakeOn::ClaimStale,
                        WakeOn::ClaimNoProgress,
                    ]
                    .into_iter()
                    .any(|w| engine.wakes_on(w));
                    if state.settings.chat_id.is_some()
                        && !state.settings.paused
                        && wants
                        && now_ms() - state.last_github >= GITHUB_EVERY_MS
                    {
                        let local = match host.local_progress(&engine).await {
                            Ok(local) => Some(local),
                            Err(e) => {
                                tracing::warn!(error = %e, "agent run look failed");
                                engine.set_problem(Some(format!(
                                    "Couldn't check terminal sessions: {e}"
                                )));
                                None
                            }
                        };
                        due.push((engine, local));
                    }
                }
                drop(host);
                if !due.is_empty() {
                    look_at_github(&due).await;
                }
            }
        });
    }

    /// What this computer sees of claims (#12032): each app chat's last
    /// output by the harness session a claim marker names, and builds
    /// running in worktrees of the agent's checkout named for an issue.
    async fn local_progress(&self, engine: &WakeEngine) -> Result<LocalProgress, EngineError> {
        let now = now_ms();
        let runs = tokio::task::spawn_blocking(coder_lease::runs::here)
            .await
            .map_err(|e| EngineError::Other(e.to_string()))?
            .map_err(|e| EngineError::Other(e.to_string()))?;
        let mut local = LocalProgress::default().with_runs(&runs);
        let (Some(workspace), Some(sessions)) = (self.workspace().cloned(), self.sessions()) else {
            return Ok(local);
        };
        let rows = workspace.merged_sessions(&sessions.watch_sessions().borrow());
        let chats = workspace.watch_chats().borrow().clone();
        for chat in &chats {
            let row = rows.iter().find(|r| r.chat_id == chat.id);
            let working = row
                .filter(|r| r.status == openagents_proto::SessionStatus::Working)
                .and_then(|r| r.started_at);
            let Some(last) = row
                .and_then(|r| r.last_output_at)
                .into_iter()
                .chain(working)
                .chain(chat.last_message_at)
                .map(|t| t.timestamp_millis())
                .max()
            else {
                continue;
            };
            // An app chat titled for an issue works it, whoever holds the
            // claim (a coordinator session often does).
            for number in crate::agent_wakes::issue_refs(chat.title.as_deref().unwrap_or("")) {
                let seen = local.titled.entry(number).or_insert(last);
                *seen = (*seen).max(last);
            }
            local.chats.insert(format!("chat:{}", chat.id), last);
            let (Some(config), Some(id)) = (
                chat.config.as_ref(),
                chat.harness_session_id.as_deref().filter(|s| !s.is_empty()),
            ) else {
                continue;
            };
            let Some(harness) = serde_json::to_value(config.harness)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
            else {
                continue;
            };
            local.chats.insert(format!("{harness}:{id}"), last);
        }
        // The checkout the agent's chat works in.
        let checkout = engine
            .chat()
            .and_then(|id| chats.iter().find(|c| c.id == id))
            .and_then(|c| {
                c.cwd.clone().or_else(|| {
                    let space = c.space_id.as_deref()?;
                    workspace.space(space).ok().flatten().map(|s| s.path)
                })
            });
        if let Some(checkout) = checkout {
            local.builds = builds_here(&checkout, now).await;
        }
        Ok(local)
    }

    /// Mark this engine as the headless daemon: Agent Lab then offers the
    /// computer as one that keeps an agent's chat and wakes running when a
    /// desktop app restarts (#12050).
    pub fn set_always_on(&self) {
        self.inner
            .always_on
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub(crate) fn is_always_on(&self) -> bool {
        self.inner
            .always_on
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub(crate) fn agent_wakes(&self) -> Option<&Arc<AgentWakes>> {
        self.inner.agent_wakes.get()
    }

    /// The chat of the only agent with check-ins on this computer, when
    /// exactly one has a chat chosen.
    pub(crate) fn only_checked_in_chat(&self) -> Option<String> {
        let chats: Vec<String> = self
            .agent_wakes()?
            .all()
            .into_iter()
            .filter_map(|e| e.chat())
            .collect();
        match chats.as_slice() {
            [one] => Some(one.clone()),
            _ => None,
        }
    }

    /// The engine whose agent runs in the chat `chat`, if any.
    pub(crate) fn wake_engine_for_chat(&self, chat: &str) -> Option<Arc<WakeEngine>> {
        self.agent_wakes()?.by_chat(chat)
    }

    /// The settings card's view: every agent's wake engine on this host.
    pub fn agent_wakes_status(&self) -> WakesStatus {
        let Some(wakes) = self.agent_wakes() else {
            return WakesStatus {
                problem: Some("Agent check-ins aren't available on this computer.".into()),
                ..WakesStatus::default()
            };
        };
        let chats = self
            .workspace()
            .map(|w| w.watch_chats().borrow().clone())
            .unwrap_or_default();
        let device = &self.inner.config.device_id;
        let candidates = chats
            .iter()
            .filter(|c| &c.device_id == device && !c.archived)
            .filter_map(|c| {
                Some(WakeCandidate {
                    chat_id: c.id.clone(),
                    title: title_of(c),
                    agent: chat_agent(c)?,
                })
            })
            .collect();
        let agents = wakes
            .all()
            .into_iter()
            .map(|engine| {
                let state = engine.state();
                let chat = state
                    .settings
                    .chat_id
                    .as_ref()
                    .and_then(|id| chats.iter().find(|c| &c.id == id));
                AgentWakeStatus {
                    agent: engine.agent().to_owned(),
                    agent_name: chat
                        .and_then(|c| {
                            openagents_proto::saved_agent::agent_name(
                                c.config.as_ref(),
                                c.title.as_deref(),
                            )
                        })
                        .unwrap_or_else(|| "Agent".into()),
                    chat_title: chat.map(title_of),
                    wakes: if state.wakes_on.is_empty() {
                        WakeOn::ALL.to_vec()
                    } else {
                        state.wakes_on.clone()
                    },
                    grants: state.grants.clone(),
                    next_check_in: engine.next_scheduled(),
                    problem: engine.problem(),
                    turned_on_at: state.turned_on_at,
                    decisions: engine.read(crate::agent_wakes::STATUS_ENTRIES),
                    log_kept_before_grant: !state.log_grant_moved
                        && !state
                            .grants
                            .iter()
                            .any(|g| g.scopes.contains(&Scope::LogWrite))
                        && engine.kept_a_log(),
                    settings: state.settings,
                }
            })
            .collect();
        WakesStatus {
            agents,
            candidates,
            problem: None,
        }
    }

    /// The owner's settings for one agent; its chat must run on this
    /// computer and belong to that agent.
    pub fn set_agent_wakes(&self, set: SetWakes) -> Result<WakesStatus, EngineError> {
        let wakes = self
            .agent_wakes()
            .ok_or_else(|| error("Agent check-ins aren't available on this computer."))?;
        if let Some(chat) = &set.settings.chat_id {
            let row = self
                .workspace()
                .ok_or_else(|| error("This computer is not ready."))?
                .chat(chat)?
                .ok_or_else(|| error("That chat no longer exists."))?;
            if row.device_id != self.inner.config.device_id {
                return Err(error("Choose a chat that runs on this computer."));
            }
            // A chat stamped with another agent is refused; an older chat
            // learns its agent's stamp on its next turn.
            if !openagents_proto::saved_agent::is_agent_chat(row.config.as_ref())
                || chat_agent(&row).is_some_and(|a| a != set.agent)
            {
                return Err(error("Choose a chat that runs this agent."));
            }
        }
        let engine = wakes.ensure(&set.agent)?;
        engine.set(set.settings, now_ms())?;
        engine.set_problem(None);
        Ok(self.agent_wakes_status())
    }

    /// The owner grants an agent scopes, through the account. Never-list
    /// scopes are refused here before anything is sent.
    pub async fn grant_agent(&self, request: GrantRequest) -> Result<WakesStatus, EngineError> {
        let scopes = request
            .scopes
            .iter()
            .map(|w| Scope::grantable(w))
            .collect::<Result<Vec<_>, _>>()
            .map_err(EngineError::Other)?;
        if scopes.is_empty() {
            return Err(error("Choose at least one power to grant."));
        }
        let engine = self
            .agent_wakes()
            .ok_or_else(|| error("Agent check-ins aren't available on this computer."))?
            .ensure(&request.agent)?;
        let lease = self.wakes_lease().ok_or_else(|| {
            error("We couldn't connect to your account to grant an agent powers. Try again.")
        })?;
        let agents = lease
            .client()
            .agents(lease.session())
            .map_err(|_| error("We couldn't connect to your account. Try again."))?;
        let words: Vec<String> = scopes.iter().map(|s| s.word().to_owned()).collect();
        let over = serde_json::from_value(
            serde_json::to_value(&request.over).map_err(|e| error(&e.to_string()))?,
        )
        .map_err(|e| error(&e.to_string()))?;
        agents
            .grant(&request.agent, words, over, &lease.cancel)
            .await
            .map_err(EngineError::AgentService)?;
        self.refresh_grants(&engine, &lease).await?;
        Ok(self.agent_wakes_status())
    }

    /// The owner revokes one of an agent's grants, through the account.
    pub async fn revoke_agent_grant(
        &self,
        revoke: RevokeGrant,
    ) -> Result<WakesStatus, EngineError> {
        let engine = self
            .agent_wakes()
            .and_then(|w| w.engine(&revoke.agent))
            .ok_or_else(|| error("That agent has no check-ins on this computer."))?;
        let lease = self.wakes_lease().ok_or_else(|| {
            error("We couldn't connect to your account to change an agent's powers. Try again.")
        })?;
        let agents = lease
            .client()
            .agents(lease.session())
            .map_err(|_| error("We couldn't connect to your account. Try again."))?;
        agents
            .revoke_grant(&revoke.agent, &revoke.grant, &lease.cancel)
            .await
            .map_err(EngineError::AgentService)?;
        self.refresh_grants(&engine, &lease).await?;
        Ok(self.agent_wakes_status())
    }

    fn wakes_lease(&self) -> Option<Lease> {
        self.openagents_lease()
    }

    /// Read the agent's grants from the account into its engine. A standing
    /// authority saved before grants, or one its definition asked for when
    /// this host turned its check-ins on, becomes an account grant once.
    async fn refresh_grants(&self, engine: &WakeEngine, lease: &Lease) -> Result<(), EngineError> {
        let agents = lease
            .client()
            .agents(lease.session())
            .map_err(|_| error("We couldn't connect to your account. Try again."))?;
        let state = engine.state();
        if state.authority.is_some() || state.wants_standing {
            let listed = agents
                .grants(engine.agent(), &lease.cancel)
                .await
                .map_err(EngineError::AgentService)?;
            let has_standing = listed
                .into_iter()
                .filter_map(from_account)
                .any(|g| g.is_standing());
            if !has_standing {
                let words = Scope::CHAT.iter().map(|s| s.word().to_owned()).collect();
                agents
                    .grant(
                        engine.agent(),
                        words,
                        openagents_account::openagents::agents::GrantOver::Account,
                        &lease.cancel,
                    )
                    .await
                    .map_err(EngineError::AgentService)?;
            }
            engine.update(|s| {
                s.authority = None;
                s.wants_standing = false;
            })?;
        }
        let mut grants: Vec<Grant> = agents
            .grants(engine.agent(), &lease.cancel)
            .await
            .map_err(EngineError::AgentService)?
            .into_iter()
            .filter_map(from_account)
            .collect();
        // A log kept here before `log.write` existed becomes that grant
        // once (#12045). An account that can't take it yet leaves the log
        // open and the rest of the refresh goes on; the next one tries again.
        let has_log = |g: &[Grant]| g.iter().any(|g| g.scopes.contains(&Scope::LogWrite));
        if !engine.state().log_grant_moved && !has_log(&grants) && engine.kept_a_log() {
            match agents
                .grant(
                    engine.agent(),
                    vec![Scope::LogWrite.word().to_owned()],
                    openagents_account::openagents::agents::GrantOver::Account,
                    &lease.cancel,
                )
                .await
            {
                Ok(_) => {
                    engine.update(|s| s.log_grant_moved = true)?;
                    grants = agents
                        .grants(engine.agent(), &lease.cancel)
                        .await
                        .map_err(EngineError::AgentService)?
                        .into_iter()
                        .filter_map(from_account)
                        .collect();
                }
                Err(e) => tracing::warn!(error = %e, "agent decision log: grant not moved yet"),
            }
        } else if has_log(&grants) && !engine.state().log_grant_moved {
            engine.update(|s| s.log_grant_moved = true)?;
        }
        engine.set_grants(grants, now_ms())
    }

    /// An agent's own record of a check-in (#12045). Any saved agent's chat
    /// may write to that agent's log, keyed by its definition ID, while the
    /// account grants it `log.write`; nothing here reads its name.
    pub async fn record_agent_decision(&self, record: RecordDecision) -> Result<(), EngineError> {
        let engine = self
            .decision_log_for(record.origin_chat_id.as_deref())
            .await?;
        if !engine.may_log() {
            return Err(error(NO_LOG_GRANT));
        }
        let now = now_ms();
        if let (Some(items), Some(chat)) = (record.working, record.origin_chat_id.as_deref()) {
            self.save_working_state(chat, items, now)?;
        }
        engine.record(
            &record.saw,
            &record.decided,
            &record.did,
            record.judgments,
            now,
        )
    }

    /// The wake engine whose decision log the chat `chat` writes: the agent
    /// with check-ins in it, else the saved agent the chat runs, whose
    /// grants are read from the account the first time.
    async fn decision_log_for(&self, chat: Option<&str>) -> Result<Arc<WakeEngine>, EngineError> {
        let chat = chat.ok_or_else(|| error(NOT_AN_AGENT))?;
        if let Some(engine) = self.wake_engine_for_chat(chat) {
            return Ok(engine);
        }
        let row = self
            .workspace()
            .ok_or_else(|| error("This computer is not ready."))?
            .chat(chat)?
            .filter(|row| row.device_id == self.inner.config.device_id)
            .ok_or_else(|| error(NOT_AN_AGENT))?;
        let agent = chat_agent(&row).ok_or_else(|| error(NOT_AN_AGENT))?;
        let wakes = self
            .agent_wakes()
            .ok_or_else(|| error("Agent check-ins aren't available on this computer."))?;
        if let Some(engine) = wakes.engine(&agent) {
            return Ok(engine);
        }
        let engine = wakes.ensure(&agent)?;
        if let Some(lease) = self.wakes_lease()
            && let Err(e) = self.refresh_grants(&engine, &lease).await
        {
            tracing::warn!(error = %e, "agent decision log: grants not read");
        }
        Ok(engine)
    }

    /// Whether a command `origin` sends to `target` is within the grants of
    /// the agent that runs in `origin`, and remember the chat it acted on.
    /// Commands from other chats, and from turns the owner started, pass
    /// unchanged.
    pub(crate) fn agent_admit(
        &self,
        origin: Option<&str>,
        target: &str,
        payload: Option<&SessionCommandPayload>,
    ) -> Result<(), EngineError> {
        let Some(origin) = origin else {
            return Ok(());
        };
        let Some(engine) = self.wake_engine_for_chat(origin) else {
            return Ok(());
        };
        if target == origin {
            return Ok(());
        }
        let fresh = self
            .open(target)
            .ok()
            .and_then(|h| h.doc.read_entries().ok())
            .is_some_and(|e| e.is_empty());
        let scope = match payload {
            _ if fresh => Scope::ChatStart,
            Some(SessionCommandPayload::Steer { .. }) => Scope::ChatSteer,
            Some(SessionCommandPayload::Interrupt {}) => Scope::ChatStop,
            _ => Scope::ChatMessage,
        };
        let project = self
            .workspace()
            .and_then(|w| w.chat(target).ok().flatten())
            .and_then(|c| c.space_id);
        engine
            .admits(
                scope,
                target,
                project.as_deref(),
                self.in_check_in(&engine, origin),
            )
            .map_err(EngineError::Other)?;
        engine.manage(target)
    }

    /// Whether the agent `agent` (a definition ID) may use `scope` on the
    /// chat `target`, for callers that have already proved which agent acts,
    /// such as remote run commands (#12054).
    pub fn agent_allows(&self, _agent: &str, _scope: Scope, _target: &str) -> Result<(), String> {
        // No grant is needed (owner, 2026-10-18, #12135).
        Ok(())
    }

    /// Whether the agent's current turn answers a check-in, not the owner.
    fn in_check_in(&self, engine: &WakeEngine, chat: &str) -> bool {
        self.open(chat)
            .ok()
            .and_then(|h| h.doc.read_entries().ok())
            .and_then(|entries| {
                entries
                    .iter()
                    .rev()
                    .find(|e| e.role == openagents_doc::MessageRole::User)
                    .map(|e| engine.is_wake(&e.id))
            })
            .unwrap_or(false)
    }

    /// One pass from the host loop: move old state, turn wakes on for agents
    /// that ask, read grants, then for each agent mirror its account
    /// schedule and start a check-in when one is due.
    pub(super) async fn agent_wakes_tick(&self, lease: &Lease) -> Result<(), EngineError> {
        let Some(wakes) = self.agent_wakes().cloned() else {
            return Ok(());
        };
        let workspace = self
            .workspace()
            .ok_or_else(|| error("This computer is not ready."))?;
        let sessions = self
            .sessions()
            .ok_or_else(|| error("This computer is not ready."))?;
        let chats = workspace.watch_chats().borrow().clone();
        self.migrate_legacy_wakes(&wakes, &chats)?;
        for engine in wakes.all() {
            self.drop_trial(&engine)?;
        }
        self.adopt_declaring(&wakes, &chats, now_ms())?;
        for engine in wakes.all() {
            if engine.throttle("decisions", DECISIONS_EVERY_MS, now_ms()) {
                self.mirror_decisions(&engine, lease).await;
            }
            let Some(chat) = engine.chat() else {
                continue;
            };
            let now = now_ms();
            let row = match workspace.chat(&chat)? {
                Some(row) if row.device_id == self.inner.config.device_id => row,
                Some(_) => {
                    engine.set_problem(Some(
                        "This agent's chat runs on another computer. Set it up there.".into(),
                    ));
                    continue;
                }
                None => {
                    engine.set_problem(Some(
                        "This agent's chat no longer exists. Choose it again.".into(),
                    ));
                    continue;
                }
            };
            let (wakes_on, coalesce) = declared_wakes(&row);
            engine.declare(wakes_on, coalesce)?;
            if engine.throttle("grants", GRANTS_EVERY_MS, now)
                && let Err(e) = self.refresh_grants(&engine, lease).await
            {
                tracing::warn!(error = %e, "agent wakes: grants not read");
            }
            if engine.throttle("schedule", SCHEDULE_EVERY_MS, now) {
                let name = openagents_proto::saved_agent::agent_name(
                    row.config.as_ref(),
                    row.title.as_deref(),
                )
                .unwrap_or_else(|| "Agent".into());
                match self.sync_wake_schedule(&engine, &name, lease, now).await {
                    Ok(()) => engine.set_problem(None),
                    Err(e) => {
                        tracing::warn!(error = %e, "agent wakes: schedule sync failed");
                        engine.set_problem(Some(
                            "This agent's schedule couldn't reach your account. It keeps running here."
                                .into(),
                        ));
                    }
                }
            }
            let busy = sessions.turn_in_flight(&chat);
            self.watch_own_turn(&engine, &chat, busy, now)?;
            self.tell_owner(&engine, &chat, now).await;
            self.start_check_in(&engine, &chat, busy, now, |text| {
                self.request_from_chat_row(&chat, text)
            })
            .await?;
        }
        Ok(())
    }

    /// Keep the agent's decision log and the account's the same (#12046).
    /// The account is the shared copy, so a failure only waits for the next
    /// pass.
    async fn mirror_decisions(&self, engine: &WakeEngine, lease: &Lease) {
        let Ok(agents) = lease.client().agents(lease.session()) else {
            return;
        };
        if let Err(e) = engine.mirror_decisions(&agents, &lease.cancel).await {
            tracing::warn!(error = %e, "agent wakes: decision log not mirrored");
        }
    }

    /// State saved before #12044 moves under its agent's definition ID once
    /// the chat it ran in shows which agent that is.
    fn migrate_legacy_wakes(
        &self,
        wakes: &AgentWakes,
        chats: &[openagents_proto::Chat],
    ) -> Result<(), EngineError> {
        let Some(legacy) = self.inner.agent_wakes_legacy.get() else {
            return Ok(());
        };
        let Some(chat) = AgentWakes::legacy_chat(legacy) else {
            return Ok(());
        };
        let Some(agent) = chats.iter().find(|c| c.id == chat).and_then(chat_agent) else {
            return Ok(());
        };
        if wakes.migrate(legacy, &agent)? {
            tracing::info!(agent = %agent, "agent wakes: moved state saved before grants");
        }
        Ok(())
    }

    /// On by default (#12030): for each agent whose definition declares
    /// check-ins and has no chat chosen here, pick its one chat on this
    /// computer and tell that chat so.
    fn adopt_declaring(
        &self,
        wakes: &AgentWakes,
        chats: &[openagents_proto::Chat],
        now: i64,
    ) -> Result<(), EngineError> {
        let declaring = declaring_chats(chats, &self.inner.config.device_id, |a| {
            self.is_lab_trial(a)
        });
        self.adopt_from(wakes, declaring, now)
    }

    /// Whether `assignment` is an Agent Lab trial on this computer.
    fn is_lab_trial(&self, assignment: &str) -> bool {
        self.inner
            .custom_agents
            .get()
            .is_some_and(|b| super::agent_lab::is_trial(&b.lab_root, assignment))
    }

    /// A trial chat this host picked by itself (#12109) loses check-ins,
    /// logged, so the next pick or bind is the real agent. An owner's own
    /// choice is left alone.
    fn drop_trial(&self, engine: &WakeEngine) -> Result<(), EngineError> {
        let state = engine.state();
        let (Some(chat), Some(_)) = (&state.settings.chat_id, state.turned_on_at) else {
            return Ok(());
        };
        let Some(row) = self.workspace().and_then(|w| w.chat(chat).ok().flatten()) else {
            return Ok(());
        };
        if !assignment_of(&row).is_some_and(|a| self.is_lab_trial(a)) {
            return Ok(());
        }
        tracing::warn!(chat = %chat, "agent wakes: dropping a test chat picked for check-ins");
        engine.set(
            WakeSettings {
                chat_id: None,
                ..state.settings
            },
            now_ms(),
        )
    }

    /// A chat the owner just bound to a saved agent (#12057) is their
    /// choice: when its agent declares check-ins and has no chat checking in
    /// here yet, it's picked.
    pub(super) fn adopt_bound(&self, chat: &openagents_proto::Chat) -> Result<(), EngineError> {
        let Some(wakes) = self.agent_wakes().cloned() else {
            return Ok(());
        };
        let declaring = declaring_chats(
            std::slice::from_ref(chat),
            &self.inner.config.device_id,
            |a| self.is_lab_trial(a),
        );
        self.adopt_from(&wakes, declaring, now_ms())
    }

    fn adopt_from(
        &self,
        wakes: &AgentWakes,
        declaring: Vec<Declaring>,
        now: i64,
    ) -> Result<(), EngineError> {
        let mut agents: Vec<String> = declaring.iter().filter_map(|d| d.agent.clone()).collect();
        agents.sort();
        agents.dedup();
        for agent in agents {
            let engine = wakes.ensure(&agent)?;
            let mine: Vec<Declaring> = declaring
                .iter()
                .filter(|d| d.agent.as_deref() == Some(agent.as_str()))
                .cloned()
                .collect();
            match engine.adopt(&mine, now)? {
                Adopt::Nothing => {}
                Adopt::Choose(titles) => engine.set_problem(Some(format!(
                    "{} chats on this computer run this agent: {}. Choose the one that checks in.",
                    titles.len(),
                    titles.join(", ")
                ))),
                Adopt::Picked(pick) => {
                    engine.set_problem(None);
                    let state = engine.state();
                    let text = format!(
                        "This computer turned on check-ins for {}, because its definition asks for them: it checks in every {} minutes{}. Turn them off in Settings → General → Check-ins.",
                        pick.title,
                        state.settings.every_minutes,
                        // Agents act without grants (#12135, #12155), so
                        // the note no longer mentions standing authority.
                        ""
                    );
                    if let Err(e) = self.runtime_note(&pick.chat_id, text) {
                        tracing::warn!(error = %e, "agent wakes: couldn't tell the chosen chat");
                    }
                }
            }
        }
        Ok(())
    }

    /// The stall ladder's last rung (#12032): tell the owner once per stall.
    /// Until then the check-in also puts it first in the agent's status.
    async fn tell_owner(&self, engine: &WakeEngine, chat: &str, now: i64) {
        let notices = match engine.owner_notices(now) {
            Ok(notices) => notices,
            Err(e) => {
                tracing::warn!(error = %e, "agent wakes: owner notices failed");
                return;
            }
        };
        for notice in notices {
            if let Err(error) = self
                .notify_owner_from_chat(chat, &notice.request_id, &notice.title, &notice.body)
                .await
            {
                tracing::warn!(
                    chat = %chat,
                    request = %notice.request_id,
                    error = %error,
                    "agent wakes: couldn't notify the owner"
                );
            }
        }
    }

    /// The host watchdog over the chat's own turn (#12029): a turn that went
    /// the stuck window with no output is interrupted once, and the next
    /// check-in says which call was stuck and for how long. Returns whether
    /// it stopped a turn.
    fn watch_own_turn(
        &self,
        engine: &WakeEngine,
        chat: &str,
        busy: bool,
        now: i64,
    ) -> Result<bool, EngineError> {
        let Some(stuck) = engine.watchdog(busy, now, || self.waiting_tool(chat))? else {
            return Ok(false);
        };
        tracing::warn!(chat = %chat, since = stuck.since, "agent wakes: stopping a stuck turn");
        if let Err(e) = self.queue_command(chat, SessionCommandPayload::Interrupt {}) {
            engine.set_problem(Some(format!("A stuck turn couldn't be stopped: {e}")));
            return Err(e);
        }
        Ok(true)
    }

    /// The tool call the chat's current turn is waiting on: the last
    /// unresolved tool in its newest assistant message.
    fn waiting_tool(&self, chat: &str) -> Option<String> {
        let entries = self.open(chat).ok()?.doc.read_entries().ok()?;
        let reply = entries
            .iter()
            .rev()
            .find(|e| e.role == openagents_doc::MessageRole::Assistant)?;
        reply.parts.iter().rev().find_map(|p| match p {
            openagents_doc::MessagePart::Tool {
                call,
                resolved: false,
                ..
            } => Some(tool_label(call)),
            _ => None,
        })
    }

    /// Start the check-in that is due, if any, as a message queued into the
    /// agent's chat. Returns its message id.
    async fn start_check_in(
        &self,
        engine: &WakeEngine,
        chat: &str,
        busy: bool,
        now: i64,
        request: impl FnOnce(&str) -> Option<openagents_proto::RunRequest>,
    ) -> Result<Option<String>, EngineError> {
        let Some(mut wake) = engine.due(busy, now)? else {
            return Ok(None);
        };
        // The agent's saved working state rides every check-in (#12057).
        let working = super::working_state::section(self.working_state(chat).as_ref(), now);
        wake.text = super::working_state::with_section(&wake.text, &working);
        #[cfg(unix)]
        let owner_wait = if let Some(agent) = self
            .agent_of(chat)
            .filter(|id| openagents_account::openagents::agents::valid_id(id))
        {
            use openagents_mcp::machine_status::{OwnerOptions, collect_with};
            let repository = self.workspace().and_then(|workspace| {
                let row = workspace.chat(chat).ok()??;
                row.cwd
                    .clone()
                    .or_else(|| {
                        workspace
                            .space(row.space_id.as_deref()?)
                            .ok()
                            .flatten()
                            .map(|s| s.path)
                    })
                    .map(std::path::PathBuf::from)
            });
            match collect_with(
                &agent,
                OwnerOptions {
                    repository,
                    ..OwnerOptions::default()
                },
            )
            .await
            {
                Ok(status) => {
                    wake.text.push_str("\n\n");
                    wake.text.push_str(&status.prompt_block());
                    Some((agent, status.owner_wait))
                }
                Err(error) => {
                    tracing::warn!(agent = %agent, error = %error, "couldn't read check-in facts");
                    wake.text.push_str(
                        "\n\nMachine and queue facts could not be read for this check-in.",
                    );
                    None
                }
            }
        } else {
            None
        };
        match self.queue_check_in(chat, &wake, request) {
            Ok(()) => {
                #[cfg(unix)]
                if let Some((agent, report)) = owner_wait {
                    self.notify_new_owner_items(chat, &agent, &report).await;
                }
                Ok(Some(wake.id))
            }
            Err(e) => {
                tracing::warn!(error = %e, "agent wakes: check-in not queued");
                engine.update(|s| s.open_wake = None)?;
                engine.append(&DecisionEntry::Outcome {
                    at: now,
                    id: wake.id,
                    status: "not started".into(),
                })?;
                engine.set_problem(Some(format!("A check-in couldn't start: {e}")));
                Ok(None)
            }
        }
    }

    fn queue_check_in(
        &self,
        chat: &str,
        wake: &Wake,
        request: impl FnOnce(&str) -> Option<openagents_proto::RunRequest>,
    ) -> Result<(), EngineError> {
        let handle = self.open(chat)?;
        let queued = handle.doc.read_commands()?.iter().any(|c| {
            matches!(&c.payload, SessionCommandPayload::Run { message_id, .. } if *message_id == wake.id)
        });
        if queued {
            return Ok(());
        }
        let request = request(&wake.text).ok_or_else(|| error("The agent's chat is not ready."))?;
        self.queue_command(
            chat,
            SessionCommandPayload::Run {
                request,
                message_id: wake.id.clone(),
            },
        )?;
        Ok(())
    }

    /// Keep the account's copy of the agent's schedule and this host's copy
    /// together: the newer change wins, as the account merges it. The
    /// schedule names the agent's definition ID, so it is found by agent,
    /// not by its prompt text.
    async fn sync_wake_schedule(
        &self,
        engine: &WakeEngine,
        name: &str,
        lease: &Lease,
        now: i64,
    ) -> Result<(), EngineError> {
        let client = lease
            .client()
            .schedules(lease.session())
            .map_err(|_| error("We couldn't connect to your account. Try again."))?;
        let device = self.inner.config.device_id.clone();
        let state = engine.state();
        let now_secs = (now / 1000).max(1) as u64;
        let mut changed = Vec::new();
        if state.schedule_dirty || state.schedule_id.is_none() {
            let id = match &state.schedule_id {
                Some(id) => id.clone(),
                None => schedules::new_id().map_err(EngineError::AgentService)?,
            };
            let updated = now_secs.max(state.schedule_updated + 1);
            changed.push(match &state.settings.chat_id {
                Some(chat) => Schedule {
                    id,
                    computer: device.clone(),
                    prompt: Some(format!(
                        "{name} check-in: look at what it manages, and act on what needs it."
                    )),
                    every_secs: Some(u64::from(state.settings.every_minutes) * 60),
                    chat: Some(chat.clone()),
                    chat_title: Some(name.to_owned()),
                    agent: Some(engine.agent().to_owned()),
                    paused: state.settings.paused,
                    updated,
                    ..Schedule::default()
                },
                None => Schedule {
                    id,
                    computer: device.clone(),
                    updated,
                    deleted: true,
                    ..Schedule::default()
                },
            });
        }
        let list = client
            .sync(&device, &changed, &lease.cancel)
            .await
            .map_err(EngineError::AgentService)?;
        if let Some(sent) = changed.first() {
            engine.update(|s| {
                s.schedule_id = (!sent.deleted).then(|| sent.id.clone());
                s.schedule_updated = sent.updated;
                s.schedule_dirty = false;
            })?;
            return Ok(());
        }
        // A change made elsewhere, such as pausing on the website.
        let Some(id) = state.schedule_id else {
            return Ok(());
        };
        let Some(theirs) = list.into_iter().find(|s| s.id == id) else {
            return Ok(());
        };
        if theirs.updated <= state.schedule_updated {
            return Ok(());
        }
        engine.update(|s| {
            s.schedule_updated = theirs.updated;
            if theirs.deleted {
                // Deleted on the website: stop until the owner turns it on again.
                s.schedule_id = None;
                s.settings.paused = true;
            } else {
                if s.settings.paused && !theirs.paused {
                    s.last_scheduled = now;
                }
                s.settings.paused = theirs.paused;
                if let Some(every) = theirs.every_secs {
                    s.settings.every_minutes =
                        ((every / 60) as u32).clamp(MIN_EVERY_MINUTES, MAX_EVERY_MINUTES);
                }
            }
        })?;
        Ok(())
    }
}

/// This computer's unarchived saved-agent chats whose agent's definition
/// declares check-ins. Read from the chat config the host stamps from the
/// definition, never from the chat's or agent's name. Agent Lab trial chats
/// (`is_trial` of their assignment) run a draft and never count (#12109).
fn declaring_chats(
    chats: &[openagents_proto::Chat],
    device: &str,
    is_trial: impl Fn(&str) -> bool,
) -> Vec<Declaring> {
    chats
        .iter()
        .filter(|c| c.device_id == device && !c.archived)
        .filter(|c| assignment_of(c).is_none_or(|a| !is_trial(a)))
        .filter_map(|c| {
            let declared = openagents_proto::saved_agent::declared(c.config.as_ref())?;
            declared.check_ins.then(|| Declaring {
                chat_id: c.id.clone(),
                title: title_of(c),
                agent: chat_agent(c),
                declared,
            })
        })
        .collect()
}

fn assignment_of(chat: &openagents_proto::Chat) -> Option<&str> {
    chat.config
        .as_ref()?
        .model_options
        .get(openagents_proto::saved_agent::ASSIGNMENT)?
        .as_str()
}

/// Ask the host's GitHub CLI what changed. A missing or signed-out `gh`
/// leaves only chat and schedule wake-ups.
async fn look_at_github(engines: &[(Arc<WakeEngine>, Option<LocalProgress>)]) {
    let now = now_ms();
    let listed = gh(&[
        "issue",
        "list",
        "-R",
        REPO,
        "--state",
        "all",
        "--limit",
        "100",
        "--search",
        "sort:updated-desc",
        "--json",
        "number,title,state",
    ])
    .await
    .and_then(|v| serde_json::from_value::<Vec<GhIssue>>(v).ok());
    let Some(listed) = listed else {
        // Try again at the next look rather than every half minute.
        for (engine, _) in engines {
            let _ = engine.update(|s| s.last_github = now);
        }
        return;
    };
    for (engine, _) in engines {
        if let Err(e) = engine.observe_issues(&listed, now) {
            tracing::warn!(error = %e, "agent wakes: issue look failed");
        }
    }
    // An unreadable run record is not evidence that its session is dead:
    // an agent without one keeps its previous claim observations until the
    // next successful read.
    if engines.iter().all(|(_, local)| local.is_none()) {
        return;
    }
    let query = format!("query={CLAIMS_QUERY}");
    if let Some(answer) = gh(&["api", "graphql", "-f", &query]).await {
        let issues = claim_issues(&answer);
        for (engine, local) in engines {
            let Some(local) = local else {
                continue;
            };
            if let Err(e) = engine.observe_claims(&issues, local, now) {
                tracing::warn!(error = %e, "agent wakes: claim look failed");
            }
        }
    }
}

/// Issues with a build running now in a worktree of `checkout` named for
/// them (#12032). Reads `git worktree list` and the working folders of
/// running `cargo` and `rustc` processes, each with a 10-second timeout.
async fn builds_here(checkout: &str, now: i64) -> std::collections::BTreeMap<u64, i64> {
    let worktrees = run("git", &["-C", checkout, "worktree", "list", "--porcelain"])
        .await
        .map(|out| parse_worktrees(&out))
        .unwrap_or_default();
    if worktrees.is_empty() {
        return Default::default();
    }
    let cwds = run(
        "lsof",
        &["-a", "-d", "cwd", "-c", "cargo", "-c", "rustc", "-Fn"],
    )
    .await
    .map(|out| parse_lsof_cwds(&out))
    .unwrap_or_default();
    builds_by_issue(&worktrees, &cwds, now)
}

async fn run(program: &str, args: &[&str]) -> Option<String> {
    let mut command = tokio::process::Command::new(program);
    command
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(std::time::Duration::from_secs(10), command.output())
        .await
        .ok()?
        .ok()?;
    // lsof exits 1 when nothing matches; its output is still the answer.
    String::from_utf8(output.stdout).ok()
}

/// `git worktree list --porcelain`: each worktree's folder and branch.
fn parse_worktrees(out: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut path: Option<String> = None;
    for line in out.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            if let Some(done) = path.replace(p.to_owned()) {
                found.push((done, String::new()));
            }
        } else if let Some(b) = line.strip_prefix("branch ") {
            if let Some(p) = path.take() {
                found.push((p, b.trim_start_matches("refs/heads/").to_owned()));
            }
        }
    }
    found.extend(path.map(|p| (p, String::new())));
    found
}

/// `lsof -Fn`: the `n` lines are the processes' working folders.
fn parse_lsof_cwds(out: &str) -> Vec<String> {
    out.lines()
        .filter_map(|l| l.strip_prefix('n'))
        .map(str::to_owned)
        .collect()
}

/// Issue numbers a worktree is named for: runs of 4 to 6 digits in its
/// folder name or branch, like `oa-wt-foreman-12029` or `agent/12029-fix`.
fn issue_numbers(path: &str, branch: &str) -> Vec<u64> {
    let folder = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    format!("{folder} {branch}")
        .split(|c: char| !c.is_ascii_digit())
        .filter(|run| (4..=6).contains(&run.len()))
        .filter_map(|run| run.parse().ok())
        .collect()
}

/// Issues whose worktree has a build running in it now.
fn builds_by_issue(
    worktrees: &[(String, String)],
    cwds: &[String],
    now: i64,
) -> std::collections::BTreeMap<u64, i64> {
    let mut builds = std::collections::BTreeMap::new();
    for (path, branch) in worktrees {
        let inside = |cwd: &String| std::path::Path::new(cwd).starts_with(path);
        if cwds.iter().any(inside) {
            for number in issue_numbers(path, branch) {
                builds.insert(number, now);
            }
        }
    }
    builds
}

async fn gh(args: &[&str]) -> Option<serde_json::Value> {
    let mut command = tokio::process::Command::new("gh");
    openagents_harness::compose_login_shell_path(&mut command);
    command
        .args(args)
        .env("GH_PROMPT_DISABLED", "1")
        .env("NO_COLOR", "1")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(std::time::Duration::from_secs(60), command.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }
    serde_json::from_slice(&output.stdout).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_doc::{MessagePart, MessageRole, MessageStatus, SessionMessageEntry};
    use openagents_proto::agent_wakes::{WakeReason, WakeSettings};
    use openagents_proto::{Session, SessionStatus};

    const OWN: &str = "f0000000-0000-4000-8000-000000000001";
    const AGENT: &str = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
    const WORKER: &str = "f0000000-0000-4000-8000-000000000002";
    const MIN: i64 = 60_000;

    /// #12032, #12036, and #12040: final stalls and new owner items notify
    /// once, using the saved agent definition and its own chat.
    #[tokio::test]
    async fn final_stalls_and_new_owner_items_notify_once_through_the_sdk() {
        use crate::HarnessRegistry;
        use axum::{
            Json, Router,
            routing::{get, post},
        };
        use openagents_account::{
            Controller,
            desktop::{self, MemoryStore},
            openagents::{Client, Deployment, owner_notifications::NotifyOwner},
        };
        use openagents_proto::{
            ChatConfig, SandboxLevel,
            saved_agent::{AGENT, ASSIGNMENT},
        };
        use serde_json::json;

        let root = tempfile::tempdir().unwrap();
        let registry = Arc::new(HarnessRegistry::new());
        let core = crate::EngineCore::assemble_with_identity(
            root.path(),
            registry.clone(),
            HarnessId::Mock,
            None,
            "acct_stall_fixture",
            "stall_fixture",
        )
        .unwrap();
        let host = &core.doc_host;
        let agent = "b".repeat(64);
        let assignment = "a".repeat(64);
        let definition = json!({
            "id": agent, "revision": 1, "deleted": false,
            "spec": {"name": "Any helper", "description": "", "instructions": "Check the work",
                "runtime": "codex", "model": null, "access": "read_only", "fallbacks": []}
        });
        let binding = json!({
            "id": assignment, "revision": 1,
            "spec": {"agent": agent, "agent_revision": 1,
                "host": host.inner.config.device_id, "project": null, "task": "Check work"},
            "definition": definition, "chat": OWN, "state": "queued", "commands": [],
            "result": null, "archived": false, "active_command": null, "after_seq": 0
        });
        let received = Arc::new(std::sync::Mutex::new(Vec::<NotifyOwner>::new()));
        let captured = received.clone();
        // A local fake replaces the account's gateway boundary. Service tests
        // separately exercise delivery through the NIP-98 fake push gateway.
        let api = Router::new()
            .route(
                "/v1/account",
                get(|| async {
                    Json(
                        json!({"account": {"id": "acct_stall_fixture", "label": "Stall fixture"},
                    "workspaces": []}),
                    )
                }),
            )
            .route(
                &format!("/v1/agent-assignments/{assignment}"),
                get(move || {
                    let binding = binding.clone();
                    async move { Json(binding) }
                }),
            )
            .route(
                &format!("/v1/agent-definitions/{agent}"),
                get(move || {
                    let definition = definition.clone();
                    async move { Json(definition) }
                }),
            )
            .route(
                "/v1/notify",
                post(move |Json(note): Json<NotifyOwner>| {
                    let captured = captured.clone();
                    async move {
                        captured.lock().unwrap().push(note);
                        Json(json!({"ok": true}))
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, api).await.unwrap() });
        let controller = Controller::new(
            Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap(),
            Arc::new(MemoryStore::default()),
            root.path().join("account/signed-out"),
        );
        controller
            .import_credential(format!("sess_{:064x}", 9004))
            .await;
        assert!(controller.lease().is_some());
        desktop::register(root.path(), &controller);
        let auth = crate::Engine::build_auth(&crate::EngineConfig {
            data_dir: root.path().into(),
            edge_url: origin,
            edge_token: None,
            ipc_port: 0,
            default_harness: HarnessId::Mock,
            org_id: None,
            workos_client_id: None,
        })
        .await;
        host.start_custom_agents(auth, registry, root.path().join("labs"));
        core.workspace
            .create_chat(
                OWN,
                None,
                Some(&host.inner.config.device_id),
                Some(ChatConfig {
                    harness: HarnessId::Mock,
                    model: None,
                    reasoning: None,
                    model_options: [
                        (ASSIGNMENT.into(), json!(assignment)),
                        (AGENT.into(), json!(agent)),
                    ]
                    .into_iter()
                    .collect(),
                    sandbox: SandboxLevel::WorkspaceWrite,
                }),
                Some(root.path().to_string_lossy().into_owned()),
            )
            .unwrap();
        // Keep this clock-driven ladder separate from the host's background loop.
        let foreman = WakeEngine::open(&root.path().join("fixture-ladder"), &agent);
        foreman
            .set(
                WakeSettings {
                    chat_id: Some(OWN.into()),
                    stall_minutes: 20,
                    ..WakeSettings::default()
                },
                0,
            )
            .unwrap();
        let working = Session {
            started_at: chrono::DateTime::from_timestamp_millis(0),
            last_output_at: chrono::DateTime::from_timestamp_millis(0),
            ..row(WORKER, SessionStatus::Working, None)
        };
        foreman
            .observe(&[working], |_| true, |_| "Worker".into(), 20 * MIN)
            .unwrap();
        for minute in [20, 30, 39] {
            host.tell_owner(&foreman, OWN, minute * MIN).await;
            assert!(
                received.lock().unwrap().is_empty(),
                "only the last rung notifies"
            );
        }
        host.tell_owner(&foreman, OWN, 40 * MIN).await;
        host.tell_owner(&foreman, OWN, 50 * MIN).await;
        {
            let received = received.lock().unwrap();
            assert_eq!(received.len(), 1, "one request per stall");
            let request = &received[0];
            assert_eq!(request.note.agent, agent);
            assert_eq!(request.note.chat, OWN);
            assert_eq!(request.note.title, "Work has stalled");
            assert!(request.note.body.contains("Worker"));
            assert!(request.note.body.contains("40 minutes"));
            assert_eq!(request.note.deep_link(), format!("openagents://chat/{OWN}"));
            assert!(openagents_account::openagents::agents::valid_id(
                &request.request_id
            ));
        }
        #[cfg(unix)]
        {
            received.lock().unwrap().clear();
            let mut report: openagents_mcp::machine_status::OwnerStatus =
                serde_json::from_value(json!({
                    "repository": root.path(),
                    "entries": [{"id": "c".repeat(64), "what": "Approve the store release",
                        "source": "NEEDS_OWNER.md", "line": 1, "since": 100, "age_seconds": 400}],
                    "deploy_permission": null, "errors": [], "read_sources": ["NEEDS_OWNER.md"]
                }))
                .unwrap();
            host.notify_new_owner_items(OWN, &agent, &report).await;
            report.entries[0].age_seconds = Some(500);
            host.notify_new_owner_items(OWN, &agent, &report).await;
            let notes = received.lock().unwrap();
            assert_eq!(
                notes.len(),
                1,
                "one new owner item sends one request; age changes stay silent"
            );
            assert_eq!(notes[0].note.agent, agent);
            assert_eq!(notes[0].note.chat, OWN);
            assert_eq!(notes[0].note.title, "Your help is needed");
            assert_eq!(notes[0].note.body, "Approve the store release");
            assert!(openagents_account::openagents::agents::valid_id(
                &notes[0].request_id
            ));
        }
        core.shutdown().await;
        server.abort();
    }

    fn request(cwd: &str) -> impl FnOnce(&str) -> Option<openagents_proto::RunRequest> + '_ {
        move |text| {
            Some(openagents_proto::RunRequest {
                prompt: text.into(),
                harness: Some(HarnessId::Mock),
                model: None,
                reasoning: None,
                model_options: Default::default(),
                cwd: cwd.into(),
                sandbox: openagents_proto::SandboxLevel::WorkspaceWrite,
                auto_approve: false,
                resume: None,
                attachments: vec![],
                worktree: None,
                mcp: None,
            })
        }
    }

    fn runs(host: &DocHost) -> Vec<(String, String)> {
        host.open(OWN)
            .unwrap()
            .doc()
            .read_commands()
            .unwrap()
            .into_iter()
            .filter_map(|c| match c.payload {
                SessionCommandPayload::Run {
                    request,
                    message_id,
                } => Some((message_id, request.prompt)),
                _ => None,
            })
            .collect()
    }

    fn row(chat: &str, status: SessionStatus, completed: Option<&str>) -> Session {
        Session {
            last_completed_turn: completed.map(str::to_owned),
            chat_id: chat.into(),
            device_id: "fixture-mac".into(),
            status,
            started_at: None,
            updated_at: chrono::Utc::now(),
            running_subagents: 0,
            background_tasks: 0,
            last_output_at: None,
        }
    }

    fn chat(
        id: &str,
        device: &str,
        title: &str,
        options: serde_json::Value,
    ) -> openagents_proto::Chat {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "deviceId": device,
            "title": title,
            "archived": false,
            "createdAt": "2026-10-17T00:00:00Z",
            "config": {
                "harness": "claude-code",
                "modelOptions": options,
                "sandbox": "danger-full-access"
            }
        }))
        .unwrap()
    }

    /// #12030: only this computer's chats whose agent declares check-ins
    /// count, whatever they're called.
    #[test]
    fn declaring_chats_come_from_the_definition_not_the_name() {
        use openagents_proto::saved_agent::{AGENT, AGENT_NAME, ASSIGNMENT, AUTONOMY};
        let declares = serde_json::json!({
            ASSIGNMENT: "a1", AGENT_NAME: "Night shift", AGENT: "def-1",
            AUTONOMY: {"checkIns": true, "everyMinutes": 10, "authority": true}
        });
        let mut archived = chat("c4", "mac", "Old", declares.clone());
        archived.archived = true;
        let chats = [
            chat(
                "c1",
                "mac",
                "Foreman",
                serde_json::json!({ASSIGNMENT: "a0", AGENT_NAME: "Foreman"}),
            ),
            chat("c2", "mac", "Night shift", declares.clone()),
            chat("c3", "other-mac", "Night shift", declares.clone()),
            archived,
            chat("c5", "mac", "Plain", serde_json::json!({})),
        ];
        let found = declaring_chats(&chats, "mac", |_| false);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].chat_id, "c2");
        // #12109: an Agent Lab trial of the same agent never counts.
        assert!(declaring_chats(&chats, "mac", |a| a == "a1").is_empty());
        assert_eq!(found[0].agent.as_deref(), Some("def-1"));
        assert_eq!(found[0].declared.every_minutes, Some(10));
    }

    /// #12049: a fork carries its original's declared autonomy, so on the
    /// same computer it wakes on exactly the same events.
    #[test]
    fn a_fork_wakes_on_the_same_events_as_its_original() {
        use openagents_proto::saved_agent::{AGENT, AGENT_NAME, ASSIGNMENT, AUTONOMY};
        let autonomy = serde_json::json!({
            "checkIns": true, "everyMinutes": 10, "authority": true,
            "wakes": ["chat_finished", "issue_opened", "claim_stale"], "coalesceSecs": 60
        });
        let official = chat(
            "c1",
            "mac",
            "Foreman",
            serde_json::json!({ASSIGNMENT: "a1", AGENT_NAME: "Foreman", AGENT: "def-official", AUTONOMY: autonomy}),
        );
        let fork = chat(
            "c2",
            "mac",
            "Release Foreman",
            serde_json::json!({ASSIGNMENT: "a2", AGENT_NAME: "Release Foreman", AGENT: "def-fork", AUTONOMY: autonomy}),
        );
        let (wakes, coalesce) = declared_wakes(&official);
        assert_eq!(wakes.len(), 3);
        assert_eq!(coalesce, Some(60_000));
        assert_eq!(declared_wakes(&fork), (wakes, coalesce));
    }

    /// #12032: a claim whose worktree shows an active build counts as
    /// progressing.
    #[test]
    fn a_build_in_an_issues_worktree_counts_as_progress() {
        let porcelain = "worktree /Users/me/work/openagents\nHEAD abc\nbranch refs/heads/main\n\n\
            worktree /Users/me/work/oa-wt-foreman-12029\nHEAD def\ndetached\n\n\
            worktree /Users/me/work/wt-b\nHEAD 123\nbranch refs/heads/agent/12040-phone-push\n\n\
            worktree /Users/me/work/wt-idle-12035\nHEAD 456\nbranch refs/heads/agent/12035\n";
        let worktrees = parse_worktrees(porcelain);
        assert_eq!(worktrees.len(), 4);
        assert_eq!(worktrees[2].1, "agent/12040-phone-push");
        let lsof = "p100\nccargo\nfcwd\nn/Users/me/work/oa-wt-foreman-12029/apps/openagents\n\
            p101\ncrustc\nfcwd\nn/Users/me/work/wt-b\n";
        let cwds = parse_lsof_cwds(lsof);
        let builds = builds_by_issue(&worktrees, &cwds, 7);
        assert_eq!(
            builds.into_iter().collect::<Vec<_>>(),
            [(12029, 7), (12040, 7)],
            "the idle worktree and main show no build"
        );
        assert_eq!(issue_numbers("/x/openagents", "main"), Vec::<u64>::new());
    }

    fn fixture_host(root: &std::path::Path) -> DocHost {
        let store = Arc::new(openagents_sync::DocsStore::open(root).unwrap());
        let host = DocHost::new(
            store,
            DocHostConfig {
                device_id: "fixture-mac".into(),
                default_harness: HarnessId::Mock,
                edge: None,
            },
        );
        host.start_agent_wakes(root.join("agent-wakes"), root.join("foreman"));
        host
    }

    fn grant_log(engine: &WakeEngine) {
        let mut grants = engine.state().grants;
        grants.push(Grant {
            id: format!("log-{}", engine.agent()),
            scopes: vec![Scope::LogWrite],
            over: Over::Account,
            granted_at: 0,
        });
        engine.set_grants(grants, 0).unwrap();
    }

    fn decision(chat: &str, saw: &str) -> RecordDecision {
        RecordDecision {
            origin_chat_id: Some(chat.into()),
            saw: saw.into(),
            decided: "carry on".into(),
            did: "nothing".into(),
            working: None,
            judgments: Vec::new(),
        }
    }

    fn decisions_saw(engine: &WakeEngine) -> Vec<String> {
        engine
            .read(100)
            .into_iter()
            .filter_map(|e| match e {
                DecisionEntry::Decision { saw, .. } => Some(saw),
                _ => None,
            })
            .collect()
    }

    /// #12046: a decision recorded on one host reaches the account, and a new
    /// host for the same agent starts with the account's log.
    #[tokio::test]
    async fn a_decision_reaches_the_account_and_a_new_host_starts_with_it() {
        use axum::{
            Json, Router,
            extract::{Query, State},
            routing::get,
        };
        use openagents_account::{
            Controller,
            desktop::{self, MemoryStore},
            openagents::{Client, Deployment},
        };
        use serde_json::{Value, json};
        use std::collections::HashMap;
        use std::sync::Mutex;

        type Log = Arc<Mutex<Vec<Value>>>;
        async fn record(State(log): State<Log>, Json(body): Json<Value>) -> Json<Value> {
            let mut log = log.lock().unwrap();
            if !log.iter().any(|e| e["request_id"] == body["request_id"]) {
                let cursor = (log.len() + 1).to_string();
                log.push(json!({"cursor": cursor, "request_id": body["request_id"], "entry": body["entry"]}));
            }
            Json(json!({"recorded": true}))
        }
        async fn page(
            State(log): State<Log>,
            Query(q): Query<HashMap<String, String>>,
        ) -> Json<Value> {
            let after: usize = q.get("after").and_then(|a| a.parse().ok()).unwrap_or(0);
            let data: Vec<Value> = log.lock().unwrap().iter().skip(after).cloned().collect();
            Json(json!({"data": data, "next": null}))
        }
        let log = Log::default();
        let path = format!("/v1/agent-definitions/{AGENT}/decisions");
        let api = Router::new()
            .route(
                "/v1/account",
                get(|| async {
                    Json(json!({"account": {"id": "acct_mirror_fixture", "label": "Mirror"}, "workspaces": []}))
                }),
            )
            .route(&path, get(page).post(record))
            .with_state(log.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, api).await.unwrap() });
        let root = tempfile::tempdir().unwrap();
        let controller = Controller::new(
            Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap(),
            Arc::new(MemoryStore::default()),
            root.path().join("account/signed-out"),
        );
        controller
            .import_credential(format!("sess_{:064x}", 9046))
            .await;
        desktop::register(root.path(), &controller);
        let lease = controller.lease().expect("signed in");
        let agents = lease.client().agents(lease.session()).unwrap();

        let first = WakeEngine::open(&root.path().join("host-a"), AGENT);
        first
            .record("saw a", "decided a", "did a", vec![], 1)
            .unwrap();
        first
            .record("saw b", "decided b", "did b", vec![], 2)
            .unwrap();
        first
            .mirror_decisions(&agents, &lease.cancel)
            .await
            .unwrap();
        // Repeating changes nothing on the account or in the file.
        first
            .mirror_decisions(&agents, &lease.cancel)
            .await
            .unwrap();
        assert_eq!(log.lock().unwrap().len(), 2);
        assert_eq!(first.read(10).len(), 2);

        let second = WakeEngine::open(&root.path().join("host-b"), AGENT);
        assert!(second.read(10).is_empty());
        second
            .mirror_decisions(&agents, &lease.cancel)
            .await
            .unwrap();
        assert_eq!(second.read(10), first.read(10));
        second
            .record("saw c", "decided c", "did c", vec![], 3)
            .unwrap();
        second
            .mirror_decisions(&agents, &lease.cancel)
            .await
            .unwrap();
        first
            .mirror_decisions(&agents, &lease.cancel)
            .await
            .unwrap();
        assert_eq!(log.lock().unwrap().len(), 3);
        assert_eq!(decisions_saw(&first), ["saw a", "saw b", "saw c"]);
        assert_eq!(decisions_saw(&second), decisions_saw(&first));
        server.abort();
    }

    /// #12045: any saved agent keeps its own decision log, keyed by its
    /// definition ID, only while it holds `log.write`; a log kept before
    /// the grant existed stays open until the account takes that grant.
    #[tokio::test]
    async fn each_agent_keeps_its_own_log_under_its_log_grant() {
        const OTHER: &str = "c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3";
        const OTHER_CHAT: &str = "f0000000-0000-4000-8000-000000000004";
        let root = tempfile::tempdir().unwrap();
        let host = fixture_host(root.path());
        let mut engines = Vec::new();
        for (agent, chat) in [(AGENT, OWN), (OTHER, OTHER_CHAT)] {
            let engine = host.agent_wakes().unwrap().ensure(agent).unwrap();
            engine
                .set(
                    WakeSettings {
                        chat_id: Some(chat.into()),
                        ..WakeSettings::default()
                    },
                    0,
                )
                .unwrap();
            engines.push(engine);
        }

        // No grant and no earlier log: refused, in plain words.
        let refused = host
            .record_agent_decision(decision(OWN, "first look"))
            .await
            .unwrap_err()
            .to_string();
        assert!(refused.contains(NO_LOG_GRANT), "{refused}");
        assert!(decisions_saw(&engines[0]).is_empty());

        // With the grant, each chat writes to its own agent's log only.
        for engine in &engines {
            grant_log(engine);
        }
        host.record_agent_decision(decision(OWN, "own"))
            .await
            .unwrap();
        host.record_agent_decision(decision(OTHER_CHAT, "other"))
            .await
            .unwrap();
        assert_eq!(decisions_saw(&engines[0]), ["own"]);
        assert_eq!(decisions_saw(&engines[1]), ["other"]);
        assert!(
            root.path()
                .join("agent-wakes")
                .join(OTHER)
                .join("decisions.jsonl")
                .is_file()
        );

        // Revoked: refused again once the log grant has been the account's.
        engines[1].update(|s| s.log_grant_moved = true).unwrap();
        engines[1].set_grants(Vec::new(), 1).unwrap();
        assert!(
            host.record_agent_decision(decision(OTHER_CHAT, "late"))
                .await
                .is_err()
        );
        // A log kept here before the grant existed stays open until then.
        engines[0].set_grants(Vec::new(), 1).unwrap();
        assert!(engines[0].kept_a_log() && engines[0].may_log());
        host.record_agent_decision(decision(OWN, "still logging"))
            .await
            .unwrap();
        // A chat that runs no saved agent is refused.
        assert!(
            host.record_agent_decision(decision(WORKER, "x"))
                .await
                .is_err()
        );
    }

    /// #12044: a never-list scope is refused by the host before anything
    /// reaches the account, for every agent.
    #[tokio::test]
    async fn the_host_refuses_never_list_grants() {
        let root = tempfile::tempdir().unwrap();
        let host = fixture_host(root.path());
        for scopes in [
            vec!["deploy"],
            vec!["chat.message", "secrets"],
            vec!["close_issue"],
        ] {
            let refused = host
                .grant_agent(GrantRequest {
                    agent: AGENT.into(),
                    scopes: scopes.iter().map(|s| s.to_string()).collect(),
                    over: Over::Account,
                })
                .await
                .unwrap_err()
                .to_string();
            assert!(refused.contains(NEVER_REFUSAL), "{refused}");
        }
    }

    /// #12044: a fork of an agent under another name, with the same grants,
    /// checks in and steers on the same terms; each agent has its own log.
    #[tokio::test]
    async fn a_fork_with_the_same_grants_checks_in_and_steers() {
        const FORK: &str = "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";
        const FORK_CHAT: &str = "f0000000-0000-4000-8000-000000000003";
        let root = tempfile::tempdir().unwrap();
        let host = fixture_host(root.path());
        let cwd = root.path().to_string_lossy().into_owned();
        for (agent, chat) in [(AGENT, OWN), (FORK, FORK_CHAT)] {
            let engine = host.agent_wakes().unwrap().ensure(agent).unwrap();
            engine
                .set(
                    WakeSettings {
                        chat_id: Some(chat.into()),
                        ..WakeSettings::default()
                    },
                    0,
                )
                .unwrap();
            engine
                .set_grants(
                    vec![Grant {
                        id: format!("g-{agent}"),
                        scopes: Scope::CHAT.to_vec(),
                        over: Over::Account,
                        granted_at: 0,
                    }],
                    0,
                )
                .unwrap();
            let wake = host
                .start_check_in(&engine, chat, false, 10 * MIN, request(&cwd))
                .await
                .unwrap()
                .expect("a check-in");
            // Its turn answers the check-in, so steering needs its grant.
            host.open(chat)
                .unwrap()
                .writer()
                .push_message(&SessionMessageEntry {
                    id: wake,
                    role: MessageRole::User,
                    parts: vec![MessagePart::Text {
                        id: "t".into(),
                        text: "check-in".into(),
                    }],
                    created_at: 1,
                    device_id: "fixture-mac".into(),
                    status: Some(MessageStatus::Complete),
                    continuation_of: None,
                    duration_ms: None,
                })
                .unwrap();
            host.agent_admit(
                Some(chat),
                WORKER,
                Some(&SessionCommandPayload::Steer {
                    prompt: "carry on".into(),
                    message_id: None,
                }),
            )
            .unwrap();
        }
        let status = host.agent_wakes_status();
        assert_eq!(status.agents.len(), 2);
        assert!(status.agents.iter().all(|a| a.standing()));
        // Revoking the fork's grant doesn't stop it (#12135).
        let fork = host.agent_wakes().unwrap().engine(FORK).unwrap();
        fork.set_grants(vec![], 11 * MIN).unwrap();
        let stop = SessionCommandPayload::Interrupt {};
        assert!(
            host.agent_admit(Some(FORK_CHAT), WORKER, Some(&stop))
                .is_ok()
        );
        assert!(host.agent_admit(Some(OWN), WORKER, Some(&stop)).is_ok());
        // A caller that proved which agent acts needs no grant (#12135).
        assert!(host.agent_allows(AGENT, Scope::ChatStop, WORKER).is_ok());
        assert!(host.agent_allows(FORK, Scope::ChatStop, WORKER).is_ok());
        assert!(
            host.agent_allows(&"c".repeat(64), Scope::ChatStop, WORKER)
                .is_ok()
        );
        // Remote run commands (#12054) ask the same grants through the hook.
        let hook = host
            .inner
            .run_command_authority
            .lock()
            .unwrap()
            .clone()
            .expect("installed at start");
        assert!(hook(AGENT, "chat.stop", WORKER).is_ok());
        assert!(hook(FORK, "chat.stop", WORKER).is_ok());
        assert!(
            hook(AGENT, "deploy", WORKER)
                .unwrap_err()
                .contains(NEVER_REFUSAL)
        );
    }

    /// #12057: the working state the agent saves with a decision is in
    /// the first check-in after a restart, with its stall timer still
    /// counting from when the host first saw it.
    #[tokio::test]
    async fn the_first_check_in_after_a_restart_resumes_the_working_state() {
        use openagents_proto::agent_wakes::{WorkItem, WorkStatus};
        let root = tempfile::tempdir().unwrap();
        let cwd = root.path().to_string_lossy().into_owned();
        let open = || {
            let store = Arc::new(openagents_sync::DocsStore::open(root.path()).unwrap());
            let host = DocHost::new(
                store,
                DocHostConfig {
                    device_id: "fixture-mac".into(),
                    default_harness: HarnessId::Mock,
                    edge: None,
                },
            );
            host.start_agent_wakes(root.path().join("agent-wakes"), root.path().join("foreman"));
            host
        };
        let host = open();
        let engine = host.agent_wakes().unwrap().ensure(AGENT).unwrap();
        engine
            .set(
                WakeSettings {
                    chat_id: Some(OWN.into()),
                    every_minutes: 10,
                    ..WakeSettings::default()
                },
                0,
            )
            .unwrap();
        let first = host
            .start_check_in(&engine, OWN, false, 10 * MIN, request(&cwd))
            .await
            .unwrap()
            .expect("a check-in");
        assert!(
            runs(&host)
                .iter()
                .any(|(id, text)| *id == first && text.contains("no saved working state yet"))
        );
        // The record_decision call is stamped at the host's clock.
        grant_log(&engine);
        host.record_agent_decision(RecordDecision {
            origin_chat_id: Some(OWN.into()),
            saw: "#12071 is on a chat".into(),
            decided: "watch it".into(),
            did: "nothing".into(),
            working: Some(vec![WorkItem {
                issue: Some(12071),
                chat: Some(WORKER.into()),
                status: WorkStatus::Working,
                note: "shared-cache fix".into(),
                since: 0,
            }]),
            judgments: Vec::new(),
        })
        .await
        .unwrap();
        drop(host);

        let host = open();
        let engine = host.agent_wakes().unwrap().engine(AGENT).expect("saved");
        let wake = host
            .start_check_in(&engine, OWN, false, now_ms() + 60 * MIN, request(&cwd))
            .await
            .unwrap()
            .expect("the scheduled check-in after the restart");
        let text = runs(&host)
            .into_iter()
            .find(|(id, _)| *id == wake)
            .map(|(_, text)| text)
            .unwrap();
        assert!(
            text.contains("- #12071 · chat f0000000 · working for 60 minutes: shared-cache fix"),
            "{text}"
        );
        assert!(
            host.record_agent_decision(RecordDecision {
                origin_chat_id: Some(WORKER.into()),
                saw: "x".into(),
                decided: "x".into(),
                did: "x".into(),
                working: Some(vec![]),
                judgments: Vec::new(),
            })
            .await
            .is_err(),
            "only the checked-in chat writes its working state"
        );
    }

    fn interrupts(host: &DocHost) -> usize {
        host.open(OWN)
            .unwrap()
            .doc()
            .read_commands()
            .unwrap()
            .into_iter()
            .filter(|c| matches!(c.payload, SessionCommandPayload::Interrupt {}))
            .count()
    }

    /// #12029's fixture: the chat stays `working` with no output for 10
    /// minutes while its turn waits on one `curl`. The host interrupts it
    /// once, starts exactly one check-in that names the call and how long it
    /// ran, and logs the stopped turn as `stuck`.
    #[tokio::test]
    async fn a_stuck_turn_is_interrupted_once_and_checked_in_with_its_tool() {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(openagents_sync::DocsStore::open(root.path()).unwrap());
        let host = DocHost::new(
            store,
            DocHostConfig {
                device_id: "fixture-mac".into(),
                default_harness: HarnessId::Mock,
                edge: None,
            },
        );
        host.start_agent_wakes(root.path().join("agent-wakes"), root.path().join("foreman"));
        let engine = host.agent_wakes().unwrap().ensure(AGENT).unwrap();
        engine
            .set(
                WakeSettings {
                    chat_id: Some(OWN.into()),
                    every_minutes: 60,
                    ..WakeSettings::default()
                },
                0,
            )
            .unwrap();
        let cwd = root.path().to_string_lossy().into_owned();
        // The turn's reply so far: one shell call with no result.
        let tool: MessagePart = serde_json::from_value(serde_json::json!({
            "kind": "tool",
            "id": "toolu_1",
            "call": {"kind": "exec", "command": "curl -s https://openagents.com/api/health"},
            "resolved": false
        }))
        .unwrap();
        host.open(OWN)
            .unwrap()
            .writer()
            .push_message(&SessionMessageEntry {
                id: "reply-1".into(),
                role: MessageRole::Assistant,
                parts: vec![tool],
                created_at: 2 * MIN,
                device_id: "fixture-mac".into(),
                status: Some(MessageStatus::Streaming),
                continuation_of: None,
                duration_ms: None,
            })
            .unwrap();
        let working = Session {
            started_at: chrono::DateTime::from_timestamp_millis(2 * MIN),
            last_output_at: chrono::DateTime::from_timestamp_millis(2 * MIN),
            ..row(OWN, SessionStatus::Working, None)
        };
        let title = |_: &str| "Agent".to_string();
        engine
            .observe(std::slice::from_ref(&working), |_| false, title, 2 * MIN)
            .unwrap();
        assert!(!host.watch_own_turn(&engine, OWN, true, 11 * MIN).unwrap());
        assert!(host.watch_own_turn(&engine, OWN, true, 12 * MIN).unwrap());
        assert_eq!(interrupts(&host), 1);
        // The interrupt takes a while to land: no second stop, no check-in.
        for minute in [13, 20, 25] {
            engine
                .observe(
                    std::slice::from_ref(&working),
                    |_| false,
                    title,
                    minute * MIN,
                )
                .unwrap();
            assert!(
                !host
                    .watch_own_turn(&engine, OWN, true, minute * MIN)
                    .unwrap()
            );
            assert_eq!(
                host.start_check_in(&engine, OWN, true, minute * MIN, request(&cwd))
                    .await
                    .unwrap(),
                None
            );
        }
        assert_eq!(interrupts(&host), 1);
        engine
            .observe(
                &[row(OWN, SessionStatus::Idle, None)],
                |_| false,
                title,
                26 * MIN,
            )
            .unwrap();
        let id = host
            .start_check_in(&engine, OWN, false, 26 * MIN, request(&cwd))
            .await
            .unwrap()
            .expect("one check-in");
        let queued = runs(&host);
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].0, id);
        assert!(queued[0].1.contains(
            "Your last turn was stuck in `Bash` (`curl -s https://openagents.com/api/health`) for 10 minutes, so the host stopped it."
        ), "{}", queued[0].1);
        assert_eq!(
            host.start_check_in(&engine, OWN, false, 27 * MIN, request(&cwd))
                .await
                .unwrap(),
            None
        );
        let log = host.agent_wakes_status().agents[0].decisions.clone();
        assert_eq!(
            log.iter()
                .filter(|e| matches!(e, DecisionEntry::Outcome { status, .. } if status == "stuck"))
                .count(),
            1
        );
        assert!(matches!(
            log.last(),
            Some(DecisionEntry::Wake { reason: WakeReason::Stuck, id: w, .. }) if *w == id
        ));
    }

    /// #11965's fixture: a schedule tick and a chat finishing each queue one
    /// Foreman turn and log it, with no account involved.
    #[tokio::test]
    async fn a_schedule_tick_and_a_finished_chat_each_start_one_logged_turn() {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(openagents_sync::DocsStore::open(root.path()).unwrap());
        let host = DocHost::new(
            store,
            DocHostConfig {
                device_id: "fixture-mac".into(),
                default_harness: HarnessId::Mock,
                edge: None,
            },
        );
        host.start_agent_wakes(root.path().join("agent-wakes"), root.path().join("foreman"));
        let engine = host.agent_wakes().unwrap().ensure(AGENT).unwrap();
        engine
            .set(
                WakeSettings {
                    chat_id: Some(OWN.into()),
                    every_minutes: 20,
                    paused: false,
                    ..WakeSettings::default()
                },
                0,
            )
            .unwrap();
        let cwd = root.path().to_string_lossy().into_owned();

        // The schedule tick.
        assert_eq!(
            host.start_check_in(&engine, OWN, false, 19 * MIN, request(&cwd))
                .await
                .unwrap(),
            None
        );
        let scheduled = host
            .start_check_in(&engine, OWN, false, 20 * MIN, request(&cwd))
            .await
            .unwrap()
            .expect("the schedule starts a turn");
        let queued = runs(&host);
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].0, scheduled);
        assert!(queued[0].1.starts_with("Check-in"));
        // Queuing the same check-in again changes nothing.
        let wake = Wake {
            id: scheduled.clone(),
            text: queued[0].1.clone(),
        };
        host.queue_check_in(OWN, &wake, request(&cwd)).unwrap();
        assert_eq!(runs(&host).len(), 1);

        // A chat Foreman messaged finishes its turn.
        host.agent_admit(
            Some(OWN),
            WORKER,
            Some(&SessionCommandPayload::Interrupt {}),
        )
        .unwrap();
        let title = |_: &str| "Fix the build".to_string();
        engine
            .observe(
                &[row(WORKER, SessionStatus::Working, None)],
                |_| false,
                title,
                21 * MIN,
            )
            .unwrap();
        engine
            .observe(
                &[row(WORKER, SessionStatus::Idle, Some("turn-1"))],
                |_| false,
                title,
                22 * MIN,
            )
            .unwrap();
        assert_eq!(
            host.start_check_in(&engine, OWN, true, 30 * MIN, request(&cwd))
                .await
                .unwrap(),
            None,
            "never over the agent's own turn"
        );
        let finished = host
            .start_check_in(&engine, OWN, false, 30 * MIN, request(&cwd))
            .await
            .unwrap()
            .expect("the finished chat starts a turn");
        let queued = runs(&host);
        assert_eq!(queued.len(), 2);
        assert_eq!(queued[1].0, finished);
        assert!(
            queued[1]
                .1
                .contains("Chat \"Fix the build\" (f0000000) finished its turn.")
        );

        // Both turns are in the log, which only the agent's own chat may add to.
        grant_log(&engine);
        host.record_agent_decision(RecordDecision {
            origin_chat_id: Some(OWN.into()),
            saw: "the build chat finished".into(),
            decided: "nothing else needed".into(),
            did: "nothing".into(),
            working: None,
            judgments: Vec::new(),
        })
        .await
        .unwrap();
        assert!(
            host.record_agent_decision(RecordDecision {
                origin_chat_id: Some(WORKER.into()),
                saw: "x".into(),
                decided: "x".into(),
                did: "x".into(),
                working: None,
                judgments: Vec::new(),
            })
            .await
            .is_err()
        );
        let log = host.agent_wakes_status().agents[0].decisions.clone();
        let wakes: Vec<(String, WakeReason)> = log
            .iter()
            .filter_map(|e| match e {
                DecisionEntry::Wake { id, reason, .. } => Some((id.clone(), *reason)),
                _ => None,
            })
            .collect();
        assert_eq!(
            wakes,
            [
                (scheduled, WakeReason::Schedule),
                (finished.clone(), WakeReason::Events)
            ]
        );
        assert!(matches!(log.last(), Some(DecisionEntry::Decision { .. })));

        // While Foreman answers a check-in, changing other chats needs the
        // standing authority; the owner's own turns don't.
        let handle = host.open(OWN).unwrap();
        handle
            .writer()
            .push_message(&SessionMessageEntry {
                id: finished,
                role: MessageRole::User,
                parts: vec![MessagePart::Text {
                    id: "t".into(),
                    text: "check-in".into(),
                }],
                created_at: 1,
                device_id: "fixture-mac".into(),
                status: Some(MessageStatus::Complete),
                continuation_of: None,
                duration_ms: None,
            })
            .unwrap();
        let stop = SessionCommandPayload::Interrupt {};
        assert!(host.agent_admit(Some(OWN), WORKER, Some(&stop)).is_ok());
        assert!(
            host.agent_admit(Some("someone-else"), WORKER, Some(&stop))
                .is_ok()
        );
        engine
            .set_grants(
                vec![Grant {
                    id: "g".into(),
                    scopes: Scope::CHAT.to_vec(),
                    over: Over::Account,
                    granted_at: 31 * MIN,
                }],
                31 * MIN,
            )
            .unwrap();
        assert!(host.agent_admit(Some(OWN), WORKER, Some(&stop)).is_ok());
    }
}
