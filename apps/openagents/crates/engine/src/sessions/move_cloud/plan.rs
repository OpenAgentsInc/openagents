//! Move plans (#11933): move several chats to one cloud computer at once,
//! and bring them back.
//!
//! A plan is one durable record, saved before anything is sent, with one
//! entry per chat. Each chat also keeps its own [`MoveRecord`] (#11932),
//! which holds every Computers API request identity, so running a plan
//! again after a crash or a lost reply sends the same requests: never a
//! second computer, a second copy, or a second continuation.
//!
//! Each chat goes Planned, Pausing, Captured, Provisioned, Restored,
//! Continued, Released. A failure leaves the chat on this computer with a
//! reason in plain words; running the plan again picks up where it stopped.
//!
//! Starting a plan and bringing a chat back need the person's approval,
//! which only the app records. The agent tools can plan and ask but can't
//! approve, so a tool call can't spend money or move a chat on its own.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use openagents_rpc::moves::{
    AfterBringBack, BringBackView, ChatProgress, MoveState, PlanView, Requester,
};
use openagents_sdk::CancellationToken;
use openagents_sdk::computers::{CommandOutcome, ComputerRef, Computers, CreateOptions, WaitFor};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    ChatToMove, CloudLogins, Destination, LocalRoots, MoveError, MoveRecord, MoveStore, Mover,
    Outcome, RestoreTarget, git, q, resumed_command,
};

const PLAN_VERSION: &str = "openagents.move-plan.v1";
/// How long a working agent gets to finish its step before its turn is
/// interrupted.
pub const PAUSE_WAIT: Duration = Duration::from_secs(600);
/// What a working agent is told before it moves.
pub const STEER_PROMPT: &str =
    "Finish the current step, then stop. This chat is moving to a cloud computer.";
const BACK_TIMEOUT: Duration = Duration::from_secs(1800);
const AWAKE_TIMEOUT: Duration = Duration::from_secs(600);
/// A resumed agent runs for up to an hour on the cloud computer.
const RUN_MINUTES: u32 = 60;

/// What the engine does to the chats themselves. The engine implements it
/// over its chats and runs; tests use a fake.
pub trait MoveHooks: Send + Sync {
    /// Ask a working agent to finish its step and stop, and interrupt its
    /// turn after `wait`. True when the turn was interrupted. A chat waiting
    /// for an answer moves as it is.
    fn pause(
        &self,
        chat: &str,
        wait: Duration,
    ) -> impl Future<Output = Result<bool, String>> + Send;
    /// Mark the chat as continued on the cloud computer. The same `key`
    /// always names the same continuation.
    fn continue_on(
        &self,
        key: &str,
        chat: &str,
        computer: &str,
        command: &str,
    ) -> impl Future<Output = Result<String, String>> + Send;
    /// Stop this computer's copy of the agent.
    fn release(&self, chat: &str) -> impl Future<Output = Result<(), String>> + Send;
    /// Continue the chat on this computer, with the branch that holds the
    /// cloud computer's work when there is one. The same `key` sends at
    /// most once.
    fn continue_here(
        &self,
        key: &str,
        chat: &str,
        branch: Option<&str>,
    ) -> impl Future<Output = Result<(), String>> + Send;
}

/// The saved cloud computer work for a Bring back.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackBundle {
    pub commit: String,
    pub sha256: String,
    pub size: u64,
    pub pieces: u32,
}

/// One chat's Bring back.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackRecord {
    pub after: AfterBringBack,
    pub requested_by: Requester,
    pub approved: bool,
    /// The cloud agent was stopped.
    pub agent_stopped: bool,
    pub bundle: Option<BackBundle>,
    pub branch: Option<String>,
    pub continued: bool,
    pub computer_done: bool,
    pub done: bool,
}

/// One chat in a plan.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanChat {
    pub chat: String,
    pub title: String,
    pub agent: String,
    pub state: MoveState,
    pub reason: Option<String>,
    /// Set once the pause finished: whether the turn was interrupted.
    pub interrupted: Option<bool>,
    pub continuation: Option<String>,
    pub back: Option<BackRecord>,
}

/// The durable plan.
#[derive(Clone, Serialize, Deserialize)]
pub struct MovePlan {
    v: String,
    pub id: String,
    pub created_unix: u64,
    pub requested_by: Requester,
    pub approved_unix: Option<u64>,
    pub destination: Destination,
    pub estimate_minutes: u32,
    /// The cloud computer, once it exists.
    pub computer: Option<ComputerRef>,
    pub chats: Vec<PlanChat>,
}

