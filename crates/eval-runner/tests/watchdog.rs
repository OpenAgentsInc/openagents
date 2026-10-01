//! Under systemd's watchdog, each answered liveness probe tells systemd the
//! runner is live (`WATCHDOG=1`), so a runner stuck anywhere the probes
//! can't see is restarted by the unit's `WatchdogSec`.
//!
//! This is its own test binary because it sets `NOTIFY_SOCKET` for the
//! whole process, before any thread starts.

#![cfg(unix)]

mod support;

use std::sync::Arc;
use std::time::Duration;

use coder::relay::Identity;
use coder::relay::liveness::Liveness;
use eval_runner::config::Limits;
use futures_util::{SinkExt as _, StreamExt as _};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite};

#[test]
fn an_answered_probe_pets_the_systemd_watchdog() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("notify");
    // SAFETY: no other thread exists yet in this test binary: the test
    // harness runs its only test on this thread, and the runtime starts
    // below.
    unsafe { std::env::set_var("NOTIFY_SOCKET", &path) };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime
        .block_on(async {
            tokio::time::timeout(Duration::from_secs(60), async {
                let notify = tokio::net::UnixDatagram::bind(&path).unwrap();
                let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
                let url = format!("ws://{}", listener.local_addr().unwrap());
                let dir = tempfile::tempdir().unwrap();
                let limits = Limits {
                    runs_per_trainer: None,
                    turns_per_day: None,
                    jobs: 1,
                    concurrency: 1,
                };
                let (runner, _memory, key) =
                    support::memory_runner(dir.path(), &support::door(), limits);
                let identity =
                    Arc::new(Identity::from_text(&support::hex(&key), "runner").unwrap());
                let liveness = Liveness {
                    probe: Duration::from_millis(200),
                    renew: Duration::from_secs(3_600),
                };
                tokio::spawn(async move {
                    let _ = eval_runner::runner::listen(&url, &identity, &runner, liveness).await;
                });
                let (tcp, _) = listener.accept().await.unwrap();
                let mut socket = accept_async(tcp).await.unwrap();
                let text = |value: Value| tungstenite::Message::Text(value.to_string().into());
                socket.send(text(json!(["AUTH", "c"]))).await.unwrap();
                // A relay that answers the subscription and every probe.
                tokio::spawn(async move {
                    while let Some(Ok(frame)) = socket.next().await {
                        let tungstenite::Message::Text(frame) = frame else {
                            continue;
                        };
                        let value: Value = serde_json::from_str(&frame).unwrap();
                        let reply = match value[0].as_str() {
                            Some("AUTH") => json!(["OK", value[1]["id"], true, ""]),
                            Some("REQ") => json!(["EOSE", value[1]]),
                            _ => continue,
                        };
                        socket.send(text(reply)).await.unwrap();
                    }
                });
                let mut buffer = [0u8; 64];
                // The subscription's EOSE and then two answered probes.
                for _ in 0..3 {
                    let length = notify.recv(&mut buffer).await.unwrap();
                    assert_eq!(&buffer[..length], b"WATCHDOG=1");
                }
            })
            .await
        })
        .expect("the test finishes inside its bound");
}
