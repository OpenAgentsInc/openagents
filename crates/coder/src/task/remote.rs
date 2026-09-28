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

use coder_host::{Code, Note, TaskCreate, TaskRef, Tasks};
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
}

impl Inbox {
    /// An inbox over the task store at `store`. `workspaces` maps the labels
    /// a device may name to their roots; a device never sends a path.
    #[must_use]
    pub fn new(store: impl Into<PathBuf>, workspaces: BTreeMap<String, PathBuf>) -> Self {
        Self {
            store: store.into(),
            workspaces,
            autostart: None,
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

    fn apply(&self, command: &Command) -> Result<TaskRef, Code> {
        let bytes = serde_json::to_vec(command).map_err(|_| Code::Malformed)?;
        let mut store = Store::open(&self.store).map_err(refusal)?;
        let receipt = store.apply(&bytes).map_err(refusal)?;
        Ok(reference(&receipt))
    }
}

impl Tasks for Inbox {
    fn create(&self, key: &str, device: &str, task: &TaskCreate) -> Result<TaskRef, Code> {
        let root = self
            .workspaces
            .get(&task.workspace)
            .ok_or(Code::Forbidden)?;
        // Under the owner's policy the task records the engine's model, which
        // its grant must name; otherwise no model, as always.
        let model = self
            .autostart
            .as_ref()
            .and_then(|autostart| autostart.model_for(&task.workspace));
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
            autostart.eligible(key, device, &task.workspace);
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

    fn current(&self) -> Vec<TaskRef> {
        Store::open(&self.store)
            .and_then(|store| store.list())
            .map(|tasks| tasks.iter().map(current).collect())
            .unwrap_or_default()
    }

    /// A task that ended for lack of model capacity: the auto-start policy
    /// ended it before a run, or its run stopped when the last admitted
    /// provider refused. The reset comes from the policy's record or the
    /// capacity book.
    fn note(&self, id: &str) -> Option<Note> {
        let task = Store::open(&self.store).ok()?.show(id).ok()?;
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
            let until = self.autostart.as_ref()?.no_capacity(id)?;
            return Some(Note::NoCapacity { until });
        }
        None
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

/// A stored task's revision and phase. A finished run that failed or was
/// stopped reports that, not completion. A run that stopped for lack of
/// model capacity reports a stop, so its summary can say why.
fn current(task: &super::Task) -> TaskRef {
    use super::Execution;
    TaskRef {
        task: task.task_id.clone(),
        revision: task.revision,
        phase: match (task.status, task.execution) {
            (Status::Finished, _) if ended_without_capacity(task) => Phase::Cancelled,
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
        Error::InvalidCommand(_) | Error::UnsupportedSchema => Code::Malformed,
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
}
