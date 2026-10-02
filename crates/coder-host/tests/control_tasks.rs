//! Task operations over the same-user control socket (#10206).
//!
//! A host keeps its owner key, signs the request as a phone would, and
//! dispatches it. A host whose owner key lives elsewhere, such as one the
//! person's other computer paired, still takes the request: the socket is
//! this account's own. Every way the owner key cannot be used is its own
//! plain refusal, never one opaque message.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use coder_host::access::Code;
use coder_host::access::protocol::{
    Operation, Outcome, QueueEdit, TaskCommand, TaskCreate, TaskQueue,
};
use coder_host::config::{Config, Control, Iroh};
use coder_host::serve::keys::{FileKeySource, KeyName, KeySource, Keys};
use coder_host::{Principal, Standing, TaskRef, Tasks};
use nostr::activity_summary::Phase;
use openagents_connect::control::{Op, Reply};

#[path = "support/connect.rs"]
mod support;

/// A task owner that records each creation once per idempotency key.
#[derive(Default)]
struct Recorder(Mutex<BTreeMap<String, (TaskRef, String, TaskCreate)>>);

impl Recorder {
    fn created(&self) -> Vec<(TaskRef, String, TaskCreate)> {
        self.0.lock().unwrap().values().cloned().collect()
    }
}

impl Tasks for Recorder {
    fn create(&self, key: &str, device: &str, task: &TaskCreate) -> Result<TaskRef, Code> {
        let mut tasks = self.0.lock().unwrap();
        let (found, _, _) = tasks.entry(key.to_owned()).or_insert_with(|| {
            (
                TaskRef {
                    task: coder_host::reach::new_id(),
                    revision: 1,
                    phase: Phase::Queued,
                },
                device.to_owned(),
                task.clone(),
            )
        });
        Ok(found.clone())
    }
    fn steer(&self, _: &str, _: &str, _: &str, _: u64, _: &str) -> Result<TaskRef, Code> {
        Err(Code::Unavailable)
    }
    fn cancel(&self, _: &str, _: &str, _: &str, _: u64, _: &str) -> Result<TaskRef, Code> {
        Err(Code::Unavailable)
    }
    fn command(&self, _: &Principal, _: &TaskCommand, _: Standing<'_>) -> Result<TaskRef, Code> {
        Err(Code::Unavailable)
    }
    fn queue(
        &self,
        _: &Principal,
        task: &str,
        _: &QueueEdit,
        _: Standing<'_>,
    ) -> Result<(TaskQueue, Option<TaskRef>), Code> {
        Ok((
            TaskQueue {
                task: task.into(),
                revision: 1,
                lease: None,
                items: Vec::new(),
            },
            None,
        ))
    }
}

struct Fixture {
    temp: tempfile::TempDir,
    socket: std::path::PathBuf,
    source: Arc<FileKeySource>,
    tasks: Arc<Recorder>,
    running: coder_host::Running,
    config: Config,
}

fn config(temp: &tempfile::TempDir, relay: String, keys: Option<Keys>) -> Config {
    let mut config = Config::new(temp.path().join("access"), vec![relay], 1);
    config.policy = support::POLICY;
    config.keys = keys;
    config.iroh = Some(Iroh::loopback());
    config.control = Some(Control {
        path: support::control_path(temp),
        root: temp.path().join("host"),
        autostart: None,
        tasks: temp.path().join("tasks"),
        uid: coder_host::control::own_uid(),
    });
    let checkout = temp.path().join("checkout");
    std::fs::create_dir_all(&checkout).unwrap();
    config.workspaces = BTreeMap::from([(
        "checkout".to_owned(),
        std::fs::canonicalize(&checkout).unwrap(),
    )]);
    config
}

/// A host whose keys live in a key directory, as `coder host serve --keys`
/// keeps them; it makes its own owner key on first start.
async fn keyed() -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let (relay, _task, _) = support::relay::start().await;
    let source = Arc::new(FileKeySource::new(temp.path().join("keys")));
    let config = config(&temp, relay, Some(Keys(source.clone())));
    let tasks = Arc::new(Recorder::default());
    let running = coder_host::start(config.clone(), tasks.clone())
        .await
        .unwrap();
    Fixture {
        socket: support::control_path(&temp),
        temp,
        source,
        tasks,
        running,
        config,
    }
}

impl Fixture {
    /// Start the same host again, as after its key directory changed.
    async fn restart(self) -> Self {
        self.running.shutdown().await;
        let running = coder_host::start(self.config.clone(), self.tasks.clone())
            .await
            .unwrap();
        Self { running, ..self }
    }

    fn write_owner(&self, text: &str) {
        std::fs::write(self.source.path(KeyName::Owner), text).unwrap();
    }
}

fn create(title: &str) -> Operation {
    Operation::CreateTask {
        task: TaskCreate {
            title: title.into(),
            prompt: "Read the README and report.".into(),
            workspace: "checkout".into(),
            images: Vec::new(),
            engine: None,
        },
    }
}

