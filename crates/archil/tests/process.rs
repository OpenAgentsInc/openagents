mod common;

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use archil::{
    Error, ProcessConnectOptions, ProcessStartOptions, ProcessStatus, ProcessStream, Sandbox,
    Terminal,
};
use common::ws::*;
use common::*;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::time::timeout;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer};

const SID: &str = "11111111-1111-1111-1111-111111111111";

async fn sandbox(
    server: &MockServer,
    ws_urls: Vec<String>,
) -> (Sandbox, Arc<std::sync::atomic::AtomicUsize>) {
    Mock::given(method("GET"))
        .and(path(format!("/api/sandboxes/{SID}")))
        .respond_with(ok(sandbox_json(SID, "running")))
        .mount(server)
        .await;
    let hits = mount_connections(server, SID, ws_urls).await;
    (client(server).sandboxes().get(SID).await.unwrap(), hits)
}

#[tokio::test]
async fn exec_collects_both_streams_and_the_exit_code() {
    let server = MockServer::start().await;
    let url = runtime(|mut ws, _| async move {
        let start = recv_json(&mut ws).await;
        assert_eq!(
            start,
            json!({"type": "start", "command": "echo hi", "env": {}})
        );
        started(&mut ws, "p1").await;
        send_out(&mut ws, 1, 0, b"hello ").await;
        send_out(&mut ws, 2, 6, b"err").await;
        send_out(&mut ws, 1, 9, b"world\n").await;
        exit(&mut ws, "completed", 15, Some(0)).await;
        let _ = ws.next().await;
    })
    .await;
    let (sb, _) = sandbox(&server, vec![url]).await;
    let r = sb
        .exec("echo hi", ProcessStartOptions::new())
        .await
        .unwrap();
    assert_eq!(r.status, ProcessStatus::Completed);
    assert_eq!(r.exit_code, Some(0));
    assert_eq!(
        (r.stdout.as_str(), r.stderr.as_str()),
        ("hello world\n", "err")
    );
    assert_eq!(r.exit_reason, None);
}

#[tokio::test]
async fn start_options_are_sent_on_the_wire() {
    let server = MockServer::start().await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    let url = runtime(move |mut ws, _| {
        let sink = sink.clone();
        async move {
            let start = recv_json(&mut ws).await;
            sink.lock().unwrap().push(start);
            started(&mut ws, "p").await;
            exit(&mut ws, "completed", 0, Some(0)).await;
            let _ = ws.next().await;
        }
    })
    .await;
    let (sb, _) = sandbox(&server, vec![url]).await;
    sb.exec(
        "make",
        ProcessStartOptions::new()
            .cwd("/work")
            .env("A", "1")
            .timeout_seconds(30)
            .terminal(Terminal::size(120, 40)),
    )
    .await
    .unwrap();
    sb.exec(
        "top",
        ProcessStartOptions::new().terminal(Terminal::Enabled(true)),
    )
    .await
    .unwrap();
    sb.exec(
        "vim",
        ProcessStartOptions::new().terminal(Terminal::Size {
            cols: None,
            rows: Some(10),
        }),
    )
    .await
    .unwrap();
    let seen = seen.lock().unwrap().clone();
    assert_eq!(
        seen[0],
        json!({"type": "start", "command": "make", "cwd": "/work", "env": {"A": "1"}, "timeout_seconds": 30,
               "terminal": {"cols": 120, "rows": 40}})
    );
    assert_eq!(seen[1]["terminal"], json!(true));
    assert_eq!(
        seen[2]["terminal"],
        json!({"cols": 80, "rows": 10}),
        "unset dimensions default to 80x24"
    );
}

