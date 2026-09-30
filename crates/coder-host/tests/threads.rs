//! A paired phone reads the host's chat threads, continues one, stops
//! a reply, and starts Coder from an offer (NIP-HOST `thread.list`,
//! `thread.read`, `thread.send`, `thread.stop`, `thread.run`).
//!
//! The host runs with a control socket, the one the desktop app and
//! `openagents chat` use, and its threads ask a scripted chat worker on a
//! local relay. The owner starts a thread over the socket; a phone paired
//! with a connect code lists it, reads its turns over iroh, watches the
//! reply to its own follow-up stream in, and the owner then reads the
//! follow-up and reply back over the socket. Nothing reaches a public relay
//! or worker, or the real home.

use std::time::{Duration, Instant};

use coder_host::access::protocol::{Operation, Outcome};
use coder_host::access::thread::{ThreadPage, ThreadRole};
use coder_host::access::{Code, Right};
use coder_host::client::Link;
use coder_host::config::ChatDoor;
use coder_host::{Error, Result};
use openagents_chat::service::{Command, Snapshot};
use openagents_connect::control::{Op, Reply};

#[path = "support/connect.rs"]
mod support;

#[path = "support/chat_worker.rs"]
mod chat_worker;

use support::{Options, Phone, call, host_with, now};

async fn chat(host: &support::Host, command: Command) -> Snapshot {
    match call(&host.socket, Op::Chat { command }).await.unwrap() {
        Reply::Chat { snapshot } => snapshot,
        other => panic!("not a chat answer: {other:?}"),
    }
}

async fn read(link: &Link, thread: &str) -> Result<ThreadPage> {
    match link
        .call(Operation::ReadThread {
            thread: thread.to_owned(),
            before: None,
        })
        .await?
    {
        Outcome::Thread { thread } => Ok(*thread),
        other => panic!("not a thread page: {other:?}"),
    }
}

fn refusal(error: &Error) -> Option<(Code, Option<Right>)> {
    match error {
        Error::Access(error) => Some((error.code, error.missing)),
        _ => None,
    }
}

