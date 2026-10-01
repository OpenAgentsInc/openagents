//! Task operations a resident host admitted for an enrolled device.
//!
//! `coder host serve` checks the device's grant and its `operate` right,
//! then calls this inbox. A creation is an ordinary inert submission: it
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

use coder_host::access::protocol::{QueueEdit, QueueItem, QueueLease, TaskQueue};
use coder_host::{
    Code, CommandAction, Note, Principal, Standing, TaskCommand, TaskCreate, TaskRef, Tasks,
};
use nostr::activity_summary::Phase;

use super::capacity::Provider;
use super::{
    Action, COMMAND_SCHEMA, Command, Error, Receipt, RequestedConfiguration, Status, Store,
    TaskIntent, Workspace,
};

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
}

/// The most engine preferences an inbox holds for creates not yet made.
const MAX_PREFERENCES: usize = 64;

impl Inbox {
    /// An inbox over the task store at `store`. `workspaces` maps the labels
    /// a device may name to their roots; a device never sends a path.
    #[must_use]
    pub fn new(store: impl Into<PathBuf>, workspaces: BTreeMap<String, PathBuf>) -> Self {
        Self {
            store: store.into(),
            workspaces,
            autostart: None,
            preferences: Arc::new(std::sync::Mutex::new(BTreeMap::new())),
            settings: None,
        }
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
            if let Some(label) = super::commands::label_for(&self.workspaces, &task) {
                autostart.eligible_turn(&item.task, &item.device, label, item.turn);
                any = true;
            }
        }
        drop(store);
        if any {
            autostart.sweep_soon();
        }
    }

    fn apply(&self, command: &Command) -> Result<TaskRef, Code> {
        let bytes = serde_json::to_vec(command).map_err(|_| Code::Malformed)?;
        let mut store = Store::open(&self.store).map_err(refusal)?;
        let receipt = store.apply(&bytes).map_err(refusal)?;
        Ok(reference(&receipt))
    }
}

impl Tasks for Inbox {
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
                reason: "Steered by an enrolled device".into(),
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
        let (recorded, continued) = super::commands::record(
            &self.store,
            &sender,
            &request,
            &super::adapter::STEERING,
            &admitted,
            super::autostart::unix_now(),
        )
        .map_err(|error| match error {
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
                Err(error) => eprintln!("coder host: device commands: {error}"),
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

    /// The length and modification time of the store's task, command, and
    /// archive files: every change a summary reports writes one of them.
    fn stamp(&self) -> Option<Vec<u8>> {
        let mut stamp = Vec::new();
        for name in [
            super::STORE_FILE,
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
        Some(stamp)
    }

    /// A task that ended for lack of model capacity: the auto-start policy
    /// ended it before a run, or its run stopped when the last admitted
    /// provider refused. The reset comes from the policy's record or the
    /// capacity book. Or a task the policy ended because its owner process
    /// never admitted it, with the cause the owner reported.
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
    /// review identity.
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
        super::publish::Publisher::new(&self.store, &super::publish::GhForge)
            .publish(task, &reviewed)
            .map_err(|refusal| match refusal {
                super::publish::Refusal::NoWorktree => Code::Unsupported,
                super::publish::Refusal::Store(_) => Code::Unavailable,
            })
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
}

/// Whether the task's run ended because no admitted provider had capacity.
fn ended_without_capacity(task: &super::Task) -> bool {
    task.run
        .as_ref()
        .and_then(|run| run.result.as_ref())
        .is_some_and(|result| result.ending == super::capacity::NO_CAPACITY_ENDING)
}

/// The journal's sender for a host principal.
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
        Error::Conflict | Error::InvalidTransition => Code::Conflict,
        Error::RevisionMismatch => Code::Stale,
        Error::NotFound => Code::Forbidden,
        Error::LimitExceeded => Code::Bounds,
        Error::Io(_)
        | Error::Corrupt(_)
        | Error::UnsafePath
        | Error::Busy
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
}