impl MovePlan {
    fn chat_mut(&mut self, chat: &str) -> Option<&mut PlanChat> {
        self.chats.iter_mut().find(|c| c.chat == chat)
    }
    fn chat(&self, chat: &str) -> Option<&PlanChat> {
        self.chats.iter().find(|c| c.chat == chat)
    }
    /// The plan as the app and the tools see it.
    pub fn view(&self) -> PlanView {
        PlanView {
            id: self.id.clone(),
            created_unix: self.created_unix,
            requested_by: self.requested_by,
            approved: self.approved_unix.is_some(),
            computer: self.computer.as_ref().map(|c| c.id().to_owned()),
            destination: match &self.destination {
                Destination::Existing { computer } => computer.clone(),
                Destination::New { .. } => "A new cloud computer".into(),
            },
            estimate_minutes: self.estimate_minutes,
            chats: self
                .chats
                .iter()
                .map(|c| ChatProgress {
                    chat: c.chat.clone(),
                    title: c.title.clone(),
                    agent: c.agent.clone(),
                    state: c.state,
                    reason: c.reason.clone(),
                    bring_back: c.back.as_ref().map(|b| BringBackView {
                        after: b.after,
                        requested_by: b.requested_by,
                        approved: b.approved,
                        branch: b.branch.clone(),
                        done: b.done,
                    }),
                })
                .collect(),
        }
    }
}

/// The paid cloud computer time a plan expects: setup, then the agents'
/// run of up to an hour, all on one computer.
pub fn estimate_minutes(chats: usize) -> u32 {
    if chats == 0 {
        return 0;
    }
    5 + 2 * chats.min(64) as u32 + RUN_MINUTES
}

/// Why a plan action was refused, in plain words.
#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    #[error("No agents are running on this computer, so there's nothing to move.")]
    NothingToMove,
    #[error("These chats are already moving to a cloud computer.")]
    AlreadyMoving,
    #[error("We couldn't find that move.")]
    NotFound,
    #[error(
        "This move isn't approved yet. The person using this computer can approve it in the OpenAgents app."
    )]
    NotApproved,
    #[error("This move has started, so it can't be dropped. Bring the chats back instead.")]
    Started,
    #[error("This chat isn't on a cloud computer, so there's nothing to bring back.")]
    NotMoved,
    #[error("We couldn't save the move on this computer: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Move(#[from] MoveError),
}

/// The plans, one JSON file each.
pub struct PlanStore {
    dir: PathBuf,
}

fn valid_plan_id(id: &str) -> bool {
    id.len() == 21 && id.starts_with("plan-") && id[5..].bytes().all(|b| b.is_ascii_hexdigit())
}