#[tokio::test]
async fn output_streams_through_callback_and_next_output() {
    let server = MockServer::start().await;
    let url = runtime(|mut ws, _| async move {
        let _ = recv_json(&mut ws).await;
        started(&mut ws, "p1").await;
        send_out(&mut ws, 1, 0, b"one\n").await;
        send_out(&mut ws, 2, 4, b"warn\n").await;
        send_out(&mut ws, 1, 9, b"two\n").await;
        exit(&mut ws, "completed", 13, Some(0)).await;
        let _ = ws.next().await;
    })
    .await;
    let (sb, _) = sandbox(&server, vec![url]).await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    let mut p = sb
        .run(
            "x",
            ProcessStartOptions::new()
                .stream_output(true)
                .collect_output(false)
                .on_output(move |o| {
                    sink.lock()
                        .unwrap()
                        .push((o.stream, o.offset, o.data.to_vec()))
                }),
        )
        .await
        .unwrap();
    assert_eq!(p.id(), "p1");
    let mut pulled = vec![];
    while let Some(o) = timeout(Duration::from_secs(5), p.next_output())
        .await
        .unwrap()
    {
        pulled.push((o.stream, o.offset, o.data.to_vec()));
    }
    let expected = vec![
        (ProcessStream::Stdout, 0, b"one\n".to_vec()),
        (ProcessStream::Stderr, 4, b"warn\n".to_vec()),
        (ProcessStream::Stdout, 9, b"two\n".to_vec()),
    ];
    assert_eq!(pulled, expected);
    assert_eq!(*seen.lock().unwrap(), expected);
    let r = p.wait().await.unwrap();
    assert_eq!(
        (r.stdout.as_str(), r.stderr.as_str()),
        ("", ""),
        "collect_output(false) retains nothing"
    );
    assert_eq!(p.cursor(), 13);
}

#[tokio::test]
async fn multibyte_characters_split_across_frames_decode_correctly() {
    let server = MockServer::start().await;
    let url = runtime(|mut ws, _| async move {
        let _ = recv_json(&mut ws).await;
        started(&mut ws, "p").await;
        let euro = "€".as_bytes();
        send_out(&mut ws, 1, 0, &euro[..1]).await;
        send_out(&mut ws, 1, 1, &euro[1..]).await;
        send_out(&mut ws, 1, 3, "ok".as_bytes()).await;
        exit(&mut ws, "completed", 5, Some(0)).await;
        let _ = ws.next().await;
    })
    .await;
    let (sb, _) = sandbox(&server, vec![url]).await;
    assert_eq!(
        sb.exec("x", ProcessStartOptions::new())
            .await
            .unwrap()
            .stdout,
        "€ok"
    );
}

#[tokio::test]
async fn replayed_and_overlapping_frames_are_deduplicated_by_offset() {
    let server = MockServer::start().await;
    let url = runtime(|mut ws, _| async move {
        let _ = recv_json(&mut ws).await;
        started(&mut ws, "p").await;
        send_out(&mut ws, 1, 0, b"abc").await;
        send_out(&mut ws, 1, 0, b"abcdef").await; // overlaps: only "def" is new
        send_out(&mut ws, 1, 3, b"def").await; // entirely old: ignored
        exit(&mut ws, "completed", 6, Some(0)).await;
        let _ = ws.next().await;
    })
    .await;
    let (sb, _) = sandbox(&server, vec![url]).await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    let r = sb
        .exec(
            "x",
            ProcessStartOptions::new()
                .on_output(move |o| sink.lock().unwrap().push((o.offset, o.data.to_vec()))),
        )
        .await
        .unwrap();
    assert_eq!(r.stdout, "abcdef");
    assert_eq!(
        *seen.lock().unwrap(),
        vec![(0, b"abc".to_vec()), (3, b"def".to_vec())]
    );
}

#[tokio::test]
async fn attach_resumes_from_a_cursor() {
    let server = MockServer::start().await;
    let url = runtime(|mut ws, _| async move {
        let attach = recv_json(&mut ws).await;
        assert_eq!(
            attach,
            json!({"type": "attach", "process_id": "p9", "offset": 4})
        );
        send_json(&mut ws, json!({"type": "attached", "process_id": "p9"})).await;
        send_out(&mut ws, 1, 0, b"old!new").await; // first 4 bytes were already seen
        exit(&mut ws, "completed", 7, Some(3)).await;
        let _ = ws.next().await;
    })
    .await;
    let (sb, _) = sandbox(&server, vec![url]).await;
    let p = sb
        .attach("p9", ProcessConnectOptions::new().offset(4))
        .await
        .unwrap();
    assert_eq!(p.cursor(), 4);
    let r = p.wait().await.unwrap();
    assert_eq!(r.stdout, "new");
    assert_eq!(r.exit_code, Some(3));
    assert_eq!(p.status(), ProcessStatus::Completed);
}

