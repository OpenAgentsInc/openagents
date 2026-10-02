// Following detached output from a byte cursor, killing a process tree, and
// streaming into an async sink.
mod support;

use std::time::Duration;

use base64::Engine;
use boat::{CommandFrame, Error, OutputCursor, Signal, WaitOptions, models::*, shell_quote};
use support::{Reply, serve_sequence};

const LOGS: &str =
    r#""logPath":"~/.ascii/processes/70.log","errLogPath":"~/.ascii/processes/70.err.log""#;

fn status(running: bool, exit: &str, lost: bool) -> Vec<u8> {
    let state = if lost {
        "lost"
    } else if running {
        "running"
    } else {
        "exited"
    };
    format!(
        r#"{{"ok":true,"type":"command.status","success":true,"processId":7,"pid":70,"status":"{state}","running":{running},"exitCode":{exit},"stdout":"","stderr":"",{LOGS}}}"#
    )
    .into_bytes()
}

fn read(stdout: &[u8], stderr: &[u8]) -> Vec<u8> {
    let b64 = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
    let text = format!("{}\n{}\n", b64(stdout), b64(stderr));
    serde_json::to_vec(&serde_json::json!({
        "ok": true, "type": "command.finished", "success": true, "exitCode": 0,
        "stdout": text, "stderr": "", "timedOut": false
    }))
    .expect("json")
}

fn fast() -> WaitOptions {
    WaitOptions {
        interval: Duration::from_millis(1),
        timeout: Duration::from_secs(10),
        ..Default::default()
    }
}

fn sent_command(request: &support::Request) -> String {
    let body: serde_json::Value = serde_json::from_slice(&request.body).expect("body");
    body["command"].as_str().expect("command").to_owned()
}

