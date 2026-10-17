#![cfg(unix)]

use std::cell::Cell;
use std::rc::Rc;

use openagents_harness::process::LaunchBroker;
use openagents_harness::{
    CancellationToken, ClaudeHarness, CodexHarness, Harness, OpencodeHarness, RunControls,
};
use openagents_proto::RunRequest;
use tokio::sync::{mpsc, oneshot};

#[tokio::test]
async fn every_native_driver_waits_for_the_borrowed_host_wrapper() {
    for (driver, with_mcp) in [
        ("claude", false),
        ("codex", false),
        ("opencode", false),
        ("opencode", true),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("started");
        let binary = directory.path().join(driver);
        std::fs::write(
            &binary,
            format!("#!/bin/sh\ntouch '{}'\nexit 1\n", marker.display()),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let (broker, mut launches) = LaunchBroker::channel();
        let harness: Box<dyn Harness> = match driver {
            "claude" => Box::new(
                ClaudeHarness::new()
                    .with_executable(&binary)
                    .with_launch_broker(broker),
            ),
            "codex" => Box::new(
                CodexHarness::new()
                    .with_executable(&binary)
                    .with_launch_broker(broker),
            ),
            _ => Box::new(
                OpencodeHarness::new()
                    .with_executable(&binary)
                    .with_launch_broker(broker),
            ),
        };
        let mut request: RunRequest = serde_json::from_value(serde_json::json!({
            "prompt": "fixture", "model": null, "reasoning": null,
            "cwd": directory.path(), "sandbox": "read-only", "resume": null,
        }))
        .unwrap();
        if with_mcp {
            request.mcp = Some(openagents_proto::McpServer {
                name: "fixture".into(),
                command: "/bin/false".into(),
                ..Default::default()
            });
        }
        let (_steer, steering) = mpsc::channel(1);
        let controls = RunControls {
            realtime: None,
            execution_lease: None,
            request_input: Box::new(|_| {
                let (_, rx) = oneshot::channel();
                rx
            }),
            steering,
            interrupt: CancellationToken::new(),
            turn: Default::default(),
        };
        // Rc<Cell<_>> deliberately cannot enter a Send + Sync harness task.
        let calls = Rc::new(Cell::new(0));
        let wrap = |command: std::process::Command| {
            assert!(!marker.exists(), "{driver} started before host admission");
            if !with_mcp {
                assert_eq!(command.get_current_dir(), Some(directory.path()));
            }
            let first = command.get_args().next().unwrap().to_string_lossy();
            assert_eq!(
                first,
                match driver {
                    "claude" => "--print",
                    "codex" => "app-server",
                    _ if with_mcp => "--version",
                    _ => "serve",
                }
            );
            calls.set(calls.get() + 1);
            Err("fixture host refusal".into())
        };
        let run = harness.run(request, controls);
        tokio::pin!(run);
        let result = loop {
            tokio::select! {
                result = &mut run => break result,
                launch = launches.recv() => launch.expect("driver requested admission").apply(&wrap),
                _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => panic!("{driver} did not request admission"),
            }
        };
        let error = match result {
            Ok(_) => panic!("{driver} ignored the host refusal"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("fixture host refusal"),
            "{driver}: {error}"
        );
        assert_eq!(calls.get(), 1, "{driver}");
        assert!(!marker.exists(), "{driver} started after refusal");
    }
}

#[cfg(feature = "supervised")]
#[tokio::test]
async fn shared_native_protocols_run_inside_the_host_supervisor() {
    use futures::StreamExt;
    use openagents_harness::process::{SupervisedEvent, Supervision};
    use openagents_proto::{AgentEvent, DoneStatus};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    for driver in ["claude", "codex"] {
        let directory = tempfile::tempdir().unwrap();
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(format!("fake-{driver}.sh"));
        let binary = directory.path().join(driver);
        let source = std::fs::read_to_string(fixture).unwrap();
        let source = source.replace(
            "\"cwd\":\"/tmp\"",
            &format!("\"cwd\":\"{}\"", directory.path().display()),
        );
        std::fs::write(&binary, source).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let stopped = Arc::new(Mutex::new(None));
        let observed = stopped.clone();
        let (broker, mut launches) = LaunchBroker::channel();
        let broker = broker.supervised(Supervision {
            limits: supervise::Limits::within(Duration::from_secs(10)).keeping(1024 * 1024),
            observe: Arc::new(move |event| {
                if let SupervisedEvent::Stopped(result) = event {
                    *observed.lock().unwrap() = Some(result);
                }
            }),
        });
        let harness: Box<dyn Harness> = match driver {
            "claude" => Box::new(
                ClaudeHarness::new()
                    .with_executable(binary)
                    .with_launch_broker(broker),
            ),
            _ => Box::new(
                CodexHarness::new()
                    .with_executable(binary)
                    .with_launch_broker(broker),
            ),
        };
        let mut request: RunRequest = serde_json::from_value(serde_json::json!({
            "prompt": "scenario:happy", "model": null, "reasoning": null,
            "cwd": directory.path(), "sandbox": "read-only", "resume": null, "autoApprove": true,
        }))
        .unwrap();
        if driver == "codex" {
            request.model = Some("gpt-5.6-sol".into());
            request.reasoning = Some(openagents_proto::ReasoningLevel::Ultra);
            request
                .model_options
                .insert("serviceTier".into(), "fast".into());
        }
        let (_steer, steering) = mpsc::channel(1);
        let interrupt = CancellationToken::new();
        let controls = RunControls {
            realtime: None,
            execution_lease: None,
            request_input: Box::new(|_| {
                let (_, rx) = oneshot::channel();
                rx
            }),
            steering,
            interrupt: interrupt.clone(),
            turn: Default::default(),
        };
        let run = harness.run(request, controls);
        tokio::pin!(run);
        let mut stream = loop {
            tokio::select! {
                result = &mut run => break result.unwrap(),
                launch = launches.recv() => launch.unwrap().apply(&Ok),
            }
        };
        tokio::time::timeout(Duration::from_secs(15), async {
            let mut started = false;
            let mut text = false;
            let mut done = false;
            while let Some(event) = stream.next().await {
                match event.unwrap() {
                    AgentEvent::SessionStarted { .. } => started = true,
                    AgentEvent::TextDelta { .. } => text = true,
                    AgentEvent::Done {
                        status: DoneStatus::Completed,
                        ..
                    } => {
                        done = true;
                        interrupt.cancel();
                    }
                    _ => {}
                }
            }
            assert!(started && text && done, "{driver}: missing native events");
        })
        .await
        .unwrap();
        let stopped = stopped.lock().unwrap();
        let stopped = stopped.as_ref().expect("supervisor acknowledged cleanup");
        assert!(stopped.group_clear, "{driver}");
        assert!(stopped.memory.is_some(), "{driver}");
    }
}