impl PlanStore {
    pub fn open(dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }
    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }
    pub fn load(&self, id: &str) -> Result<MovePlan, PlanError> {
        if !valid_plan_id(id) {
            return Err(PlanError::NotFound);
        }
        let bytes = match std::fs::read(self.path(id)) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(PlanError::NotFound),
            Err(e) => return Err(e.into()),
        };
        let plan: MovePlan = serde_json::from_slice(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        if plan.v != PLAN_VERSION || plan.id != id {
            return Err(PlanError::NotFound);
        }
        Ok(plan)
    }
    pub fn save(&self, plan: &MovePlan) -> Result<(), PlanError> {
        let bytes = serde_json::to_vec_pretty(plan)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let tmp = self.dir.join(format!("{}.tmp", plan.id));
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, self.path(&plan.id))?;
        Ok(())
    }
    fn remove(&self, id: &str) -> Result<(), PlanError> {
        match std::fs::remove_file(self.path(id)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
    /// Every plan, newest first.
    pub fn list(&self) -> Result<Vec<MovePlan>, PlanError> {
        let mut plans = Vec::new();
        for entry in std::fs::read_dir(&self.dir)?.flatten() {
            let name = entry.file_name();
            let Some(id) = name.to_str().and_then(|n| n.strip_suffix(".json")) else {
                continue;
            };
            if let Ok(plan) = self.load(id) {
                plans.push(plan);
            }
        }
        plans.sort_by(|a, b| (b.created_unix, &b.id).cmp(&(a.created_unix, &a.id)));
        Ok(plans)
    }
    /// The plan that has this chat and hasn't finished with it.
    pub fn active_for(&self, chat: &str) -> Result<Option<MovePlan>, PlanError> {
        Ok(self
            .list()?
            .into_iter()
            .find(|p| p.chat(chat).is_some_and(|c| !c.state.finished())))
    }

    /// The person approved the plan in the app.
    pub fn approve(&self, id: &str, now: u64) -> Result<MovePlan, PlanError> {
        let mut plan = self.load(id)?;
        if plan.approved_unix.is_none() {
            plan.approved_unix = Some(now);
            self.save(&plan)?;
        }
        Ok(plan)
    }

    /// Drop a plan that hasn't started.
    pub fn discard(&self, moves: &MoveStore, id: &str, now: u64) -> Result<(), PlanError> {
        let plan = self.load(id)?;
        if plan.approved_unix.is_some() {
            return Err(PlanError::Started);
        }
        for chat in &plan.chats {
            moves.retire(&chat.chat, now)?;
        }
        self.remove(id)
    }

    /// Ask to bring a moved chat back. It runs once approved.
    pub fn request_bring_back(
        &self,
        chat: &str,
        after: AfterBringBack,
        requested_by: Requester,
    ) -> Result<MovePlan, PlanError> {
        let mut plan = self
            .active_for(chat)?
            .filter(|p| p.chat(chat).is_some_and(|c| c.state.moved()))
            .ok_or(PlanError::NotMoved)?;
        let entry = plan.chat_mut(chat).ok_or(PlanError::NotMoved)?;
        match &mut entry.back {
            Some(back) if back.approved => {}
            Some(back) => {
                back.after = after;
                back.requested_by = requested_by;
            }
            None => {
                entry.back = Some(BackRecord {
                    after,
                    requested_by,
                    approved: false,
                    agent_stopped: false,
                    bundle: None,
                    branch: None,
                    continued: false,
                    computer_done: false,
                    done: false,
                })
            }
        }
        self.save(&plan)?;
        Ok(plan)
    }

    /// The person approved bringing this chat back.
    pub fn approve_bring_back(&self, id: &str, chat: &str) -> Result<MovePlan, PlanError> {
        let mut plan = self.load(id)?;
        let back = plan
            .chat_mut(chat)
            .and_then(|c| c.back.as_mut())
            .ok_or(PlanError::NotMoved)?;
        if !back.approved {
            back.approved = true;
            self.save(&plan)?;
        }
        Ok(plan)
    }
}

/// A chat to plan, with what the card shows.
pub struct PlanInput {
    pub chat: ChatToMove,
    pub title: String,
}

fn rank(state: MoveState) -> u8 {
    match state {
        MoveState::Planned => 0,
        MoveState::Pausing => 1,
        MoveState::Captured => 2,
        MoveState::Provisioned => 3,
        MoveState::Restored => 4,
        MoveState::Continued => 5,
        MoveState::Released => 6,
        MoveState::StaysHere | MoveState::BroughtBack => 7,
    }
}

fn plain(error: &MoveError) -> String {
    match error {
        MoveError::Computers(_) => {
            "We couldn't reach the cloud computer. Try the move again; this chat stays on this computer until then.".into()
        }
        MoveError::Io(_) => "We couldn't save the move on this computer. Try it again.".into(),
        MoveError::Git(_) => {
            "Git couldn't save this project, so this chat stays on this computer for now.".into()
        }
        MoveError::NotPlanned | MoveError::InvalidChat => {
            "This chat's move is missing. Plan it again.".into()
        }
    }
}

fn not_ready() -> String {
    "We couldn't get the cloud computer ready. Try the move again; this chat stays on this computer until then.".into()
}
fn back_failed() -> String {
    "We couldn't bring this chat's work back yet. Try again; it keeps running on the cloud computer until then.".into()
}

/// Every plain-words reason a plan can give, for the copy guard.
#[cfg(test)]
pub(crate) fn all_reasons() -> Vec<String> {
    let mut all = vec![not_ready(), back_failed()];
    for e in [
        MoveError::Io(std::io::Error::other("x")),
        MoveError::Git("x".into()),
        MoveError::NotPlanned,
    ] {
        all.push(plain(&e));
    }
    for e in [
        PlanError::NothingToMove,
        PlanError::AlreadyMoving,
        PlanError::NotFound,
        PlanError::NotApproved,
        PlanError::Started,
        PlanError::NotMoved,
    ] {
        all.push(e.to_string());
    }
    all
}

/// Runs plans against one account's Computers client.
pub struct Planner<'a, H> {
    pub plans: &'a PlanStore,
    pub moves: &'a MoveStore,
    pub computers: &'a Computers<'a>,
    pub logins: &'a CloudLogins,
    pub roots: &'a LocalRoots,
    pub hooks: &'a H,
    pub cancel: CancellationToken,
    pub pause_wait: Duration,
}

