//! Task operations a resident host admitted for an enrolled device.
//!
//! `coder host serve` checks the device's grant and its operation's right,
//! then calls this inbox. Evidence reads need `observe`; commands need
//! `operate`. A creation is an ordinary inert submission: it
//! records intent and starts nothing, so an enrolled device never gains
//! execution authority. The local owner still needs its own explicit grant.
//! A steer is a correction and a cancel is a cancellation, with the
//! semantics `docs/coder/runtime/task-owner.md` describes, including
//! superseding a running context.
//!
//! Every command's identity derives from the NIP-HOST request ID, and its
//! bytes derive only from the request, so a retry after an uncertain save
//! is an exact-byte retry that returns the original receipt.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use coder_host::access::protocol::{Operation, QueueEdit, QueueItem, QueueLease, TaskQueue};
use coder_host::{
    Code, CommandAction, Note, Principal, Standing, TaskCommand, TaskCreate, TaskRef, Tasks,
};
use nostr::activity_summary::Phase;

use super::capacity::Provider;
use super::{
    Action, COMMAND_SCHEMA, Command, Error, Receipt, RequestedConfiguration, Status, Store,
    TaskIntent, Workspace,
};

#[path = "remote_observe.rs"]
mod observation;

#[cfg(test)]
#[path = "remote_control_tests.rs"]
mod control_tests;

/// The durable inbox behind a resident host.
#[derive(Clone, Debug)]
pub struct Inbox {
    store: PathBuf,
    workspaces: BTreeMap<String, PathBuf>,
    autostart: Option<Arc<super::autostart::Autostart>>,
    /// The engine the person asked for, by the create request it is for
    /// (#10076): set only by the host itself, from its own chat's typed
    /// offer, just before it creates that task; a device never sets it.
    preferences: Arc<std::sync::Mutex<BTreeMap<String, nostr::cj_conversation::Engine>>>,
    /// The owner's settings file, whose `coder.start` says whether a
    /// device's coding reply starts Coder at once (#10101); `None` reads
    /// [`super::settings::path`].
    settings: Option<PathBuf>,
    /// The workshop agents this host answers for
    /// (`docs/verse/workshop-agent.md`).
    agents: Option<Arc<super::agent_host::Agents>>,
    cloud: Option<Arc<dyn coder_host::cloud::Cloud>>,
    projects: Option<Arc<dyn coder_host::projects::Projects>>,
}

/// The most engine preferences an inbox holds for creates not yet made.
const MAX_PREFERENCES: usize = 64;

impl Inbox {
    #[must_use]
    pub fn with_cloud(mut self, cloud: Arc<dyn coder_host::cloud::Cloud>) -> Self {
        self.cloud = Some(cloud);
        self
    }
    #[must_use]
    pub fn with_projects(mut self, projects: Arc<dyn coder_host::projects::Projects>) -> Self {
        self.projects = Some(projects);
        self
    }
    /// An inbox over the task store at `store`. `workspaces` maps the labels
    /// a device may name to their roots; a device never sends a path.
    #[must_use]
    pub fn new(store: impl Into<PathBuf>, workspaces: BTreeMap<String, PathBuf>) -> Self {
        let store = store.into();
        if super::present(&store) {
            let _ = super::targets::cleanup(&store);
        }
        Self {
            store,
            workspaces,
            autostart: None,
            preferences: Arc::new(std::sync::Mutex::new(BTreeMap::new())),
            settings: None,
            agents: None,
            cloud: None,
            projects: None,
        }
    }

    /// Answer `studio.agent.*` for the workshop agents under `agents`'s
    /// host root, and run their standing jobs on each sweep.
    #[must_use]
    pub fn with_agents(mut self, agents: super::agent_host::Agents) -> Self {
        // A workshop agent who signs her commits signs her merged change.
        super::studio::git::set_seat_signer(
            &self.store,
            super::agent_git_sign::seat_signer(agents.root()),
        );
        let agents = match &self.autostart {
            Some(autostart) => {
                let autostart = autostart.clone();
                agents.with_sweep(Arc::new(move || autostart.sweep_soon()))
            }
            None => agents,
        };
        self.agents = Some(Arc::new(agents));
        self
    }

    /// Read `coder.start` from `file` instead of [`super::settings::path`].
    #[must_use]
    pub fn with_settings(mut self, file: impl Into<PathBuf>) -> Self {
        self.settings = Some(file.into());
        self
    }

    /// Whether a device's coding reply starts Coder here at once (#10101):
    /// the owner's auto-start policy is on, so a created task runs, and
    /// their `coder.start` is `at_once`. A settings file Coder's loader
    /// refuses asks first, as [`super::local::Local::asks_first`] does.
    #[must_use]
    pub fn starts_at_once(&self) -> bool {
        let on = self
            .autostart
            .as_ref()
            .and_then(|autostart| autostart.policy())
            .is_some_and(|policy| policy.enabled);
        on && {
            let file = self.settings.clone().unwrap_or_else(super::settings::path);
            super::settings::Settings::load(&file)
                .is_ok_and(|settings| settings.coder.start == super::settings::Start::AtOnce)
        }
    }

    /// Consult the owner's auto-start policy on every creation. With the
    /// policy off, creation stays the inert submission it is without one.
    #[must_use]
    pub fn with_autostart(mut self, autostart: Arc<super::autostart::Autostart>) -> Self {
        self.autostart = Some(autostart);
        self
    }

    /// The task store directory.
    #[must_use]
    pub fn store(&self) -> &Path {
        &self.store
    }

    /// Tell the auto-start policy about tasks a device command continued,
    /// so each new turn starts under the same bounds as a new task. Without
    /// the policy a continued task waits, inert.
    fn continued(&self, continued: &[super::commands::Continued]) {
        let Some(autostart) = &self.autostart else {
            return;
        };
        let Ok(store) = Store::open(&self.store) else {
            return;
        };
        let mut any = false;
        for item in continued {
            let Ok(task) = store.show(&item.task) else {
                continue;
            };
            // A studio task works in its own worktree; its later turns
            // belong to the workspace of the checkout that worktree came from.
            let label = super::commands::label_for(&self.workspaces, &task).or_else(|| {
                let record = super::local::record(&self.store, &item.task)?;
                self.workspaces
                    .iter()
                    .find(|(_, root)| root.to_string_lossy() == record.checkout)
                    .map(|(label, _)| label.as_str())
            });
            if let Some(label) = label {
                autostart.eligible_turn(&item.task, &item.device, label, item.turn);
                any = true;
            }
        }
        drop(store);
        if any {
            autostart.sweep_soon();
        }
    }

