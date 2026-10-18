//! Foreman's wake-ups on the host that runs its chat (#11965). The account
//! schedule is an ordinary scheduled prompt for this computer, kept through
//! the SDK; a check-in is a message queued into Foreman's chat the way the
//! app sends one, so the existing ledger runs it and a retry never doubles
//! it. See `crate::foreman` for the state, log and coalescing.
use super::*;
use crate::foreman::{
    Adopt, CLAIMS_QUERY, Declaring, Foreman, GhIssue, LocalProgress, Wake, claim_issues,
    tool_label,
};
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
                let state = foreman.state();
                if state.settings.chat_id.is_none()
                    || state.settings.paused
                    || now_ms() - state.last_github < GITHUB_EVERY_MS
                {
                    continue;
                }
                let local = host.local_progress().await;
                drop(host);
                look_at_github(&foreman, &local).await;
            }
        });
    }

    /// What this computer sees of claims (#12032): each app chat's last
    /// output by the harness session a claim marker names, and builds
    /// running in worktrees of the agent's checkout named for an issue.
    async fn local_progress(&self) -> LocalProgress {
        let now = now_ms();
        let (Some(workspace), Some(sessions)) = (self.workspace().cloned(), self.sessions())
        else {
            return LocalProgress::default();
        };
        let rows = workspace.merged_sessions(&sessions.watch_sessions().borrow());
        let chats = workspace.watch_chats().borrow().clone();
        let mut local = LocalProgress::default();
        for chat in &chats {
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
            let row = rows.iter().find(|r| r.chat_id == chat.id);
            let working = row
                .filter(|r| r.status == openagents_proto::SessionStatus::Working)
                .and_then(|r| r.started_at);
            let last = row
                .and_then(|r| r.last_output_at)
                .into_iter()
                .chain(working)
                .chain(chat.last_message_at)
                .map(|t| t.timestamp_millis())
                .max();
            if let Some(last) = last {
                local.chats.insert(format!("{harness}:{id}"), last);
            }
        }
        // The checkout the agent's chat works in.
        let checkout = self
            .foreman()
            .and_then(|f| f.chat())
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
        local
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
                turned_on_at: None,
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
            settings: state.settings.clone(),
            candidates,
            authority: state.authority,
            next_check_in: foreman.next_scheduled(),
            problem: foreman.problem(),
            decisions: foreman.read(crate::foreman::STATUS_ENTRIES),
            turned_on_at: state.turned_on_at,
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
        if foreman.chat().is_none() {
            self.foreman_adopt(&foreman, now_ms())?;
        }
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
        let busy = sessions.turn_in_flight(&chat);
        self.foreman_watch(&chat, busy, now)?;
        self.foreman_wake(&chat, busy, now, |text| {
            self.request_from_chat_row(&chat, text)
        })?;
        Ok(())
    }

    /// On by default (#12030): with no chat chosen, pick the one chat on this
    /// computer whose agent declares check-ins, and tell that chat so.
    fn foreman_adopt(&self, foreman: &Foreman, now: i64) -> Result<(), EngineError> {
        let Some(workspace) = self.workspace() else {
            return Ok(());
        };
        let chats = workspace.watch_chats().borrow().clone();
        let declaring = declaring_chats(&chats, &self.inner.config.device_id);
        match foreman.adopt(&declaring, now)? {
            Adopt::Nothing => {}
            Adopt::Choose(titles) => foreman.set_problem(Some(format!(
                "{} saved agents on this computer ask to check in on their own: {}. Choose the one that does in Foreman's chat above.",
                titles.len(),
                titles.join(", ")
            ))),
            Adopt::Picked(pick) => {
                foreman.set_problem(None);
                let state = foreman.state();
                let what = if state.authority.is_some() {
                    "check-ins and standing authority"
                } else {
                    "check-ins"
                };
                let text = format!(
                    "This computer turned on {what} for {}, because its definition asks for them: it checks in every {} minutes{}. Turn either off in Settings → General → Foreman.",
                    pick.title,
                    state.settings.every_minutes,
                    if state.authority.is_some() {
                        " and may message, start, steer and stop chats without asking"
                    } else {
                        ""
                    }
                );
                if let Err(e) = self.runtime_note(&pick.chat_id, text) {
                    tracing::warn!(error = %e, "foreman: couldn't tell the chosen chat");
                }
            }
        }
        Ok(())
    }

    /// The host watchdog over the chat's own turn (#12029): a turn that went
    /// the stuck window with no output is interrupted once, and the next
    /// check-in says which call was stuck and for how long. Returns whether
    /// it stopped a turn.
    fn foreman_watch(&self, chat: &str, busy: bool, now: i64) -> Result<bool, EngineError> {
        let Some(foreman) = self.foreman().cloned() else {
            return Ok(false);
        };
        let Some(stuck) = foreman.watchdog(busy, now, || self.waiting_tool(chat))? else {
            return Ok(false);
        };
        tracing::warn!(chat = %chat, since = stuck.since, "foreman: stopping a stuck turn");
        if let Err(e) = self.queue_command(chat, SessionCommandPayload::Interrupt {}) {
            foreman.set_problem(Some(format!("A stuck turn couldn't be stopped: {e}")));
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

/// This computer's unarchived saved-agent chats whose agent's definition
/// declares check-ins. Read from the chat config the host stamps from the
/// definition, never from the chat's or agent's name.
fn declaring_chats(chats: &[openagents_proto::Chat], device: &str) -> Vec<Declaring> {
    chats
        .iter()
        .filter(|c| c.device_id == device && !c.archived)
        .filter_map(|c| {
            let declared = openagents_proto::saved_agent::declared(c.config.as_ref())?;
            declared.check_ins.then(|| Declaring {
                chat_id: c.id.clone(),
                title: c
                    .title
                    .clone()
                    .filter(|t| !t.trim().is_empty())
                    .unwrap_or_else(|| "Untitled".into()),
                agent: c
                    .config
                    .as_ref()
                    .and_then(|k| k.model_options.get(openagents_proto::saved_agent::AGENT))
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
                declared,
            })
        })
        .collect()
}

/// Ask the host's GitHub CLI what changed. A missing or signed-out `gh`
/// leaves only chat and schedule wake-ups.
async fn look_at_github(foreman: &Foreman, local: &LocalProgress) {
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
        if let Err(e) = foreman.observe_claims(&claim_issues(&answer), local, now) {
            tracing::warn!(error = %e, "foreman: claim look failed");
        }
    }
}

/// Issues with a build running now in a worktree of `checkout` named for
/// them (#12032). Reads `git worktree list` and the working folders of
/// running `cargo` and `rustc` processes, each with a 10-second timeout.
async fn builds_here(checkout: &str, now: i64) -> std::collections::BTreeMap<u64, i64> {
    let worktrees = run(
        "git",
        &["-C", checkout, "worktree", "list", "--porcelain"],
    )
    .await
    .map(|out| parse_worktrees(&out))
    .unwrap_or_default();
    if worktrees.is_empty() {
        return Default::default();
    }
    let cwds = run("lsof", &["-a", "-d", "cwd", "-c", "cargo", "-c", "rustc", "-Fn"])
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
        let inside = |cwd: &String| {
            std::path::Path::new(cwd).starts_with(path)
        };
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
        let found = declaring_chats(&chats, "mac");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].chat_id, "c2");
        assert_eq!(found[0].agent.as_deref(), Some("def-1"));
        assert_eq!(found[0].declared.every_minutes, Some(10));
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

    fn interrupts(host: &DocHost) -> usize {
        host.open(FOREMAN)
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
        host.start_foreman(root.path().join("foreman"));
        let foreman = host.foreman().unwrap().clone();
        foreman
            .set(
                ForemanSettings {
                    chat_id: Some(FOREMAN.into()),
                    every_minutes: 60,
                    ..ForemanSettings::default()
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
        host.open(FOREMAN)
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
            ..row(FOREMAN, SessionStatus::Working, None)
        };
        let title = |_: &str| "Foreman".to_string();
        foreman
            .observe(std::slice::from_ref(&working), |_| false, title, 2 * MIN)
            .unwrap();
        assert!(!host.foreman_watch(FOREMAN, true, 11 * MIN).unwrap());
        assert!(host.foreman_watch(FOREMAN, true, 12 * MIN).unwrap());
        assert_eq!(interrupts(&host), 1);
        // The interrupt takes a while to land: no second stop, no check-in.
        for minute in [13, 20, 25] {
            foreman
                .observe(
                    std::slice::from_ref(&working),
                    |_| false,
                    title,
                    minute * MIN,
                )
                .unwrap();
            assert!(!host.foreman_watch(FOREMAN, true, minute * MIN).unwrap());
            assert_eq!(
                host.foreman_wake(FOREMAN, true, minute * MIN, request(&cwd))
                    .unwrap(),
                None
            );
        }
        assert_eq!(interrupts(&host), 1);
        foreman
            .observe(
                &[row(FOREMAN, SessionStatus::Idle, None)],
                |_| false,
                title,
                26 * MIN,
            )
            .unwrap();
        let id = host
            .foreman_wake(FOREMAN, false, 26 * MIN, request(&cwd))
            .unwrap()
            .expect("one check-in");
        let queued = runs(&host);
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].0, id);
        assert!(queued[0].1.contains(
            "Your last turn was stuck in `Bash` (`curl -s https://openagents.com/api/health`) for 10 minutes, so the host stopped it."
        ), "{}", queued[0].1);
        assert_eq!(
            host.foreman_wake(FOREMAN, false, 27 * MIN, request(&cwd))
                .unwrap(),
            None
        );
        let log = host.foreman_status().decisions;
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
        host.start_foreman(root.path().join("foreman"));
        let foreman = host.foreman().unwrap().clone();
        foreman
            .set(
                ForemanSettings {
                    chat_id: Some(FOREMAN.into()),
                    every_minutes: 20,
                    paused: false,
                    authority: false,
                    ..ForemanSettings::default()
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
                    ..ForemanSettings::default()
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