/// The plan shared by the chats moving at once; every change is saved.
struct Shared<'a> {
    plans: &'a PlanStore,
    plan: Mutex<MovePlan>,
}

impl Shared<'_> {
    fn get(&self) -> MovePlan {
        self.plan.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    fn chat(&self, chat: &str) -> Option<PlanChat> {
        self.get().chat(chat).cloned()
    }
    fn update(&self, chat: &str, change: impl FnOnce(&mut PlanChat)) -> Result<(), PlanError> {
        let mut plan = self.plan.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(c) = plan.chat_mut(chat) {
            change(c);
        }
        self.plans.save(&plan)
    }
    fn set(&self, chat: &str, state: MoveState) -> Result<(), PlanError> {
        self.update(chat, |c| {
            c.state = state;
            c.reason = None;
        })
    }
    fn reason(&self, chat: &str, reason: String) -> Result<(), PlanError> {
        self.update(chat, |c| c.reason = Some(reason))
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

impl<H: MoveHooks> Planner<'_, H> {
    fn mover(&self) -> Mover<'_> {
        Mover {
            store: self.moves,
            computers: self.computers,
            logins: self.logins,
            roots: self.roots,
            cancel: self.cancel.clone(),
        }
    }

    /// Save a plan for these chats. Chats already moving are left out.
    pub async fn plan(
        &self,
        chats: Vec<PlanInput>,
        computer: Option<String>,
        requested_by: Requester,
        now: u64,
    ) -> Result<MovePlan, PlanError> {
        if chats.is_empty() {
            return Err(PlanError::NothingToMove);
        }
        let mut free = Vec::new();
        for input in chats {
            if self.plans.active_for(&input.chat.chat)?.is_none() {
                free.push(input);
            }
        }
        if free.is_empty() {
            return Err(PlanError::AlreadyMoving);
        }
        let destination = match computer {
            Some(computer) => {
                self.computers
                    .reference(&computer)
                    .map_err(MoveError::from)?;
                Destination::Existing { computer }
            }
            None => {
                let size = self
                    .computers
                    .capabilities(&self.cancel)
                    .await
                    .ok()
                    .and_then(|c| {
                        ["medium", "small"]
                            .into_iter()
                            .find(|s| c.sizes.iter().any(|have| have == s))
                            .map(str::to_owned)
                            .or_else(|| c.sizes.first().cloned())
                    });
                let options = CreateOptions {
                    size,
                    needs: Some(
                        openagents_sdk::generated::computers::AppCreateComputerNeeds::Variant0(
                            openagents_sdk::generated::computers::AppComputerNeeds {
                                credential_proxy: Some(true),
                                files: Some(true),
                                ..Default::default()
                            },
                        ),
                    ),
                    ..Default::default()
                };
                Destination::New {
                    request: self
                        .computers
                        .create_request(options)
                        .map_err(MoveError::from)?,
                }
            }
        };
        let id = format!("plan-{}", &uuid::Uuid::new_v4().simple().to_string()[..16]);
        let mut entries = Vec::new();
        for input in free {
            let chat = input.chat.chat.clone();
            // A chat that came back, or stayed here, can move again.
            if self.moves.load(&chat)?.is_some() {
                self.moves.retire(&chat, now)?;
            }
            let agent = super::MoveHarness::from_id(input.chat.harness)
                .map(|h| h.name().to_owned())
                .unwrap_or_else(|| "Agent".into());
            let record = self.moves.plan(input.chat, destination.clone(), now)?;
            let (state, reason) = match &record.outcome {
                Some(Outcome::StaysLocal { reason }) => {
                    (MoveState::StaysHere, Some(reason.clone()))
                }
                _ => (MoveState::Planned, None),
            };
            entries.push(PlanChat {
                chat,
                title: input.title,
                agent,
                state,
                reason,
                interrupted: None,
                continuation: None,
                back: None,
            });
        }
        let moving = entries.iter().filter(|c| !c.state.finished()).count();
        let plan = MovePlan {
            v: PLAN_VERSION.into(),
            id,
            created_unix: now,
            requested_by,
            approved_unix: None,
            destination,
            estimate_minutes: estimate_minutes(moving),
            computer: None,
            chats: entries,
        };
        self.plans.save(&plan)?;
        Ok(plan)
    }

    /// The person approved the plan in the app.
    pub fn approve(&self, id: &str, now: u64) -> Result<MovePlan, PlanError> {
        self.plans.approve(id, now)
    }

    /// Drop a plan that hasn't started.
    pub fn discard(&self, id: &str, now: u64) -> Result<(), PlanError> {
        self.plans.discard(self.moves, id, now)
    }

    /// Run an approved plan, or continue it from where it stopped.
    pub async fn start(&self, id: &str) -> Result<MovePlan, PlanError> {
        let plan = self.plans.load(id)?;
        if plan.approved_unix.is_none() {
            return Err(PlanError::NotApproved);
        }
        let chats: Vec<String> = plan.chats.iter().map(|c| c.chat.clone()).collect();
        let shared = Shared {
            plans: self.plans,
            plan: Mutex::new(plan),
        };
        futures::future::join_all(chats.iter().map(|chat| self.prepare(&shared, chat))).await;
        let captured = shared
            .get()
            .chats
            .iter()
            .any(|c| c.state == MoveState::Captured);
        if captured && let Err(reason) = self.provision(&shared).await {
            for c in shared.get().chats {
                if c.state == MoveState::Captured {
                    shared.reason(&c.chat, reason.clone())?;
                }
            }
        }
        futures::future::join_all(chats.iter().map(|chat| self.finish(&shared, chat))).await;
        Ok(shared.get())
    }

    async fn stays_here(&self, shared: &Shared<'_>, chat: &str, reason: String) {
        let plan = shared.get();
        let interrupted = plan.chat(chat).and_then(|c| c.interrupted) == Some(true);
        let _ = shared.update(chat, |c| {
            c.state = MoveState::StaysHere;
            c.reason = Some(reason);
        });
        if interrupted {
            let key = format!("here-{}-{chat}", plan.id);
            if let Err(error) = self.hooks.continue_here(&key, chat, None).await {
                tracing::warn!(chat, %error, "couldn't continue a chat that stayed here");
            }
        }
    }

    /// Pause the agent and capture its project and conversation.
    async fn prepare(&self, shared: &Shared<'_>, chat: &str) {
        let Some(c) = shared.chat(chat) else { return };
        if rank(c.state) >= rank(MoveState::Captured) {
            return;
        }
        if c.interrupted.is_none() {
            if shared.set(chat, MoveState::Pausing).is_err() {
                return;
            }
            match self.hooks.pause(chat, self.pause_wait).await {
                Ok(interrupted) => {
                    if shared
                        .update(chat, |c| c.interrupted = Some(interrupted))
                        .is_err()
                    {
                        return;
                    }
                }
                Err(reason) => return self.stays_here(shared, chat, reason).await,
            }
        }
        match self.mover().capture(chat).await {
            Ok(None) => {
                let _ = shared.set(chat, MoveState::Captured);
            }
            Ok(Some(Outcome::StaysLocal { reason })) => self.stays_here(shared, chat, reason).await,
            Ok(Some(Outcome::Resumed { .. })) => {
                let _ = shared.set(chat, MoveState::Restored);
            }
            Err(error) => {
                let _ = shared.reason(chat, plain(&error));
            }
        }
    }

    /// Make or attach to the plan's one computer, and hand it to every
    /// captured chat.
    async fn provision(&self, shared: &Shared<'_>) -> Result<(), String> {
        let plan = shared.get();
        let reference = match &plan.computer {
            Some(reference) => reference.clone(),
            None => {
                let reference = match &plan.destination {
                    Destination::Existing { computer } => self
                        .computers
                        .reference(computer)
                        .map_err(|_| not_ready())?,
                    Destination::New { request } => self
                        .computers
                        .create(request, &self.cancel)
                        .await
                        .map_err(|error| {
                            tracing::info!(%error, "move: create refused");
                            not_ready()
                        })?
                        .reference()
                        .clone(),
                };
                let mut saved = self.plans.load(&plan.id).map_err(|_| not_ready())?;
                saved.computer = Some(reference.clone());
                self.plans.save(&saved).map_err(|_| not_ready())?;
                *shared.plan.lock().unwrap_or_else(|e| e.into_inner()) = saved;
                reference
            }
        };
        for c in shared.get().chats {
            if c.state != MoveState::Captured {
                continue;
            }
            let mut record = self
                .moves
                .load(&c.chat)
                .map_err(|_| not_ready())?
                .ok_or_else(not_ready)?;
            if record.computer.is_none() {
                record.computer = Some(reference.clone());
                self.moves.save(&record).map_err(|_| not_ready())?;
            }
            shared
                .set(&c.chat, MoveState::Provisioned)
                .map_err(|_| not_ready())?;
        }
        Ok(())
    }

    /// Restore and resume on the computer, continue the chat there, and
    /// release this computer's copy.
    async fn finish(&self, shared: &Shared<'_>, chat: &str) {
        let Some(c) = shared.chat(chat) else { return };
        if c.state == MoveState::Provisioned {
            match self.mover().run(chat).await {
                Ok(Outcome::Resumed { .. }) => {
                    if shared.set(chat, MoveState::Restored).is_err() {
                        return;
                    }
                }
                Ok(Outcome::StaysLocal { reason }) => {
                    return self.stays_here(shared, chat, reason).await;
                }
                Err(error) => {
                    let _ = shared.reason(chat, plain(&error));
                    return;
                }
            }
        }
        let Some(c) = shared.chat(chat) else { return };
        if c.state == MoveState::Restored {
            let record = self.moves.load(chat).ok().flatten();
            let command = match record.as_ref().and_then(|r| r.outcome.as_ref()) {
                Some(Outcome::Resumed { command, .. }) => command.clone(),
                _ => return,
            };
            let computer = record
                .as_ref()
                .and_then(|r| r.computer.as_ref())
                .map(|c| c.id().to_owned())
                .unwrap_or_default();
            let key = format!("move-{}-{chat}", shared.get().id);
            match self
                .hooks
                .continue_on(&key, chat, &computer, &command)
                .await
            {
                Ok(continuation) => {
                    let saved = shared.update(chat, |c| {
                        c.state = MoveState::Continued;
                        c.reason = None;
                        c.continuation = Some(continuation);
                    });
                    if saved.is_err() {
                        return;
                    }
                }
                Err(reason) => {
                    let _ = shared.reason(chat, reason);
                    return;
                }
            }
        }
        let Some(c) = shared.chat(chat) else { return };
        if c.state == MoveState::Continued {
            match self.hooks.release(chat).await {
                Ok(()) => {
                    let _ = shared.set(chat, MoveState::Released);
                }
                Err(reason) => {
                    let _ = shared.reason(chat, reason);
                }
            }
        }
    }

    /// Ask to bring a moved chat back. It runs once approved.
    pub fn request_bring_back(
        &self,
        chat: &str,
        after: AfterBringBack,
        requested_by: Requester,
    ) -> Result<MovePlan, PlanError> {
        self.plans.request_bring_back(chat, after, requested_by)
    }

    /// The person approved bringing this chat back.
    pub fn approve_bring_back(&self, id: &str, chat: &str) -> Result<MovePlan, PlanError> {
        self.plans.approve_bring_back(id, chat)
    }

    /// Bring an approved chat back: stop the cloud agent, fetch the cloud
    /// computer's work to a local branch, continue the chat here, then stop
    /// or delete the computer as chosen. Safe to run again.
    pub async fn bring_back(&self, id: &str, chat: &str) -> Result<MovePlan, PlanError> {
        let plan = self.plans.load(id)?;
        let entry = plan.chat(chat).ok_or(PlanError::NotMoved)?;
        let back = entry.back.clone().ok_or(PlanError::NotMoved)?;
        if back.done {
            return Ok(plan);
        }
        if !back.approved {
            return Err(PlanError::NotApproved);
        }
        let shared = Shared {
            plans: self.plans,
            plan: Mutex::new(plan),
        };
        if let Err(reason) = self.bring_back_steps(&shared, chat).await {
            shared.reason(chat, reason)?;
        }
        Ok(shared.get())
    }

    async fn bring_back_steps(&self, shared: &Shared<'_>, chat: &str) -> Result<(), String> {
        let fail = |_| back_failed();
        let mut record = self
            .moves
            .load(chat)
            .map_err(|_| back_failed())?
            .ok_or_else(back_failed)?;
        let computer = record.computer.clone().ok_or_else(back_failed)?;
        let capture = record.capture.clone().ok_or_else(back_failed)?;
        let back = |shared: &Shared<'_>| {
            shared
                .chat(chat)
                .and_then(|c| c.back)
                .ok_or_else(back_failed)
        };
        let update = |shared: &Shared<'_>, change: &dyn Fn(&mut BackRecord)| {
            shared
                .update(chat, |c| {
                    if let Some(b) = c.back.as_mut() {
                        change(b);
                    }
                })
                .map_err(|_| back_failed())
        };
        // 1. Stop the agent on the cloud computer, so its work holds still.
        if !back(shared)?.agent_stopped {
            if let Some(reference) = resumed_command(self.computers, &record) {
                let reference = reference.map_err(fail)?;
                let now = self
                    .computers
                    .get_command(&reference, &self.cancel)
                    .await
                    .map_err(fail)?;
                if now.outcome().is_none() {
                    self.computers
                        .cancel_command(&reference, &self.cancel)
                        .await
                        .map_err(fail)?;
                }
            }
            update(shared, &|b| b.agent_stopped = true)?;
        }
        // 2. Save the computer's work, committed or not, as one bundle.
        if back(shared)?.bundle.is_none() {
            let state = self
                .computers
                .get(&computer, &self.cancel)
                .await
                .map_err(fail)?;
            if !state.awake() {
                if state.stopped() {
                    self.computers
                        .wake(&computer, &self.cancel)
                        .await
                        .map_err(fail)?;
                }
                self.computers
                    .wait(&computer, WaitFor::Awake, AWAKE_TIMEOUT, &self.cancel)
                    .await
                    .map_err(fail)?;
            }
            let target = record.target.clone().ok_or_else(back_failed)?;
            let script = back_script(&record, &capture.root, &target);
            let mover = self.mover();
            let request = mover
                .prepare_command(
                    &mut record,
                    |r| &mut r.back,
                    &computer,
                    script,
                    BACK_TIMEOUT,
                    None,
                    "",
                )
                .await
                .map_err(|e| plain(&e))?;
            let (outcome, stdout) = mover
                .run_command(&request, BACK_TIMEOUT)
                .await
                .map_err(|e| plain(&e))?;
            let text = String::from_utf8_lossy(&stdout);
            let field = |name: &str| {
                text.lines()
                    .find_map(|l| l.strip_prefix(name).map(str::to_owned))
            };
            if outcome != CommandOutcome::Succeeded {
                return Err(back_failed());
            }
            let bundle = (|| {
                Some(BackBundle {
                    commit: field("commit=")?,
                    sha256: field("sha=")?,
                    size: field("size=")?.trim().parse().ok()?,
                    pieces: field("pieces=")?.trim().parse().ok()?,
                })
            })()
            .ok_or_else(back_failed)?;
            update(shared, &|b| b.bundle = Some(bundle.clone()))?;
        }
        // 3. Fetch it onto a local branch.
        let back_now = back(shared)?;
        let bundle = back_now.bundle.clone().ok_or_else(back_failed)?;
        let branch = format!("openagents/cloud-{}", &chat[..8.min(chat.len())]);
        if back_now.branch.is_none() {
            let local = self.moves.blobs(chat).join("back.bundle");
            if !local.is_file() || sha256_file(&local).ok().as_deref() != Some(&bundle.sha256) {
                let part = local.with_extension("part");
                std::fs::create_dir_all(local.parent().unwrap_or(Path::new(".")))
                    .map_err(|_| back_failed())?;
                let mut bytes = Vec::with_capacity(bundle.size as usize);
                for i in 0..bundle.pieces {
                    let path = format!("{}/back/piece.{i:03}", record.remote_dir());
                    let file = self
                        .computers
                        .read_file(&computer, &path, &self.cancel)
                        .await
                        .map_err(fail)?;
                    bytes.extend_from_slice(&file.bytes);
                }
                if format!("{:x}", Sha256::digest(&bytes)) != bundle.sha256 {
                    return Err(back_failed());
                }
                std::fs::write(&part, &bytes).map_err(|_| back_failed())?;
                std::fs::rename(&part, &local).map_err(|_| back_failed())?;
            }
            let spec = format!("+refs/openagents/back/{chat}:refs/heads/{branch}");
            let path = local.to_string_lossy().into_owned();
            git(&capture.root, &["fetch", "-q", &path, &spec], &[])
                .await
                .map_err(|_| back_failed())?;
            update(shared, &|b| b.branch = Some(branch.clone()))?;
        }
        // 4. Continue the chat here.
        if !back(shared)?.continued {
            let key = format!("back-{}-{chat}", shared.get().id);
            self.hooks.continue_here(&key, chat, Some(&branch)).await?;
            update(shared, &|b| b.continued = true)?;
        }
        // 5. Stop or delete the computer, once no other chat runs on it.
        let back_now = back(shared)?;
        if !back_now.computer_done {
            let others = shared
                .get()
                .chats
                .iter()
                .any(|c| c.chat != chat && c.state.moved());
            if !others {
                match back_now.after {
                    AfterBringBack::Stop => {
                        self.computers
                            .stop(&computer, &self.cancel)
                            .await
                            .map_err(fail)?;
                    }
                    AfterBringBack::Delete => {
                        self.computers
                            .delete(&computer, &self.cancel)
                            .await
                            .map_err(fail)?;
                    }
                    AfterBringBack::Keep => {}
                }
            }
            update(shared, &|b| b.computer_done = true)?;
        }
        shared
            .update(chat, |c| {
                c.state = MoveState::BroughtBack;
                c.reason = None;
                if let Some(b) = c.back.as_mut() {
                    b.done = true;
                }
            })
            .map_err(|_| back_failed())?;
        self.moves
            .retire(chat, now_unix())
            .map_err(|_| back_failed())?;
        Ok(())
    }
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    Ok(format!("{:x}", Sha256::digest(std::fs::read(path)?)))
}