/// A second phone paired with only `observe`, and its link.
async fn observer(host: &support::Host) -> (Phone, Link) {
    let watcher = Phone::new().await;
    let at = now();
    let invitation = host
        .store
        .invite(
            &host.relay,
            coder_host::access::Rights::parse_list("observe").unwrap(),
            at,
            at + 86_400,
        )
        .unwrap();
    let parsed =
        coder_host::access::protocol::HostInvitation::parse(&invitation.code, at, support::POLICY)
            .unwrap();
    let pending =
        coder_host::access::client::prepare_redeem(&parsed, &watcher.secret, at, support::POLICY)
            .unwrap();
    let reply = host
        .store
        .handle(
            &pending.event,
            &host.relay,
            at,
            &mut coder_host::access::host::Unconnected,
        )
        .unwrap();
    let access = coder_host::access::client::finish_redeem(
        &parsed,
        &pending,
        &reply,
        &watcher.secret,
        at,
        support::POLICY,
    )
    .unwrap();
    let observer = watcher.device(access);
    let link = watcher.link(host, &observer).await.unwrap();
    (watcher, link)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_paired_phone_reads_a_host_thread_and_continues_it() {
    let worker = support::key();
    let (door, payloads) = chat_worker::start(worker).await;
    let host = host_with(Options {
        chat_door: Some(ChatDoor {
            relay: door,
            worker: chat_worker::worker_key(&worker),
        }),
        ..Options::default()
    })
    .await;

    // The owner starts a thread over the control socket, as `openagents
    // chat` does against a running host.
    let thread = "4a".repeat(16);
    chat(
        &host,
        Command::Create {
            chat: thread.clone(),
        },
    )
    .await;
    chat(
        &host,
        Command::Send {
            chat: thread.clone(),
            request: "11".repeat(16),
            text: "Write a haiku about rain".into(),
        },
    )
    .await;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let snapshot = chat(
            &host,
            Command::Read {
                chat: thread.clone(),
                before: None,
            },
        )
        .await;
        if !snapshot.busy && snapshot.turns.len() == 2 {
            assert_eq!(snapshot.turns[1].text, "Rain on the roof. (turns: 1)");
            break;
        }
        assert!(Instant::now() < deadline, "the worker never answered");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // A phone pairs with a connect code and opens a direct channel.
    let phone = Phone::new().await;
    let (_, code) = host.code().await;
    let (_, access, _) = phone.redeem(&code, &host.relay, now()).await;
    let device = phone.device(access.unwrap());
    let link = phone.link(&host, &device).await.unwrap();

    // It lists the thread and reads its turns.
    let Outcome::Threads { threads } = link.call(Operation::ListThreads {}).await.unwrap() else {
        panic!("a thread list")
    };
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].thread, thread);
    assert_eq!(threads[0].title, "Write a haiku about rain");
    let page = read(&link, &thread).await.unwrap();
    assert_eq!((page.start, page.total), (0, 2));
    assert_eq!(page.turns[0].role, ThreadRole::User);
    assert_eq!(page.turns[0].text, "Write a haiku about rain");
    assert_eq!(page.turns[1].role, ThreadRole::Assistant);
    assert_eq!(page.turns[1].model.as_deref(), Some("fixture-model"));
    assert!(!page.busy);

    // A follow-up from the phone appends through the host, and its reply
    // streams in while the phone polls.
    let send = Operation::SendThread {
        thread: thread.clone(),
        request: "22".repeat(16),
        text: "And in the snow?".into(),
    };
    let Outcome::Dispatched { receipt } = link.call(send.clone()).await.unwrap() else {
        panic!("a receipt")
    };
    assert_eq!(
        (receipt.operation.as_str(), receipt.reference.as_str()),
        ("thread.send", thread.as_str())
    );
    let mut streamed = false;
    let deadline = Instant::now() + Duration::from_secs(20);
    let page = loop {
        let page = read(&link, &thread).await.unwrap();
        streamed |= page.busy && !page.partial.is_empty();
        if !page.busy && page.total == 4 {
            break page;
        }
        assert!(
            Instant::now() < deadline,
            "the follow-up was never answered"
        );
        tokio::time::sleep(Duration::from_millis(40)).await;
    };
    assert!(streamed, "the reply streamed to the phone");
    assert_eq!(page.turns[2].text, "And in the snow?");
    assert_eq!(
        page.turns[2].request.as_deref(),
        Some("22".repeat(16).as_str())
    );
    // The worker answered with the whole thread as context.
    assert_eq!(page.turns[3].text, "Rain on the roof. (turns: 3)");
    assert_eq!(payloads.lock().unwrap().len(), 2);

    // A retry of the same send changes nothing; other text under its ID
    // refuses.
    link.call(send).await.unwrap();
    let conflict = link
        .call(Operation::SendThread {
            thread: thread.clone(),
            request: "22".repeat(16),
            text: "Something else".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(refusal(&conflict), Some((Code::Conflict, None)));
    // An unknown thread refuses.
    let unknown = read(&link, &"5b".repeat(16)).await.unwrap_err();
    assert_eq!(refusal(&unknown), Some((Code::Unavailable, None)));

    // The owner sees the phone's follow-up and its reply, as the desktop
    // and `openagents chat read` do.
    let snapshot = chat(
        &host,
        Command::Read {
            chat: thread.clone(),
            before: None,
        },
    )
    .await;
    let texts: Vec<&str> = snapshot.turns.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "Write a haiku about rain",
            "Rain on the roof. (turns: 1)",
            "And in the snow?",
            "Rain on the roof. (turns: 3)"
        ]
    );
    assert_eq!(payloads.lock().unwrap().len(), 2);

    // A device that may only observe reads threads but cannot continue one.
    let (_watcher, link) = observer(&host).await;
    assert_eq!(read(&link, &thread).await.unwrap().total, 4);
    let refused = link
        .call(Operation::SendThread {
            thread: thread.clone(),
            request: "33".repeat(16),
            text: "May I?".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(
        refusal(&refused),
        Some((Code::MissingRight, Some(Right::Operate)))
    );
    host.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn thread_reads_are_not_retained_so_polling_never_fills_the_store() {
    let host = host_with(Options::default()).await;
    let thread = "6c".repeat(16);
    chat(
        &host,
        Command::Create {
            chat: thread.clone(),
        },
    )
    .await;
    let phone = Phone::new().await;
    let (_, code) = host.code().await;
    let (_, access, _) = phone.redeem(&code, &host.relay, now()).await;
    let device = phone.device(access.unwrap());
    let link = phone.link(&host, &device).await.unwrap();
    let store = host.temp.path().join("access");
    let size = || {
        std::fs::read_dir(&store)
            .unwrap()
            .filter_map(|entry| entry.ok()?.metadata().ok())
            .map(|meta| meta.len())
            .sum::<u64>()
    };
    read(&link, &thread).await.unwrap();
    let before = size();
    for _ in 0..40 {
        let page = read(&link, &thread).await.unwrap();
        assert_eq!(page.title, "New chat");
        link.call(Operation::ListThreads {}).await.unwrap();
    }
    assert!(
        size() <= before + 4096,
        "reads grew the access store from {before} to {} bytes",
        size()
    );
    host.running.shutdown().await;
}

async fn stop(link: &Link, thread: &str, request: Option<String>) -> Result<Outcome> {
    link.call(Operation::StopThread {
        thread: thread.to_owned(),
        request,
    })
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_phone_stops_a_host_threads_streaming_reply_and_the_owner_reads_it_stopped() {
    let worker = support::key();
    let (door, payloads) = chat_worker::start(worker).await;
    let host = host_with(Options {
        chat_door: Some(ChatDoor {
            relay: door,
            worker: chat_worker::worker_key(&worker),
        }),
        ..Options::default()
    })
    .await;
    let thread = "7d".repeat(16);
    chat(
        &host,
        Command::Create {
            chat: thread.clone(),
        },
    )
    .await;
    let phone = Phone::new().await;
    let (_, code) = host.code().await;
    let (_, access, _) = phone.redeem(&code, &host.relay, now()).await;
    let device = phone.device(access.unwrap());
    let link = phone.link(&host, &device).await.unwrap();

    // A stop naming a message the thread is not answering changes nothing:
    // this is how a phone learns the host can stop, before it shows the
    // stop control.
    let probe = stop(&link, &thread, Some("99".repeat(16))).await.unwrap();
    assert!(matches!(probe, Outcome::Dispatched { .. }));
    assert_eq!(read(&link, &thread).await.unwrap().total, 0);

    // The phone asks for a long reply and stops it while it streams.
    let send = "3e".repeat(16);
    link.call(Operation::SendThread {
        thread: thread.clone(),
        request: send.clone(),
        text: "Tell me slowly about rain".into(),
    })
    .await
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let page = read(&link, &thread).await.unwrap();
        if page.busy && page.partial.starts_with("Rain on the roof") {
            break;
        }
        assert!(Instant::now() < deadline, "the reply never streamed");
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
    // A stop for another message, as from a phone that read the thread
    // before this message, leaves the reply streaming.
    stop(&link, &thread, Some("99".repeat(16))).await.unwrap();
    assert!(read(&link, &thread).await.unwrap().busy);
    let Outcome::Dispatched { receipt } = stop(&link, &thread, Some(send.clone())).await.unwrap()
    else {
        panic!("a receipt")
    };
    assert_eq!(
        (receipt.operation.as_str(), receipt.reference.as_str()),
        ("thread.stop", thread.as_str())
    );

    // The partial is kept as a stopped reply, and the worker's later
    // partials do not reach it.
    let page = read(&link, &thread).await.unwrap();
    assert!(!page.busy);
    assert_eq!(page.total, 2);
    let reply = page.turns.last().unwrap();
    assert_eq!(reply.role, ThreadRole::Assistant);
    assert!(reply.stopped, "the reply carries its stopped marker");
    assert!(reply.text.starts_with("Rain on the roof"), "{}", reply.text);
    assert!(!reply.text.contains("(turns:"), "the result never arrived");
    tokio::time::sleep(Duration::from_millis(800)).await;
    let later = read(&link, &thread).await.unwrap();
    assert_eq!(later.turns, page.turns);
    assert!(!later.busy);

    // Stopping again, under a new request or as an exact retry, changes
    // nothing.
    stop(&link, &thread, Some(send.clone())).await.unwrap();
    assert_eq!(read(&link, &thread).await.unwrap().turns, page.turns);

    // The owner reads it stopped, as the desktop and `openagents chat
    // read` do over the socket.
    let snapshot = chat(
        &host,
        Command::Read {
            chat: thread.clone(),
            before: None,
        },
    )
    .await;
    assert!(!snapshot.busy);
    let last = snapshot.turns.last().unwrap();
    assert!(last.stopped);
    assert_eq!(last.text, reply.text);

    // The thread is not stuck: a new message is answered in full.
    link.call(Operation::SendThread {
        thread: thread.clone(),
        request: "4e".repeat(16),
        text: "And in the snow?".into(),
    })
    .await
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let page = read(&link, &thread).await.unwrap();
        if !page.busy && page.total == 4 {
            assert!(!page.turns[3].stopped);
            assert_eq!(page.turns[3].text, "Rain on the roof. (turns: 3)");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the next message was never answered"
        );
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
    assert_eq!(payloads.lock().unwrap().len(), 2);

    // A device that may only observe is refused the stop, and an unknown
    // thread is unavailable.
    let (_watcher, watching) = observer(&host).await;
    let refused = stop(&watching, &thread, None).await.unwrap_err();
    assert_eq!(
        refusal(&refused),
        Some((Code::MissingRight, Some(Right::Operate)))
    );
    let unknown = stop(&link, &"5b".repeat(16), None).await.unwrap_err();
    assert_eq!(refusal(&unknown), Some((Code::Unavailable, None)));
    host.running.shutdown().await;
}

/// A task owner that records each creation once per idempotency key.
struct Recorder(std::sync::Mutex<Vec<(String, coder_host::TaskRef, coder_host::TaskCreate)>>);

impl Recorder {
    fn created(&self) -> Vec<(coder_host::TaskRef, coder_host::TaskCreate)> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .map(|(_, task, input)| (task.clone(), input.clone()))
            .collect()
    }
}

impl coder_host::Tasks for Recorder {
    fn create(
        &self,
        key: &str,
        _: &str,
        task: &coder_host::TaskCreate,
    ) -> std::result::Result<coder_host::TaskRef, Code> {
        let mut tasks = self.0.lock().unwrap();
        if let Some((_, found, _)) = tasks.iter().find(|(held, _, _)| held == key) {
            return Ok(found.clone());
        }
        let found = coder_host::TaskRef {
            task: coder_host::reach::new_id(),
            revision: 1,
            phase: nostr::activity_summary::Phase::Queued,
        };
        tasks.push((key.to_owned(), found.clone(), task.clone()));
        Ok(found)
    }

    fn steer(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: u64,
        _: &str,
    ) -> std::result::Result<coder_host::TaskRef, Code> {
        Err(Code::Unavailable)
    }

    fn cancel(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: u64,
        _: &str,
    ) -> std::result::Result<coder_host::TaskRef, Code> {
        Err(Code::Unavailable)
    }
}

fn json_keys(value: &serde_json::Value, found: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                found.push(key.clone());
                json_keys(child, found);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                json_keys(item, found);
            }
        }
        _ => {}
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_phone_runs_coder_from_a_host_threads_offer() {
    let worker = support::key();
    let (door, _payloads) = chat_worker::start(worker).await;
    let recorder = std::sync::Arc::new(Recorder(std::sync::Mutex::new(vec![])));
    let host = host_with(Options {
        chat_door: Some(ChatDoor {
            relay: door,
            worker: chat_worker::worker_key(&worker),
        }),
        tasks: Some(recorder.clone()),
        ..Options::default()
    })
    .await;
    let thread = "8e".repeat(16);
    chat(
        &host,
        Command::Create {
            chat: thread.clone(),
        },
    )
    .await;
    chat(
        &host,
        Command::Send {
            chat: thread.clone(),
            request: "55".repeat(16),
            text: "offer coder a haiku".into(),
        },
    )
    .await;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let snapshot = chat(
            &host,
            Command::Read {
                chat: thread.clone(),
                before: None,
            },
        )
        .await;
        if !snapshot.busy && snapshot.turns.len() == 2 {
            break;
        }
        assert!(Instant::now() < deadline, "the worker never answered");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let phone = Phone::new().await;
    let (_, code) = host.code().await;
    let (_, access, _) = phone.redeem(&code, &host.relay, now()).await;
    let device = phone.device(access.unwrap());
    let link = phone.link(&host, &device).await.unwrap();
    let page = read(&link, &thread).await.unwrap();
    assert!(page.coder.is_none());
    assert!(page.turns[0].extras.is_empty());
    let reply = &page.turns[1];
    assert_eq!(reply.text, "Rain on the roof. (turns: 1)");
    assert_eq!(
        openagents_chat::router::Offer::parse(&reply.extras.offers[0]),
        Some(openagents_chat::router::Offer::RunCoder)
    );
    assert_eq!(reply.extras.followups[0].label, "Say it shorter");
    assert_eq!(reply.extras.cards[0]["card"], "news");
    let encoded = serde_json::to_value(reply).unwrap();
    let mut keys = vec![];
    json_keys(&encoded, &mut keys);
    for kept in ["judgment", "tier", "route", "bank"] {
        assert!(!keys.iter().any(|key| key == kept), "{keys:?}");
    }

    let Outcome::Dispatched { receipt } = link
        .call(Operation::RunThread {
            thread: thread.clone(),
        })
        .await
        .unwrap()
    else {
        panic!("a receipt")
    };
    assert_eq!(receipt.operation.as_str(), "thread.run");
    let created = recorder.created();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].0.task, receipt.reference);
    assert_eq!(created[0].1.workspace, "checkout");
    assert!(
        created[0].1.prompt.contains("offer coder a haiku"),
        "{}",
        created[0].1.prompt
    );
    let started = read(&link, &thread).await.unwrap();
    let coder = started.coder.as_ref().expect("the thread names its task");
    assert_eq!(coder.task, receipt.reference);
    assert_eq!(coder.project.as_deref(), Some("checkout"));
    let snapshot = chat(
        &host,
        Command::Read {
            chat: thread.clone(),
            before: None,
        },
    )
    .await;
    assert_eq!(
        snapshot.coder.as_ref().map(|coder| coder.task.as_str()),
        Some(receipt.reference.as_str())
    );

    let Outcome::Dispatched { receipt: again } = link
        .call(Operation::RunThread {
            thread: thread.clone(),
        })
        .await
        .unwrap()
    else {
        panic!("a receipt")
    };
    assert_eq!(again.reference, receipt.reference);
    assert_eq!(recorder.created().len(), 1);

    let (_watcher, watching) = observer(&host).await;
    let refused = watching
        .call(Operation::RunThread {
            thread: thread.clone(),
        })
        .await
        .unwrap_err();
    assert_eq!(
        refusal(&refused),
        Some((Code::MissingRight, Some(Right::Operate)))
    );
    host.running.shutdown().await;
}
