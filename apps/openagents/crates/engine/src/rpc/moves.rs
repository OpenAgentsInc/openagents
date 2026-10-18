//! Move plans over the engine (#11933): the `PlanMove` family of methods and
//! the [`MoveHooks`] the planner drives this engine's chats through.
//!
//! Plans and per-chat move records live under `<data>/moves`. Starting a plan
//! or a Bring back runs in the background; `MoveStatus` reads its progress.
//! Only `ApproveMove` approves, and the agent tools don't offer it.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use chat_home::{Home, PartRef};
use openagents_proto::{ChatConfig, SessionStatus};
use openagents_rpc::moves::{
    APPROVE_MOVE, ApproveMoveParams, BRING_BACK, BringBackParams, DISCARD_MOVE, MOVE_STATUS,
    MoveStatus, PLAN_MOVE, PlanIdParams, PlanMoveParams, START_MOVE,
};
use openagents_rpc::{RpcError, RpcReply, parse_params};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{EngineRpc, MutateParams};
use crate::sessions::move_cloud::plan::{
    MoveHooks, PAUSE_WAIT, PlanError, PlanInput, PlanStore, Planner, STEER_PROMPT,
};
use crate::sessions::move_cloud::{
    CONTINUE_PROMPT, ChatToMove, CloudLogins, LocalRoots, LoginSource, MoveStore,
};

/// Every move method this module answers.
pub(super) fn handles(method: &str) -> bool {
    matches!(
        method,
        PLAN_MOVE | APPROVE_MOVE | START_MOVE | MOVE_STATUS | BRING_BACK | DISCARD_MOVE
    )
}

/// The logins a cloud computer may use, by reference: `<data>/moves/logins.json`
/// names a registered credential source and rule per agent. Never a login.
#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
struct LoginsFile {
    #[serde(default)]
    claude_code: Option<LoginSource>,
    #[serde(default)]
    codex: Option<LoginSource>,
    #[serde(default)]
    git: Option<LoginSource>,
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// A stable chat ID for a key, so a retry names the same chat.
fn keyed_id(key: &str) -> String {
    let digest = Sha256::digest(key.as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    uuid::Builder::from_random_bytes(bytes)
        .into_uuid()
        .to_string()
}

fn failed(error: impl std::fmt::Display) -> RpcError {
    RpcError::Failed(error.to_string())
}

fn signed_out() -> RpcError {
    RpcError::Failed("Connecting to your account… Try moving the chats again in a moment.".into())
}

/// Plans and Bring backs running in this process, so a second request
/// doesn't run the same one twice at once.
fn running() -> &'static Mutex<HashSet<String>> {
    static RUNNING: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    RUNNING.get_or_init(Default::default)
}

enum Job {
    Start(String),
    Back(String, String),
}

impl Job {
    fn key(&self) -> String {
        match self {
            Self::Start(plan) => plan.clone(),
            Self::Back(plan, chat) => format!("{plan}/{chat}"),
        }
    }
}

impl EngineRpc {
    fn moves_dir(&self) -> PathBuf {
        self.repos.data_dir().join("moves")
    }

    fn move_stores(&self) -> Result<(PlanStore, MoveStore), RpcError> {
        let dir = self.moves_dir();
        Ok((
            PlanStore::open(dir.join("plans")).map_err(failed)?,
            MoveStore::open(dir.join("chats")).map_err(failed)?,
        ))
    }