#[tokio::test]
async fn follower_delivers_output_once_in_order_then_the_exit() {
    let (client, job) = serve_sequence(
        vec![
            Reply::new(200, &[], &status(true, "null", false)),
            Reply::new(200, &[], &read(b"hello ", b"warn")),
            Reply::new(200, &[], &status(false, "3", false)),
            Reply::new(200, &[], &read(b"world\n", b"")),
        ],
        |b| b,
    )
    .await;
    let mut follower = client
        .follow_command("bx_23456789", 7, fast())
        .expect("follower");
    let mut frames = Vec::new();
    while let Some(frame) = follower.next().await.expect("frame") {
        frames.push(frame);
    }
    assert!(matches!(&frames[0], CommandFrame::Stdout(t) if t == "hello "));
    assert!(matches!(&frames[1], CommandFrame::Stderr(t) if t == "warn"));
    assert!(matches!(&frames[2], CommandFrame::Stdout(t) if t == "world\n"));
    assert!(matches!(
        frames[3],
        CommandFrame::Exit {
            exit_code: Some(3),
            success: false,
            ..
        }
    ));
    assert_eq!(frames.len(), 4);
    assert_eq!(
        follower.cursor(),
        OutputCursor {
            stdout: 12,
            stderr: 4
        }
    );
    let seen = job.await.expect("job");
    assert_eq!(
        seen[0].target,
        "/api/v1/sandboxes/bx_23456789/commands/7?tailBytes=1"
    );
    let first = sent_command(&seen[1]);
    assert!(
        first.contains(
            r#"r "$HOME"/'.ascii/processes/70.log' 1; r "$HOME"/'.ascii/processes/70.err.log' 1"#
        ),
        "{first}"
    );
    let second = sent_command(&seen[3]);
    assert!(
        second.contains(r#"70.log' 7; r "$HOME"/'.ascii/processes/70.err.log' 5"#),
        "{second}"
    );
}

#[tokio::test]
async fn follower_resumes_from_a_saved_cursor() {
    let (client, job) = serve_sequence(
        vec![
            Reply::new(200, &[], &status(false, "0", false)),
            Reply::new(200, &[], &read(b"tail", b"")),
        ],
        |b| b,
    )
    .await;
    let output = client
        .follow_command_from(
            "bx_23456789",
            7,
            OutputCursor {
                stdout: 100,
                stderr: 20,
            },
            fast(),
        )
        .expect("follower")
        .collect()
        .await
        .expect("output");
    assert_eq!(output.stdout, "tail");
    assert_eq!(output.exit_code(), Some(0));
    let seen = job.await.expect("job");
    assert!(sent_command(&seen[1]).contains("70.log' 101;"));
    assert!(sent_command(&seen[1]).contains("70.err.log' 21"));
}

#[tokio::test]
async fn follower_keeps_a_split_character_for_the_next_read() {
    let (client, job) = serve_sequence(
        vec![
            Reply::new(200, &[], &status(true, "null", false)),
            Reply::new(200, &[], &read(b"a\xC3", b"")),
            Reply::new(200, &[], &status(false, "0", false)),
            Reply::new(200, &[], &read(b"\xC3\xA9b", b"")),
        ],
        |b| b,
    )
    .await;
    let mut follower = client
        .follow_command("bx_23456789", 7, fast())
        .expect("follower");
    let mut stdout = Vec::new();
    while let Some(frame) = follower.next().await.expect("frame") {
        if let CommandFrame::Stdout(text) = frame {
            stdout.push(text);
        }
    }
    assert_eq!(stdout, ["a", "éb"]);
    assert_eq!(follower.cursor().stdout, 4);
    let seen = job.await.expect("job");
    assert!(sent_command(&seen[3]).contains("70.log' 2;"));
}

#[tokio::test]
async fn follower_rides_out_a_transient_failure_and_reports_a_lost_process() {
    let unavailable = br#"{"ok":false,"type":"sandbox.error","status":503,"code":"unavailable","message":"m","requestId":"req_1","error":{"code":"unavailable","message":"m","status":503}}"#;
    let (client, _job) = serve_sequence(
        vec![
            Reply::new(200, &[], &status(true, "null", false)),
            Reply::new(503, &[], unavailable),
            Reply::new(200, &[], &status(false, "null", true)),
            Reply::new(200, &[], &read(b"", b"")),
        ],
        |b| b,
    )
    .await;
    let output = client
        .follow_command("bx_23456789", 7, fast())
        .expect("follower")
        .collect()
        .await
        .expect("output");
    assert!(matches!(
        output.last,
        Some(CommandFrame::Error { error: Some(ref e), retryable: false, .. }) if e == "lost"
    ));
}

#[tokio::test]
async fn follower_stops_at_cancellation_without_touching_the_command() {
    let options = fast();
    options.cancellation.cancel();
    let (client, job) = serve_sequence(vec![], |b| b).await;
    let mut follower = client
        .follow_command("bx_23456789", 7, options)
        .expect("follower");
    assert!(matches!(follower.next().await, Err(Error::Cancelled)));
    assert!(job.await.expect("job").is_empty());
}

#[tokio::test]
async fn kill_signals_the_process_tree_once() {
    let killed = br#"{"ok":true,"type":"command.finished","success":true,"exitCode":0,"stdout":"killed\n","stderr":"","timedOut":false}"#;
    let gone = br#"{"ok":true,"type":"command.finished","success":true,"exitCode":0,"stdout":"gone\n","stderr":"","timedOut":false}"#;
    let (client, job) = serve_sequence(
        vec![Reply::new(200, &[], killed), Reply::new(200, &[], gone)],
        |b| b,
    )
    .await;
    assert!(
        client
            .kill_command("bx_23456789", 70, Signal::Term)
            .await
            .expect("kill")
    );
    assert!(
        !client
            .kill_command("bx_23456789", 70, Signal::Kill)
            .await
            .expect("kill")
    );
    assert!(matches!(
        client.kill_command("bx_23456789", 1, Signal::Kill).await,
        Err(Error::Configuration(_))
    ));
    let seen = job.await.expect("job");
    assert_eq!(seen.len(), 2);
    let script = sent_command(&seen[0]);
    assert!(script.contains("pgrep -P") && script.contains("kill -s TERM $(t 70)"));
    assert!(sent_command(&seen[1]).contains("kill -s KILL"));
}

#[tokio::test]
async fn run_streaming_hands_frames_to_the_sink_and_returns_the_end() {
    let body = concat!(
        "{\"type\":\"started\"}\n",
        "{\"type\":\"stdout\",\"data\":\"a\"}\n",
        "{\"type\":\"stderr\",\"data\":\"b\"}\n",
        "{\"type\":\"exit\",\"exitCode\":0,\"success\":true,\"timedOut\":false}\n"
    );
    let (client, _job) = serve_sequence(
        vec![
            Reply::new(
                200,
                &[("content-type", "application/x-ndjson")],
                body.as_bytes(),
            ),
            Reply::new(
                200,
                &[("content-type", "application/x-ndjson")],
                b"{\"type\":\"started\"}\n",
            ),
        ],
        |b| b,
    )
    .await;
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let consumer = tokio::spawn(async move {
        let mut seen = Vec::new();
        while let Some(frame) = rx.recv().await {
            seen.push(format!("{frame:?}"));
        }
        seen
    });
    let end = client
        .run_streaming(
            "bx_23456789",
            CommandRequest {
                command: "true".into(),
                ..Default::default()
            },
            &fast(),
            |frame| {
                let tx = tx.clone();
                async move {
                    let _ = tx.send(frame).await;
                }
            },
        )
        .await
        .expect("end");
    assert!(matches!(
        end,
        CommandFrame::Exit {
            exit_code: Some(0),
            ..
        }
    ));
    let early = client
        .run_streaming(
            "bx_23456789",
            CommandRequest::default(),
            &fast(),
            |_| async {},
        )
        .await;
    assert!(matches!(early, Err(ref e) if e.may_be_running()));
    drop(tx);
    assert_eq!(
        consumer.await.expect("consumer"),
        ["Started", "Stdout(1 bytes)", "Stderr(1 bytes)"]
    );
}

#[test]
fn shell_quote_keeps_home_and_escapes_quotes() {
    assert_eq!(shell_quote("a b"), "'a b'");
    assert_eq!(shell_quote("it's"), r"'it'\''s'");
    assert_eq!(shell_quote("~/x y"), r#""$HOME"/'x y'"#);
    assert_eq!(shell_quote("$(rm)"), "'$(rm)'");
}
