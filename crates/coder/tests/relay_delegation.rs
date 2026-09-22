//! A fan-out whose adapter is a worker on the far side of a relay.
//!
//! `relay_lifecycle.rs` proves the door's socket bookkeeping. This file
//! proves the delegation contract over it: a probe lands the capability in
//! one of the three presence states, each bounded task becomes exactly one
//! encrypted kind-25900 request and no more, the worker's `busy` comes back
//! as a typed refusal beside the answers, and the record of a delegation
//! carries the request's ID, the model the worker named, and the feedback
//! it streamed. The relay here is honest but bookkeeping; the worker is a
//! script. Everything runs on loopback with keys made in the test.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use coder::capability::Presence;
use coder::delegate::{Bounds, Status, Task};
use coder::relay::{FEEDBACK_KIND, Identity, REQUEST_KIND, RESULT_KIND, RelayDoor};
use coder::survey::Survey;
use futures_util::{SinkExt, StreamExt};
use nostr::domain::{Event, Tag};
use nostr::nip44;
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite};

const CONTACT: Duration = Duration::from_secs(2);
const ANSWER: Duration = Duration::from_secs(5);
const WORKER: u8 = 0xaa;
const CLIENT: u8 = 0x0b;

type Server = WebSocketStream<TcpStream>;

/// What the worker behind the relay is like.
#[derive(Clone, Copy)]
enum Worker {
    /// Holds an executor door: probes say `delegates: true`, tasks are
    /// answered with the text they asked for, `model` named, and
    /// `feedback` partials ahead of the result.
    Delegating { feedback: u64 },
    /// Holds a model door only: probes say `delegates: false`.
    ModelOnly,
    /// Refuses everything `busy`.
    Busy,
    /// Admits at most `jobs` tasks at once; the rest are `busy`.
    Bounded { jobs: usize },
    /// Nobody is there: requests are accepted and never answered.
    Nobody,
}

/// What the tests read back.
#[derive(Default)]
struct Ledger {
    /// Requests the relay admitted, by kind.
    requests: AtomicUsize,
    /// Requests whose content decrypted to a readable payload under the
    /// worker's key.
    encrypted: AtomicUsize,
    /// Requests with a `p` tag naming the worker.
    addressed: AtomicUsize,
    /// Jobs the worker had open at once, at most.
    peak_jobs: AtomicUsize,
    /// Task prompts the worker read, in the order it read them.
    prompts: Mutex<Vec<String>>,
}

fn identity(byte: u8) -> Identity {
    Identity::from_secret(SecretKey::from_byte_array([byte; 32]).unwrap()).unwrap()
}

fn xonly(pubkey: &str) -> XOnlyPublicKey {
    let mut bytes = [0u8; 32];
    for (index, pair) in pubkey.as_bytes().chunks_exact(2).enumerate() {
        let high = (pair[0] as char).to_digit(16).unwrap();
        let low = (pair[1] as char).to_digit(16).unwrap();
        bytes[index] = ((high << 4) | low) as u8;
    }
    XOnlyPublicKey::from_byte_array(bytes).unwrap()
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

async fn send(socket: &mut Server, value: Value) -> bool {
    socket
        .send(tungstenite::Message::Text(value.to_string().into()))
        .await
        .is_ok()
}

async fn read(socket: &mut Server) -> Option<Value> {
    loop {
        match socket.next().await? {
            Ok(tungstenite::Message::Text(text)) => {
                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    return Some(value);
                }
            }
            Ok(_) => {}
            Err(_) => return None,
        }
    }
}

/// An event from the worker, encrypted to the customer and bound to the
/// request, as the worker publishes it.
fn bound(worker: &Identity, label: &str, request: &Event, kind: u16, payload: &Value) -> Value {
    let customer = xonly(&request.pubkey);
    let conversation = nip44::conversation_key(worker.secret(), &customer);
    let ciphertext = nip44::encrypt(
        &payload.to_string(),
        &conversation,
        secp256k1::rand::random::<[u8; 32]>(),
    )
    .unwrap();
    let event = worker.signer().sign(
        unix_now(),
        kind,
        vec![
            Tag::new(vec!["e".into(), request.id.clone()]),
            Tag::new(vec!["p".into(), request.pubkey.clone()]),
        ],
        ciphertext,
    );
    json!(["EVENT", label, event])
}

/// The worker reads the request the way the real one does: NIP-44 under
/// the conversation key of its secret and the customer's pubkey.
fn decrypt(worker: &Identity, request: &Event) -> Option<Value> {
    let customer = xonly(&request.pubkey);
    let conversation = nip44::conversation_key(worker.secret(), &customer);
    let clear = nip44::decrypt(&request.content, &conversation).ok()?;
    serde_json::from_str(&clear).ok()
}