#[tokio::test]
async fn handshake_failures_are_reported() {
    let server = MockServer::start().await;
    let url = runtime(|mut ws, i| async move {
        let _ = recv_json(&mut ws).await;
        match i {
            0 => {
                send_json(
                    &mut ws,
                    json!({"type": "error", "error": "quota", "message": "too many execs"}),
                )
                .await
            }
            1 => send_json(&mut ws, json!({"type": "attached", "process_id": "p"})).await,
            2 => {
                send_json(
                    &mut ws,
                    json!({"type": "started", "process_id": "different"}),
                )
                .await
            }
            3 => {
                // Closes without ever saying "started".
                let _ = ws.close(None).await;
            }
            _ => {}
        }
        let _ = ws.next().await;
    })
    .await;
    let (sb, _) = sandbox(&server, vec![url]).await;
    let e = sb.run("x", ProcessStartOptions::new()).await.unwrap_err();
    assert!(matches!(e, Error::Process(_)));
    assert_eq!(e.to_string(), "quota: too many execs");
    let e = sb.run("x", ProcessStartOptions::new()).await.unwrap_err();
    assert_eq!(e.to_string(), "Expected started, received attached");
    let e = sb
        .attach("wanted", ProcessConnectOptions::new())
        .await
        .unwrap_err();
    assert_eq!(e.to_string(), "Expected attached, received started");
    let e = sb.run("x", ProcessStartOptions::new()).await.unwrap_err();
    assert_eq!(e.to_string(), "Process connection closed before ready");
}

#[tokio::test]
async fn runtime_must_echo_the_attached_process_id() {
    let server = MockServer::start().await;
    let url = runtime(|mut ws, _| async move {
        let _ = recv_json(&mut ws).await;
        send_json(
            &mut ws,
            json!({"type": "attached", "process_id": "someone-else"}),
        )
        .await;
        let _ = ws.next().await;
    })
    .await;
    let (sb, _) = sandbox(&server, vec![url]).await;
    let e = sb
        .attach("mine", ProcessConnectOptions::new())
        .await
        .unwrap_err();
    assert_eq!(e.to_string(), "Runtime returned a different process ID");
}

#[tokio::test]
async fn malformed_output_frame_fails_the_connection() {
    let server = MockServer::start().await;
    let url = runtime(|mut ws, _| async move {
        let _ = recv_json(&mut ws).await;
        started(&mut ws, "p").await;
        ws.send(tokio_tungstenite::tungstenite::Message::Binary(
            vec![9u8, 0, 0].into(),
        ))
        .await
        .unwrap();
        let _ = ws.next().await;
    })
    .await;
    let (sb, _) = sandbox(&server, vec![url]).await;
    let p = sb.run("x", ProcessStartOptions::new()).await.unwrap();
    let e = timeout(Duration::from_secs(5), p.wait())
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(e.to_string(), "Invalid process output frame");
}

#[tokio::test]
async fn non_success_exit_statuses_carry_reason_and_missing_code() {
    let server = MockServer::start().await;
    let url = runtime(|mut ws, i| async move {
        let _ = recv_json(&mut ws).await;
        started(&mut ws, "p").await;
        let (status, reason) = [("timed_out", "deadline exceeded"), ("cancelled", "killed"), ("failed", "oom")][i % 3];
        send_json(&mut ws, json!({"type": "exit", "status": status, "cursor": 0, "exit_code": null, "exit_reason": reason})).await;
        let _ = ws.next().await;
    })
    .await;
    let (sb, _) = sandbox(&server, vec![url]).await;
    let mut seen = vec![];
    for _ in 0..3 {
        let r = sb
            .exec("x", ProcessStartOptions::new().timeout_seconds(1))
            .await
            .unwrap();
        seen.push((r.status, r.exit_code, r.exit_reason.unwrap()));
    }
    assert_eq!(
        seen,
        [
            (
                ProcessStatus::TimedOut,
                None,
                "deadline exceeded".to_string()
            ),
            (ProcessStatus::Cancelled, None, "killed".to_string()),
            (ProcessStatus::Failed, None, "oom".to_string()),
        ]
    );
}