    /// **Always allow for this seat** (`studio.decision.always`): approve
    /// the waiting studio task's step through the `studio.decision.answer`
    /// path under the device's command ID, then keep a standing rule for
    /// that seat and step, recorded by `principal`. The host keeps the
    /// rule only when `rule` is the exact text it offers for the step
    /// now; another text, a moved task, or a step the approval did not
    /// name refuses as `stale`. The intent answers once per request ID.
    fn allow_always(
        &self,
        key: &str,
        principal: &Principal,
        op: &Operation,
        standing: Standing<'_>,
    ) -> Result<String, Code> {
        use super::interaction::{self, Step};
        use super::studio::{Studio, rules::Author};
        let Operation::AllowAlways {
            decision,
            based_on,
            rule,
            command,
            issued_at,
        } = op
        else {
            return Err(Code::Unsupported);
        };
        if !Studio::present(&self.store) {
            return Err(studio_refused(Code::Forbidden, "This host has no studio."));
        }
        let (seat, step) = {
            let studio = Studio::open(&self.store).map_err(studio_refusal)?;
            if let Some(reference) = studio.answered(key) {
                return Ok(reference);
            }
            let seat = studio.seat_of(decision).ok_or_else(|| {
                studio_refused(Code::Forbidden, "No studio seat holds this task.")
            })?;
            let task = Store::open(&self.store)
                .and_then(|tasks| tasks.show(decision))
                .map_err(|_| {
                    studio_refused(Code::Forbidden, "The task inbox does not hold this task.")
                })?;
            let moved = || {
                studio_refused(
                    Code::Stale,
                    "The task no longer asks for this approval. Read its decision again.",
                )
            };
            if task.revision != *based_on
                || interaction::pending(&task) != Some(interaction::Kind::Approval)
            {
                return Err(moved());
            }
            let step = super::local::asked_in(Some(&self.store), decision)
                .as_deref()
                .and_then(Step::in_reply)
                .ok_or_else(moved)?;
            if Studio::offer(&seat, &step).as_deref() != Some(rule.as_str()) {
                return Err(studio_refused(
                    Code::Stale,
                    "The rule this approval offers changed. Read its decision again.",
                ));
            }
            // A full rule book refuses before the step is approved.
            if studio.rules().len() >= super::studio::rules::MAX_RULES
                && studio.standing_rule(&seat, &step).is_none()
            {
                return Err(studio_refused(
                    Code::Bounds,
                    "The studio holds the most standing rules. Remove one before you add another.",
                ));
            }
            (seat, step)
        };
        let answer = Operation::AnswerDecision {
            decision: decision.clone(),
            based_on: *based_on,
            text: format!("Approved. Always allowed for this seat: {rule}"),
            command: command.clone(),
            issued_at: *issued_at,
        };
        let reference = self.studio_intent(key, principal, &answer, standing)?;
        let mut studio = Studio::open(&self.store).map_err(studio_refusal)?;
        let by = Author {
            device: principal.device.clone(),
            grant: principal.grant.clone(),
            epoch: principal.epoch,
        };
        studio
            .allow_always(&seat, &step, rule, by, super::autostart::unix_now())
            .map_err(studio_refusal)?;
        studio
            .record_answer(key, &reference)
            .map_err(studio_refusal)?;
        Ok(reference)
    }

    /// The host's policy for standing rules: answer each waiting studio
    /// approval a rule admits, once, as the rule's author, after
    /// `standing` rechecks that author's grant. The rule is consumed for
    /// the wait before the answer is recorded, so a failed answer leaves
    /// the wait to the person rather than answering it twice.
    fn apply_standing_rules(&self, standing: Standing<'_>) {
        use super::studio::Studio;
        if !Studio::present(&self.store) {
            return;
        }
        let due = {
            let (Ok(tasks), Ok(studio)) = (Store::open(&self.store), Studio::open(&self.store))
            else {
                return;
            };
            let asked = |task: &str| super::local::asked_in(Some(&self.store), task);
            studio.standing_answers(&tasks, &asked)
        };
        for due in due {
            let principal = Principal {
                device: due.rule.by.device.clone(),
                grant: due.rule.by.grant.clone(),
                epoch: due.rule.by.epoch,
            };
            if !standing(&principal) {
                continue;
            }
            let now = super::autostart::unix_now();
            let consumed =
                Studio::open(&self.store).and_then(|mut studio| studio.note_applied(&due, now));
            if !matches!(consumed, Ok(true)) {
                continue;
            }
            let answer = TaskCommand {
                command: due.command(),
                task: due.task.clone(),
                action: CommandAction::Answer,
                based_on: due.revision,
                text: due.answer(),
                emulate: false,
                issued_at: now,
            };
            if let Err(code) = self.command(&principal, &answer, standing) {
                eprintln!(
                    "openagents host: studio: a standing rule's answer to {} was refused: {code:?}",
                    due.task
                );
            }
        }
    }

    fn apply(&self, command: &Command) -> Result<TaskRef, Code> {
        let bytes = serde_json::to_vec(command).map_err(|_| Code::Malformed)?;
        let mut store = Store::open(&self.store).map_err(refusal)?;
        let receipt = store.apply(&bytes).map_err(refusal)?;
        Ok(reference(&receipt))
    }

    /// Mark studio task `task` merged in its coordinator once its merge
    /// landed, so its plan entry is done, and ask the auto-start policy for
    /// a pass so the entries that wait on it start. A failure is logged:
    /// the coordinator's next pass marks the merge itself.
    fn studio_merged(&self, task: &str) {
        let marked = super::studio::Studio::open(&self.store)
            .and_then(|mut studio| studio.note_merged(task));
        match marked {
            Ok(true) => {
                if let Some(autostart) = &self.autostart {
                    autostart.sweep_soon();
                }
            }
            Ok(false) => {}
            Err(error) => {
                eprintln!("openagents host: studio: cannot mark task {task} merged: {error}");
            }
        }
    }

    /// Bind `principal` as the approver of the step studio task `task`
    /// asks to approve at revision `based_on`, under the answer's
    /// `command` ID. Returns the approval subject, or `None` when the task
    /// asks no approval at that revision: a question, or a stale answer
    /// the command journal refuses.
    fn bind_approver(
        &self,
        principal: &Principal,
        task: &str,
        based_on: u64,
        text: &str,
        command: &str,
    ) -> Result<Option<String>, Code> {
        use super::studio::approvals::{Action, Approver};
        let record = Store::open(&self.store)
            .map_err(refusal)?
            .show(task)
            .map_err(|_| {
                studio_refused(Code::Forbidden, "The task inbox does not hold this task.")
            })?;
        let Some(action) = Action::of(&record).filter(|action| action.revision == based_on) else {
            return Ok(None);
        };
        let approver = Approver {
            device: principal.device.clone(),
            grant: principal.grant.clone(),
            epoch: principal.epoch,
        };
        let mut studio = super::studio::Studio::open(&self.store).map_err(studio_refusal)?;
        let bound = studio
            .bind_approval(
                &action,
                approver,
                text,
                command,
                super::autostart::unix_now(),
            )
            .map_err(studio_refusal)?;
        Ok(Some(bound.subject))
    }
}

impl Inbox {
    fn command_inner(
        &self,
        principal: &Principal,
        command: &TaskCommand,
        expected: Option<u64>,
        standing: Standing<'_>,
    ) -> Result<TaskRef, Code> {
        use super::commands::{Kind, Outcome, Rejection, Request, State};
        let sender = sender(principal);
        let request = Request {
            command: command.command.clone(),
            task: command.task.clone(),
            kind: match command.action {
                CommandAction::Send => Kind::Send,
                CommandAction::Queue => Kind::Queue,
                CommandAction::Steer => Kind::Steer,
                CommandAction::Interrupt => Kind::Interrupt,
                CommandAction::Answer => Kind::Answer,
            },
            based_on: command.based_on,
            text: command.text.clone(),
            emulate: command.emulate,
            issued_at: command.issued_at,
        };
        // The host admitted this request's grant a moment ago; any other
        // sender's held command is rechecked.
        let admitted =
            |other: &super::commands::Sender| *other == sender || standing(&principal_of(other));
        let result = match expected {
            Some(revision) => super::commands::record_at_revision(
                &self.store,
                &sender,
                &request,
                revision,
                &super::adapter::STEERING,
                &admitted,
                super::autostart::unix_now(),
            ),
            None => super::commands::record(
                &self.store,
                &sender,
                &request,
                &super::adapter::STEERING,
                &admitted,
                super::autostart::unix_now(),
            ),
        };
        let (recorded, continued) = result.map_err(|error| match error {
            Error::NotFound => Code::Forbidden,
            other => refusal(other),
        })?;
        self.continued(&continued);
        let task = recorded.task.as_ref().map(current).ok_or(Code::Forbidden)?;
        match recorded.state {
            State::Done(Outcome::Rejected { reason }) => Err(match reason {
                Rejection::Conflict => Code::Conflict,
                Rejection::Unsupported => Code::Unsupported,
                Rejection::Unavailable => Code::Unavailable,
                Rejection::Revoked => Code::Revoked,
                Rejection::Stale => Code::Stale,
                Rejection::Missing => Code::Forbidden,
                Rejection::Bounds => Code::Bounds,
            }),
            State::Done(Outcome::Expired) => Err(Code::Expired),
            State::Done(Outcome::Superseded) => Err(Code::Stale),
            _ => Ok(task),
        }
    }
}