fn refusal(code: &str, message: &str) -> Value {
    json!({
        "v": 2, "type": "status", "status": "error",
        "code": code, "message": message,
    })
}

/// One connection: challenge, then one job, answered by the scripted
/// worker. Jobs open their own connection, so one connection is one job.
async fn serve(tcp: TcpStream, script: Worker, ledger: Arc<Ledger>, active: Arc<AtomicUsize>) {
    let mut socket = accept_async(tcp).await.unwrap();
    send(&mut socket, json!(["AUTH", "delegation-challenge"])).await;
    let worker = identity(WORKER);
    let mut label = String::new();
    while let Some(frame) = read(&mut socket).await {
        match frame[0].as_str().unwrap_or_default() {
            "AUTH" => {
                let id = frame[1]["id"].as_str().unwrap_or_default().to_string();
                send(&mut socket, json!(["OK", id, true, ""])).await;
            }
            "REQ" => {
                label = frame[1].as_str().unwrap_or_default().to_string();
            }
            "EVENT" => {
                let request: Event = serde_json::from_value(frame[1].clone()).unwrap();
                assert_eq!(request.kind, REQUEST_KIND, "a job is a kind-25900 request");
                ledger.requests.fetch_add(1, Ordering::SeqCst);
                if request
                    .tags
                    .iter()
                    .any(|tag| tag.name() == Some("p") && tag.value() == Some(worker.pubkey()))
                {
                    ledger.addressed.fetch_add(1, Ordering::SeqCst);
                }
                assert!(
                    serde_json::from_str::<Value>(&request.content).is_err(),
                    "the request's content travels encrypted, not as JSON"
                );
                let Some(payload) = decrypt(&worker, &request) else {
                    panic!("the request did not decrypt under the worker's key");
                };
                ledger.encrypted.fetch_add(1, Ordering::SeqCst);
                send(&mut socket, json!(["OK", request.id, true, ""])).await;

                let frames = if payload["type"] == "probe" {
                    probe_frames(script, &worker, &label, &request)
                } else {
                    let prompt = payload["task"].as_str().unwrap_or_default().to_string();
                    assert!(
                        payload["delegation"]["minutes"].as_u64().is_some(),
                        "a delegated task carries its bound"
                    );
                    ledger.prompts.lock().await.push(prompt.clone());
                    task_frames(script, &worker, &label, &request, &prompt, &ledger, &active).await
                };
                for frame in frames {
                    if !send(&mut socket, frame).await {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
}

fn probe_frames(script: Worker, worker: &Identity, label: &str, request: &Event) -> Vec<Value> {
    let probe = |door: &str, model: &str, delegates: bool| {
        vec![bound(
            worker,
            label,
            request,
            RESULT_KIND,
            &json!({
                "v": 2, "type": "result", "text": "here",
                "probe": {"door": door, "model": model, "delegates": delegates},
            }),
        )]
    };
    match script {
        Worker::Delegating { .. } | Worker::Bounded { .. } => {
            probe("executor (devin-local)", "devin-local", true)
        }
        Worker::ModelOnly => probe("open-responses", "gpt-5", false),
        Worker::Busy => vec![bound(
            worker,
            label,
            request,
            FEEDBACK_KIND,
            &refusal(
                "busy",
                "this worker is running as many jobs as it admits at once",
            ),
        )],
        Worker::Nobody => Vec::new(),
    }
}

async fn task_frames(
    script: Worker,
    worker: &Identity,
    label: &str,
    request: &Event,
    prompt: &str,
    ledger: &Ledger,
    active: &AtomicUsize,
) -> Vec<Value> {
    let busy = || {
        vec![bound(
            worker,
            label,
            request,
            FEEDBACK_KIND,
            &refusal(
                "busy",
                "this worker is running as many jobs as it admits at once",
            ),
        )]
    };
    let answer = |feedback: u64| async move {
        let mut frames = Vec::new();
        for seq in 0..feedback {
            frames.push(bound(
                worker,
                label,
                request,
                FEEDBACK_KIND,
                &json!({"v": 2, "type": "partial", "seq": seq, "delta": "."}),
            ));
        }
        frames.push(bound(
            worker,
            label,
            request,
            RESULT_KIND,
            &json!({
                "v": 2, "type": "result",
                "text": format!("answer to {prompt}"),
                "model": "devin-local",
            }),
        ));
        frames
    };
    match script {
        Worker::Delegating { feedback } => answer(feedback).await,
        Worker::Bounded { jobs } => {
            let open = active.fetch_add(1, Ordering::SeqCst) + 1;
            ledger.peak_jobs.fetch_max(open, Ordering::SeqCst);
            if open > jobs {
                active.fetch_sub(1, Ordering::SeqCst);
                return busy();
            }
            // Hold the slot long enough for the rest of the fan-out to
            // arrive and find it taken.
            tokio::time::sleep(Duration::from_millis(300)).await;
            let frames = answer(0).await;
            active.fetch_sub(1, Ordering::SeqCst);
            frames
        }
        Worker::Busy => busy(),
        Worker::ModelOnly => vec![bound(
            worker,
            label,
            request,
            FEEDBACK_KIND,
            &refusal("no_executor", "this worker holds no executor door"),
        )],
        Worker::Nobody => Vec::new(),
    }
}

/// A relay on loopback with the scripted worker behind it.
async fn spawn_relay(script: Worker) -> (String, Arc<Ledger>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let ledger = Arc::new(Ledger::default());
    let active = Arc::new(AtomicUsize::new(0));
    let served = Arc::clone(&ledger);
    tokio::spawn(async move {
        loop {
            let (tcp, _) = listener.accept().await.unwrap();
            tokio::spawn(serve(tcp, script, Arc::clone(&served), Arc::clone(&active)));
        }
    });
    (url, ledger)
}

fn door(url: &str) -> RelayDoor {
    RelayDoor::new(url, xonly(identity(WORKER).pubkey()), identity(CLIENT))
        .waiting(CONTACT, ANSWER)
        .connecting(Duration::from_millis(500))
}

/// A checkout declaring one relay capability, and nothing this host can run.
fn survey() -> (tempfile::TempDir, Survey) {
    let repository = tempfile::tempdir().unwrap();
    let capabilities = repository.path().join("capabilities");
    std::fs::create_dir_all(&capabilities).unwrap();
    std::fs::write(
        capabilities.join("devin-relay.json"),
        serde_json::to_string(&capability::executor_document(
            "devin-relay",
            "",
            Vec::new(),
            serde_json::json!({
                "transport": "nostr-cj",
                "name": "Devin, through a worker",
                "concurrent_max": 6
            }),
        ))
        .unwrap(),
    )
    .unwrap();
    let survey = Survey::read(Some(repository.path()), repository.path());
    (repository, survey)
}

fn tasks(count: usize) -> Vec<Task> {
    (0..count)
        .map(|index| Task::asking(&format!("task {index}")).bounded(Bounds::minutes(1)))
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_worker_that_delegates_makes_the_capability_present() {
    let (url, ledger) = spawn_relay(Worker::Delegating { feedback: 0 }).await;
    let (_repository, mut survey) = survey();
    let answered = survey.probe_relays_through(Ok(door(&url))).await;
    assert!(answered.is_some(), "the door that answered is handed back");
    let found = survey.capability("devin-relay").unwrap();
    assert_eq!(found.presence.state(), "present", "{:?}", found.presence);
    let Presence::Present { version, .. } = &found.presence else {
        unreachable!()
    };
    assert_eq!(version, "executor (devin-local) devin-local");
    assert_eq!(
        ledger.requests.load(Ordering::SeqCst),
        1,
        "a probe is one job"
    );
    assert_eq!(ledger.encrypted.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_worker_without_an_executor_is_present_and_unavailable() {
    let (url, _) = spawn_relay(Worker::ModelOnly).await;
    let (_repository, mut survey) = survey();
    survey.probe_relays_through(Ok(door(&url))).await;
    let found = survey.capability("devin-relay").unwrap();
    let Presence::Unavailable { refusal, .. } = &found.presence else {
        panic!("{:?}", found.presence);
    };
    assert_eq!(found.presence.state(), "present_unavailable");
    assert_eq!(refusal, "no_executor");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_worker_that_declines_the_probe_is_unavailable_with_its_code() {
    let (url, _) = spawn_relay(Worker::Busy).await;
    let (_repository, mut survey) = survey();
    survey.probe_relays_through(Ok(door(&url))).await;
    let found = survey.capability("devin-relay").unwrap();
    let Presence::Unavailable {
        refusal, detail, ..
    } = &found.presence
    else {
        panic!("{:?}", found.presence);
    };
    assert_eq!(refusal, "busy");
    assert!(detail.contains("as many jobs"), "{detail}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_relay_with_nobody_behind_it_is_unavailable_not_absent() {
    let (url, _) = spawn_relay(Worker::Nobody).await;
    let (_repository, mut survey) = survey();
    let door = door(&url).waiting(Duration::from_millis(300), Duration::from_millis(300));
    survey.probe_relays_through(Ok(door)).await;
    let found = survey.capability("devin-relay").unwrap();
    let Presence::Unavailable { refusal, .. } = &found.presence else {
        panic!("{:?}", found.presence);
    };
    assert_eq!(refusal, "no_worker");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_relay_to_ask_is_absent() {
    let (_repository, mut survey) = survey();
    let answered = survey
        .probe_relays_through(Err("CODER_WORKER is not set".into()))
        .await;
    assert!(answered.is_none());
    let found = survey.capability("devin-relay").unwrap();
    let Presence::Absent { reason, .. } = &found.presence else {
        panic!("{:?}", found.presence);
    };
    assert_eq!(reason, "CODER_WORKER is not set");

    // A relay that is not listening is absent too: the door was built,
    // and there was nothing at the far end of it.
    let closed = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", closed.local_addr().unwrap());
    drop(closed);
    survey.probe_relays_through(Ok(door(&url))).await;
    let found = survey.capability("devin-relay").unwrap();
    assert_eq!(found.presence.state(), "absent", "{:?}", found.presence);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_task_is_one_encrypted_request_and_its_record_says_what_answered() {
    let (url, ledger) = spawn_relay(Worker::Delegating { feedback: 3 }).await;
    let door = door(&url);
    let delegations = door.fan_out("devin-relay", tasks(6), 6).await;

    assert_eq!(delegations.len(), 6);
    assert_eq!(
        ledger.requests.load(Ordering::SeqCst),
        6,
        "one request per task, no retries"
    );
    assert_eq!(
        ledger.encrypted.load(Ordering::SeqCst),
        6,
        "every request decrypted under the worker's key"
    );
    assert_eq!(
        ledger.addressed.load(Ordering::SeqCst),
        6,
        "every request names the worker"
    );
    let mut prompts = ledger.prompts.lock().await.clone();
    prompts.sort();
    assert_eq!(
        prompts,
        (0..6).map(|i| format!("task {i}")).collect::<Vec<_>>()
    );

    let mut requests = std::collections::HashSet::new();
    for delegation in &delegations {
        assert!(
            matches!(delegation.status, Status::Answered),
            "{:?}",
            delegation.status
        );
        assert_eq!(
            delegation.output,
            format!("answer to {}", delegation.task.prompt)
        );
        let relayed = delegation
            .relayed
            .as_ref()
            .expect("a relay delegation records where it went");
        assert_eq!(relayed.relay, url);
        assert_eq!(relayed.worker, identity(WORKER).pubkey());
        assert_eq!(relayed.request.len(), 64, "the request's event ID");
        assert!(
            requests.insert(relayed.request.clone()),
            "one request ID per task"
        );
        assert_eq!(relayed.model.as_deref(), Some("devin-local"));
        assert_eq!(
            relayed.feedback, 3,
            "the partials the worker streamed ahead of its result"
        );
        assert_eq!(delegation.capability, "devin-relay");
        assert_eq!(delegation.concurrent_max, 6);
        assert!(delegation.boundary.is_none(), "nothing ran on this host");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_workers_busy_is_a_typed_refusal_beside_the_answers() {
    let (url, ledger) = spawn_relay(Worker::Bounded { jobs: 2 }).await;
    let door = door(&url);
    let delegations = door.fan_out("devin-relay", tasks(6), 6).await;

    let answered = delegations
        .iter()
        .filter(|d| matches!(d.status, Status::Answered))
        .count();
    let busy: Vec<_> = delegations
        .iter()
        .filter(|d| matches!(&d.status, Status::Refused(code) if code == "busy"))
        .collect();
    assert_eq!(
        answered,
        2,
        "{:?}",
        delegations.iter().map(|d| &d.status).collect::<Vec<_>>()
    );
    assert_eq!(busy.len(), 4);
    assert_eq!(
        ledger.requests.load(Ordering::SeqCst),
        6,
        "a refusal is not retried"
    );
    for refused in busy {
        assert!(
            refused.detail.contains("as many jobs"),
            "{}",
            refused.detail
        );
        assert!(refused.output.is_empty());
        let relayed = refused.relayed.as_ref().unwrap();
        assert_eq!(
            relayed.request.len(),
            64,
            "a refused job still has a name in the relay's log"
        );
        assert!(relayed.model.is_none(), "nothing generated");
    }
    assert!(
        delegations
            .iter()
            .all(|d| !matches!(d.status, Status::Harness(_))),
        "a worker's bound is not a harness failure"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_terminals_width_bounds_how_many_jobs_are_open_at_once() {
    let (url, ledger) = spawn_relay(Worker::Bounded { jobs: 6 }).await;
    let door = door(&url);
    let delegations = door.fan_out("devin-relay", tasks(6), 2).await;
    assert!(
        delegations
            .iter()
            .all(|d| matches!(d.status, Status::Answered))
    );
    assert!(
        ledger.peak_jobs.load(Ordering::SeqCst) <= 2,
        "{} jobs open at once under a width of 2",
        ledger.peak_jobs.load(Ordering::SeqCst)
    );
    assert!(delegations.iter().all(|d| d.concurrent_max == 2));
}