/// The command that commits the cloud computer's work, tracked and
/// untracked, on top of its branch through a private index, bundles it, and
/// cuts the bundle into pieces the files API can read.
fn back_script(record: &MoveRecord, local_root: &Path, target: &RestoreTarget) -> String {
    let root = match target {
        RestoreTarget::SamePath => local_root.to_string_lossy().into_owned(),
        RestoreTarget::Elsewhere { root } => root.clone(),
    };
    let dir = format!("\"$HOME\"/{}", q(&format!("{}/back", record.remote_dir())));
    let git_ref = format!("refs/openagents/back/{}", record.chat);
    format!(
        r#"set -eu
P={p}; D={dir}; REF={r}
sum() {{ if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1; }}
mkdir -p "$D"; cd "$P"
export GIT_INDEX_FILE="$D/index"; rm -f "$GIT_INDEX_FILE"
export GIT_AUTHOR_NAME=OpenAgents GIT_AUTHOR_EMAIL=move@openagents.com GIT_COMMITTER_NAME=OpenAgents GIT_COMMITTER_EMAIL=move@openagents.com
if git rev-parse -q --verify 'HEAD^{{commit}}' >/dev/null; then
  git read-tree HEAD; git add -A -- .; T=$(git write-tree)
  if [ "$T" = "$(git rev-parse 'HEAD^{{tree}}')" ]; then C=$(git rev-parse HEAD); else C=$(git commit-tree --no-gpg-sign "$T" -p HEAD -m 'Work from the cloud computer'); fi
else
  git add -A -- .; T=$(git write-tree); C=$(git commit-tree --no-gpg-sign "$T" -m 'Work from the cloud computer')
fi
rm -f "$GIT_INDEX_FILE"; unset GIT_INDEX_FILE
git update-ref "$REF" "$C"
rm -f "$D/bundle" "$D"/piece.*
git bundle create -q "$D/bundle" "$REF"
S=$(wc -c < "$D/bundle" | tr -d ' '); N=0
while [ $((N * 4194304)) -lt "$S" ]; do
  dd if="$D/bundle" of="$D/piece.$(printf '%03d' "$N")" bs=4194304 skip="$N" count=1 2>/dev/null
  N=$((N + 1))
done
printf 'commit=%s\nsha=%s\nsize=%s\npieces=%s\n' "$C" "$(sum "$D/bundle")" "$S" "$N""#,
        p = q(&root),
        r = q(&git_ref),
    )
}

#[cfg(test)]
mod tests;
