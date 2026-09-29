//! The runner's relay connection never trusts a quiet socket (#9946).
//!
//! `relay.openagents.com` sits behind Google's front end, which can keep a
//! subscriber's connection established after the relay instance behind it
//! is gone. These tests run [`eval_runner::runner::listen`] against a fake
//! relay on loopback: a relay that stops answering is noticed and the
//! runner subscribes again; a relay that answers its probes keeps the
//! connection; and the subscription is renewed on an overlapping
//! connection, with a request delivered during the renewal answered, and a
//! request delivered on both connections answered once.

mod support;

use std::sync::Arc;
use std::time::Duration;

use coder::relay::Identity;
use coder::relay::liveness::Liveness;
use eval_runner::config::Limits;
use eval_runner::runner::{Ended, Runner, listen};
use eval_runner::wire::memory::Memory;
use futures_util::{SinkExt as _, StreamExt as _};
use nostr::cj_conversation::{SubjectSource, SuiteSource};
use nostr::domain::Event;
use nostr::eval_ext::hosted;
use nostr::execution;
use serde_json::{Value, json};
use support::{Phone, door, memory_runner};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite};

type Server = WebSocketStream<TcpStream>;

const TEST_BOUND: Duration = Duration::from_secs(60);

async fn bounded<T>(test: impl Future<Output = T>) -> T {
    tokio::time::timeout(TEST_BOUND, test)
        .await
        .expect("the test finishes inside its bound")
}

fn limits() -> Limits {
    Limits {
        runs_per_trainer: 3,
        turns_per_day: 500,
        jobs: 2,
        concurrency: 2,
    }
}

async fn send(socket: &mut Server, value: Value) {
    socket
        .send(tungstenite::Message::Text(value.to_string().into()))
        .await
        .unwrap();
}

/// The runner's next frame, exactly as sent.
async fn read_raw(socket: &mut Server) -> Option<Value> {
    loop {
        match socket.next().await? {
            Ok(tungstenite::Message::Text(text)) => return serde_json::from_str(&text).ok(),
            Ok(_) => {}
            Err(_) => return None,
        }
    }
}

fn is_probe(value: &Value) -> bool {
    (value[0] == "REQ" || value[0] == "CLOSE")
        && value[1].as_str().is_some_and(|id| id.starts_with("alive-"))
}

/// The runner's next frame, with its probes answered as a live relay
/// answers them.
async fn read(socket: &mut Server) -> Option<Value> {
    loop {
        let value = read_raw(socket).await?;
        if is_probe(&value) {
            if value[0] == "REQ" {
                send(socket, json!(["EOSE", value[1]])).await;
            }
            continue;
        }
        return Some(value);
    }
}

/// Accepts the runner's next connection, authenticates it, and confirms
/// its requests subscription; returns the socket and the filter.
async fn subscribe(listener: &TcpListener, runner: &str) -> (Server, Value) {
    let (tcp, _) = listener.accept().await.unwrap();
    let mut socket = accept_async(tcp).await.unwrap();
    send(&mut socket, json!(["AUTH", "runner-challenge"])).await;
    loop {
        let frame = read(&mut socket).await.expect("the runner stays connected");
        match frame[0].as_str().unwrap_or_default() {
            "AUTH" => {
                let auth: Event = serde_json::from_value(frame[1].clone()).unwrap();
                auth.validate_crypto().unwrap();
                assert_eq!(auth.pubkey, runner);
                send(&mut socket, json!(["OK", auth.id, true, ""])).await;
            }
            "REQ" => {
                assert_eq!(frame[1], "jobs");
                let filter = frame[2].clone();
                assert_eq!(filter["kinds"], json!([execution::REQUEST_KIND]));
                assert_eq!(filter["#p"], json!([runner]));
                send(&mut socket, json!(["EOSE", "jobs"])).await;
                return (socket, filter);
            }
            other => panic!("unexpected frame before the subscription: {other} {frame}"),
        }
    }
}

async fn deliver(socket: &mut Server, request: &Event) {
    send(socket, json!(["EVENT", "jobs", request])).await;
}

/// A runner in memory, listening to the fake relay at `url` the way
/// `eval-runner serve` does, reconnecting after each end, which it reports.
struct Listening {
    runner: Arc<Runner>,
    memory: Arc<Memory>,
    ends: mpsc::UnboundedReceiver<Ended>,
    _dir: tempfile::TempDir,
}