fn local_queue_edit(edit: &QueueEdit) -> super::commands::QueueEdit {
    use super::commands::QueueEdit as Edit;
    match edit {
        QueueEdit::List {} => Edit::List,
        QueueEdit::Lease {} => Edit::Lease,
        QueueEdit::Release {} => Edit::Release,
        QueueEdit::Edit { command, text } => Edit::Edit {
            command: command.clone(),
            text: text.clone(),
        },
        QueueEdit::Remove { command } => Edit::Remove {
            command: command.clone(),
        },
        QueueEdit::Reorder { commands } => Edit::Reorder {
            commands: commands.clone(),
        },
        QueueEdit::SendNow { command } => Edit::SendNow {
            command: command.clone(),
        },
    }
}

impl Tasks for Inbox {
    fn cloud(
        &self,
        request: &str,
        principal: &Principal,
        op: &Operation,
    ) -> Result<coder_host::access::Outcome, Code> {
        self.cloud
            .as_ref()
            .ok_or(Code::Unsupported)?
            .execute(request, principal, op)
    }
    fn cloud_admit_recovery(
        &self,
        device: &str,
        admission: &coder_host::access::cloud::Admission,
    ) -> Result<(), Code> {
        self.cloud
            .as_ref()
            .ok_or(Code::Unsupported)?
            .admit_recovery(device, admission)
    }
    fn project_list(
        &self,
        device: &str,
        workspace: &str,
    ) -> Result<coder_host::access::project::List, Code> {
        self.projects
            .as_ref()
            .ok_or(Code::Unsupported)?
            .list(device, workspace)
    }
    fn project_read(
        &self,
        device: &str,
        query: &coder_host::access::project::Query,
    ) -> Result<coder_host::access::project::Page, Code> {
        self.projects
            .as_ref()
            .ok_or(Code::Unsupported)?
            .read(device, query)
    }
    fn project_original(
        &self,
        device: &str,
        query: &coder_host::access::project::OriginalQuery,
    ) -> Result<coder_host::access::project::Chunk, Code> {
        self.projects
            .as_ref()
            .ok_or(Code::Unsupported)?
            .original(device, query)
    }
    fn task_list(
        &self,
        query: &coder_host::access::task_read::ListQuery,
    ) -> Result<coder_host::access::task_read::List, Code> {
        observation::list(self, query)
    }

    fn task_read(
        &self,
        query: &coder_host::access::task_read::PageQuery,
    ) -> Result<coder_host::access::task_read::Page, Code> {
        observation::page(self, query)
    }

    fn task_original(
        &self,
        query: &coder_host::access::task_read::OriginalQuery,
    ) -> Result<coder_host::access::task_read::OriginalChunk, Code> {
        observation::original(self, query)
    }

    fn terminal_binding(&self, task: &str) -> Result<coder_host::tasks::TerminalBinding, Code> {
        if super::archive::archived(&self.store).contains(task) {
            return Err(Code::Forbidden);
        }
        if super::studio::git::seat_of(&self.store, task).is_none() {
            return Err(Code::Unsupported);
        }
        let record = super::local::record(&self.store, task).ok_or(Code::Unavailable)?;
        let directory = Path::new(&record.worktree)
            .canonicalize()
            .map_err(|_| Code::Unavailable)?;
        if !directory.is_dir() {
            return Err(Code::Unavailable);
        }
        Ok(coder_host::tasks::TerminalBinding {
            directory,
            interactive: !record.shape.read_only,
        })
    }

    fn agent(
        &self,
        key: &str,
        principal: &Principal,
        op: &Operation,
    ) -> Result<serde_json::Value, Code> {
        match &self.agents {
            Some(agents) => agents.answer(key, principal, op),
            None => Err(coder_host::tasks::refuse(
                Code::Unsupported,
                "This host keeps no workshop agents.",
            )),
        }
    }

    fn new_agent(
        &self,
        key: &str,
        principal: &Principal,
        op: &Operation,
        owner: Option<&secp256k1::SecretKey>,
    ) -> Result<serde_json::Value, Code> {
        let Some(agents) = &self.agents else {
            return Err(coder_host::tasks::refuse(
                Code::Unsupported,
                "This host keeps no workshop agents.",
            ));
        };
        match op {
            Operation::CrewStatus {} | Operation::ControlCrew { .. } => {
                agents.control_crew(key, principal, op)
            }
            Operation::ProposeHire { proposal } => agents.propose_hire(principal, proposal),
            Operation::ListHires {} => agents.list_hires(principal),
            Operation::DecideHire {
                decision,
                workspace,
            } => agents.decide_hire(principal, decision, workspace.as_deref(), owner),
            Operation::NewCrewAgent {
                agent,
                workspace,
                job_role,
            } => agents.create_crew(agent, std::path::Path::new(workspace), *job_role, owner),
            Operation::SetAgentCharter { .. } | Operation::RecordAgentVerdict { .. } => {
                agents.owner_crew(principal, op)
            }
            Operation::NewAgent { agent, workspace } => {
                agents.create(agent, std::path::Path::new(workspace), owner)
            }
            Operation::RetireAgent { agent } => agents.retire(agent, owner, &principal.device),
            Operation::RotateAgent { agent, reason } => {
                agents.rotate(agent, reason, owner, &principal.device)
            }
            _ => Err(Code::Unsupported),
        }
    }

    fn agent_reports(&self) -> Vec<coder_host::AgentReport> {
        self.agents
            .as_ref()
            .map(|agents| agents.reports())
            .unwrap_or_default()
    }

    fn capabilities(&self) -> Vec<String> {
        if self.starts_at_once() {
            vec![coder_host::access::protocol::CODER_START_AT_ONCE.to_owned()]
        } else {
            Vec::new()
        }
    }

    fn prefer(&self, key: &str, engine: nostr::cj_conversation::Engine) {
        let mut preferences = self
            .preferences
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if preferences.len() >= MAX_PREFERENCES
            && let Some(oldest) = preferences.keys().next().cloned()
        {
            preferences.remove(&oldest);
        }
        preferences.insert(key.to_owned(), engine);
    }