    fn cloud_logins(&self) -> CloudLogins {
        let file: LoginsFile = std::fs::read(self.moves_dir().join("logins.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        CloudLogins {
            claude_code: file.claude_code,
            codex: file.codex,
            git: file.git,
        }
    }

    /// The chats with an agent working or waiting on this computer.
    fn running_chats(&self) -> Vec<String> {
        let device = &self.engine_info.device_id;
        self.sessions
            .watch_sessions()
            .borrow()
            .iter()
            .filter(|s| {
                &s.device_id == device
                    && matches!(
                        s.status,
                        SessionStatus::Working | SessionStatus::AwaitingInput
                    )
            })
            .map(|s| s.chat_id.clone())
            .collect()
    }

    fn plan_input(&self, chat: &str) -> Option<PlanInput> {
        let row = self.workspace.chat(chat).ok().flatten()?;
        let config = row.config.as_ref()?;
        let project = row
            .harness_session_cwd
            .clone()
            .or_else(|| row.cwd.clone())
            .filter(|c| !c.is_empty())?;
        let title = row.title.clone().unwrap_or_else(|| "Chat".into());
        let seed = self.doc_host.move_seed(chat, &title).ok()?;
        Some(PlanInput {
            chat: ChatToMove {
                chat: chat.into(),
                harness: config.harness,
                project: project.into(),
                session: row.harness_session_id.clone().unwrap_or_default(),
                seed,
            },
            title,
        })
    }

    pub(super) async fn handle_move(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<RpcReply, RpcError> {
        let (plans, moves) = self.move_stores()?;
        match method {
            PLAN_MOVE => {
                let p: PlanMoveParams = parse_params(params)?;
                let chats = if p.chats.is_empty() {
                    self.running_chats()
                } else {
                    p.chats
                };
                let inputs: Vec<PlanInput> =
                    chats.iter().filter_map(|c| self.plan_input(c)).collect();
                let lease = self.doc_host.move_lease().ok_or_else(signed_out)?;
                let computers = lease.client().computers(lease.session()).map_err(failed)?;
                let logins = self.cloud_logins();
                let roots = LocalRoots::from_env();
                let hooks = EngineHooks {
                    rpc: self.clone(),
                    account: lease.session().account().id.clone(),
                };
                let planner = Planner {
                    plans: &plans,
                    moves: &moves,
                    computers: &computers,
                    logins: &logins,
                    roots: &roots,
                    hooks: &hooks,
                    cancel: lease.cancel.clone(),
                    pause_wait: PAUSE_WAIT,
                };
                let plan = planner
                    .plan(inputs, p.computer, p.requested_by, now_unix())
                    .await
                    .map_err(failed)?;
                RpcReply::value(&plan.view())
            }
            APPROVE_MOVE => {
                let p: ApproveMoveParams = parse_params(params)?;
                let (plan, job) = match p.bring_back {
                    Some(chat) => (
                        plans
                            .approve_bring_back(&p.plan_id, &chat)
                            .map_err(failed)?,
                        Job::Back(p.plan_id.clone(), chat),
                    ),
                    None => (
                        plans.approve(&p.plan_id, now_unix()).map_err(failed)?,
                        Job::Start(p.plan_id.clone()),
                    ),
                };
                self.spawn_move(job);
                RpcReply::value(&plan.view())
            }
            START_MOVE => {
                let p: PlanIdParams = parse_params(params)?;
                let id = p.plan_id.ok_or_else(|| failed(PlanError::NotFound))?;
                let plan = plans.load(&id).map_err(failed)?;
                if plan.approved_unix.is_none() {
                    return Err(failed(PlanError::NotApproved));
                }
                // An approved Bring back that stopped partway runs again too.
                for c in &plan.chats {
                    if c.back.as_ref().is_some_and(|b| b.approved && !b.done) {
                        self.spawn_move(Job::Back(id.clone(), c.chat.clone()));
                    }
                }
                self.spawn_move(Job::Start(id));
                RpcReply::value(&plan.view())
            }
            MOVE_STATUS => {
                let p: PlanIdParams = parse_params(params)?;
                let plans = match p.plan_id {
                    Some(id) => vec![plans.load(&id).map_err(failed)?],
                    None => plans.list().map_err(failed)?,
                };
                RpcReply::value(&MoveStatus {
                    plans: plans.iter().map(|p| p.view()).collect(),
                })
            }
            BRING_BACK => {
                let p: BringBackParams = parse_params(params)?;
                let plan = plans
                    .request_bring_back(&p.chat, p.after, p.requested_by)
                    .map_err(failed)?;
                RpcReply::value(&plan.view())
            }
            DISCARD_MOVE => {
                let p: PlanIdParams = parse_params(params)?;
                let id = p.plan_id.ok_or_else(|| failed(PlanError::NotFound))?;
                plans.discard(&moves, &id, now_unix()).map_err(failed)?;
                RpcReply::value(&serde_json::json!({}))
            }
            other => Err(RpcError::UnknownMethod(other.into())),
        }
    }

    fn spawn_move(&self, job: Job) {
        let key = job.key();
        if !running()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.clone())
        {
            return;
        }
        let rpc = self.clone();
        tokio::spawn(async move {
            if let Err(error) = rpc.run_move(job).await {
                tracing::warn!(%error, "move didn't finish; it can be started again");
            }
            running()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&key);
        });
    }