#[tokio::test]
async fn stdin_is_chunked_and_closable() {
    let server = MockServer::start().await;
    let received = Arc::new(Mutex::new(Vec::new()));
    let sink = received.clone();
    let url = runtime(move |mut ws, _| {
        let sink = sink.clone();
        async move {
            let _ = recv_json(&mut ws).await;
            started(&mut ws, "p").await;
            let mut sizes = vec![];
            while sizes.iter().sum::<usize>() < 2 * 1024 * 1024 + 512 * 1024 {
                sizes.push(recv_bin(&mut ws).await.len());
            }
            let close = recv_json(&mut ws).await;
            *sink.lock().unwrap() = vec![json!(sizes), close];
            exit(&mut ws, "completed", 0, Some(0)).await;
            let _ = ws.next().await;
        }
    })
    .await;
    let (sb, _) = sandbox(&server, vec![url]).await;
    let p = sb.run("cat", ProcessStartOptions::new()).await.unwrap();
    p.send_input(vec![1u8; 2 * 1024 * 1024 + 512 * 1024])
        .await
        .unwrap();
    p.close_stdin().await.unwrap();
    p.close_stdin().await.unwrap(); // idempotent
    let e = p.send_input("more").await.unwrap_err();
    assert_eq!(e.to_string(), "Process stdin is closed");
    p.wait().await.unwrap();
    let got = received.lock().unwrap().clone();
    assert_eq!(got[0], json!([1048576, 1048576, 524288]));
    assert_eq!(got[1], json!({"type": "close_stdin"}));
    // After exit stdin is closed for good.
    assert_eq!(
        p.send_input("x").await.unwrap_err().to_string(),
        "Process stdin is closed"
    );
}

#[tokio::test]
async fn disconnect_leaves_the_process_running_but_unusable() {
    let server = MockServer::start().await;
    let closed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = closed.clone();
    let url = runtime(move |mut ws, _| {
        let flag = flag.clone();
        async move {
            let _ = recv_json(&mut ws).await;
            started(&mut ws, "p").await;
            send_out(&mut ws, 1, 0, b"tick\n").await;
            while let Some(Ok(m)) = ws.next().await {
                if m.is_close() {
                    break;
                }
            }
            flag.store(true, Ordering::SeqCst);
        }
    })
    .await;
    let (sb, _) = sandbox(&server, vec![url]).await;
    let p = sb
        .run("sleep 999", ProcessStartOptions::new())
        .await
        .unwrap();
    assert!(p.connected());
    timeout(Duration::from_secs(5), p.disconnect())
        .await
        .unwrap();
    assert!(!p.connected());
    assert_eq!(
        p.status(),
        ProcessStatus::Running,
        "disconnect does not terminate the process"
    );
    assert_eq!(
        p.send_input("x").await.unwrap_err().to_string(),
        "Process is disconnected"
    );
    assert_eq!(
        p.wait().await.unwrap_err().to_string(),
        "Process connection closed before exit"
    );
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(closed.load(Ordering::SeqCst), "server saw the close");
    p.disconnect().await; // idempotent
}

#[tokio::test]
async fn wait_fails_when_the_connection_drops_before_exit() {
    let server = MockServer::start().await;
    let url = runtime(|mut ws, _| async move {
        let _ = recv_json(&mut ws).await;
        started(&mut ws, "p").await;
        send_out(&mut ws, 1, 0, b"partial").await;
        let _ = ws.close(None).await;
    })
    .await;
    let (sb, _) = sandbox(&server, vec![url]).await;
    let p = sb.run("x", ProcessStartOptions::new()).await.unwrap();
    let e = timeout(Duration::from_secs(5), p.wait())
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(e.to_string(), "Process connection closed before exit");
    assert_eq!(p.stdout(), "partial");
}

#[tokio::test]
async fn kill_and_resize_use_a_fresh_control_connection() {
    let server = MockServer::start().await;
    let controls = Arc::new(Mutex::new(Vec::new()));
    let sink = controls.clone();
    let url = runtime(move |mut ws, i| {
        let sink = sink.clone();
        async move {
            let req = recv_json(&mut ws).await;
            if i == 0 {
                assert_eq!(req["type"], "start");
                started(&mut ws, "p1").await;
                let _ = ws.next().await;
                return;
            }
            sink.lock().unwrap().push(req.clone());
            match req["type"].as_str().unwrap() {
                "kill" => send_json(&mut ws, json!({"type": "killed"})).await,
                "resize" if req["cols"] == 1 => {
                    send_json(
                        &mut ws,
                        json!({"type": "error", "error": "not_pty", "message": "no terminal"}),
                    )
                    .await
                }
                "resize" => send_json(&mut ws, json!({"type": "resized"})).await,
                _ => unreachable!(),
            }
            let _ = ws.next().await;
        }
    })
    .await;
    let (sb, conns) = sandbox(&server, vec![url]).await;
    let p = sb
        .run(
            "vim",
            ProcessStartOptions::new().terminal(Terminal::Enabled(true)),
        )
        .await
        .unwrap();
    assert_eq!(conns.load(Ordering::SeqCst), 1);
    p.resize(120, 40).await.unwrap();
    let e = p.resize(1, 1).await.unwrap_err();
    assert_eq!(e.to_string(), "not_pty: no terminal");
    p.kill().await.unwrap();
    assert_eq!(
        conns.load(Ordering::SeqCst),
        4,
        "each control request opens its own connection"
    );
    assert_eq!(
        *controls.lock().unwrap(),
        [
            json!({"type": "resize", "process_id": "p1", "cols": 120, "rows": 40}),
            json!({"type": "resize", "process_id": "p1", "cols": 1, "rows": 1}),
            json!({"type": "kill", "process_id": "p1"}),
        ]
    );
}