    fn create(&self, key: &str, device: &str, task: &TaskCreate) -> Result<TaskRef, Code> {
        let root = self
            .workspaces
            .get(&task.workspace)
            .ok_or(Code::Forbidden)?;
        // Images bind before the task exists: each must be one this device
        // sent complete (`artifact.put`), or one a retry of this request
        // already bound. The task's intent then names exactly those bytes.
        coder_host::access::media::validate_all(&task.images).map_err(|_| Code::Malformed)?;
        for image in &task.images {
            super::media::adopt(&self.store, device, key, image).map_err(|error| match error {
                Error::NotFound => Code::Conflict,
                other => refusal(other),
            })?;
        }
        // The engine the person asked for, when the host said so for this
        // request (its own chat's offer), or else the device's typed field
        // (#10081): its route goes first, if the owner's policy admits it.
        // Neither adds a route, a model, or a limit.
        let requested = self
            .preferences
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(key)
            .or(task.engine)
            .map(super::settings::provider_of);
        // Under the owner's policy the task records the engine's model, which
        // its grant must name; otherwise no model, as always.
        let model = self
            .autostart
            .as_ref()
            .and_then(|autostart| autostart.model_requesting(&task.workspace, requested));
        let command = |model: Option<String>| Command {
            schema: COMMAND_SCHEMA.into(),
            command_id: format!("host-create-{key}"),
            task_id: key.into(),
            expected_revision: None,
            action: Action::Submit {
                intent: TaskIntent {
                    title: task.title.clone(),
                    prompt: task.prompt.clone(),
                    workspace: Workspace {
                        path: root.to_string_lossy().into_owned(),
                        source_revision: None,
                    },
                    configuration: RequestedConfiguration {
                        adapter: super::adapter::NAME.into(),
                        model,
                    },
                    images: task.images.clone(),
                },
            },
        };
        let created = match self.apply(&command(model.clone())) {
            // A retry of a request first saved while the policy was in the
            // other state carries the other bytes; replay those so the retry
            // still returns the original receipt.
            Err(Code::Conflict) => {
                let other = match &model {
                    Some(_) => None,
                    None => self
                        .autostart
                        .as_ref()
                        .and_then(|a| a.policy())
                        .map(|policy| policy.engine.model),
                };
                return self.apply(&command(other));
            }
            other => other?,
        };
        if let (Some(autostart), Some(_)) = (&self.autostart, &model) {
            autostart.eligible_requesting(key, device, &task.workspace, requested);
            autostart.sweep_soon();
        }
        Ok(created)
    }

    fn steer(
        &self,
        key: &str,
        _device: &str,
        task: &str,
        revision: u64,
        prompt: &str,
    ) -> Result<TaskRef, Code> {
        self.apply(&Command {
            schema: COMMAND_SCHEMA.into(),
            command_id: format!("host-steer-{key}"),
            task_id: task.into(),
            expected_revision: Some(revision),
            action: Action::Correct {
                prompt: prompt.into(),
                reason: "Steered from a paired device".into(),
            },
        })
    }

    /// Keep an image chunk for `device` in this task store's uploads.
    fn put_artifact(
        &self,
        device: &str,
        put: &coder_host::access::media::ArtifactPut,
    ) -> Result<coder_host::access::media::ArtifactState, Code> {
        super::media::put(&self.store, device, put).map_err(|error| match error {
            Error::Corrupt(_) => Code::Conflict,
            other => refusal(other),
        })
    }

    fn archive(&self, _key: &str, device: &str, task: &str) -> Result<(), Code> {
        let by = super::archive::By::Device {
            key: device.to_owned(),
        };
        super::archive::archive(
            &self.store,
            task,
            "Archived by an enrolled device",
            by,
            super::autostart::unix_now(),
        )
        .map(|_| ())
        .map_err(refusal)
    }

    /// Record a device's durable command and evaluate its task's commands.
    /// Microcoder's stated steering decides what a steer may do. Every other
    /// sender's held command that the evaluation lets run is rechecked with
    /// `standing`.
    fn command(
        &self,
        principal: &Principal,
        command: &TaskCommand,
        standing: Standing<'_>,
    ) -> Result<TaskRef, Code> {
        self.command_inner(principal, command, None, standing)
    }

    fn command_at_revision(
        &self,
        principal: &Principal,
        command: &TaskCommand,
        revision: u64,
        standing: Standing<'_>,
    ) -> Result<TaskRef, Code> {
        self.command_inner(principal, command, Some(revision), standing)
    }

    fn queue_at_revision(
        &self,
        request: &str,
        principal: &Principal,
        task: &str,
        revision: u64,
        edit: &QueueEdit,
        queue_digest: Option<&str>,
        standing: Standing<'_>,
    ) -> Result<(TaskQueue, String, Option<TaskRef>), Code> {
        let sender = sender(principal);
        let edit = local_queue_edit(edit);
        let admitted =
            |other: &super::commands::Sender| *other == sender || standing(&principal_of(other));
        let (queue, digest, continued, changed) = super::commands::edit_queue_at_revision(
            &self.store,
            request,
            task,
            &sender,
            revision,
            &edit,
            queue_digest,
            &super::adapter::STEERING,
            &admitted,
            super::autostart::unix_now(),
        )
        .map_err(|error| match error {
            Error::NotFound => Code::Forbidden,
            other => refusal(other),
        })?;
        self.continued(&continued);
        let changed = changed
            .then(|| {
                Store::open(&self.store)
                    .and_then(|store| store.show(task))
                    .map(|task| current(&task))
            })
            .transpose()
            .map_err(refusal)?;
        Ok((queue, digest, changed))
    }

    /// List or edit a task's held messages. A device sees the text of its
    /// own messages only.
    fn queue(
        &self,
        principal: &Principal,
        task: &str,
        edit: &QueueEdit,
        standing: Standing<'_>,
    ) -> Result<(TaskQueue, Option<TaskRef>), Code> {
        use super::commands::QueueEdit as Edit;
        let sender = sender(principal);
        let edit = match edit {
            QueueEdit::List {} => Edit::List,
            QueueEdit::Lease {} => Edit::Lease,
            QueueEdit::Release {} => Edit::Release,
            QueueEdit::Edit { command, text } => Edit::Edit {
                command: command.clone(),
                text: text.clone(),
            },
            QueueEdit::Remove { command } => Edit::Remove {
                command: command.clone(),
            },
            QueueEdit::Reorder { commands } => Edit::Reorder {
                commands: commands.clone(),
            },
            QueueEdit::SendNow { command } => Edit::SendNow {
                command: command.clone(),
            },
        };
        let before = Store::open(&self.store)
            .and_then(|store| store.show(task))
            .map_err(|error| match error {
                Error::NotFound => Code::Forbidden,
                other => refusal(other),
            })?
            .revision;
        let admitted =
            |other: &super::commands::Sender| *other == sender || standing(&principal_of(other));
        let (state, continued) = super::commands::edit_queue(
            &self.store,
            task,
            &sender,
            &edit,
            &super::adapter::STEERING,
            &admitted,
            super::autostart::unix_now(),
        )
        .map_err(|error| match error {
            Error::NotFound => Code::Forbidden,
            other => refusal(other),
        })?;
        self.continued(&continued);
        let queue = TaskQueue {
            task: task.to_owned(),
            revision: state.task.revision,
            lease: state.lease.map(|lease| QueueLease {
                device: lease.device,
                expires_at: lease.expires_at,
            }),
            items: state
                .items
                .into_iter()
                .map(|item| QueueItem {
                    text: (item.device == principal.device).then_some(item.text),
                    command: item.command,
                    device: item.device,
                    priority: item.priority,
                })
                .collect(),
        };
        let changed = (state.task.revision != before).then(|| current(&state.task));
        Ok((queue, changed))
    }

    /// Evaluate held commands again, such as queued messages after a turn
    /// ends, with each sender's grant rechecked.
    fn tick(&self, standing: Standing<'_>) {
        if let Some(agents) = &self.agents {
            agents.tick();
        }
        self.apply_standing_rules(standing);
        let check = |sender: &super::commands::Sender| standing(&principal_of(sender));
        let now = super::autostart::unix_now();
        for task in super::commands::open_tasks(&self.store) {
            match super::commands::process(
                &self.store,
                &task,
                &super::adapter::STEERING,
                &check,
                now,
            ) {
                Ok(continued) => self.continued(&continued),
                Err(error) => eprintln!("openagents host: device commands: {error}"),
            }
        }
    }

