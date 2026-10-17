//! Foreman's wake-ups on the host that runs its chat (#11965). The account
//! schedule is an ordinary scheduled prompt for this computer, kept through
//! the SDK; a check-in is a message queued into Foreman's chat the way the
//! app sends one, so the existing ledger runs it and a retry never doubles
//! it. See `crate::foreman` for the state, log and coalescing.
use super::*;
use crate::foreman::{CLAIMS_QUERY, Foreman, GhIssue, Wake, claim_issues};
use openagents_account::{
    Lease,
    openagents::schedules::{self, Schedule},
};
use openagents_proto::foreman::{
    ForemanCandidate, ForemanSettings, ForemanStatus, RecordDecision, Scope,
};
use std::sync::Arc;

const SCHEDULE_EVERY_MS: i64 = 60_000;
const GITHUB_EVERY_MS: i64 = 5 * 60_000;
const REPO: &str = "OpenAgentsInc/openagents";
/// What the account's Scheduled prompts page shows for Foreman's schedule.
const SCHEDULE_PROMPT: &str =
    "Foreman check-in: review every open issue and every agent, and act on what needs it.";

fn error(message: &str) -> EngineError {
    EngineError::Other(message.into())
}

impl DocHost {
    /// Open Foreman's state under `root` and watch every device's chat
    /// status for the chats it manages. Called once at engine assembly.
    pub fn start_foreman(&self, root: std::path::PathBuf) {
        if self
            .inner
            .foreman
            .set(Arc::new(Foreman::open(&root)))
            .is_err()
        {
            return;
        }
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
                if let Some(foreman) = host.foreman() {
                    let rows = rows.borrow_and_update().clone();
                    let chats = chats.borrow().clone();
                    let chat = foreman.chat();
                    let is_child = |id: &str| {
                        chats
                            .iter()
                            .any(|c| c.id == id && chat.is_some() && c.parent_chat_id == chat)
                    };
                    let title = |id: &str| {
                        chats
                            .iter()
                            .find(|c| c.id == id)
                            .and_then(|c| c.title.clone())
                            .filter(|t| !t.trim().is_empty())
                            .unwrap_or_else(|| "Untitled".into())
                    };
                    if let Err(e) = foreman.observe(&rows, is_child, title, now_ms()) {
                        tracing::warn!(error = %e, "foreman: session look failed");
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
                let Some(foreman) = host.foreman().cloned() else {
                    return;
                };
                drop(host);
                let state = foreman.state();
                if state.settings.chat_id.is_none()
                    || state.settings.paused
                    || now_ms() - state.last_github < GITHUB_EVERY_MS
                {
                    continue;
                }
                look_at_github(&foreman).await;
            }
        });
    }

    pub(crate) fn foreman(&self) -> Option<&Arc<Foreman>> {
        self.inner.foreman.get()
    }

    /// The settings card's view.
    pub fn foreman_status(&self) -> ForemanStatus {
        let Some(foreman) = self.foreman() else {
            return ForemanStatus {
                settings: ForemanSettings::default(),
                chat_title: None,
                candidates: Vec::new(),
                authority: None,
                next_check_in: None,
                problem: Some("Foreman isn't available on this computer.".into()),
                decisions: Vec::new(),
            };
        };
        let state = foreman.state();
        let chats = self
            .workspace()
            .map(|w| w.watch_chats().borrow().clone())
            .unwrap_or_default();
        let device = &self.inner.config.device_id;
        let candidates = chats
            .iter()
            .filter(|c| {
                &c.device_id == device
                    && !c.archived
                    && c.config.as_ref().is_some_and(|k| {
                        k.model_options
                            .contains_key(super::custom_agents::ASSIGNMENT)
                    })
            })
            .map(|c| ForemanCandidate {
                chat_id: c.id.clone(),
                title: c.title.clone().unwrap_or_else(|| "Untitled".into()),
            })
            .collect();
        ForemanStatus {
            chat_title: state.settings.chat_id.as_ref().and_then(|id| {
                chats
                    .iter()
                    .find(|c| &c.id == id)
                    .map(|c| c.title.clone().unwrap_or_else(|| "Untitled".into()))
            }),
            settings: state.settings,
            candidates,
            authority: state.authority,
            next_check_in: foreman.next_scheduled(),
            problem: foreman.problem(),
            decisions: foreman.read(crate::foreman::STATUS_ENTRIES),
        }
    }

    pub fn set_foreman(&self, settings: ForemanSettings) -> Result<ForemanStatus, EngineError> {
        let foreman = self
            .foreman()
            .ok_or_else(|| error("Foreman isn't available on this computer."))?;
        if let Some(chat) = &settings.chat_id {
            let row = self
                .workspace()
                .ok_or_else(|| error("This computer is not ready."))?
                .chat(chat)?
                .ok_or_else(|| error("That chat no longer exists."))?;
            if row.device_id != self.inner.config.device_id {
                return Err(error("Choose a chat that runs on this computer."));
            }
        }
        foreman.set(settings, now_ms())?;
        foreman.set_problem(None);
        Ok(self.foreman_status())
    }

    /// Foreman's own record of a check-in; only its chat may write it.
    pub fn record_foreman_decision(&self, record: RecordDecision) -> Result<(), EngineError> {
        let foreman = self
            .foreman()
            .ok_or_else(|| error("Foreman isn't available on this computer."))?;
        if record.origin_chat_id.is_none() || record.origin_chat_id != foreman.chat() {
            return Err(error("Only Foreman's own chat can write its decision log."));
        }
        foreman.record(&record.saw, &record.decided, &record.did, now_ms())
    }

    /// Whether a command `origin` sends to `target` is within Foreman's
    /// standing authority, and remember the chat it acted on. Commands from
    /// other chats, and from turns the owner started, pass unchanged.
    pub(crate) fn foreman_admit(
        &self,
        origin: Option<&str>,
        target: &str,
        payload: Option<&SessionCommandPayload>,
    ) -> Result<(), EngineError> {
        let Some(foreman) = self.foreman() else {
            return Ok(());
        };
        let Some(chat) = foreman.chat() else {
            return Ok(());
        };
        if origin != Some(chat.as_str()) || target == chat {
            return Ok(());
        }
        let fresh = self
            .open(target)
            .ok()
            .and_then(|h| h.doc.read_entries().ok())
            .is_some_and(|e| e.is_empty());
        let scope = match payload {
            _ if fresh => Scope::Create,
            Some(SessionCommandPayload::Steer { .. }) => Scope::Steer,
            Some(SessionCommandPayload::Interrupt {}) => Scope::Stop,
            _ => Scope::Message,
        };
        foreman
            .admits(scope, self.foreman_in_check_in(&chat))
            .map_err(EngineError::Other)?;
        foreman.manage(target)
    }

    /// Whether Foreman's current turn answers a check-in, not the owner.
    fn foreman_in_check_in(&self, chat: &str) -> bool {
        let Some(foreman) = self.foreman() else {
            return false;
        };
        self.open(chat)
            .ok()
            .and_then(|h| h.doc.read_entries().ok())
            .and_then(|entries| {
                entries
                    .iter()
                    .rev()
                    .find(|e| e.role == openagents_doc::MessageRole::User)
                    .map(|e| foreman.is_wake(&e.id))
            })
            .unwrap_or(false)
    }

    /// One pass from the host loop: mirror the account schedule, then start
    /// a check-in when one is due.
    pub(super) async fn foreman_tick(&self, lease: &Lease) -> Result<(), EngineError> {
        let Some(foreman) = self.foreman().cloned() else {
            return Ok(());
        };
        let Some(chat) = foreman.chat() else {
            return Ok(());
        };
        let workspace = self
            .workspace()
            .ok_or_else(|| error("This computer is not ready."))?;
        let sessions = self
            .sessions()
            .ok_or_else(|| error("This computer is not ready."))?;
        match workspace.chat(&chat)? {
            Some(row) if row.device_id == self.inner.config.device_id => {}
            Some(_) => {
                foreman.set_problem(Some(
                    "Foreman's chat runs on another computer. Set it up there.".into(),
                ));
                return Ok(());
            }
            None => {
                foreman.set_problem(Some(
                    "Foreman's chat no longer exists. Choose it again.".into(),
                ));
                return Ok(());
            }
        }
        let now = now_ms();
        if foreman.throttle("schedule", SCHEDULE_EVERY_MS, now) {
            match self.sync_foreman_schedule(&foreman, lease, now).await {
                Ok(()) => foreman.set_problem(None),
                Err(e) => {
                    tracing::warn!(error = %e, "foreman: schedule sync failed");
                    foreman.set_problem(Some(
                        "Foreman's schedule couldn't reach your account. It keeps running here."
                            .into(),
                    ));
                }
            }
        }
        self.foreman_wake(&chat, sessions.turn_in_flight(&chat), now, |text| {
            self.request_from_chat_row(&chat, text)
        })?;
        Ok(())
    }

    /// Start the check-in that is due, if any, as a message queued into
    /// Foreman's chat. Returns its message id.
    fn foreman_wake(
        &self,
        chat: &str,
        busy: bool,
        now: i64,
        request: impl FnOnce(&str) -> Option<openagents_proto::RunRequest>,
    ) -> Result<Option<String>, EngineError> {
        let Some(foreman) = self.foreman().cloned() else {
            return Ok(None);
        };
        let Some(wake) = foreman.due(busy, now)? else {
            return Ok(None);
        };
        match self.queue_foreman_wake(chat, &wake, request) {
            Ok(()) => Ok(Some(wake.id)),
            Err(e) => {
                tracing::warn!(error = %e, "foreman: check-in not queued");
                foreman.update(|s| s.open_wake = None)?;
                foreman.append(&openagents_proto::foreman::DecisionEntry::Outcome {
                    at: now,
                    id: wake.id,
                    status: "not started".into(),
                })?;
                foreman.set_problem(Some(format!("A check-in couldn't start: {e}")));
                Ok(None)
            }
        }
    }

    fn queue_foreman_wake(
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
        let request = request(&wake.text).ok_or_else(|| error("Foreman's chat is not ready."))?;
        self.queue_command(
            chat,
            SessionCommandPayload::Run {
                request,
                message_id: wake.id.clone(),
            },
        )?;
        Ok(())
    }

    /// Keep the account's copy of Foreman's schedule and this host's copy
    /// together: the newer change wins, as the account merges it.
    async fn sync_foreman_schedule(
        &self,
        foreman: &Foreman,
        lease: &Lease,
        now: i64,
    ) -> Result<(), EngineError> {
        let client = lease
            .client()
            .schedules(lease.session())
            .map_err(|_| error("Sign in to OpenAgents again."))?;
        let device = self.inner.config.device_id.clone();
        let state = foreman.state();
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
                    prompt: Some(SCHEDULE_PROMPT.into()),
                    every_secs: Some(u64::from(state.settings.every_minutes) * 60),
                    chat: Some(chat.clone()),
                    chat_title: Some("Foreman".into()),
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
            foreman.update(|s| {
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
        foreman.update(|s| {
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
                    s.settings.every_minutes = ((every / 60) as u32).clamp(
                        openagents_proto::foreman::MIN_EVERY_MINUTES,
                        openagents_proto::foreman::MAX_EVERY_MINUTES,
                    );
                }
            }
        })?;
        Ok(())
    }
}

