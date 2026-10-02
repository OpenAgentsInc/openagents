//! The host starts again for an update only when nobody is using it
//! (2026-10-02: a rebuild restarted the host while the owner's message was
//! in flight, and the terminal said OpenAgents could not be reached).
//!
//! A client halfway through writing a request, a chat reply streaming, and
//! the moments right after a request all keep the host in use; once they
//! end, it is idle.
#![cfg(unix)]

use std::time::Duration;

use coder_host::config::ChatDoor;
use openagents_chat::service::{Command, Snapshot};
use openagents_connect::control::{Op, Reply, Request, Response};
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;

#[path = "support/connect.rs"]
mod support;

#[path = "support/chat_worker.rs"]
mod chat_worker;

use support::{Options, call, host, host_with};

/// Longer than the host's quiet window after a request (two seconds).
const PAST_QUIET: Duration = Duration::from_millis(2_600);

fn frame(request: &Request) -> Vec<u8> {
    let body = serde_json::to_vec(request).unwrap();
    let mut frame = u32::try_from(body.len()).unwrap().to_be_bytes().to_vec();
    frame.extend(body);
    frame
}

#[tokio::test]
async fn a_client_mid_request_keeps_the_host_in_use_until_it_is_answered() {
    let host = host().await;
    tokio::time::sleep(PAST_QUIET).await;
    assert!(host.running.idle(), "nobody has used it");

    // A client starts writing a request and stalls halfway, as a client
    // whose message is still on its way.
    let mut client = UnixStream::connect(&host.socket).await.unwrap();
    let bytes = frame(&Request::new(1, Op::Status {}));
    let (first, rest) = bytes.split_at(bytes.len() / 2);
    client.write_all(first).await.unwrap();
    tokio::time::sleep(PAST_QUIET).await;
    assert_eq!(host.running.in_flight(), 1);
    assert!(!host.running.idle());
    assert!(
        tokio::time::timeout(Duration::from_secs(1), host.running.until_idle())
            .await
            .is_err(),
        "a restart waiting for idle still waits"
    );

    // The rest arrives and is answered: in use a moment longer, then idle.
    client.write_all(rest).await.unwrap();
    let response: Response = openagents_connect::wire::read_message(&mut client, 1 << 20)
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(response.result, Reply::Status { .. }),
        "{response:?}"
    );
    assert_eq!(host.running.in_flight(), 0);
    assert!(!host.running.idle(), "just answered");
    tokio::time::timeout(Duration::from_secs(5), host.running.until_idle())
        .await
        .expect("idle once the quiet window passes");
    // The connection stays open, idle: an open connection is not use.
    drop(client);
}

#[tokio::test]
async fn a_client_reading_every_moment_keeps_the_host_in_use() {
    let host = host().await;
    let reading = async {
        for _ in 0..20 {
            call(&host.socket, Op::Status {}).await.unwrap();
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    };
    let watching = async {
        tokio::time::sleep(Duration::from_millis(500)).await;
        for _ in 0..10 {
            assert!(!host.running.idle(), "a client follows something");
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    };
    tokio::join!(reading, watching);
}

async fn chat(host: &support::Host, command: Command) -> Snapshot {
    match call(
        &host.socket,
        Op::Chat {
            command,
            caller: None,
        },
    )
    .await
    .unwrap()
    {
        Reply::Chat { snapshot } => snapshot,
        other => panic!("not a chat answer: {other:?}"),
    }
}

#[tokio::test]
async fn a_streaming_reply_keeps_the_host_in_use_with_nobody_reading() {
    let worker = support::key();
    let (door, _) = chat_worker::start(worker).await;
    let host = host_with(Options {
        chat_door: Some(ChatDoor {
            relay: door,
            worker: chat_worker::worker_key(&worker),
        }),
        ..Options::default()
    })
    .await;
    let thread = "5c".repeat(16);
    chat(
        &host,
        Command::Create {
            chat: thread.clone(),
        },
    )
    .await;
    let sent = chat(
        &host,
        Command::Send {
            chat: thread.clone(),
            request: "6d".repeat(16),
            text: "Tell me slowly about rain".into(),
        },
    )
    .await;
    assert!(sent.busy);

    // Nobody reads it: no request is in flight, yet the reply streams.
    tokio::time::sleep(PAST_QUIET).await;
    assert_eq!(host.running.in_flight(), 0);
    assert!(!host.running.idle(), "a reply is streaming");

    // Once it ends the host is idle, and the reply is in its thread.
    tokio::time::timeout(Duration::from_secs(30), host.running.until_idle())
        .await
        .expect("idle once the reply ends");
    let read = chat(
        &host,
        Command::Read {
            chat: thread.clone(),
            before: None,
        },
    )
    .await;
    assert!(!read.busy);
    assert!(
        read.turns
            .last()
            .is_some_and(|turn| turn.text.starts_with("Rain on the roof")),
        "{:?}",
        read.turns
    );
}

#[tokio::test]
async fn a_stop_drains_a_request_being_answered() {
    let host = host().await;
    let mut client = UnixStream::connect(&host.socket).await.unwrap();
    let bytes = frame(&Request::new(1, Op::Status {}));
    client.write_all(&bytes[..2]).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!host.running.drain(Duration::from_millis(500)).await);
    client.write_all(&bytes[2..]).await.unwrap();
    assert!(host.running.drain(Duration::from_secs(5)).await);
}

/// Starting again, the host takes no new request: an open connection ends
/// before its next one, and a new connection waits in the socket's queue
/// (for the program that serves it next) instead of being refused.
#[tokio::test]
async fn a_host_starting_again_leaves_new_requests_for_the_next_program() {
    let host = host().await;
    let mut open = UnixStream::connect(&host.socket).await.unwrap();
    let first = openagents_connect::control::call(&mut open, &Request::new(1, Op::Status {})).await;
    assert!(matches!(first, Ok(Reply::Status { .. })), "{first:?}");
    host.running.stop_taking_requests();
    let ended = tokio::time::timeout(
        Duration::from_secs(5),
        openagents_connect::control::call(&mut open, &Request::new(2, Op::Status {})),
    )
    .await
    .expect("the open connection ends");
    assert!(ended.is_err(), "{ended:?}");
    let mut queued = UnixStream::connect(&host.socket)
        .await
        .expect("the socket still takes connections");
    let waiting = tokio::time::timeout(
        Duration::from_secs(1),
        openagents_connect::control::call(&mut queued, &Request::new(3, Op::Status {})),
    )
    .await;
    assert!(waiting.is_err(), "nobody answers until the next program");
    assert!(host.running.drain(Duration::from_secs(1)).await);
}