    /// Archived tasks are left out, so the host never publishes their
    /// summaries again.
    fn current(&self) -> Vec<TaskRef> {
        let archived = super::archive::archived(&self.store);
        Store::open(&self.store)
            .and_then(|store| store.list())
            .map(|tasks| {
                tasks
                    .iter()
                    .filter(|task| !archived.contains(&task.task_id))
                    .map(current)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The length and modification time of the store's task directory
    /// (every task write renames a file into it), identity log, command,
    /// and archive files: every change a summary reports writes one of them.
    fn stamp(&self) -> Option<Vec<u8>> {
        let mut stamp = Vec::new();
        for name in [
            super::TASK_DIR,
            super::IDENTITY_FILE,
            super::commands::FILE,
            super::archive::FILE,
        ] {
            let (length, modified) = match std::fs::metadata(self.store.join(name)) {
                Ok(metadata) => (
                    metadata.len(),
                    metadata
                        .modified()
                        .ok()
                        .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
                        .map_or(0, |at| at.as_nanos()),
                ),
                Err(_) => (u64::MAX, 0),
            };
            stamp.extend_from_slice(&length.to_le_bytes());
            stamp.extend_from_slice(&modified.to_le_bytes());
        }
        if let Some(agents) = &self.agents {
            stamp.extend_from_slice(&agents.stamp().to_le_bytes());
        }
        Some(stamp)
    }

    /// A task that ended for lack of model capacity: the auto-start policy
    /// ended it before a run, or its run stopped when the last admitted
    /// provider refused. The reset comes from the policy's record or the
    /// capacity book. Or a task the policy ended because its owner process
    /// never admitted it, with the cause the owner reported.
    /// A studio task's question or approval: its seat and plan entry
    /// title, from the coordinator's state.
    fn decision_headline(&self, id: &str) -> Option<String> {
        if !super::studio::Studio::present(&self.store) {
            return None;
        }
        let tasks = Store::open(&self.store).ok()?;
        super::interaction::pending(&tasks.show(id).ok()?)?;
        let studio = super::studio::Studio::open(&self.store).ok()?;
        studio.decision_headline(&tasks, id)
    }

    /// The studio's goals whose decision waits on the person.
    fn goal_decisions(&self) -> Vec<coder_host::GoalDecision> {
        if !super::studio::Studio::present(&self.store) {
            return Vec::new();
        }
        super::studio::Studio::open(&self.store)
            .map(|studio| studio.goal_decisions())
            .unwrap_or_default()
    }

    fn note(&self, id: &str) -> Option<Note> {
        let task = Store::open(&self.store).ok()?.show(id).ok()?;
        match super::interaction::pending(&task) {
            Some(super::interaction::Kind::Question) => return Some(Note::Question),
            Some(super::interaction::Kind::Approval) => return Some(Note::Approval),
            None => {}
        }
        if ended_without_capacity(&task) {
            let providers: Vec<Provider> = task
                .run
                .as_ref()
                .and_then(|run| run.admission.grant.adapter_configuration.as_ref())
                .map(|configuration| {
                    configuration
                        .routes()
                        .iter()
                        .filter_map(|route| Provider::from_config(&route.provider))
                        .collect()
                })
                .unwrap_or_default();
            let until = super::capacity::Book::load(&self.store)
                .earliest_reset(&providers, super::autostart::unix_now());
            return Some(Note::NoCapacity { until });
        }
        if owner_ended(&task) {
            return Some(Note::OwnerEnded);
        }
        if task.status == Status::Cancelled && task.run.is_none() {
            let autostart = self.autostart.as_ref()?;
            if let Some(until) = autostart.no_capacity(id) {
                return Some(Note::NoCapacity { until });
            }
            let cause = autostart.not_started(id, task.turn_started())?;
            return Some(Note::NotStarted { cause });
        }
        // A started turn the person asked another engine for says why that
        // engine is not the one running (#10081), until the task ends.
        if matches!(task.status, Status::Queued | Status::Running)
            && let Some(autostart) = self.autostart.as_ref()
            && let Some((asked, runs, why)) = autostart.passed_over(id)
        {
            return Some(Note::Requested {
                asked: super::settings::engine_of(asked)?,
                runs: super::settings::provider_name(runs),
                why,
            });
        }
        None
    }

    /// A local run lives in this store when the store holds its task and
    /// the run's own record beside it (`local/<task>.json`), made for that
    /// thread or for none.
    /// A local run's change at its exact revisions, sized for the wire.
    /// A task this store did not start with a worktree of its own has no
    /// change to review here.
    fn review(&self, task: &str) -> Result<coder_host::access::review::TaskReview, Code> {
        let record = super::local::record(&self.store, task).ok_or(Code::Unsupported)?;
        super::review::read_for_wire(&self.store, task, Path::new(&record.worktree), &record.base)
            .map_err(|_| Code::Unavailable)
    }

    /// Publish a local run's reviewed change through GitHub's CLI, once per
    /// review identity. A studio task's change is merged into its
    /// checkout's branch instead, off-tree and fast-forward only, and
    /// nothing is pushed ([`super::studio::git::merge`]).
    fn publish(
        &self,
        _principal: &Principal,
        task: &str,
        reviewed: &coder_host::Reviewed,
    ) -> Result<coder_host::access::review::Publication, Code> {
        let reviewed = super::publish::Reviewed {
            base: reviewed.base.clone(),
            head_commit: reviewed.head_commit.clone(),
            head: reviewed.head.clone(),
        };
        let studio = super::studio::git::seat_of(&self.store, task).is_some();
        let outcome = if studio {
            super::studio::git::merge(&self.store, task, &reviewed)
        } else {
            super::publish::Publisher::new(&self.store, &super::publish::GhForge)
                .publish(task, &reviewed)
        };
        let publication = outcome.map_err(|refusal| match refusal {
            super::publish::Refusal::NoWorktree => Code::Unsupported,
            super::publish::Refusal::Store(_) => Code::Unavailable,
        })?;
        if studio && publication.state == coder_host::access::review::PublishState::Published {
            self.studio_merged(task);
        }
        Ok(publication)
    }

    fn local_run(&self, task: &str, thread: &str) -> bool {
        let Some(record) = super::local::record(&self.store, task) else {
            return false;
        };
        record.thread.as_deref().is_none_or(|bound| bound == thread)
            && Store::open(&self.store).is_ok_and(|store| store.show(task).is_ok())
    }

    fn cancel(
        &self,
        key: &str,
        _device: &str,
        task: &str,
        revision: u64,
        reason: &str,
    ) -> Result<TaskRef, Code> {
        // A stuck run, whose process is gone, stops by being ended
        // (#10124); a live one is asked to stop.
        let mut store = Store::open(&self.store).map_err(refusal)?;
        if let Some(ended) = store.settle(task).map_err(refusal)? {
            return Ok(current(&ended));
        }
        drop(store);
        self.apply(&Command {
            schema: COMMAND_SCHEMA.into(),
            command_id: format!("host-cancel-{key}"),
            task_id: task.into(),
            expected_revision: Some(revision),
            action: Action::Cancel {
                reason: reason.into(),
            },
        })
    }

    /// The studio of this task store, joined with its tasks. A store
    /// without a studio has an empty one; reading never creates it.
    fn studio(&self) -> Result<coder_host::access::studio::View, Code> {
        use coder_host::access::studio::{MAX_REPOSITORIES, Repository, View};
        let mut view = if super::studio::Studio::present(&self.store) {
            let tasks = Store::open(&self.store).map_err(refusal)?;
            let studio = super::studio::Studio::open(&self.store).map_err(studio_refusal)?;
            studio.wire(&tasks, &self.store)
        } else {
            View::default()
        };
        // An admitted workspace can receive its first goal before it has
        // any goal-derived summary. Publish its label, never its root.
        for workspace in self.workspaces.keys() {
            if view.repositories.len() >= MAX_REPOSITORIES {
                break;
            }
            if !view
                .repositories
                .iter()
                .any(|repo| &repo.workspace == workspace)
            {
                view.repositories.push(Repository {
                    workspace: workspace.clone(),
                    goals: 0,
                    open_tasks: 0,
                });
            }
        }
        view.canonicalize();
        Ok(view)
    }

    /// A studio intent on this store's coordinator. A task's question or
    /// approval is answered through the durable command journal under
    /// the device's command ID, as `task.command` answers one; every
    /// other intent answers once per request ID. An answer to an
    /// approval first binds the answering device as the approver of that
    /// exact step ([`super::studio::approvals`]), and the binding is
    /// consumed when the journal accepts the answer: `operate` sends the
    /// answer, it does not approve.
    fn studio_intent(
        &self,
        key: &str,
        principal: &Principal,
        op: &Operation,
        standing: Standing<'_>,
    ) -> Result<String, Code> {
        use super::studio::{NewGoal, Party, PlanOutcome, Repository, Studio};
        // Pausing or resuming a workshop agent is the same intent as a
        // seat's (`studio.seat.pause`); her record keeps it.
        if let (Some(agents), Operation::PauseSeat { seat } | Operation::ResumeSeat { seat }) =
            (&self.agents, op)
            && agents.holds(seat)
        {
            agents.answer(key, principal, op)?;
            return Ok(seat.clone());
        }
        if matches!(op, Operation::AllowAlways { .. }) {
            return self.allow_always(key, principal, op, standing);
        }
        if let Operation::AnswerDecision {
            decision,
            based_on,
            text,
            command,
            issued_at,
        } = op
        {
            let studio = Studio::open(&self.store).map_err(studio_refusal)?;
            let for_task = studio.state().goal(decision).is_none();
            if for_task && !studio.holds_task(decision) {
                return Err(studio_refused(
                    Code::Forbidden,
                    "No studio goal or task has this decision.",
                ));
            }
            drop(studio);
            if for_task {
                let bound = self.bind_approver(principal, decision, *based_on, text, command)?;
                let answer = TaskCommand {
                    command: command.clone(),
                    task: decision.clone(),
                    action: CommandAction::Answer,
                    based_on: *based_on,
                    text: text.clone(),
                    emulate: false,
                    issued_at: *issued_at,
                };
                let answered = self.command(principal, &answer, standing)?;
                if let Some(subject) = bound {
                    Studio::open(&self.store)
                        .and_then(|mut studio| {
                            studio.consume_approval(&subject, command, super::autostart::unix_now())
                        })
                        .map_err(studio_refusal)?;
                }
                return Ok(answered.task);
            }
        }
        let now = super::autostart::unix_now();
        let mut tasks = Store::open(&self.store).map_err(refusal)?;
        let mut studio = Studio::open(&self.store).map_err(studio_refusal)?;
        if let Some(autostart) = &self.autostart {
            studio = studio
                .with_host_root(autostart.root())
                .with_worktrees(super::studio::git::worktrees_dir(autostart.root()));
        }
        if let Some(reference) = studio.answered(key) {
            return Ok(reference);
        }
        let (reference, released) = match op {
            Operation::SubmitGoal {
                text,
                workspace,
                lead,
            } => {
                let root = self.workspaces.get(workspace).ok_or_else(|| {
                    studio_refused(
                        Code::Forbidden,
                        &format!("This host admits no workspace labeled `{workspace}`."),
                    )
                })?;
                let goal = NewGoal {
                    text: text.clone(),
                    repository: Repository {
                        label: workspace.clone(),
                        path: root.to_string_lossy().into_owned(),
                    },
                    lead: lead.clone(),
                };
                let (goal_id, _) = studio
                    .submit_goal(&mut tasks, goal, now)
                    .map_err(studio_refusal)?;
                (goal_id, true)
            }
            Operation::MessageSeat { seat, text } => {
                let to = seat
                    .clone()
                    .map_or(Party::Everyone, |name| Party::Seat { name });
                studio
                    .message(&tasks, Party::Person, to, text, now)
                    .map_err(studio_refusal)?;
                (seat.clone().unwrap_or_else(|| "everyone".into()), false)
            }
            Operation::PauseSeat { seat } => {
                studio.pause_seat(seat).map_err(studio_refusal)?;
                (seat.clone(), false)
            }
            Operation::ResumeSeat { seat } => {
                let released = studio
                    .resume_seat(&mut tasks, seat, now)
                    .map_err(studio_refusal)?;
                (seat.clone(), !released.is_empty())
            }
            Operation::StopSeat { seat } => {
                studio.stop_seat(&mut tasks, seat).map_err(studio_refusal)?;
                (seat.clone(), false)
            }
            Operation::ReassignTask { task, seat } => {
                studio.reassign(task, seat).map_err(studio_refusal)?;
                (task.clone(), false)
            }
            Operation::CancelStudioTask { task } => {
                studio
                    .cancel_task(&mut tasks, task, now)
                    .map_err(studio_refusal)?;
                (task.clone(), false)
            }
            Operation::RetryTask { task } => {
                let fresh = studio
                    .retry_task(&mut tasks, task, now)
                    .map_err(studio_refusal)?;
                (fresh, true)
            }
            Operation::PrioritizeTask { task } => {
                studio.prioritize(task).map_err(studio_refusal)?;
                (task.clone(), false)
            }
            Operation::AnswerDecision {
                decision,
                based_on,
                text,
                ..
            } => {
                let outcome = studio
                    .answer_goal(&mut tasks, decision, *based_on, text, now)
                    .map_err(studio_refusal)?;
                let released =
                    matches!(outcome, PlanOutcome::Accepted { released } if !released.is_empty());
                (decision.clone(), released)
            }
            _ => {
                return Err(studio_refused(
                    Code::Unsupported,
                    "The studio does not take this operation.",
                ));
            }
        };
        studio
            .record_answer(key, &reference)
            .map_err(studio_refusal)?;
        drop(studio);
        drop(tasks);
        if released && let Some(autostart) = &self.autostart {
            autostart.sweep_soon();
        }
        Ok(reference)
    }

    /// Record a rejection in the studio's shared memory, cancelling the
    /// task if it still runs. Its worktree stays until it is archived.
    fn studio_reject(
        &self,
        _principal: &Principal,
        task: &str,
        reviewed: &coder_host::Reviewed,
        reason: &str,
    ) -> Result<(), Code> {
        if !super::studio::Studio::present(&self.store) {
            return Err(studio_refused(Code::Forbidden, "This host has no studio."));
        }
        let mut tasks = Store::open(&self.store).map_err(refusal)?;
        let mut studio = super::studio::Studio::open(&self.store).map_err(studio_refusal)?;
        studio
            .reject(&mut tasks, task, &reviewed.head, reason)
            .map(|_| ())
            .map_err(studio_refusal)
    }
}

/// The refusal a device receives for a studio coordinator failure, with
/// the coordinator's sentence noted for the host to carry
/// ([`coder_host::tasks::refuse`]): a goal with no lead seat says so,
/// rather than arriving as a bare `conflict`.
fn studio_refusal(error: super::studio::Error) -> Code {
    use super::studio::Error as Studio;
    let reason = sentence(&error.to_string());
    let code = match error {
        Studio::Tasks(error) => refusal(error),
        Studio::Invalid(_) => Code::Malformed,
        Studio::UnknownSeat(_) | Studio::UnknownGoal(_) => Code::Forbidden,
        Studio::State(_) => Code::Conflict,
        Studio::LimitExceeded(_) => Code::Bounds,
        Studio::Corrupt(_) => Code::Unavailable,
    };
    coder_host::tasks::refuse(code, reason)
}

/// A studio refusal the host decides itself, with its sentence.
fn studio_refused(code: Code, reason: &str) -> Code {
    coder_host::tasks::refuse(code, reason)
}

/// `text` as a sentence: its first letter in upper case and a closing
/// period.
fn sentence(text: &str) -> String {
    let text = text.trim();
    let mut chars = text.chars();
    let mut out: String = match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => return "The studio refused the request.".to_owned(),
    };
    if !out.ends_with(['.', '!', '?']) {
        out.push('.');
    }
    out
}

/// Whether the task's run ended because no admitted provider had capacity.
fn ended_without_capacity(task: &super::Task) -> bool {
    task.run
        .as_ref()
        .and_then(|run| run.result.as_ref())
        .is_some_and(|result| result.ending == super::capacity::NO_CAPACITY_ENDING)
}

/// The journal's sender for a host principal.
fn owner_ended(task: &super::Task) -> bool {
    task.run
        .as_ref()
        .and_then(|run| run.result.as_ref())
        .is_some_and(|result| result.ending == super::owner::OWNER_ENDED)
}

fn sender(principal: &Principal) -> super::commands::Sender {
    super::commands::Sender {
        device: principal.device.clone(),
        grant: principal.grant.clone(),
        epoch: principal.epoch,
    }
}

/// The host principal of a journal sender.
fn principal_of(sender: &super::commands::Sender) -> Principal {
    Principal {
        device: sender.device.clone(),
        grant: sender.grant.clone(),
        epoch: sender.epoch,
    }
}

/// A stored task's revision and phase. A finished run that failed or was
/// stopped reports that, not completion. A run that stopped for lack of
/// model capacity reports a stop, so its summary can say why. A turn that
/// ended with a question or an approval request waits for its answer.
fn current(task: &super::Task) -> TaskRef {
    use super::Execution;
    TaskRef {
        task: task.task_id.clone(),
        revision: task.revision,
        phase: match (task.status, task.execution) {
            (Status::Finished, _) if ended_without_capacity(task) => Phase::Cancelled,
            // A failed summary carries only "Task failed"; a run whose
            // process ended reads as stopped, with its plain reason (#10124).
            (Status::Finished, _) if owner_ended(task) => Phase::Cancelled,
            (Status::Finished, _) if super::interaction::pending(task).is_some() => Phase::Waiting,
            (Status::Queued, _) => Phase::Queued,
            (Status::Running | Status::CancelRequested, _) => Phase::Running,
            (Status::Cancelled, _) | (Status::Finished, Execution::Stopped) => Phase::Cancelled,
            (Status::Finished, Execution::Failed) => Phase::Failed,
            (Status::Finished, _) => Phase::Completed,
            (Status::Unknown, _) => Phase::Unknown,
        },
    }
}

fn reference(receipt: &Receipt) -> TaskRef {
    TaskRef {
        task: receipt.task_id.clone(),
        revision: receipt.revision,
        phase: match receipt.status {
            Status::Queued => Phase::Queued,
            Status::Running | Status::CancelRequested => Phase::Running,
            Status::Cancelled => Phase::Cancelled,
            Status::Finished => Phase::Completed,
            Status::Unknown => Phase::Unknown,
        },
    }
}

/// The refusal a device receives for a local inbox failure. Messages stay
/// local; the code carries no path or content.
fn refusal(error: Error) -> Code {
    match error {
        Error::InvalidCommand(_) | Error::SourceSnapshot(_) | Error::UnsupportedSchema => {
            Code::Malformed
        }
        Error::Conflict | Error::InvalidTransition | Error::WorkspaceBusy => Code::Conflict,
        Error::RevisionMismatch => Code::Stale,
        Error::NotFound => Code::Forbidden,
        Error::LimitExceeded => Code::Bounds,
        Error::Io(_)
        | Error::Corrupt(_)
        | Error::UnsafePath
        | Error::Busy
        | Error::BuildDiskLow { .. }
        | Error::UnsupportedPlatform
        | Error::ReopenRequired => Code::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inbox(dir: &Path) -> Inbox {
        let root = dir.join("checkout");
        std::fs::create_dir_all(&root).unwrap();
        Inbox::new(
            dir.join("tasks"),
            BTreeMap::from([("checkout".to_owned(), root)]),
        )
    }

    fn create() -> TaskCreate {
        TaskCreate {
            title: "Fix the flaky test".into(),
            prompt: "Find why the test fails one run in ten.".into(),
            workspace: "checkout".into(),
            images: Vec::new(),
            engine: None,
        }
    }

    #[test]
    fn a_fresh_studio_advertises_admitted_labels_without_creating_state() {
        let temp = tempfile::tempdir().unwrap();
        let inbox = inbox(temp.path());
        let view = inbox.studio().unwrap();
        assert_eq!(view.repositories.len(), 1);
        assert_eq!(view.repositories[0].workspace, "checkout");
        assert_eq!(view.repositories[0].goals, 0);
        assert!(view.goals.is_empty() && view.tasks.is_empty());
        assert!(!temp.path().join("tasks").exists());
        assert!(
            !serde_json::to_string(&view)
                .unwrap()
                .contains(&temp.path().to_string_lossy().to_string())
        );
        view.validate().unwrap();
    }

    #[test]
    fn creation_is_inert_idempotent_and_bound_to_labels() {
        let temp = tempfile::tempdir().unwrap();
        let inbox = inbox(temp.path());
        let key = "a".repeat(64);
        let first = inbox.create(&key, "device", &create()).unwrap();
        assert_eq!((first.task.as_str(), first.revision), (key.as_str(), 1));
        assert_eq!(first.phase, Phase::Queued);
        // An exact retry returns the same task, not a second one.
        assert_eq!(inbox.create(&key, "device", &create()).unwrap(), first);
        let store = Store::open(inbox.store()).unwrap();
        let tasks = store.list().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, Status::Queued);
        assert_eq!(tasks[0].execution, super::super::Execution::NotStarted);
        assert!(tasks[0].run.is_none());
        drop(store);
        let unknown = TaskCreate {
            workspace: "elsewhere".into(),
            ..create()
        };
        assert_eq!(
            inbox.create(&"b".repeat(64), "device", &unknown),
            Err(Code::Forbidden)
        );
    }

    /// The host sweeps as soon as the store's stamp moves: a new task moves
    /// it, and reading changes nothing.
    #[test]
    fn the_store_stamp_moves_when_a_task_changes() {
        let temp = tempfile::tempdir().unwrap();
        let inbox = inbox(temp.path());
        let empty = inbox.stamp().expect("a stamp");
        assert_eq!(inbox.stamp(), Some(empty.clone()));
        inbox.create(&"a".repeat(64), "device", &create()).unwrap();
        let created = inbox.stamp().expect("a stamp");
        assert_ne!(created, empty);
        let _ = inbox.current();
        assert_eq!(inbox.stamp(), Some(created));
    }

    #[test]
    fn an_archived_task_leaves_the_current_list_and_only_an_ended_one_archives() {
        let temp = tempfile::tempdir().unwrap();
        let inbox = inbox(temp.path());
        let device = "ab".repeat(32);
        let task = "c".repeat(64);
        inbox.create(&task, &device, &create()).unwrap();
        // A queued task is still work in progress.
        assert_eq!(
            inbox.archive(&"d".repeat(64), &device, &task),
            Err(Code::Conflict)
        );
        assert_eq!(
            inbox.archive(&"d".repeat(64), &device, &"9".repeat(64)),
            Err(Code::Forbidden)
        );
        inbox
            .cancel(&"e".repeat(64), &device, &task, 1, "Not needed")
            .unwrap();
        assert_eq!(inbox.current().len(), 1);
        inbox.archive(&"f".repeat(64), &device, &task).unwrap();
        // A retry succeeds again.
        inbox.archive(&"f".repeat(64), &device, &task).unwrap();
        assert!(inbox.current().is_empty());
        // Nothing is deleted.
        let shown = Store::open(inbox.store()).unwrap().show(&task).unwrap();
        assert_eq!(shown.status, Status::Cancelled);
        let entries = super::super::archive::entries(inbox.store()).unwrap();
        assert_eq!(
            entries[&task].by,
            super::super::archive::By::Device { key: device }
        );
    }

    #[test]
    fn steer_and_cancel_follow_the_owner_revisions() {
        let temp = tempfile::tempdir().unwrap();
        let inbox = inbox(temp.path());
        let task = "c".repeat(64);
        inbox.create(&task, "device", &create()).unwrap();
        let steered = inbox
            .steer(
                &"d".repeat(64),
                "device",
                &task,
                1,
                "Only look at the parser.",
            )
            .unwrap();
        assert_eq!((steered.revision, steered.phase), (2, Phase::Queued));
        // A steer at an old revision is stale.
        assert_eq!(
            inbox.steer(&"e".repeat(64), "device", &task, 1, "Again"),
            Err(Code::Stale)
        );
        let cancelled = inbox
            .cancel(&"f".repeat(64), "device", &task, 2, "Not needed")
            .unwrap();
        assert_eq!((cancelled.revision, cancelled.phase), (3, Phase::Cancelled));
        assert_eq!(
            inbox.cancel(&"0".repeat(64), "device", &"9".repeat(64), 1, "Missing"),
            Err(Code::Forbidden)
        );
        let shown = Store::open(inbox.store()).unwrap().show(&task).unwrap();
        assert_eq!(shown.effective_prompt(), "Only look at the parser.");
        assert_eq!(shown.status, Status::Cancelled);
    }

    fn command(
        task: &str,
        id: char,
        action: CommandAction,
        based_on: u64,
        text: &str,
    ) -> TaskCommand {
        TaskCommand {
            command: id.to_string().repeat(64),
            task: task.into(),
            action,
            based_on,
            text: text.into(),
            emulate: false,
            issued_at: super::super::autostart::unix_now(),
        }
    }

    #[test]
    fn a_queue_listing_shows_a_device_only_its_own_text() {
        let temp = tempfile::tempdir().unwrap();
        let inbox = inbox(temp.path());
        let task = "c".repeat(64);
        let (phone, tablet) = ("ab".repeat(32), "cd".repeat(32));
        let principal = |device: &str| Principal {
            device: device.into(),
            grant: Some("1".repeat(64)),
            epoch: Some(0),
        };
        let standing = |_: &Principal| true;
        inbox.create(&task, &phone, &create()).unwrap();
        inbox
            .cancel(&"e".repeat(64), &phone, &task, 1, "Ended")
            .unwrap();
        inbox
            .command(
                &principal(&phone),
                &command(&task, 'a', CommandAction::Send, 2, "First."),
                &standing,
            )
            .unwrap();
        for (device, id, text) in [(&phone, 'b', "Mine."), (&tablet, 'd', "Theirs.")] {
            let held = inbox
                .command(
                    &principal(device),
                    &command(&task, id, CommandAction::Queue, 3, text),
                    &standing,
                )
                .unwrap();
            assert_eq!(held.phase, Phase::Queued);
        }
        let (queue, changed) = inbox
            .queue(&principal(&phone), &task, &QueueEdit::List {}, &standing)
            .unwrap();
        assert!(changed.is_none());
        assert_eq!(
            queue
                .items
                .iter()
                .map(|item| item.text.as_deref())
                .collect::<Vec<_>>(),
            [Some("Mine."), None]
        );
        // Another device cannot edit without the lease the phone holds.
        inbox
            .queue(&principal(&phone), &task, &QueueEdit::Lease {}, &standing)
            .unwrap();
        assert_eq!(
            inbox
                .queue(&principal(&tablet), &task, &QueueEdit::Lease {}, &standing)
                .map(|_| ()),
            Err(Code::Conflict)
        );
        // A revoked sender's held message is refused, never run, when an
        // edit evaluates the queue again.
        let revoked = |other: &Principal| other.device != tablet;
        let (queue, _) = inbox
            .queue(&principal(&phone), &task, &QueueEdit::Release {}, &revoked)
            .unwrap();
        assert_eq!(
            queue
                .items
                .iter()
                .map(|item| item.device.as_str())
                .collect::<Vec<_>>(),
            [phone.as_str()]
        );
    }

    /// A studio refusal arrives with the coordinator's sentence, not only
    /// its code: a goal with no lead seat names the missing lead seat.
    #[test]
    fn a_studio_refusal_carries_the_coordinators_sentence() {
        let temp = tempfile::tempdir().unwrap();
        let inbox = inbox(temp.path());
        let owner = Principal {
            device: "d".repeat(64),
            grant: None,
            epoch: None,
        };
        let always = |_: &Principal| true;
        let goal = Operation::SubmitGoal {
            text: "Write CONTRIBUTING.md.".into(),
            workspace: "checkout".into(),
            lead: None,
        };
        assert_eq!(
            inbox.studio_intent(&"a".repeat(64), &owner, &goal, &always),
            Err(Code::Conflict)
        );
        let reason = coder_host::tasks::take_reason(Code::Conflict).expect("a sentence");
        assert!(reason.contains("no lead seat"), "{reason}");
        assert!(reason.starts_with('T') && reason.ends_with('.'), "{reason}");
        // The sentence is taken once.
        assert_eq!(coder_host::tasks::take_reason(Code::Conflict), None);

        // A message to a seat the studio does not have names it.
        let message = Operation::MessageSeat {
            seat: Some("nobody".into()),
            text: "Hello.".into(),
        };
        assert_eq!(
            inbox.studio_intent(&"b".repeat(64), &owner, &message, &always),
            Err(Code::Forbidden)
        );
        let reason = coder_host::tasks::take_reason(Code::Forbidden).expect("a sentence");
        assert!(reason.contains("`nobody`"), "{reason}");

        // A workspace the host does not admit says so.
        let elsewhere = Operation::SubmitGoal {
            text: "Write CONTRIBUTING.md.".into(),
            workspace: "elsewhere".into(),
            lead: None,
        };
        assert_eq!(
            inbox.studio_intent(&"c".repeat(64), &owner, &elsewhere, &always),
            Err(Code::Forbidden)
        );
        let reason = coder_host::tasks::take_reason(Code::Forbidden).expect("a sentence");
        assert!(reason.contains("`elsewhere`"), "{reason}");
    }

    #[test]
    fn a_refusal_reads_as_a_sentence() {
        assert_eq!(
            sentence("the studio has no lead seat"),
            "The studio has no lead seat."
        );
        assert_eq!(sentence("Already one."), "Already one.");
        assert_eq!(sentence("  "), "The studio refused the request.");
    }
}