fn request(byte: char) -> String {
    std::iter::repeat_n(byte, 64).collect()
}

async fn task(socket: &Path, request: String, operation: Operation) -> Reply {
    support::call(socket, Op::Task { request, operation })
        .await
        .unwrap()
}

fn dispatched(reply: Reply) -> String {
    match reply {
        Reply::Task {
            outcome: Outcome::Dispatched { receipt },
        } => {
            assert_eq!(receipt.operation, "task.create");
            receipt.reference
        }
        other => panic!("expected a dispatched task, got {other:?}"),
    }
}

fn refused(reply: Reply) -> (String, String) {
    match reply {
        Reply::Refused { code, message } => (code, message),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_host_that_keeps_its_owner_key_signs_the_task_as_the_owner() {
    let host = keyed().await;
    let reference = dispatched(task(&host.socket, request('a'), create("Signed")).await);
    let created = host.tasks.created();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].0.task, reference);
    assert_eq!(created[0].1, host.running.owner());
    // The signed path keeps the request until its reply.
    assert!(host.temp.path().join("host/task-calls").exists());
    host.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_host_whose_owner_key_lives_elsewhere_takes_the_task_from_its_own_socket() {
    // CoderOS: the person's other computer paired this host, so the key
    // directory holds the host keys and no owner key.
    let host = keyed().await;
    let owner = host.running.owner().to_owned();
    host.source.delete(KeyName::Owner).unwrap();
    let host = host.restart().await;
    assert_eq!(host.running.owner(), owner);

    let reference = dispatched(task(&host.socket, request('b'), create("Local")).await);
    // A retry of the same request reaches the task owner once.
    let again = dispatched(task(&host.socket, request('b'), create("Local")).await);
    assert_eq!(reference, again);
    let created = host.tasks.created();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].1, owner, "the task is the owner's");
    // The same identity for another task is refused.
    let (code, message) = refused(task(&host.socket, request('b'), create("Other")).await);
    assert_eq!(code, "conflict");
    assert_eq!(message, "task request identity reused");
    // The host never minted an owner key to answer.
    assert!(host.source.load(KeyName::Owner).unwrap().is_none());
    host.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_host_without_a_key_source_takes_the_task_from_its_own_socket() {
    let tasks = Arc::new(Recorder::default());
    let host = support::host_with(support::Options {
        tasks: Some(tasks.clone()),
        ..support::Options::default()
    })
    .await;
    dispatched(task(&host.socket, request('c'), create("No keys")).await);
    let Reply::Task {
        outcome: Outcome::Workspaces { workspaces },
    } = task(&host.socket, request('d'), Operation::ListWorkspaces {}).await
    else {
        panic!("workspaces")
    };
    assert_eq!(workspaces, ["checkout"]);
    assert_eq!(tasks.created().len(), 1);
    host.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stored_key_that_is_not_the_owner_does_not_sign_for_it() {
    let host = keyed().await;
    let owner = host.running.owner().to_owned();
    host.write_owner(&"11".repeat(32));
    dispatched(task(&host.socket, request('e'), create("Other key")).await);
    assert_eq!(host.tasks.created()[0].1, owner);
    host.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unreadable_owner_key_is_its_own_refusal() {
    let host = keyed().await;
    host.write_owner("not a key");
    let (code, message) = refused(task(&host.socket, request('f'), create("Bad file")).await);
    assert_eq!(code, "unavailable");
    assert_eq!(
        message,
        "the owner key could not be read from the host's key store"
    );
    assert!(host.tasks.created().is_empty());
    host.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_owner_key_that_is_not_a_key_is_its_own_refusal() {
    let host = keyed().await;
    // Hex, but zero is no secp256k1 secret.
    host.write_owner(&"00".repeat(32));
    let (code, message) = refused(task(&host.socket, request('0'), create("Zero")).await);
    assert_eq!(code, "malformed");
    assert_eq!(
        message,
        "the owner key in the host's key store is not a valid key"
    );
    assert!(host.tasks.created().is_empty());
    host.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_task_request_record_that_cannot_be_opened_is_its_own_refusal() {
    let host = keyed().await;
    // A file where the record directory goes.
    let root = host.temp.path().join("host");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("task-calls"), b"").unwrap();
    let (code, message) = refused(task(&host.socket, request('1'), create("No record")).await);
    assert_eq!(code, "unavailable");
    assert_eq!(
        message,
        "the host's task request record could not be opened"
    );
    host.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn only_task_operations_are_taken() {
    let host = keyed().await;
    host.source.delete(KeyName::Owner).unwrap();
    let host = host.restart().await;
    let (code, _) = refused(task(&host.socket, request('2'), Operation::ListDevices {}).await);
    assert_eq!(code, "forbidden");
    host.running.shutdown().await;
}