/// Ask the host's GitHub CLI what changed. A missing or signed-out `gh`
/// leaves only chat and schedule wake-ups.
async fn look_at_github(foreman: &Foreman) {
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
        let _ = foreman.update(|s| s.last_github = now);
        return;
    };
    if let Err(e) = foreman.observe_issues(&listed, now) {
        tracing::warn!(error = %e, "foreman: issue look failed");
    }
    let query = format!("query={CLAIMS_QUERY}");
    if let Some(answer) = gh(&["api", "graphql", "-f", &query]).await {
        if let Err(e) = foreman.observe_claims(&claim_issues(&answer), now) {
            tracing::warn!(error = %e, "foreman: claim look failed");
        }
    }
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
    use openagents_proto::foreman::{DecisionEntry, WakeReason};
    use openagents_proto::{Session, SessionStatus};

    const FOREMAN: &str = "f0000000-0000-4000-8000-000000000001";
    const WORKER: &str = "f0000000-0000-4000-8000-000000000002";
    const MIN: i64 = 60_000;

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
        host.open(FOREMAN)
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
        }
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
        host.start_foreman(root.path().join("foreman"));
        let foreman = host.foreman().unwrap().clone();
        foreman
            .set(
                ForemanSettings {
                    chat_id: Some(FOREMAN.into()),
                    every_minutes: 20,
                    paused: false,
                    authority: false,
                },
                0,
            )
            .unwrap();
        let cwd = root.path().to_string_lossy().into_owned();

        // The schedule tick.
        assert_eq!(
            host.foreman_wake(FOREMAN, false, 19 * MIN, request(&cwd))
                .unwrap(),
            None
        );
        let scheduled = host
            .foreman_wake(FOREMAN, false, 20 * MIN, request(&cwd))
            .unwrap()
            .expect("the schedule starts a turn");
        let queued = runs(&host);
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].0, scheduled);
        assert!(queued[0].1.starts_with("Foreman check-in"));
        // Queuing the same check-in again changes nothing.
        let wake = Wake {
            id: scheduled.clone(),
            text: queued[0].1.clone(),
        };
        host.queue_foreman_wake(FOREMAN, &wake, request(&cwd))
            .unwrap();
        assert_eq!(runs(&host).len(), 1);

        // A chat Foreman messaged finishes its turn.
        host.foreman_admit(
            Some(FOREMAN),
            WORKER,
            Some(&SessionCommandPayload::Interrupt {}),
        )
        .unwrap();
        let title = |_: &str| "Fix the build".to_string();
        foreman
            .observe(
                &[row(WORKER, SessionStatus::Working, None)],
                |_| false,
                title,
                21 * MIN,
            )
            .unwrap();
        foreman
            .observe(
                &[row(WORKER, SessionStatus::Idle, Some("turn-1"))],
                |_| false,
                title,
                22 * MIN,
            )
            .unwrap();
        assert_eq!(
            host.foreman_wake(FOREMAN, true, 30 * MIN, request(&cwd))
                .unwrap(),
            None,
            "never over Foreman's own turn"
        );
        let finished = host
            .foreman_wake(FOREMAN, false, 30 * MIN, request(&cwd))
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

        // Both turns are in the log, which only Foreman's chat may add to.
        host.record_foreman_decision(RecordDecision {
            origin_chat_id: Some(FOREMAN.into()),
            saw: "the build chat finished".into(),
            decided: "nothing else needed".into(),
            did: "nothing".into(),
        })
        .unwrap();
        assert!(
            host.record_foreman_decision(RecordDecision {
                origin_chat_id: Some(WORKER.into()),
                saw: "x".into(),
                decided: "x".into(),
                did: "x".into(),
            })
            .is_err()
        );
        let log = host.foreman_status().decisions;
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
        let handle = host.open(FOREMAN).unwrap();
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
        assert!(
            host.foreman_admit(Some(FOREMAN), WORKER, Some(&stop))
                .is_err()
        );
        assert!(
            host.foreman_admit(Some("someone-else"), WORKER, Some(&stop))
                .is_ok()
        );
        foreman
            .set(
                ForemanSettings {
                    chat_id: Some(FOREMAN.into()),
                    every_minutes: 20,
                    paused: false,
                    authority: true,
                },
                31 * MIN,
            )
            .unwrap();
        assert!(
            host.foreman_admit(Some(FOREMAN), WORKER, Some(&stop))
                .is_ok()
        );
    }
}
