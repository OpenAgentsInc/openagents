//! The CAP/CJ binding of NIP-HOST over a synthetic NIP-42 relay.
//!
//! The same request gets the same outcome through the direct artifact
//! binding and through CJ execution: a granted operation, a missing right, a
//! revoked grant, a stale epoch, and a reused request ID. A CJ `task.create`
//! reaches the task owner once, and the binding refuses a relayed request and
//! a foreign target before NIP-HOST admission sees them.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use coder_host::access::cj::{self, Capability, fetch_capability};
use coder_host::access::protocol::{
    CommandAction, Operation, Outcome, QueueEdit, REQUEST, Request, TaskCommand, TaskCreate,
    TaskQueue,
};
use coder_host::access::{Access, Client, Code, Pending, RelayPolicy, Right, Rights};
use coder_host::config::Config;
use coder_host::reach::pubkey;
use coder_host::{Principal, Standing, TaskRef, Tasks};
use nostr::activity_summary::Phase;
use nostr::domain::Event;
use secp256k1::SecretKey;
use serde_json::json;

#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;

fn key() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}

fn now() -> u64 {
    coder_host::unix_time().unwrap()
}

/// A task owner that records each creation once per idempotency key, and
/// each durable command once per device and command ID.
#[derive(Default)]
struct Recorder(
    Mutex<BTreeMap<String, (TaskRef, TaskCreate)>>,
    Mutex<BTreeMap<(String, String), TaskCommand>>,
);

impl Recorder {
    fn created(&self) -> Vec<(TaskRef, TaskCreate)> {
        self.0.lock().unwrap().values().cloned().collect()
    }
}

impl Tasks for Recorder {
    fn create(&self, key: &str, _: &str, task: &TaskCreate) -> Result<TaskRef, Code> {
        let mut tasks = self.0.lock().unwrap();
        let (found, _) = tasks.entry(key.to_owned()).or_insert_with(|| {
            let found = TaskRef {
                task: coder_host::reach::new_id(),
                revision: 1,
                phase: Phase::Queued,
            };
            (found, task.clone())
        });
        Ok(found.clone())
    }
    fn steer(&self, _: &str, _: &str, _: &str, _: u64, _: &str) -> Result<TaskRef, Code> {
        Err(Code::Unavailable)
    }
    fn cancel(&self, _: &str, _: &str, _: &str, _: u64, _: &str) -> Result<TaskRef, Code> {
        Err(Code::Unavailable)
    }
    fn command(
        &self,
        principal: &Principal,
        command: &TaskCommand,
        _: Standing<'_>,
    ) -> Result<TaskRef, Code> {
        let mut commands = self.1.lock().unwrap();
        let key = (principal.device.clone(), command.command.clone());
        if commands.get(&key).is_some_and(|held| held != command) {
            return Err(Code::Conflict);
        }
        commands.insert(key, command.clone());
        Ok(TaskRef {
            task: command.task.clone(),
            revision: 2,
            phase: Phase::Queued,
        })
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
                revision: 2,
                lease: None,
                items: Vec::new(),
            },
            None,
        ))
    }
}

struct Fixture {
    _temp: tempfile::TempDir,
    relay: String,
    store: coder_host::access::host::Host,
    running: coder_host::Running,
    tasks: Arc<Recorder>,
}

async fn fixture() -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let (relay, _task, _events) = relay::start().await;
    let access = temp.path().join("access");
    let store = coder_host::access::host::Host::new(&access, POLICY);
    store.init(&pubkey(&key())).unwrap();
    let mut config = Config::new(access, vec![relay.clone()], 1);
    config.policy = POLICY;
    let tasks = Arc::new(Recorder::default());
    let running = coder_host::start(config, tasks.clone()).await.unwrap();
    Fixture {
        _temp: temp,
        relay,
        store,
        running,
        tasks,
    }
}

/// An enrolled device's key, saved access, and client.
struct Enrolled {
    secret: SecretKey,
    access: Access,
    client: Client,
}

impl Fixture {
    async fn enroll(&self, rights: Rights, secret: SecretKey) -> Enrolled {
        let code = self
            .store
            .invite(&self.relay, rights, now(), now() + 3600)
            .unwrap()
            .code;
        let access = coder_host::access::client::redeem(&code, &secret, POLICY)
            .await
            .unwrap();
        let client = Client::device(access.clone(), secret, POLICY).unwrap();
        Enrolled {
            secret,
            access,
            client,
        }
    }