fn start(url: &str, liveness: Liveness) -> Listening {
    let dir = tempfile::tempdir().unwrap();
    let (runner, memory, key) = memory_runner(dir.path(), &door(), limits());
    let identity = Arc::new(Identity::from_text(&support::hex(&key), "runner").unwrap());
    let (report, ends) = mpsc::unbounded_channel();
    let (url, listening) = (url.to_string(), Arc::clone(&runner));
    tokio::spawn(async move {
        loop {
            let ended = listen(&url, &identity, &listening, liveness).await;
            if report.send(ended).is_err() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });
    Listening {
        runner,
        memory,
        ends,
        _dir: dir,
    }
}

/// A request the runner refuses `not_admitted` at once: a draft that asks
/// to run commands.
fn refused_request(runner: &Runner, phone: &Phone) -> Event {
    let tool = &runner.catalog().tools[0];
    let draft = json!({
        "v": nostr::cj_conversation::DRAFT_SCHEMA,
        "tool": {"name": "Map brief", "summary": "Maps, then briefs.", "catalog": null,
                 "skill": "When asked about a repository, name its main language.",
                 "uses": [tool.aliases[0].id]},
        "cases": [{
            "id": "overview-0",
            "kind": "should-fire",
            "prompt": "+++\nv = \"openagents.eval-case.v1\"\n[run]\nallowed_operations = [\"read\", \"exec\"]\n+++\n\nGive me an overview of the repository.\n",
            "graders": [{"name": "said", "text": "+++\ntype = \"regex\"\ntarget = \"last_message\"\n+++\n\nRust\n"}],
        }],
    });
    let input = hosted::run_input(
        &SuiteSource::Draft,
        &SubjectSource::Draft,
        Some(&draft),
        1,
        None,
    )
    .unwrap();
    phone.request(runner.pubkey(), &input).0
}

/// The runner's answers bound to `request`.
fn answers_to(memory: &Memory, request: &Event) -> usize {
    memory
        .events
        .lock()
        .unwrap()
        .iter()
        .filter(|event| event.tag_values("e").any(|id| id == request.id))
        .count()
}

async fn answered(memory: &Memory, request: &Event) {
    while answers_to(memory, request) == 0 {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn liveness(probe_ms: u64, renew_ms: u64) -> Liveness {
    Liveness {
        probe: Duration::from_millis(probe_ms),
        renew: Duration::from_millis(renew_ms),
    }
}

/// A relay that stops answering without closing the socket is noticed.
///
/// This is #9946 as the runner would meet it: the relay takes the
/// subscription and then answers nothing while the socket stays open. The
/// runner's probe goes unanswered, the connection ends naming the fault,
/// the runner connects and subscribes again, and it answers the next
/// request.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_relay_that_goes_silent_is_noticed_and_rejoined() {
    bounded(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let mut listening = start(&url, liveness(300, 3_600_000));
        let key = listening.runner.pubkey().to_string();

        let (mut silent, _) = subscribe(&listener, &key).await;
        // The relay reads what the runner sends, as a front end does, but
        // nothing comes back: not the probe's answer, not a close.
        let probe = read_raw(&mut silent).await.expect("the runner probes");
        assert!(is_probe(&probe) && probe[0] == "REQ", "{probe}");
        assert_eq!(probe[2]["limit"], 0, "{probe}");

        let ended = listening.ends.recv().await.unwrap();
        assert!(ended.subscribed, "{ended:?}");
        assert!(ended.why.contains("stopped answering"), "{ended:?}");

        let (mut socket, _) = subscribe(&listener, &key).await;
        let phone = Phone::new();
        let request = refused_request(&listening.runner, &phone);
        deliver(&mut socket, &request).await;
        tokio::spawn(async move { while read(&mut socket).await.is_some() {} });
        answered(&listening.memory, &request).await;
    })
    .await;
}

/// A relay that answers every probe keeps its connection: probes alone
/// never end a healthy one.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_relay_that_answers_its_probes_keeps_the_connection() {
    bounded(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let mut listening = start(&url, liveness(200, 3_600_000));
        let key = listening.runner.pubkey().to_string();
        let (mut socket, _) = subscribe(&listener, &key).await;

        // Ten probe intervals, each answered by `read`.
        let answering = async { while read(&mut socket).await.is_some() {} };
        let _ = tokio::time::timeout(Duration::from_secs(2), answering).await;
        assert!(
            listening.ends.try_recv().is_err(),
            "a connection whose probes are answered is kept"
        );
        let phone = Phone::new();
        let request = refused_request(&listening.runner, &phone);
        deliver(&mut socket, &request).await;
        tokio::spawn(async move { while read(&mut socket).await.is_some() {} });
        answered(&listening.memory, &request).await;
    })
    .await;
}

/// The subscription is renewed before the relay's front end would end it,
/// and the two overlap: the successor is subscribed before the old one is
/// closed, a request the relay delivers on the old connection in that
/// moment is still answered, and a request delivered on both is answered
/// once, so it's never run twice.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_subscription_is_renewed_on_an_overlapping_connection() {
    bounded(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let listening = start(&url, liveness(3_600_000, 400));
        let key = listening.runner.pubkey().to_string();
        let (mut old, _) = subscribe(&listener, &key).await;

        // The successor connects while the first connection is still open.
        let (mut new, _) = subscribe(&listener, &key).await;

        // The old subscription is closed only now, and a request the relay
        // put on it before the close still reaches the runner.
        let phone = Phone::new();
        let late = refused_request(&listening.runner, &phone);
        deliver(&mut old, &late).await;
        loop {
            let frame = read(&mut old)
                .await
                .expect("the old connection is closed only after");
            if frame[0] == "CLOSE" {
                assert_eq!(frame[1], "jobs");
                break;
            }
        }
        answered(&listening.memory, &late).await;

        // One request on both subscriptions is answered once.
        let both = refused_request(&listening.runner, &phone);
        deliver(&mut old, &both).await;
        deliver(&mut new, &both).await;
        answered(&listening.memory, &both).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(answers_to(&listening.memory, &both), 1);
        assert_eq!(answers_to(&listening.memory, &late), 1);

        // The renewed connection carries the next request as the first did.
        let next = refused_request(&listening.runner, &phone);
        deliver(&mut new, &next).await;
        answered(&listening.memory, &next).await;
        assert_eq!(answers_to(&listening.memory, &next), 1);
    })
    .await;
}