#[tokio::test]
async fn kill_after_exit_is_a_noop_and_resize_after_exit_fails() {
    let server = MockServer::start().await;
    let url = runtime(|mut ws, _| async move {
        let _ = recv_json(&mut ws).await;
        started(&mut ws, "p").await;
        exit(&mut ws, "completed", 0, Some(0)).await;
        let _ = ws.next().await;
    })
    .await;
    let (sb, conns) = sandbox(&server, vec![url]).await;
    let p = sb.run("x", ProcessStartOptions::new()).await.unwrap();
    p.wait().await.unwrap();
    p.kill().await.unwrap();
    assert_eq!(conns.load(Ordering::SeqCst), 1);
    assert_eq!(
        p.resize(80, 24).await.unwrap_err().to_string(),
        "Process has exited"
    );
}

#[tokio::test]
async fn a_control_reply_of_the_wrong_type_is_an_error() {
    let server = MockServer::start().await;
    let url = runtime(|mut ws, i| async move {
        let _ = recv_json(&mut ws).await;
        if i == 0 {
            started(&mut ws, "p").await;
        } else {
            send_json(&mut ws, json!({"type": "resized"})).await;
        }
        let _ = ws.next().await;
    })
    .await;
    let (sb, _) = sandbox(&server, vec![url]).await;
    let p = sb.run("x", ProcessStartOptions::new()).await.unwrap();
    assert_eq!(
        p.kill().await.unwrap_err().to_string(),
        "Expected killed, received resized"
    );
}

#[tokio::test]
async fn connection_attempts_fetch_a_fresh_url_and_retry_with_backoff() {
    let server = MockServer::start().await;
    // A port nothing listens on, then a working runtime.
    let dead = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let dead_url = format!("ws://{}/ws", dead.local_addr().unwrap());
    drop(dead);
    let good = runtime(|mut ws, _| async move {
        let _ = recv_json(&mut ws).await;
        started(&mut ws, "p").await;
        exit(&mut ws, "completed", 0, Some(0)).await;
        let _ = ws.next().await;
    })
    .await;
    let (sb, conns) = sandbox(&server, vec![dead_url.clone(), dead_url, good]).await;
    let r = sb.exec("x", ProcessStartOptions::new()).await.unwrap();
    assert_eq!(r.status, ProcessStatus::Completed);
    assert_eq!(
        conns.load(Ordering::SeqCst),
        3,
        "one signed URL per attempt"
    );
}

#[tokio::test]
async fn connection_gives_up_after_four_attempts() {
    let server = MockServer::start().await;
    let dead = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let dead_url = format!("ws://{}/ws", dead.local_addr().unwrap());
    drop(dead);
    let (sb, conns) = sandbox(&server, vec![dead_url]).await;
    let e = sb.run("x", ProcessStartOptions::new()).await.unwrap_err();
    assert!(
        e.to_string()
            .starts_with("Process connection failed after 4 attempts"),
        "{e}"
    );
    assert_eq!(conns.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn connection_url_errors_are_not_swallowed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/sandboxes/{SID}")))
        .respond_with(ok(sandbox_json(SID, "stopped")))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/api/sandboxes/{SID}/connections")))
        .respond_with(err(
            409,
            "sandbox is not running",
            Some("sandbox_not_running"),
        ))
        .mount(&server)
        .await;
    let sb = client(&server).sandboxes().get(SID).await.unwrap();
    let e = sb.run("x", ProcessStartOptions::new()).await.unwrap_err();
    assert_eq!(
        (e.status(), e.code()),
        (Some(409), Some("sandbox_not_running"))
    );
}