    async fn run_move(&self, job: Job) -> Result<(), RpcError> {
        let (plans, moves) = self.move_stores()?;
        let lease = self.doc_host.move_lease().ok_or_else(signed_out)?;
        let computers = lease.client().computers(lease.session()).map_err(failed)?;
        let logins = self.cloud_logins();
        let roots = LocalRoots::from_env();
        let hooks = EngineHooks {
            rpc: self.clone(),
            account: lease.session().account().id.clone(),
        };
        let planner = Planner {
            plans: &plans,
            moves: &moves,
            computers: &computers,
            logins: &logins,
            roots: &roots,
            hooks: &hooks,
            cancel: lease.cancel.clone(),
            pause_wait: PAUSE_WAIT,
        };
        match job {
            Job::Start(id) => planner.start(&id).await.map(drop),
            Job::Back(id, chat) => planner.bring_back(&id, &chat).await.map(drop),
        }
        .map_err(failed)
    }
}

/// What the planner does to this engine's chats.
struct EngineHooks {
    rpc: EngineRpc,
    account: String,
}

impl EngineHooks {
    /// Saves `key` before the first send, so a key sends at most once.
    fn first_send(&self, key: &str) -> Result<bool, String> {
        let dir = self.rpc.moves_dir().join("sent");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let marker = dir.join(format!("{:x}", Sha256::digest(key.as_bytes())));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(marker)
        {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
            Err(e) => Err(e.to_string()),
        }
    }
}

const POLL: Duration = Duration::from_millis(250);

impl MoveHooks for EngineHooks {
    async fn pause(&self, chat: &str, wait: Duration) -> Result<bool, String> {
        let sessions = &self.rpc.sessions;
        // A pending question can't cross to the other computer, so its turn
        // ends here and the agent asks again there.
        if sessions
            .session_status(chat)
            .is_some_and(|s| s.status == SessionStatus::AwaitingInput)
        {
            sessions.stop_turn(chat).await.map_err(|e| e.to_string())?;
            return Ok(true);
        }
        if !sessions.turn_in_flight(chat) {
            return Ok(false);
        }
        if sessions.live_run_steerable(chat) {
            let _ = sessions.steer(chat, STEER_PROMPT, None).await;
        }
        let deadline = Instant::now() + wait;
        while Instant::now() < deadline {
            if !sessions.turn_in_flight(chat) {
                return Ok(false);
            }
            tokio::time::sleep(POLL).await;
        }
        sessions.stop_turn(chat).await.map_err(|e| e.to_string())?;
        Ok(true)
    }

    async fn continue_on(
        &self,
        key: &str,
        chat: &str,
        _computer: &str,
        _command: &str,
    ) -> Result<String, String> {
        // A linked continuation in the account (#11898), seeded from this
        // chat; a retry reuses the saved destination.
        let target = PartRef {
            id: keyed_id(key),
            home: Home::Cloud,
        };
        self.rpc
            .doc_host
            .continue_on(&self.account, chat, target)
            .await
            .map(|continued| continued.target.id)
            .map_err(|e| e.to_string())
    }

    async fn release(&self, chat: &str) -> Result<(), String> {
        self.rpc
            .sessions
            .terminate(chat)
            .await
            .map(drop)
            .map_err(|e| e.to_string())
    }

    async fn continue_here(
        &self,
        key: &str,
        chat: &str,
        branch: Option<&str>,
    ) -> Result<(), String> {
        let row = self
            .rpc
            .workspace
            .chat(chat)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Open the chat again.".to_string())?;
        let text = match branch {
            Some(branch) => format!(
                "{CONTINUE_PROMPT} Your work from the cloud computer is on the local branch `{branch}`. Check it out or merge it before you go on."
            ),
            None => CONTINUE_PROMPT.to_owned(),
        };
        let Some(part) = row.links.continued_by.clone() else {
            // The chat never left: continue it where it is.
            if self.first_send(key)? {
                self.rpc
                    .doc_host
                    .queue_message_with_behavior(chat, &text, vec![], false)
                    .map_err(|e| e.to_string())?;
            }
            return Ok(());
        };
        // The chat continues in the account: continue that part here, in a
        // new chat in the same project.
        let id = keyed_id(key);
        if self.rpc.workspace.chat(&id).ok().flatten().is_none() {
            let revision = self
                .rpc
                .doc_host
                .part_revision(&part.id)
                .await
                .map_err(|e| e.to_string())?;
            let source = chat_home::Source {
                account: self.account.clone(),
                chat: part.id.clone(),
                revision,
            };
            let mut config: ChatConfig = row
                .config
                .clone()
                .ok_or_else(|| "Open the chat again.".to_string())?;
            config.model_options.insert(
                chat_home::REQUEST_OPTION.into(),
                serde_json::to_value(&source).map_err(|e| e.to_string())?,
            );
            self.rpc
                .mutate(MutateParams::CreateChat {
                    chat_id: id.clone(),
                    space_id: row.space_id.clone(),
                    device_id: Some(self.rpc.engine_info.device_id.clone()),
                    config: Some(config),
                    branch: Some(branch.unwrap_or_default().to_owned()).filter(|b| !b.is_empty()),
                    cwd: row.cwd.clone(),
                })
                .map_err(|e| e.to_string())?;
        }
        if self.first_send(key)? {
            self.rpc
                .doc_host
                .queue_message_with_behavior(&id, &text, vec![], false)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyed_ids_are_stable_chat_ids() {
        let a = keyed_id("back-plan-0123456789abcdef-chat");
        assert_eq!(a, keyed_id("back-plan-0123456789abcdef-chat"));
        assert_ne!(a, keyed_id("move-plan-0123456789abcdef-chat"));
        assert!(chat_home::valid_id(&a), "{a}");
    }
}