    /// The capability the host published, read the way a device reads it.
    async fn capability(&self, reader: &SecretKey) -> Capability {
        let host = self.running.host_key();
        for _ in 0..100 {
            if let Ok(capability) = fetch_capability(&self.relay, reader, host, POLICY).await {
                return capability;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("the host published no capability")
    }

    /// The host's signed reply through the direct artifact binding.
    async fn artifact_reply(&self, secret: &SecretKey, pending: &Pending) -> Event {
        let mut session = coder_connect::transport::Session::connect(&self.relay, secret, POLICY)
            .await
            .unwrap();
        session
            .exchange_event(
                &pending.event,
                &pending.request.request,
                (pending.request.issued_at, pending.request.expires_at),
                self.running.host_key(),
                &pubkey(secret),
            )
            .await
            .unwrap()
    }
}

/// A request signed by `secret` that names any grant and epoch.
fn forge(device: &Enrolled, grant: &str, epoch: u64, op: Operation) -> Pending {
    let issued = now();
    let request = Request {
        v: REQUEST.into(),
        requires: vec![],
        request: coder_host::reach::new_id(),
        host: device.access.grant.host.clone(),
        grant: Some(grant.into()),
        epoch: Some(epoch),
        relay: device.access.grant.relay.clone(),
        issued_at: issued,
        expires_at: issued + 60,
        op,
    };
    reseal(device, request)
}

/// The same request under a new signature and nonce: other event bytes.
fn reseal(device: &Enrolled, request: Request) -> Pending {
    let event = coder_connect::protocol::seal(
        &request,
        REQUEST,
        &device.secret,
        &request.host,
        &request.request,
        request.issued_at,
        request.expires_at,
    )
    .unwrap();
    Pending { request, event }
}

fn create() -> Operation {
    Operation::CreateTask {
        task: TaskCreate {
            title: "Check the build".into(),
            prompt: "Run the tests and report.".into(),
            workspace: "checkout".into(),
        },
    }
}

/// An outcome with the parts that must agree across bindings: the outcome
/// kind and operation, or the refusal code and missing right.
fn verdict(result: &Result<Outcome, coder_host::access::Error>) -> String {
    match result {
        Ok(Outcome::Dispatched { receipt }) => format!("dispatched {}", receipt.operation),
        Ok(outcome) => format!("ok {outcome:?}"),
        Err(error) => format!("refused {:?} {:?}", error.code, error.missing),
    }
}

/// Send a fresh request through each binding and return both verdicts.
async fn both(
    capability: &Capability,
    device: &Enrolled,
    prepare: impl Fn() -> Pending,
) -> (String, String) {
    let artifact = prepare();
    let artifact = device.client.send(&artifact).await;
    let execution = prepare();
    let execution = device.client.send_cj(capability, &execution).await;
    (verdict(&artifact), verdict(&execution))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn each_binding_gives_the_same_outcome_for_the_same_request() {
    let fixture = fixture().await;
    let operator = fixture.enroll(Rights::standard(), key()).await;
    let observer = fixture
        .enroll(Rights::new([Right::Observe]).unwrap(), key())
        .await;
    let capability = fixture.capability(&operator.secret).await;
    assert_eq!(capability.host(), fixture.running.host_key());

    // A granted operation dispatches through either binding.
    let prepare = || operator.client.prepare(create(), now()).unwrap();
    let (artifact, execution) = both(&capability, &operator, prepare).await;
    assert_eq!(artifact, "dispatched task.create");
    assert_eq!(execution, artifact);
    assert_eq!(fixture.tasks.created().len(), 2);

    // A device without `operate` is refused naming the right.
    let prepare = || observer.client.prepare(create(), now()).unwrap();
    let (artifact, execution) = both(&capability, &observer, prepare).await;
    assert_eq!(artifact, "refused MissingRight Some(Operate)");
    assert_eq!(execution, artifact);

    // Revoke a device, then enroll it again at the next epoch. Its old grant
    // is revoked, and its new grant at the old epoch is stale.
    let lost = fixture.enroll(Rights::standard(), key()).await;
    let old_grant = lost.access.grant.grant.clone();
    fixture.store.revoke(&pubkey(&lost.secret), now()).unwrap();
    let again = fixture.enroll(Rights::standard(), lost.secret).await;
    assert_eq!(again.access.grant.epoch, 1);
    let prepare = || forge(&again, &old_grant, 0, create());
    let (artifact, execution) = both(&capability, &again, prepare).await;
    assert_eq!(artifact, "refused Revoked None");
    assert_eq!(execution, artifact);
    let new_grant = again.access.grant.grant.clone();
    let prepare = || forge(&again, &new_grant, 0, create());
    let (artifact, execution) = both(&capability, &again, prepare).await;
    assert_eq!(artifact, "refused Stale None");
    assert_eq!(execution, artifact);
    assert_eq!(fixture.tasks.created().len(), 2);

    // One request ID is one logical operation across both bindings: the
    // retained reply returns byte for byte, and the owner records it once.
    let pending = operator.client.prepare(create(), now()).unwrap();
    let first = fixture.artifact_reply(&operator.secret, &pending).await;
    let over_cj = operator
        .client
        .exchange_cj(&capability, &pending)
        .await
        .unwrap();
    assert_eq!(over_cj, first);
    let again_cj = operator
        .client
        .exchange_cj(&capability, &pending)
        .await
        .unwrap();
    assert_eq!(again_cj, first);
    assert_eq!(fixture.tasks.created().len(), 3);

    // Other bytes under a used request ID refuse as `conflict` either way.
    let changed = reseal(&operator, pending.request.clone());
    assert_ne!(changed.event.id, pending.event.id);
    let artifact = operator.client.send(&changed).await;
    let execution = operator.client.send_cj(&capability, &changed).await;
    assert_eq!(verdict(&artifact), "refused Conflict None");
    assert_eq!(verdict(&execution), verdict(&artifact));
    assert_eq!(fixture.tasks.created().len(), 3);
    fixture.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_task_created_over_cj_reaches_the_task_owner_once() {
    let fixture = fixture().await;
    let device = fixture.enroll(Rights::standard(), key()).await;
    let capability = fixture.capability(&device.secret).await;
    let pending = device.client.prepare(create(), now()).unwrap();
    let Outcome::Dispatched { receipt } =
        device.client.send_cj(&capability, &pending).await.unwrap()
    else {
        panic!("task.create answered another outcome")
    };
    assert_eq!(receipt.operation, "task.create");
    let created = fixture.tasks.created();
    assert_eq!(created.len(), 1);
    let (task, body) = &created[0];
    assert_eq!(receipt.reference, task.task);
    assert_eq!(body.title, "Check the build");
    // A completed CJ result is the handling receipt: the task is queued, not
    // finished.
    assert_eq!(task.phase, Phase::Queued);

    // A retry of the same request is the same logical operation.
    let Outcome::Dispatched { receipt: retried } =
        device.client.send_cj(&capability, &pending).await.unwrap()
    else {
        panic!("the retry answered another outcome")
    };
    assert_eq!(retried, receipt);
    assert_eq!(fixture.tasks.created().len(), 1);

    // The device list shows the device the host last saw over CJ.
    let admin = fixture
        .enroll(Rights::new([Right::AccessRead]).unwrap(), key())
        .await;
    let Outcome::Devices { devices } = admin
        .client
        .call_cj(&capability, Operation::ListDevices {})
        .await
        .unwrap()
    else {
        panic!("device.list answered another outcome")
    };
    let seen = devices
        .iter()
        .find(|entry| entry.device == pubkey(&device.secret))
        .unwrap();
    assert!(seen.last_seen.is_some());
    fixture.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_binding_refuses_a_relayed_request_and_a_foreign_target() {
    let fixture = fixture().await;
    let device = fixture.enroll(Rights::standard(), key()).await;
    let capability = fixture.capability(&device.secret).await;
    let host = fixture.running.host_key().to_owned();

    // Another key wraps the device's signed request in its own job.
    let pending = device.client.prepare(create(), now()).unwrap();
    let thief = key();
    let body = capability.execute(&pending);
    let event = cj::seal_execute(&thief, &host, &body, now()).unwrap();
    let error = cj::exchange(
        &fixture.relay,
        &thief,
        &host,
        &pending.request.request,
        &event,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, Code::Forbidden, "{error:?}");

    // A target that is not the host's definition is an identity mismatch.
    let mut body = capability.execute(&pending);
    body["target"]["artifact"]["digest"] = json!(format!("sha256:{}", "0".repeat(64)));
    let event = cj::seal_execute(&device.secret, &host, &body, now()).unwrap();
    let error = cj::exchange(
        &fixture.relay,
        &device.secret,
        &host,
        &pending.request.request,
        &event,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, Code::Unsupported, "{error:?}");

    // Neither reached NIP-HOST admission; the untouched request still
    // dispatches once through the binding.
    assert!(fixture.tasks.created().is_empty());
    let outcome = device.client.send_cj(&capability, &pending).await.unwrap();
    assert!(matches!(outcome, Outcome::Dispatched { .. }));
    assert_eq!(fixture.tasks.created().len(), 1);
    fixture.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn durable_commands_and_queue_edits_travel_over_cj_as_over_the_artifact_binding() {
    let fixture = fixture().await;
    let device = fixture.enroll(Rights::standard(), key()).await;
    let capability = fixture.capability(&device.secret).await;
    for operation in ["task.command", "task.queue"] {
        assert!(
            capability.definition()["binding_contract"]["operations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|name| name == operation),
            "the host advertises {operation}"
        );
    }
    let task = "a".repeat(64);
    let command = TaskCommand {
        command: "c".repeat(64),
        task: task.clone(),
        action: CommandAction::Queue,
        based_on: 1,
        text: "Then update the changelog.".into(),
        emulate: false,
        issued_at: now(),
    };
    let queue = || Operation::CommandTask {
        command: command.clone(),
    };
    let prepare = || device.client.prepare(queue(), now()).unwrap();
    let (artifact, execution) = both(&capability, &device, prepare).await;
    assert_eq!(artifact, "dispatched task.command");
    assert_eq!(execution, artifact);
    // Both requests carried one device-minted command: the owner holds it once.
    assert_eq!(fixture.tasks.1.lock().unwrap().len(), 1);
    let list = || Operation::QueueTask {
        task: task.clone(),
        edit: QueueEdit::List {},
    };
    let prepare = || device.client.prepare(list(), now()).unwrap();
    let (artifact, execution) = both(&capability, &device, prepare).await;
    assert!(artifact.starts_with("ok Queue"), "{artifact}");
    assert_eq!(execution, artifact);
    fixture.running.shutdown().await;
}
